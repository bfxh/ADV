"""tests/test_s180_propose.py —— 修复提案流（G1：隔离工作树里出提案，**永不碰主树**）。

用小 git 仓（含一个 ruff 可修文件：未用 import）做真端到端：提案能产出可 apply 的 patch、
主树逐位不变、隔离树清理干净、未知检查名被拒、MCP 侧无授权被拒。
"""
import subprocess

import pytest

import hub_propose
import registry


def _git(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", shell=False, timeout=120)


@pytest.fixture()
def mini_repo(tmp_path, monkeypatch):
    """小 git 仓（不碰真仓）：UNIFIED_RX_HUB_REPO 指向它。"""
    root = tmp_path / "repo"
    root.mkdir()
    (root / "a.py").write_text("import os\n\n\nprint('x')\n", encoding="utf-8")
    _git(root, "init", "-q")
    for args in (("add", "-A"), ("commit", "-qm", "init")):
        cp = _git(root, "-c", "user.name=t", "-c", "user.email=t@t", *args)
        assert cp.returncode == 0, cp.stderr
    monkeypatch.setenv("UNIFIED_RX_HUB_REPO", str(root))
    return root


def test_propose_isolation_and_patch(mini_repo):
    """提案出 patch（删未用 import）+ 主树逐位不变 + 隔离树无残留 + G1 标记。"""
    before = (mini_repo / "a.py").read_text(encoding="utf-8")
    res = hub_propose.propose(["ruff-fix"])
    assert res["ok"] is True, res
    assert res["unchanged_main"] is True
    assert (mini_repo / "a.py").read_text(encoding="utf-8") == before   # 主树未被改动
    assert res["patch"] and "-import os" in res["patch"]
    assert "a.py" in res["files"]
    assert res["never_merged"] is True and res["apply_hint"]
    assert "fix/proposal-" in res["branch_suggestion"]
    wt = _git(mini_repo, "worktree", "list").stdout
    assert "adv-propose-" not in wt                                    # 隔离树清理干净


def test_proposed_patch_applies_cleanly(mini_repo):
    """提案可用：`git apply --check` 必须通过（patch 不是散文）。"""
    res = hub_propose.propose(["ruff-fix"])
    pf = mini_repo / "proposal.patch"
    pf.write_text(res["patch"], encoding="utf-8")
    cp = _git(mini_repo, "apply", "--check", str(pf))
    assert cp.returncode == 0, cp.stderr


def test_propose_ignores_git_locator_env(mini_repo, monkeypatch):
    """回归（S180 实锤）：git 钩子会注入 GIT_DIR / GIT_INDEX_FILE 等定位变量——
    不清掉的话 `git -C <隔离树>` 会被指回主仓、worktree 流程莫名失败
    （表征：pre-commit 钩子里 hub-gate 黄、独立跑门却绿）。这里把它变成可测条件。"""
    monkeypatch.setenv("GIT_DIR", str(mini_repo / ".git"))
    monkeypatch.setenv("GIT_INDEX_FILE", str(mini_repo / ".git" / "index"))
    res = hub_propose.propose(["ruff-fix"])
    assert res["ok"] is True, res
    assert res["unchanged_main"] is True
    assert "-import os" in res["patch"]


def test_propose_rejects_unknown_check(mini_repo):
    res = hub_propose.propose(["not-a-check"])
    assert res["ok"] is False and "未知检查" in res["error"]


def test_propose_rejects_non_repo(tmp_path, monkeypatch):
    plain = tmp_path / "plain"
    plain.mkdir()
    monkeypatch.setenv("UNIFIED_RX_HUB_REPO", str(plain))
    res = hub_propose.propose(["ruff-fix"])
    assert res["ok"] is False and "git 仓" in res["error"]


def test_mcp_side_propose_requires_authorized():
    denied = registry.call("hub_propose", {"checks": ["ruff-fix"]})
    assert denied["ok"] is False


def test_propose_tool_registered():
    tools = {t["name"] for t in registry.list_tools()}
    assert "hub_propose" in tools
