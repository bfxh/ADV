"""M4-1 金丝雀：防泄漏三条判据 + 冻结集 + 地板基线。

金丝雀的意义：判据必须**能判红**——所以这里正反两面都钉：泄漏形态（原样搬、长片段、近似复制）
必须被抓住，扰动后的正常查询必须放过。阈值本身也钉住（改阈值等于改评测集，必须显式重录）。
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HERE = ROOT / "bench" / "retrieval"
sys.path.insert(0, str(HERE))
import protocol  # noqa: E402

DOC = "fn load_workspace(root: Path) -> Result<Workspace> { /* 读成员清单 */ }"
OTHER = "def scan_paths(paths): return [p for p in paths]"


def test_thresholds_are_pinned():
    # 阈值就是口径：动它必须显式重录评测集（改这里 = 改口径，要走账本）
    assert protocol.MAX_TOKEN_RUN == 6
    assert protocol.MAX_JACCARD == 0.60


def test_verbatim_query_is_caught_by_c1():
    ok, why = protocol.verdict("fn load_workspace(root: Path)", DOC, [DOC, OTHER])
    assert not ok and why[0].startswith("C1"), why


def test_long_shared_run_is_caught_by_c2():
    # 词序打乱、但保留了 7 个连续词 ⇒ C2 必须响（C1 抓不住它——不是连续子串）
    query = "load workspace root path result workspace fn"
    ok, why = protocol.verdict(query, DOC, [DOC, OTHER])
    assert not ok, "这条本该被 C2 拦下"
    assert any(w.startswith(("C2", "C3")) for w in why), why


def test_near_duplicate_doc_is_caught_by_c3():
    # 查询与另一篇文档词集几乎重合 ⇒ C3 响
    twin = "fn load_workspace root Path Result Workspace 读成员清单"
    ok, why = protocol.verdict("load workspace root path result", DOC, [twin, OTHER])
    assert not ok and any(w.startswith("C3") for w in why), why


def test_perturbed_query_passes():
    ok, why = protocol.verdict("workspace loader root parse", DOC, [DOC, OTHER])
    assert ok, why


def test_empty_query_is_refused():
    ok, why = protocol.verdict("   ", DOC, [DOC])
    assert not ok and why[0].startswith("C0"), why


def test_identifier_splitter_handles_both_conventions():
    assert protocol.split_identifier("loadWorkspace") == ["load", "workspace"]
    assert protocol.split_identifier("HTTPServerConfig") == ["http", "server", "config"]
    assert protocol.split_identifier("tail_segment") == ["tail", "segment"]


def test_frozen_set_is_deterministic_and_still_clean():
    # 门入口本身（含逐字节对账 + 冻结件重过判据 + 基线对账）
    cp = subprocess.run([sys.executable, "-X", "utf8", str(HERE / "check.py")],
                        capture_output=True, text=True, encoding="utf-8", errors="replace")
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "RETRIEVAL-CHECK OK" in cp.stdout, cp.stdout


def test_frozen_queries_carry_their_gold_labels():
    rows = [json.loads(l) for l in (HERE / "queries.jsonl").read_text(encoding="utf-8").splitlines() if l.strip()]
    assert len(rows) == 120, len(rows)
    for q in rows:
        assert q["relevant"] and q["source"] in q["relevant"], q
        assert q["kind"] in {"ident", "sentence", "combo"}, q


def test_type_gate_spawns_mypy_in_utf8_mode():
    """门不许依赖调用者的 locale：`mypy.ini` 里有中文注释，非 UTF-8 模式下 configparser 用 gbk 读它
    会当场 UnicodeDecodeError（mypy rc=2、诊断 0 条）。2026-10-08 实测踩到：经 local_gate 跑就绿、
    单独跑就红。这条金丝雀钉住 run_mypy 的 spawn 形状。"""
    src = (ROOT / "scripts" / "type_gate.py").read_text(encoding="utf-8")
    assert '"-X", "utf8", "-m", "mypy"' in src, "type_gate 没有把 mypy 子进程拉进 UTF-8 模式"
    assert 'PYTHONUTF8="1"' in src, "type_gate 没有给子进程兜底 PYTHONUTF8"
