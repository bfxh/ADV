"""S208 契约：测试卡死要留下现场，门步超时要干净判红，两者都不许靠 traceback 结束。

由来（实测的两处真空）：
- `conftest.py` 里**没有任何 stall 检测**，一条挂住的测试会一路静默到 `local_gate.py`
  的 3600 秒上限；那一步的 `subprocess.run(timeout=3600)` 又没有 try/except，
  于是抛 `TimeoutExpired` traceback 打断整条门链——剩下的步骤连跑都不跑。
- 本仓零依赖红线（`ci-requirements.txt` 钉版），不能引 pytest-timeout，
  所以护栏用 stdlib `faulthandler.dump_traceback_later`。

三条判据全部走真路径（subprocess 起 pytest / 起 local_gate），不 import 私有函数当尺：
1. 睡过上限 ⇒ 转储**文件**里出现全部线程栈并点到卡住的函数，且**测试仍然通过**（只诊断不杀）；
2. `UNIFIED_RX_TEST_STALL_S=0` ⇒ 一字节转储都不打（关得掉，不是永远挂着）；
3. 步骤超预算 ⇒ LOCAL-GATE 以 rc=1 干净判红、点名步骤、给复现命令行，且无 traceback。
"""
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PROBE = ROOT / "bench" / "fixtures" / "stall_probe" / "test_stall_probe.py"


def _pytest(stall_s: str, probe_s: str, log: str) -> subprocess.CompletedProcess:
    env = dict(os.environ, PYTHONUTF8="1", UNIFIED_RX_TEST_STALL_S=str(stall_s),
               UNIFIED_RX_STALL_PROBE_S=probe_s, UNIFIED_RX_TEST_STALL_LOG=log)
    return subprocess.run([sys.executable, "-X", "utf8", "-m", "pytest", str(PROBE),
                           "-rA", "-p", "no:cacheprovider"], capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          cwd=str(ROOT), env=env, timeout=180, shell=False)


def test_stall_watchdog_dumps_where_it_hangs(tmp_path):
    """第一版栽过的坑：转储打 stderr 会被 pytest 的按测试捕获吃掉——测试最终通过时那段栈
    直接丢弃，而"很慢但还是过了"正是最需要看它的时刻。所以判据看**转储文件**。"""
    log = str(tmp_path / "stall.log")
    r = _pytest(stall_s="1", probe_s="3", log=log)
    assert r.returncode == 0, f"护栏把慢判成了红：\n{(r.stdout + r.stderr)[-800:]}"
    assert "stall-watchdog" in r.stdout, f"头部没交代转储落在哪：\n{r.stdout[-800:]}"
    assert os.path.isfile(log), "睡过了上限却没留下转储文件"
    blob = pathlib.Path(log).read_text(encoding="utf-8", errors="replace")
    assert "most recent call first" in blob, f"转储里没有线程栈：\n{blob[-600:]}"
    assert "test_stall_probe" in blob, f"转储没指到卡住的那个函数：\n{blob[-600:]}"


def test_watchdog_can_be_turned_off(tmp_path):
    log = str(tmp_path / "off.log")
    r = _pytest(stall_s="0", probe_s="2", log=log)
    assert r.returncode == 0
    assert "已关闭" in r.stdout, f"关掉时头部要如实说：\n{r.stdout[-600:]}"
    assert not os.path.isfile(log), "关掉了还创建转储文件"


def test_step_timeout_is_clean_fail_not_traceback(tmp_path):
    env = dict(os.environ, PYTHONUTF8="1", UNIFIED_RX_GATE_STEP_TIMEOUT_S="0.05")
    r = subprocess.run([sys.executable, "-X", "utf8", "scripts/local_gate.py",
                        "--only", "stress"], capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=str(ROOT), env=env,
                       timeout=180, shell=False)
    blob = r.stdout + r.stderr
    assert r.returncode == 1, f"超预算却没判红：rc={r.returncode}\n{blob[-600:]}"
    assert "TIMEOUT" in blob and "stress" in blob, f"判红了但没点名步骤：\n{blob[-600:]}"
    assert "复现：" in blob, f"没给一键复现命令：\n{blob[-600:]}"
    assert "Traceback (most recent call last)" not in blob, \
        f"又用 traceback 打断整条门链了：\n{blob[-900:]}"


def test_probe_file_is_not_in_the_normal_suite():
    """探针必须留在 tests/ 之外，否则它自己就成了那条卡住的测试。"""
    listed = subprocess.run([sys.executable, "-X", "utf8", "scripts/shard_plan.py",
                             "--plan", "1", "--shard", "1"], capture_output=True,
                            text=True, encoding="utf-8", cwd=str(ROOT), timeout=120,
                            shell=False)
    assert "stall_probe" not in listed.stdout, "探针进了分片计划（它会把整片睡死）"
