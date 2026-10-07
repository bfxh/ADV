"""提交路径门的自证（DD-0008 的判据本体）。

为什么需要它：本仓最贵的一类假绿是"**版本化的钩子文件**被当成**装好的钩子**"——
2026-10-06 实测三处文档都写着 core.hooksPath 已设，而 `git config --get core.hooksPath`
无值、`.git/hooks/` 只剩 `.sample`：提交路径上一道门都没装，文档却言之凿凿。

它判什么（三种形态，只有第三种红）：
  - 已装：hooksPath 指向 `.githooks` **且** 安装记号（`adv.hooksInstalledAt`）在位；
  - 未装：两者都没有 —— **不红**（新克隆/CI 天然没钩子，把机器级事实判红=噪声）；
  - **不一致**：记号说有、实际没设（或钩子文件丢了）⇒ 红：这正是 DD-0008 抓到的形状，
    文档/记号与现实脱钩时必须有人喊。
用法：
  python -X utf8 scripts/hook_status.py            # 报告（不一致才非零）
  python -X utf8 scripts/hook_status.py --require  # 要求必须已装（操作者显式选择）
  python -X utf8 scripts/hook_status.py --install  # 装（记 hooksPath + 落时间/HEAD 记号，幂等）
"""
from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HOOKS_REL = ".githooks"
MARKER = "adv.hooksInstalledAt"


def _git(*args: str, root: pathlib.Path | None = None) -> str:
    cp = subprocess.run(["git", "-C", str(root or ROOT), *args],
                        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (cp.stdout or "").strip()


def status(root: pathlib.Path | None = None) -> tuple[str, str]:
    """返回 (判定, 说明)。判定 ∈ {installed, absent, inconsistent}。"""
    r = root or ROOT
    hooks_path = _git("config", "--get", "core.hooksPath", root=r)
    marker = _git("config", "--get", MARKER, root=r)
    pre_commit = r / HOOKS_REL / "pre-commit"
    ok_path = hooks_path.replace("\\", "/").rstrip("/") in (HOOKS_REL, f"./{HOOKS_REL}")
    if ok_path and marker and pre_commit.is_file():
        return "installed", f"已装：core.hooksPath={hooks_path}，记号 {marker}"
    if not hooks_path and not marker:
        return "absent", ("未装：core.hooksPath 未设、也无安装记号"
                          "（装：python -X utf8 scripts/hook_status.py --install）")
    missing = []
    if not ok_path:
        missing.append(f"hooksPath={hooks_path or '（未设）'}")
    if not marker:
        missing.append("缺安装记号")
    if not pre_commit.is_file():
        missing.append(f"缺 {HOOKS_REL}/pre-commit")
    return "inconsistent", (
        "不一致：记号/文档说有门，实际没装上（" + "、".join(missing) + "）"
        "——DD-0008 抓的就是这个形状：版本化文件 ≠ 装好的钩子"
    )


def install(root: pathlib.Path | None = None) -> int:
    r = root or ROOT
    head = _git("rev-parse", "--short", "HEAD", root=r) or "no-head"
    stamp = f"{_git('log', '-1', '--format=%cI', root=r) or '?'}@{head}"
    for k, v in (("core.hooksPath", HOOKS_REL), (MARKER, stamp)):
        cp = subprocess.run(["git", "-C", str(r), "config", k, v],
                            capture_output=True, text=True, encoding="utf-8", errors="replace")
        if cp.returncode != 0:
            print(f"装失败：git config {k} {v}：{cp.stderr.strip()}")
            return 1
    print(f"已装：core.hooksPath={HOOKS_REL}，记号 {MARKER}={stamp}")
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT), help="仓库根（默认本仓；测试用临时仓）")
    ap.add_argument("--require", action="store_true", help="未装也判非零（显式要求）")
    ap.add_argument("--install", action="store_true", help="装上并落记号（幂等）")
    a = ap.parse_args(argv)
    root = pathlib.Path(a.root).resolve()
    if a.install:
        return install(root)
    verdict, why = status(root)
    print(f"HOOK-STATUS {verdict}：{why}")
    if verdict == "inconsistent":
        return 1
    if verdict == "absent" and a.require:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
