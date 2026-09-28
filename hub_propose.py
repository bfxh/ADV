"""hub_propose.py —— 修复提案流（S180，spec/HUB.md §七 M1 收尾）。

**G1 的机械兑现**：平台只出**提案**，永不改动主树、永不提交、永不合并。

做法：`git worktree add --detach <tmp> HEAD` 造隔离树 → 在隔离树里跑**白名单**机械修复
（ruff --fix / ruff format / cargo fmt）→ `git diff` 即提案 patch → 移除隔离树。
主树在提案前后必须**逐位不变**（HEAD + 工作树状态；防骗门 `propose-isolation` 判据守）。

白名单与命令白名单同精神：**不提供任意命令面**——`checks` 参数只能从 FIX_CHECKS 里选名字。
"""
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

import hub_core

FIX_CHECKS: dict[str, list[str]] = {
    "ruff-fix": [sys.executable, "-m", "ruff", "check", "--fix", "."],
    "ruff-format": [sys.executable, "-m", "ruff", "format", "."],
    "cargo-fmt": ["cargo", "fmt", "--manifest-path", "rust/Cargo.toml"],
}
_MAX_PATCH = 200_000          # patch 文本上限（超则截断并如实标注）


def repo_root() -> pathlib.Path:
    """提案作用仓：env UNIFIED_RX_HUB_REPO 覆盖（测试用小仓），缺省本仓。"""
    raw = (os.environ.get("UNIFIED_RX_HUB_REPO") or "").strip()
    return pathlib.Path(raw) if raw else hub_core.REPO_ROOT


# git 钩子/IDE 会注入这些定位变量：不清掉的话 `git -C <隔离树>` 会被指回主仓，
# 表现为 worktree add 莫名失败（S180 实锤：pre-commit 钩子里 hub-gate 黄、门里绿）。
_ENV_STRIP = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_PREFIX",
              "GIT_OBJECT_DIRECTORY", "GIT_COMMON_DIR", "GIT_ALTERNATE_OBJECT_DIRECTORIES")


def _git_env() -> dict[str, str]:
    return {k: v for k, v in os.environ.items() if k not in _ENV_STRIP}


def _git(root: pathlib.Path, args: list[str], timeout: int = 120) -> tuple[int, str, str]:
    try:
        cp = subprocess.run(["git", "-C", str(root), *args], capture_output=True,
                            timeout=timeout, shell=False, env=_git_env())
    except (OSError, subprocess.SubprocessError) as exc:
        return 255, "", str(exc)
    return (cp.returncode, cp.stdout.decode("utf-8", "replace"),
            cp.stderr.decode("utf-8", "replace"))


def _cleanup(root: pathlib.Path, wt: pathlib.Path, tmp: pathlib.Path) -> None:
    _git(root, ["worktree", "remove", "--force", str(wt)])
    _git(root, ["worktree", "prune"])
    shutil.rmtree(tmp, ignore_errors=True)


def propose(checks: list[str], timeout_s: int = 600) -> dict:
    """在隔离树里跑机械修复 → 返回 {patch, files, checks, unchanged_main, ...}。"""
    unknown = [c for c in checks if c not in FIX_CHECKS]
    if not checks or unknown:
        return {"ok": False, "error": f"未知检查 {unknown}；可选 {sorted(FIX_CHECKS)}"}
    root = repo_root()
    rc, head_before, err = _git(root, ["rev-parse", "HEAD"])
    if rc != 0:
        return {"ok": False, "error": f"不是可用的 git 仓：{err.strip()}"}
    _, status_before, _ = _git(root, ["status", "--porcelain"])
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-propose-"))
    wt = tmp / "wt"
    rc, _out, err = _git(root, ["worktree", "add", "--detach", str(wt), "HEAD"])
    if rc != 0:
        shutil.rmtree(tmp, ignore_errors=True)
        return {"ok": False, "error": f"worktree add 失败：{err.strip()}"}
    runs: list[dict] = []
    try:
        for name in checks:
            t0 = time.time()
            try:
                cp = subprocess.run(FIX_CHECKS[name], cwd=str(wt), capture_output=True,
                                    timeout=timeout_s, shell=False)
                tail = (cp.stdout + cp.stderr).decode("utf-8", "replace")[-2000:]
                runs.append({"check": name, "exit": cp.returncode,
                             "ms": int((time.time() - t0) * 1000), "out_tail": tail})
            except (OSError, subprocess.SubprocessError) as exc:
                runs.append({"check": name, "exit": None, "ms": int((time.time() - t0) * 1000),
                             "out_tail": str(exc)})
        _, patch, _ = _git(wt, ["diff"])
        _, files_raw, _ = _git(wt, ["status", "--porcelain"])
    finally:
        _cleanup(root, wt, tmp)
    _, head_after, _ = _git(root, ["rev-parse", "HEAD"])
    _, status_after, _ = _git(root, ["status", "--porcelain"])
    truncated = len(patch) > _MAX_PATCH
    files = [ln[3:] for ln in files_raw.splitlines() if ln.strip()]
    return {
        "ok": True,
        "unchanged_main": head_after == head_before and status_after == status_before,
        "head": head_before.strip(),
        "patch": patch[:_MAX_PATCH],
        "patch_truncated": truncated,
        "files": files,
        "checks": runs,
        # G1：平台只给提案与落地建议，**不做**提交/合并/推送
        "branch_suggestion": f"fix/proposal-{time.strftime('%Y%m%d-%H%M')}",
        "apply_hint": "git apply -  （粘贴 patch）；或 git checkout -b <branch_suggestion> "
                      "&& git apply - —— 之后照常走门禁与人审",
        "never_merged": True,
    }
