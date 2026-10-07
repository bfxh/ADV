"""DD-0008 的判据测试：提交路径门的自证三态。

红只允许出现在**不一致**那一态：新克隆/CI 天然没钩子，把"未装"判红就是噪声，
而"记号说有、实际没装"正是 2026-10-06 抓到的形状（三处文档写着已设 hooksPath，实测零安装）。
"""
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "hook_status.py"


def _run(*args, cwd=None):
    return subprocess.run([sys.executable, "-X", "utf8", str(SCRIPT), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(cwd or ROOT))


def _repo(tmp_path):
    subprocess.run(["git", "init", "-q", str(tmp_path)], capture_output=True, text=True)
    return tmp_path


def test_absent_is_not_red_but_require_flips_it(tmp_path):
    r = _repo(tmp_path)
    cp = _run("--root", str(r))
    assert cp.returncode == 0, "未装不该判红（新克隆/CI 都不装钩子）：" + cp.stdout
    assert "HOOK-STATUS absent" in cp.stdout, cp.stdout
    cp = _run("--root", str(r), "--require")
    assert cp.returncode == 1, "显式 --require 时未装必须非零：" + cp.stdout


def test_install_then_report_says_installed(tmp_path):
    r = _repo(tmp_path)
    (r / ".githooks").mkdir()
    (r / ".githooks" / "pre-commit").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    assert _run("--root", str(r), "--install").returncode == 0
    cp = _run("--root", str(r))
    assert cp.returncode == 0 and "HOOK-STATUS installed" in cp.stdout, cp.stdout
    got = subprocess.run(["git", "-C", str(r), "config", "--get", "core.hooksPath"],
                         capture_output=True, text=True, encoding="utf-8").stdout.strip()
    assert got == ".githooks", got


def test_marker_without_hook_is_the_dd0008_shape_and_reddens(tmp_path):
    r = _repo(tmp_path)
    subprocess.run(["git", "-C", str(r), "config", "adv.hooksInstalledAt", "2026-10-05@abc"],
                   capture_output=True, text=True)
    cp = _run("--root", str(r))
    assert cp.returncode == 1, "记号说有、实际没装 ⇒ 必须红：" + cp.stdout
    assert "不一致" in cp.stdout, cp.stdout
