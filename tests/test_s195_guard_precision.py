"""S195 判据：防幻觉守卫的**精度与诚实性**（外部审计 2026-09-30 的四条，我逐条复现后修）。

为什么单独立判据：旧契约（`tests/test_v2.py::test_guard_hallucination`）只锁了**下划线形态**的
假工具名必须 refuted，恰好绕开了"裸词误杀"那条路径——**判据的形态选择本身就是盲区**。
本文件两头都钉：

① **不冤判**：`` `git` ``/`` `parser` `` 这类非工具主张不产出 refuted，且如实计入 `skipped`
   （正文里出现"存在幻觉"是误杀的真实代价：守卫会被整体忽略）；
② **仍能抓**：笔误（编辑距离 ≤2）、下划线形态、假文件、假符号、行号 0 —— 一条都不能放过；
③ **不静默**：未覆盖扩展名 / 沙盒外 / 扫描上限，都要在 `skipped` 或 `unverifiable` 里看得见。
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import tools  # noqa: E402,F401
from tools import guard as G  # noqa: E402
from tools import guard_checks as GC  # noqa: E402


def _run(text, root):
    return G.hallucination_guard(text, root=str(root))


def _statuses(r, kind=None):
    return [(x["kind"], x["decl"], x["status"]) for x in r["results"]
            if kind is None or x["kind"] == kind]


# ---------- ① 不冤判（P1 的修） ----------

def test_bare_words_are_not_refuted_and_are_counted(tmp_path):
    r = _run("见 `git` 与 `parser` 与 `log` 三处", tmp_path)
    assert r["refuted"] == 0, r
    assert r["skipped"]["not_a_tool_claim"] == 3, r
    assert "存在被证伪" not in r["结论"], f"非幻觉文本不得被扣『存在幻觉』：{r['结论']}"


def test_real_tool_still_verified(tmp_path):
    r = _run("用 `bug_scan` 扫", tmp_path)
    assert ("tool", "`bug_scan`", "verified") in _statuses(r), r


def test_typo_and_shape_still_refuted(tmp_path):
    """金丝雀另一头：**能被疑似的**必须抓——笔误（距离 ≤2）与命名形态（下划线分段）。"""
    r = _run("用 `bug_scna` 扫；`not_a_tool_xyz` 不存在", tmp_path)
    got = {d for _, d, s in _statuses(r) if s == "refuted"}
    assert {"`bug_scna`", "`not_a_tool_xyz`"} <= got, r
    assert r["skipped"]["not_a_tool_claim"] == 0, r


def test_tool_claim_status_table():
    """分类表直接钉（纯函数，免走工具面）：在册 / 形态 / 笔误 / 都不是。"""
    names = {"bug_scan", "fs_read", "hallucination_guard"}
    assert GC.tool_claim_status("bug_scan", names) == "verified"
    assert GC.tool_claim_status("not_a_tool_xyz", names) == "refuted"      # 形态
    assert GC.tool_claim_status("bug_scna", names) == "refuted"            # 距离 2
    assert GC.tool_claim_status("git", names) is None                      # 非主张
    assert GC.tool_claim_status("parser", names) is None
    assert GC.tool_claim_status("log", names) is None


# ---------- ② 仍能抓（P2/P3/P4 的修） ----------

def test_doc_and_config_claims_are_really_checked(tmp_path):
    """P2：`README.md:12` 曾**直接消失**（连 unverifiable 都不是）——现在真查。"""
    (tmp_path / "README.md").write_text("\n".join(f"line{i}" for i in range(20)), encoding="utf-8")
    (tmp_path / "config.toml").write_text("a = 1\nb = 2\n", encoding="utf-8")
    r = _run("见 README.md:12 与 config.toml:7 与 nope.md:3", tmp_path)
    got = {d: s for _, d, s in _statuses(r, "file")}
    assert got.get("README.md:12") == "verified", r          # 在界内
    assert got.get("config.toml:7") == "refuted", r          # 行越界（文件 2 行）
    assert got.get("nope.md:3") == "refuted", r              # 文件不存在——曾静默消失


def test_unchecked_extension_is_visible_not_silent(tmp_path):
    """P2 的第二半：表外扩展名不判死，但**必须看得见**（本仓"能力缺席不静默"红线）。"""
    r = _run("见 foo.xyz:1 处", tmp_path)
    assert r["skipped"]["unchecked_ext"] == 1, r
    items = [x for x in r["results"] if x["kind"] == "file"]
    assert items and items[0]["status"] == "unverifiable" and "未检查" in items[0]["detail"], r


def test_line_zero_is_refuted(tmp_path):
    """P3：行号必须 1 ≤ n ≤ 总行数——`f.py:0` 曾判 verified。"""
    (tmp_path / "f.py").write_text("a = 1\nb = 2\nc = 3\n", encoding="utf-8")
    r = _run("见 f.py:0 与 f.py:1 与 f.py:9", tmp_path)
    got = {d: s for _, d, s in _statuses(r, "file")}
    assert got == {"f.py:0": "refuted", "f.py:1": "verified", "f.py:9": "refuted"}, r


def test_symbols_are_really_checked(tmp_path):
    """P4：符号分支曾是死分支（恒 unverifiable）——现在有界全仓检索真判。"""
    (tmp_path / "m.py").write_text("class GuardedThing:\n    pass\n", encoding="utf-8")
    r = _run("见 `GuardedThing` 与 `NoSuchSymQq`", tmp_path)
    got = {d: s for _, d, s in _statuses(r, "symbol")}
    assert got.get("`GuardedThing`") == "verified", r
    assert got.get("`NoSuchSymQq`") == "refuted", r


def test_symbol_scan_budget_is_honest(monkeypatch, tmp_path):
    """扫描达上限 ⇒ unverifiable（**不冤判**）——口径写进 detail。"""
    (tmp_path / "m.py").write_text("class AnotherThing:\n    pass\n", encoding="utf-8")
    monkeypatch.setattr(GC, "_MAX_FILES", 0)
    r = _run("见 `AnotherThing`", tmp_path)
    items = [x for x in r["results"] if x["kind"] == "symbol"]
    assert items and items[0]["status"] == "unverifiable", r
    assert "上限" in items[0]["detail"], r


def test_sandbox_outside_is_unverifiable(tmp_path):
    """沙盒纪律不变：沙盒外声明不读不判（既不假 verified 也不冤判 refuted）。"""
    outside = pathlib.Path("C:/Windows/System32") if sys.platform == "win32" else pathlib.Path("/etc")
    r = G.hallucination_guard("见 fake.py:5 与 `FakeSym`", root=str(outside))
    for x in r["results"]:
        assert x["status"] == "unverifiable" and "沙盒外" in x["detail"], r


# ---------- ③ 契约与确定性 ----------

def test_root_default_is_documented_as_must_give():
    """P6 的一半（文档面）：`root` 缺省=服务进程 cwd 会产出一批假 refuted ⇒ 描述里必须写明。"""
    from registry import _TOOLS
    desc = _TOOLS["hallucination_guard"]["schema"]["properties"]["root"]["description"]
    assert "必须给" in desc, desc


def test_same_input_same_verdict(tmp_path):
    (tmp_path / "README.md").write_text("x\n", encoding="utf-8")
    text = "见 `git` 与 README.md:1 与 `NoSuchSymQq` 与 foo.xyz:1"
    a, b = _run(text, tmp_path), _run(text, tmp_path)
    assert _statuses(a) == _statuses(b) and a["skipped"] == b["skipped"], (a, b)
