"""cargo-lock 的金丝雀：合成一个**只含路径依赖**的小工作区，把漂移钉成会判红的门。

语料锚的是 M3-1 真实翻车形状——成员 `a` 后来才声明 `b = { path = "../b" }`，锁里 `a` 的
依赖清单没有 `b`。这里的期望值来自锁文件的实际内容（生成锁时 `a` 还没有那条依赖），
不是由门作者按同一假设手写第二遍：门若把这种漂移读成绿，本测试当场红。

刻意**不设 skipif**：cargo 不可用时脚本判 inconclusive，测试会带着 `UNVERIFIABLE`
点名失败，而不是静默跳过（跳过的金丝雀等于没有金丝雀）。
"""
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "cargo_lock.py"

_PKG = '[package]\nname="{name}"\nversion="0.1.0"\nedition="2021"\n{deps}'


def _run(*args, cwd=None):
    return subprocess.run([sys.executable, "-X", "utf8", str(SCRIPT), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(cwd or ROOT))


def _cargo(root: pathlib.Path, *args):
    return subprocess.run(["cargo", *args], cwd=str(root), capture_output=True,
                          text=True, encoding="utf-8", errors="replace")


def _fixture(tmp_path: pathlib.Path) -> pathlib.Path:
    """a、b 两个成员；先让 a **不**依赖 b，锁就在这个状态下生成。"""
    for name in ("a", "b"):
        (tmp_path / name / "src").mkdir(parents=True)
        (tmp_path / name / "src" / "lib.rs").write_text("", encoding="utf-8")
    (tmp_path / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["a", "b"]\nresolver = "2"\n', encoding="utf-8")
    (tmp_path / "a" / "Cargo.toml").write_text(_PKG.format(name="a", deps=""),
                                               encoding="utf-8")
    (tmp_path / "b" / "Cargo.toml").write_text(_PKG.format(name="b", deps=""),
                                               encoding="utf-8")
    gen = _cargo(tmp_path, "metadata", "--format-version", "1")
    assert gen.returncode == 0, "合成语料本身编不出来（cargo 不可用？）：" + gen.stderr
    assert (tmp_path / "Cargo.lock").is_file(), "生成锁失败，语料不成立"
    return tmp_path


def _a_declares_b(root: pathlib.Path) -> None:
    (root / "a" / "Cargo.toml").write_text(
        _PKG.format(name="a", deps='\n[dependencies]\nb = { path = "../b" }\n'),
        encoding="utf-8")


def test_drifted_lock_is_red(tmp_path):
    ws = _fixture(tmp_path)
    _a_declares_b(ws)
    lock = (ws / "Cargo.lock").read_text(encoding="utf-8")
    assert 'b = { path = "../b" }' not in lock, "前提不成立：锁已经同步，就没有漂移可测"
    cp = _run("--root", str(ws))
    assert cp.returncode == 1, "锁漂移必须判红（这正是不补门时全绿带过的那一步）：" + cp.stdout
    assert "CARGO-LOCK RED" in cp.stdout, cp.stdout


def test_same_fixture_turns_green_once_the_lock_is_regenerated(tmp_path):
    """同一把尺两态对照：只重生成锁、不动判据，红要能翻绿（否则门是在钉住环境而非漂移）。"""
    ws = _fixture(tmp_path)
    _a_declares_b(ws)
    assert _run("--root", str(ws)).returncode == 1, "改前基线：漂移态应先判红"
    regen = _cargo(ws, "metadata", "--format-version", "1")
    assert regen.returncode == 0, regen.stderr
    cp = _run("--root", str(ws))
    assert cp.returncode == 0, "锁补上后同一判据必须绿：" + cp.stdout
    assert "CARGO-LOCK GREEN" in cp.stdout, cp.stdout


def test_missing_lock_is_red_not_quiet_green(tmp_path):
    ws = _fixture(tmp_path)
    (ws / "Cargo.lock").unlink()
    cp = _run("--root", str(ws))
    assert cp.returncode == 1, "依赖账整份不在 ⇒ 不许判绿也不许退成判不了：" + cp.stdout
    assert "CARGO-LOCK RED" in cp.stdout, cp.stdout


def test_wrong_root_is_inconclusive(tmp_path):
    cp = _run("--root", str(tmp_path))
    assert cp.returncode == 2, "根不对 ⇒ 判不了（2），不得混进漂移账：" + cp.stdout
    assert "CARGO-LOCK INCONCLUSIVE" in cp.stdout, cp.stdout


def test_this_repo_is_not_drifted():
    """本仓自己在门口径下必须是绿——否则上面几条测的是别人家的树。"""
    cp = _run()
    assert cp.returncode == 0, "根 Cargo.lock 与本仓声明不同步：" + cp.stdout
