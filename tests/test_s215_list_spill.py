"""S215（片 D-2）：分析类回包「大列表长尾溢出」（tools/spill.compact_large_lists）的守门测试。

canary 纪律锁四件事，核心是「**信息不减**」——切的是呈现，不是数据：
  1. 召回等价：inline 首屏 ∪ 落盘长尾 == 原始全量列表（逐条相等，非仅计数）；
  2. 短列表不动：条目数 ≤ head_n ⇒ 原样返回、meta 空；
  3. 未超字节阈不动：条目多但序列化小 ⇒ 不切（避免小结果也被落盘往返）；
  4. 落盘不可用 ⇒ 宁大勿丢：返回原始全量、meta 空（绝不静默截断）。
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from tools import spill


def _issues(n):
    return [{"line": i, "rule": "r", "msg": "m" * 40, "file": f"f{i}.py",
             "severity": "low", "kind": "clue"} for i in range(n)]


def _sandbox(tmp_path, monkeypatch):
    monkeypatch.setenv("UNIFIED_RX_SANDBOX", str(tmp_path))


def test_recall_equivalence(tmp_path, monkeypatch):
    """首屏 + 落盘长尾 == 原始全量：切的是呈现不是数据。"""
    _sandbox(tmp_path, monkeypatch)
    issues = _issues(200)
    data = {"total": 200, "by_rule": {"r": 200}, "issues": issues}
    out, meta = spill.compact_large_lists(data, "bug_scan", head_n=30, thresh_bytes=8 * 1024)
    assert meta, "200 条大列表应触发切尾"
    m = meta[0]
    assert m["field"] == "issues" and m["kept"] == 30 and m["total"] == 200
    tail = json.loads(pathlib.Path(m["tail_path"]).read_text(encoding="utf-8"))
    restored = out["issues"] + tail
    assert restored == issues, "head∪tail 与原始逐条不等 ⇒ 信息被切丢了"
    assert "fs_read" in m["fetch"]


def test_inline_shrinks(tmp_path, monkeypatch):
    """体积确实降下来：切尾后内联远小于原始。"""
    _sandbox(tmp_path, monkeypatch)
    data = {"issues": _issues(300)}
    out, meta = spill.compact_large_lists(data, "bug_scan", head_n=30, thresh_bytes=8 * 1024)
    full_b = len(json.dumps(data["issues"]).encode("utf-8"))
    kept_b = len(json.dumps(out["issues"]).encode("utf-8"))
    assert meta and kept_b < full_b * 0.25, "切尾后内联没显著变小——没起到封顶作用"


def test_short_list_untouched(tmp_path, monkeypatch):
    """条目数 ≤ head_n ⇒ 原样、meta 空（不误伤小结果）。"""
    _sandbox(tmp_path, monkeypatch)
    data = {"issues": _issues(10)}
    out, meta = spill.compact_large_lists(data, "bug_scan", head_n=30, thresh_bytes=1)
    assert meta == [] and out["issues"] == data["issues"]


def test_below_byte_threshold_untouched(tmp_path, monkeypatch):
    """条目多但序列化未超阈 ⇒ 不切（避免小结果也被落盘往返）。"""
    _sandbox(tmp_path, monkeypatch)
    data = {"issues": list(range(200))}  # 200 个 int，序列化很小
    out, meta = spill.compact_large_lists(data, "bug_scan", head_n=30, thresh_bytes=64 * 1024)
    assert meta == [] and out["issues"] == data["issues"]


def test_no_sandbox_never_drops_data(tmp_path, monkeypatch):
    """落盘不可用（无沙盒）⇒ 返回原始全量、meta 空——宁大勿丢。"""
    monkeypatch.delenv("UNIFIED_RX_SANDBOX", raising=False)
    issues = _issues(200)
    data = {"issues": issues}
    out, meta = spill.compact_large_lists(data, "bug_scan", head_n=30, thresh_bytes=1024)
    assert meta == [], "落盘不可用却声称切了——会误导消费方"
    assert out["issues"] == issues, "落盘失败还把数据切没了 ⇒ 静默丢信息"


# ---- wire 集成（server.tool_reply 的两级溢出通道互斥）----

def test_wire_tier2_compacts_subthreshold_list(tmp_path, monkeypatch):
    """亚阈值(整包未超 48KB)但含大列表 ⇒ tool_reply 走切尾，内联带 _spilled_lists。"""
    import server
    monkeypatch.setenv("UNIFIED_RX_SANDBOX", str(tmp_path))
    monkeypatch.setenv("UNIFIED_RX_SPILL_KB", "48")          # 整包阈 48KB
    data = {"total": 200, "by_rule": {"r": 200}, "issues": _issues(200)}  # ~46KB < 48KB
    resp = server.tool_reply(1, "bug_scan", {"ok": True, "result": data})
    sc = resp["result"]["structuredContent"]
    assert sc["ok"] and "_spilled_lists" in sc["data"], "亚阈值大列表应被切尾并留取用元信息"
    m = sc["data"]["_spilled_lists"][0]
    tail = json.loads(pathlib.Path(m["tail_path"]).read_text(encoding="utf-8"))
    assert sc["data"]["issues"] + tail == data["issues"], "wire 切尾也必须召回等价"


def test_wire_tier1_whole_spill_beats_tier2(tmp_path, monkeypatch):
    """整包超阈 ⇒ 仍走老的整包溢出（落**完整**结果），不插手切尾——保 test_s161 语义。"""
    import server
    monkeypatch.setenv("UNIFIED_RX_SANDBOX", str(tmp_path))
    monkeypatch.setenv("UNIFIED_RX_SPILL_KB", "1")            # 阈 1KB，逼整包溢出
    data = {"issues": _issues(200)}                           # ~46KB > 1KB
    resp = server.tool_reply(1, "bug_scan", {"ok": True, "result": data})
    sc = resp["result"]["structuredContent"]
    assert "spilled" in sc and "_spilled_lists" not in str(sc), "超阈应整包溢出而非切尾"
    on_disk = json.loads(pathlib.Path(sc["spilled"]["path"]).read_text(encoding="utf-8"))
    assert on_disk["data"] == data, "整包溢出落的必须是完整结果"

