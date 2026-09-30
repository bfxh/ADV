"""S193 判据：覆盖率门**失败取证**（`scripts/coverage_gate.py`）。

由来（出处可追）：PR #108 的 `gates` 里 coverage-gate 红，而门只打印 cargo 输出的**最后
300 字符** ⇒ `panicked at …` 那行正好被截掉，"FAIL 不静默"只做到"说失败"、拿不到真因，
我只能靠金丝雀反推根因。本判据把"失败必须说清为什么"钉住，并证明**旧行为会被判红**：
夹具把 panic 行放在 300 字符之外 ⇒ 断言异常文本里必须有它。

判据两头都有：①失败路径**点名**（`panicked at`/`error`/`test failed` 行必现）+ 尾巴；
②成功路径不受影响（仍返回解析出的覆盖率）；③输出形状不认识仍抛且带前缀；④确定性。
"""
import json
import subprocess
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

import coverage_gate as C  # noqa: E402

# 让"最后 300 字符"旧口径必然丢掉 panic 行：panic 行之后塞 > 300 字符的噪声
_PANIC = "thread 'nonatomic_write_is_reported_at_exact_line' panicked at tests\\save_rules_test.rs:90:5:"
_NOISE = "\n".join(f"noise line {i} " + "x" * 40 for i in range(20))


def _fake_run(rc=101, stdout="", stderr=""):
    def run(*_a, **_k):
        return types.SimpleNamespace(returncode=rc, stdout=stdout, stderr=stderr)
    return run


def test_failure_names_the_panic_line_beyond_300_chars(monkeypatch):
    """金丝雀：panic 行在 300 字符之外 ⇒ 必须仍然出现在异常文本里（旧口径会丢）。"""
    stderr = f"{_PANIC}\n{_NOISE}"
    assert stderr.find(_PANIC) < len(stderr) - 300, "夹具前提：panic 行在尾部 300 字符之外"
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=101, stderr=stderr))
    try:
        C.measure()
    except RuntimeError as e:
        msg = str(e)
    else:
        raise AssertionError("非零退出必须抛 RuntimeError")
    assert _PANIC in msg, f"取证里必须点名 panic 行：{msg[:400]}"
    assert "rc=101" in msg, msg
    assert "|" in msg, "取证是逐行拼接（可读）"


def test_failure_falls_back_to_tail_when_no_keyword(monkeypatch):
    """没有 panic/error 关键词时，给尾巴若干行（不是一片空白）。"""
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=2, stderr="alpha\nbeta\ngamma"))
    try:
        C.measure()
    except RuntimeError as e:
        msg = str(e)
    else:
        raise AssertionError("非零退出必须抛 RuntimeError")
    assert "gamma" in msg and "alpha" in msg, msg


def test_success_path_still_returns_coverage(monkeypatch):
    """成功路径不受取证改动影响：仍解析 llvm-cov 的两种形状。"""
    flat = json.dumps({"totals": {"line_percent": 67.8651}})
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=0, stdout=flat))
    assert abs(C.measure() - 67.8651) < 1e-9
    nested = json.dumps({"data": [{"totals": {"lines": {"percent": 71.25}}}]})
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=0, stdout=nested))
    assert C.measure() == 71.25


def test_unknown_shape_still_fails_loudly(monkeypatch):
    """形状不认识（不是 JSON / 缺字段）仍抛，且带上原文头部——不许静默返回 0。"""
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=0, stdout="not json at all"))
    try:
        C.measure()
    except RuntimeError as e:
        assert "不是 JSON" in str(e), e
    else:
        raise AssertionError("坏 JSON 必须抛")
    monkeypatch.setattr(C.subprocess, "run", _fake_run(rc=0, stdout=json.dumps({"other": 1})))
    try:
        C.measure()
    except RuntimeError as e:
        assert "缺行覆盖率字段" in str(e), e
    else:
        raise AssertionError("缺字段必须抛")


def test_context_helper_is_deterministic():
    """同一输入两次拼接逐字相同（去重保序，无哈希序渗入）。"""
    a = C._failure_context("out1\nout2", "err1\npanicked at x.rs:1:1")
    b = C._failure_context("out1\nout2", "err1\npanicked at x.rs:1:1")
    assert a == b and "panicked at" in a and a.startswith("panicked at"), a


def test_subprocess_signature_unchanged():
    """调用契约未动：argv 列表 + shell=False + 不捕获失败（rc 由我们判）。"""
    assert callable(subprocess.run)
    src = (ROOT / "scripts" / "coverage_gate.py").read_text(encoding="utf-8")
    assert "shell=False" in src and "capture_output=True" in src, "argv 列表 + 无 shell 纪律"
