"""S195 契约：守卫判定金丝雀门（`scripts/guard_gate.py` + `spec/guard-corpus.json`）。

由来：P1-P6 那批守卫缺陷（工具名误杀、扩展名静默消失、`:0` 假 verified、符号空转、
沙盒钳制）都不在 claim_gate（文档数字）与 H2（kind=file 一致率）的测量面内。
本文件把「判定形状」钉死：判据两头都有——
1. **真仓自检**：语料逐条对当前守卫输出必须成立（走真路径子进程，非 import 内部函数）；
2. **金丝雀红**：夹具语料故意改错（误杀复活 / 静默回归 / 钳制被拆的方向）⇒ 必红且点名条目；
3. **fail-closed**：语料缺失 / 坏 JSON / 空 modes / 条目缺字段 / 非法 status ⇒ 红；
4. **语料形状**：三种 kind、两种沙盒口径都在测量面内（缺一族 = 门被悄悄变窄）。
"""
import json
import pathlib
import subprocess
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "guard_gate.py"
CORPUS = ROOT / "spec" / "guard-corpus.json"
PY = sys.executable


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=600)


def _mutate(tmp_path: pathlib.Path, fn: Any) -> pathlib.Path:
    """把真语料按 fn 改一处写入 tmp，返回路径（金丝雀共用装置）。"""
    corpus: dict[str, Any] = json.loads(CORPUS.read_text(encoding="utf-8"))
    fn(corpus)
    cp = tmp_path / "corpus.json"
    cp.write_text(json.dumps(corpus, ensure_ascii=False), encoding="utf-8")
    return cp


def _case(corpus: dict[str, Any], cid: str) -> dict[str, Any]:
    for mode in corpus["modes"]:
        for case in mode["cases"]:
            if case["id"] == cid:
                return case
    raise AssertionError(f"语料缺条目 {cid}")


# ---------- 真仓自检 ----------

def test_real_repo_green() -> None:
    got = _run()
    assert got.returncode == 0, got.stdout + got.stderr
    assert "GUARD-GATE OK" in got.stdout and "cases=8" in got.stdout, got.stdout


def test_corpus_shape_three_kinds_two_modes() -> None:
    """三种 kind、双沙盒口径都在——缺一族等于门被悄悄变窄。"""
    corpus: dict[str, Any] = json.loads(CORPUS.read_text(encoding="utf-8"))
    modes = {m["mode"] for m in corpus["modes"]}
    assert modes == {"open", "restricted"}, modes
    kinds = {i["kind"] for m in corpus["modes"] for c in m["cases"] for i in c["items"]}
    assert kinds == {"tool", "file", "symbol"}, kinds
    ids = [c["id"] for m in corpus["modes"] for c in m["cases"]]
    assert len(ids) == len(set(ids)), "条目 id 重复"
    # 未核查两面都有正例（静默消失方向）与零例（误杀方向）
    nz = [c["id"] for m in corpus["modes"] for c in m["cases"]
          if c["unchecked_words"] or c["unchecked_cites"]]
    assert nz, "语料没有未核查正例——静默回归测不到"


# ---------- 金丝雀必红 ----------

def test_canary_refuted_resurrection_red(tmp_path: pathlib.Path) -> None:
    """S196 反向（外部锚）：把真实代码词 `build_chunk_mesh` 从「未核查」改回「refuted 工具」
    （= 形态规则复活）⇒ 必红且点名——期望值来自 328 份真实答案的实测分布，不是规则作者断言。"""
    def fn(corpus: dict[str, Any]) -> None:
        case = _case(corpus, "code-ids-not-tool-claims")
        case["unchecked_words"] = [w for w in case["unchecked_words"]
                                   if w != "build_chunk_mesh"]
        case["items"] = [{"decl": "`build_chunk_mesh`", "kind": "tool", "status": "refuted"}]
    got = _run("--corpus", str(_mutate(tmp_path, fn)))
    assert got.returncode != 0 and "code-ids-not-tool-claims" in got.stdout, got.stdout


def test_canary_silent_blindspot_red(tmp_path: pathlib.Path) -> None:
    """P2 反向：期望 logo.svg 静默消失（从不判也不计数）⇒ 实际它进未核查，集合不等必红。"""
    def fn(corpus: dict[str, Any]) -> None:
        case = _case(corpus, "ext-whitelist-both-faces")
        case["unchecked_cites"] = ["cov-err.txt:1"]
    got = _run("--corpus", str(_mutate(tmp_path, fn)))
    assert got.returncode != 0 and "unchecked_cites" in got.stdout, got.stdout


def test_canary_clamp_split_red(tmp_path: pathlib.Path) -> None:
    """S97 反向：把沙盒外钳制改写成「照读照判（verified）」⇒ 必红（fail-closed 被拆）。"""
    def fn(corpus: dict[str, Any]) -> None:
        case = _case(corpus, "sandbox-out-fail-closed")
        case["items"][0]["status"] = "verified"
        case["items"][0]["detail_contains"] = "在范围内"
    got = _run("--corpus", str(_mutate(tmp_path, fn)))
    assert got.returncode != 0 and "sandbox-out-fail-closed" in got.stdout, got.stdout


def test_canary_line_zero_red(tmp_path: pathlib.Path) -> None:
    """P3 反向：期望 alpha.py:0 假 verified ⇒ 必红（行号下界被拆）。"""
    def fn(corpus: dict[str, Any]) -> None:
        case = _case(corpus, "file-line-bounds")
        case["items"][0]["status"] = "verified"
    got = _run("--corpus", str(_mutate(tmp_path, fn)))
    assert got.returncode != 0 and "alpha.py:0" in got.stdout, got.stdout


# ---------- fail-closed 四态 ----------

def test_fail_closed_missing_corpus(tmp_path: pathlib.Path) -> None:
    got = _run("--corpus", str(tmp_path / "nope.json"))
    assert got.returncode != 0 and "语料缺失" in got.stdout, got.stdout


def test_fail_closed_bad_json(tmp_path: pathlib.Path) -> None:
    cp = tmp_path / "corpus.json"
    cp.write_text("{ not json", encoding="utf-8")
    got = _run("--corpus", str(cp))
    assert got.returncode != 0 and "坏 JSON" in got.stdout, got.stdout


def test_fail_closed_empty_modes(tmp_path: pathlib.Path) -> None:
    cp = tmp_path / "corpus.json"
    cp.write_text(json.dumps({"modes": []}), encoding="utf-8")
    got = _run("--corpus", str(cp))
    assert got.returncode != 0 and "modes" in got.stdout, got.stdout


def test_fail_closed_case_missing_field(tmp_path: pathlib.Path) -> None:
    def fn(corpus: dict[str, Any]) -> None:
        del _case(corpus, "file-line-bounds")["items"]
    got = _run("--corpus", str(_mutate(tmp_path, fn)))
    assert got.returncode != 0 and "缺字段" in got.stdout, got.stdout
