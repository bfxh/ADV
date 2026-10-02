"""S210 契约：回包体积尺 + 参数摘要 + 归因标记（"谁在循环 / 字节花在哪"的前提）。

由来（全部实测，不靠推测）：`~/.ADV/stats.jsonl` 127,385 条打点里只有
`tool/duration_ms/ts/src/agent` 四个维度——**回包体积根本没记**，所以"token 花在哪"
无人能答；而交叉算面板发现首屏 54 件全部真被调用过（裁首屏没肉），浪费在**重复回包**
（fs_read 32,012 次）。另一头，`agent` 只出现过自家 `surface-gate`，真宿主归因 **0 条**
⇒ "哪个智能体在死循环"答不出，熔断只能按本进程内同工具+同参数判。

七条判据（都走 registry.call 真路径，读重定向后的真实账本行）：
1. 成功回包的 `result_bytes` 必须等于返回值的 JSON 长度（尺本身自洽，不许手写）；
2. **每一条返回路径都要留痕**：未启用域 / 需授权未确认 / schema 拒 / 缓存命中 / 异常
   ——尤其"需授权却不确认"这种循环典型形状，补尺前根本不打点；
3. 同参数两次调用 `args_digest` 相同、改参数即不同（循环分组的前提）；
4. `attr` 区分 named（握手报了 clientInfo.name）与 anon（没握手），`pid` 落账；
5. 异常回包的体积**含 error_detail**（堆栈尾部真的被喂给宿主，不许只算 error 一行）；
6. 序列化不了的返回形状记 -1，不拿 0 冒充"很小"；
7. 老记录缺新字段时读方按 None 处理（不回填、不崩）。
"""
import json
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import registry
import tools  # noqa: F401  （导入即注册：registry._TOOLS 由 tools 包填充）


@pytest.fixture()
def metered(tmp_path, monkeypatch):
    """把打点账本重定向到 tmp，返回读数函数；每测独立。"""
    path = tmp_path / "stats.jsonl"
    monkeypatch.setattr(registry, "_stats_path", lambda: str(path))
    monkeypatch.setattr(registry, "_AGENT_NAME", None)

    def rows():
        if not path.is_file():
            return []
        return [json.loads(ln) for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]
    return rows


def _size(out):
    return len(json.dumps(out, ensure_ascii=False, default=str))


def test_success_reply_bytes_matches_value(metered):
    out = registry.call("fs_stat", {"path": "registry.py"})
    row = metered()[-1]
    assert out["ok"] is True, out
    assert row["result_bytes"] == _size(out), "尺与被测对象不一致（体积可能被手写）"


def test_every_denied_path_still_lands_in_the_ledger(metered):
    before = len(metered())
    r1 = registry.call("ide_build", {"path": "."})            # 需授权、未确认
    r2 = registry.call("fs_read", {"path": 12345})            # schema 拒
    registry.call("no_such_tool_xyz", {})             # 未知工具：不打点（键空间无界）
    assert r1["ok"] is False and r2["ok"] is False
    rows = metered()[before:]
    tools_seen = [x["tool"] for x in rows]
    assert "ide_build" in tools_seen, f"授权拒绝不打点=循环看不见：{tools_seen}"
    assert "fs_read" in tools_seen
    assert "no_such_tool_xyz" not in tools_seen, "未知工具名进账本会污染统计键空间"
    assert all(x["result_bytes"] > 0 for x in rows), rows


def test_args_digest_groups_identical_calls(metered):
    registry.call("fs_stat", {"path": "registry.py"})
    registry.call("fs_stat", {"path": "registry.py"})
    registry.call("fs_stat", {"path": "server.py"})
    a, b, c = [r["args_digest"] for r in metered()[-3:]]
    assert a == b != c, "同参数没聚成一类/改参数没分开——循环分组判据失效"


def test_attr_and_pid_distinguish_named_from_anon(metered, monkeypatch):
    registry.call("fs_stat", {"path": "registry.py"})
    anon = metered()[-1]
    assert anon["attr"] == "anon" and anon["agent"] is None and anon["pid"] == os.getpid()
    monkeypatch.setattr(registry, "_AGENT_NAME", "qoder-host")
    registry.call("fs_stat", {"path": "server.py"})
    named = metered()[-1]
    assert named["attr"] == "named" and named["agent"] == "qoder-host"


def test_exception_reply_counts_the_stack_it_feeds(metered, monkeypatch):
    def boom(**kwargs):
        raise RuntimeError("故意炸")
    monkeypatch.setitem(registry._TOOLS["fs_stat"], "handler", boom)
    out = registry.call("fs_stat", {"path": "registry.py"})
    row = metered()[-1]
    assert out["ok"] is False and "error_detail" in out
    assert row["result_bytes"] == _size(out), "异常回包少算堆栈尾部=低估宿主被喂的字节"


def test_unserializable_reply_is_flagged_not_zeroed():
    class Opaque:
        pass
    assert registry._reply_bytes({"x": {1, 2}}) >= 0      # set 能 default=str 序列化
    weird = registry._reply_bytes({"o": Opaque()})
    assert isinstance(weird, int) and weird != 0, f"不可序列化记 0 会被当成空回包：{weird}"


def test_cache_hit_path_reports_bytes(metered, tmp_path):
    """命中缓存的那次调用记 0 耗时（`_record_stats(name, 0.0, hit)`）——这条判据钉的正是
    "重复回包也被体积尺看见"：不记它就等于最容易省 token 的路径没数据。"""
    import tempfile

    base = os.path.join(tempfile.gettempdir(), "unified-rx-pytest", "s210-cache-probe")
    os.makedirs(base, exist_ok=True)
    target = os.path.join(base, "probe_mod.py")
    with open(target, "w", encoding="utf-8") as fh:
        fh.write("def add(a, b):\n    return a + b\n")
    first = registry.call("bug_scan", {"path": base})
    second = registry.call("bug_scan", {"path": base})
    assert first["ok"] and second["ok"], (first, second)
    rows = metered()[-2:]
    assert all(r["tool"] == "bug_scan" for r in rows), rows
    assert rows[0]["result_bytes"] == rows[1]["result_bytes"] == _size(second), \
        "命中缓存不打体积尺 ⇒ 重复回包看不见"
    assert rows[1]["duration_ms"] <= rows[0]["duration_ms"], rows
