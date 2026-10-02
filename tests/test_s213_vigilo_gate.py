"""S213：Vigilo 安全扫描增量门（scripts/vigilo_gate.py）的守门测试。

锁四件事：
  1. 基线在、门绿（棘轮：存量发现不判红）；
  2. vigilo 真在跑（不是空壳门）——输出里有发现数；
  3. 真门验证：往基线里塞一条不可能匹配的假键 ⇒ 所有真实发现都变"新增" ⇒ 判红；
  4. --write-baseline 能更新基线。
"""
import json
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "vigilo_gate.py"
BASELINE = ROOT / "spec" / "vigilo-baseline.json"


def _run(*args, cwd=ROOT):
    env = dict(os.environ, PYTHONUTF8="1")
    return subprocess.run([sys.executable, "-X", "utf8", str(GATE), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(cwd), shell=False,
                          timeout=300, env=env)


def test_baseline_exists_and_gate_green():
    """基线在、门绿——棘轮位。"""
    assert BASELINE.is_file(), "缺 spec/vigilo-baseline.json——先跑 --write-baseline"
    cp = _run()
    assert cp.returncode == 0, f"vigilo 门红了：\n{cp.stdout[-500:]}\n{cp.stderr[-500:]}"
    assert "VIGILO-GATE OK" in cp.stdout, cp.stdout


def test_vigilo_actually_scans():
    """vigilo 真在跑——输出里有发现数（不是空壳门）。"""
    cp = _run("--list")
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "VIGILO-GATE" in cp.stdout
    assert "发现=" in cp.stdout
    lines = cp.stdout.strip().split("\n")
    header = lines[0]
    count = int(header.split("发现=")[1].split()[0])
    assert count > 0, "vigilo 报了 0 条发现——要么没装好、要么扫描面空了"


def test_new_findings_cause_failure():
    """真门验证：基线里塞一条假键 ⇒ 所有真实发现都变"新增" ⇒ 判红。"""
    fake_baseline = {
        "keys": ["FAKE:nonexistent.py:999:0"],
        "count": 1,
        "by_rule": {"FAKE": 1},
    }
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False,
                                     dir=ROOT / "spec", encoding="utf-8") as f:
        json.dump(fake_baseline, f)
        tmp_path = f.name
    try:
        cp = _run("--baseline", tmp_path)
        assert cp.returncode == 1, f"假基线应该让门红，但 returncode={cp.returncode}\n{cp.stdout}"
        assert "VIGILO-GATE FAIL" in cp.stdout
        assert "新增" in cp.stdout
    finally:
        os.unlink(tmp_path)


def test_write_baseline_updates():
    """--write-baseline 能更新基线。"""
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False,
                                     dir=ROOT / "spec") as f:
        tmp_path = f.name
    try:
        cp = _run("--write-baseline", "--baseline", tmp_path)
        assert cp.returncode == 0, f"--write-baseline 失败：\n{cp.stdout}\n{cp.stderr}"
        assert "已写基线" in cp.stdout
        data = json.loads(pathlib.Path(tmp_path).read_text(encoding="utf-8"))
        assert "keys" in data
        assert data["count"] > 0
    finally:
        if os.path.exists(tmp_path):
            os.unlink(tmp_path)
