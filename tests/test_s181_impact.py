"""tests/test_s181_impact.py —— 静态语义影响面（方向①）判据：不漏、精确、退化如实。

mini 仓结构：`pkg/a.py` import `pkg/b.py`；`tests/test_a.py` **只** import `pkg/a`（对 b 是
间接依赖）；另有独立的 `pkg/c.py` + `tests/test_c.py`。核心判据 = **改 b 必须选中 test_a**。
"""
import subprocess

import pytest

import hub_impact


def _git(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", shell=False, timeout=120)


@pytest.fixture()
def repo(tmp_path, monkeypatch):
    root = tmp_path / "repo"
    (root / "pkg").mkdir(parents=True)
    (root / "tests").mkdir()
    (root / "pkg" / "__init__.py").write_text("", encoding="utf-8")
    (root / "pkg" / "b.py").write_text("def helper():\n    return 1\n", encoding="utf-8")
    (root / "pkg" / "a.py").write_text(
        "from pkg.b import helper\n\n\ndef run():\n    return helper()\n", encoding="utf-8")
    (root / "pkg" / "c.py").write_text("def solo():\n    return 2\n", encoding="utf-8")
    (root / "tests" / "test_a.py").write_text(
        "from pkg.a import run\n\n\ndef test_run():\n    assert run() == 1\n", encoding="utf-8")
    (root / "tests" / "test_c.py").write_text(
        "from pkg.c import solo\n\n\ndef test_solo():\n    assert solo() == 2\n", encoding="utf-8")
    _git(root, "init", "-q")
    for args in (("add", "-A"), ("commit", "-qm", "init")):
        cp = _git(root, "-c", "user.name=t", "-c", "user.email=t@t", *args)
        assert cp.returncode == 0, cp.stderr
    monkeypatch.setenv("UNIFIED_RX_HUB_REPO", str(root))
    return root


def test_transitive_impact_not_missed(repo):
    """改 b.py（被 a.py 间接使用）⇒ test_a.py 必须在列——**传递闭包不漏**（核心判据）。"""
    (repo / "pkg" / "b.py").write_text("def helper():\n    return 42\n", encoding="utf-8")
    res = hub_impact.impact(base="HEAD")
    assert res["ok"] is True and res["fallback"] is None, res
    assert "pkg/b.py" in res["changed"]
    assert "tests/test_a.py" in res["impacted_tests"]
    assert "pkg/a.py" in res["impacted_files"]


def test_unrelated_change_not_selected(repo):
    """改 c.py ⇒ 选 test_c.py、**不**选 test_a.py（精确性；漏选致命、误选可容忍）。"""
    (repo / "pkg" / "c.py").write_text("def solo():\n    return 3\n", encoding="utf-8")
    res = hub_impact.impact(base="HEAD")
    assert "tests/test_c.py" in res["impacted_tests"]
    assert "tests/test_a.py" not in res["impacted_tests"]


def test_conftest_change_selects_whole_subtree(repo):
    """S211：conftest.py 不被任何测试 import，导入图里没有这条边——以前改了它
    会选 0 个测试且不给理由（hub_gate 的 honest 判据在 CI 上把这个静默零结果抓红）。
    现在的正确形状是"所在子树全量入选"。"""
    (repo / "conftest.py").write_text("import os\n", encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "conftest")
    (repo / "conftest.py").write_text("import os\nimport sys\n", encoding="utf-8")
    res = hub_impact.impact(base="HEAD")
    assert "conftest.py" in res["changed"], res["changed"]
    assert res["conftest_scopes"] == [""], res["conftest_scopes"]
    assert set(res["impacted_tests"]) == {"tests/test_a.py", "tests/test_c.py"}, res["impacted_tests"]
    assert res["scope_forced_tests"] == 2, res


def test_global_scopes_are_directory_bounded():
    """根 conftest = 全仓；`tests/conftest.py` 只罩 tests/；普通文件不产生作用域。"""
    got = hub_impact._global_scopes(["conftest.py", "tests/conftest.py", "pkg/a.py",
                                     "bench/fixtures/x/conftest.py"])
    assert got == ["", "bench/fixtures/x", "tests"], got


def test_committed_change_visible_from_base(repo):
    """已提交变更同样可见（base=HEAD~1）。"""
    (repo / "pkg" / "b.py").write_text("def helper():\n    return 7\n", encoding="utf-8")
    cp = _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-aqm", "chg")
    assert cp.returncode == 0, cp.stderr
    res = hub_impact.impact(base="HEAD~1")
    assert "pkg/b.py" in res["changed"]
    assert "tests/test_a.py" in res["impacted_tests"]


def test_no_change_reports_none_fallback(repo):
    res = hub_impact.impact(base="HEAD")
    assert res["ok"] is True and res["fallback"] == "none" and res["changed"] == []


def test_non_repo_degrades_honestly(repo, tmp_path, monkeypatch):
    """非 git 仓：如实退化（不假装选过）——fallback 必须可见，且带理由。"""
    plain = tmp_path / "plain"
    plain.mkdir()
    (plain / "x.py").write_text("import os\n", encoding="utf-8")
    monkeypatch.setenv("UNIFIED_RX_HUB_REPO", str(plain))
    res = hub_impact.impact(base="HEAD")
    assert res["ok"] is True
    assert res["fallback"] in ("none", "full") and res.get("reason")


def test_impact_tool_registered_read_only():
    import registry
    tools = {t["name"]: t for t in registry.list_tools()}
    assert "hub_impact" in tools
    assert tools["hub_impact"]["annotations"]["readOnlyHint"] is True
