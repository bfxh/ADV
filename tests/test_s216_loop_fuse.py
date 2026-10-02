"""S216（片 D-3）：agent 变参空转熔断（breaker 工具级闸）的金丝雀测试。

真实缺口（先量所得）：per-key 熔断按 (工具+参数) 分组，**换参数穷举但不推进**的死循环
从 per-key 与全局 QPM(3000/min) 之间漏过去。D-3 补一把按**工具**聚合的闸：同工具窗口内
**多组不同参数**却反复返回**逐字节相同结果**达阈 → 工具级空转熔断。

锁死：真触发 · 正常 fan-out 不误伤 · 同参数走老路不误触 · 冷却期换参数也拒 · 可复位 · 阈=0 关。
"""
import pytest

import tools  # noqa: F401
from tools import breaker


class _Clock:
    def __init__(self, t=1000.0):
        self.t = t

    def __call__(self):
        return self.t


@pytest.fixture(autouse=True)
def _clean(monkeypatch):
    monkeypatch.setenv("UNIFIED_RX_BREAKER", "on")
    monkeypatch.setenv("UNIFIED_RX_BREAKER_LIMIT", "50")          # 抬高，别干扰（只验 spin）
    monkeypatch.setenv("UNIFIED_RX_BREAKER_WINDOW_S", "300")
    monkeypatch.setenv("UNIFIED_RX_BREAKER_COOLDOWN_S", "60")
    monkeypatch.setenv("UNIFIED_RX_BREAKER_SPIN_LIMIT", "4")
    monkeypatch.setenv("UNIFIED_RX_GLOBAL_QPM", "1000000")
    monkeypatch.setenv("UNIFIED_RX_DAILY_ALERT", "0")
    breaker.reset()
    yield
    breaker.reset()


def _spin_in(monkeypatch):
    monkeypatch.setattr(breaker, "_now", _Clock())


def test_varying_args_same_result_trips(monkeypatch):
    """4 组不同参数 → 同一结果 ⇒ 第 5 次 check 被工具级空转闸拒。"""
    _spin_in(monkeypatch)
    result = {"ok": True, "result": {"issues": []}}               # 恒定空结果
    for i in range(4):                                            # 达到 spin_limit=4
        breaker.record("bug_scan", {"path": f"D:/p{i}"}, result)  # 每次参数不同、结果相同
    msg = breaker.check("bug_scan", {"path": "D:/p99"})           # 换任何参数都应被拒
    assert msg and "变参空转" in msg, msg


def test_fanout_with_varying_results_not_tripped(monkeypatch):
    """正常 fan-out：参数不同、结果也不同 ⇒ 不触发空转闸（关键反例，防误伤）。"""
    _spin_in(monkeypatch)
    for i in range(6):
        breaker.record("bug_scan", {"path": f"D:/p{i}"},
                       {"ok": True, "result": {"issues": [{"line": i}]}})  # 结果各异
    assert breaker.check("bug_scan", {"path": "D:/new"}) is None


def test_spin_off_when_limit_zero(monkeypatch):
    """UNIFIED_RX_BREAKER_SPIN_LIMIT=0 ⇒ 关闭变参空转闸。"""
    monkeypatch.setenv("UNIFIED_RX_BREAKER_SPIN_LIMIT", "0")
    _spin_in(monkeypatch)
    result = {"ok": True, "result": {"issues": []}}
    for i in range(8):
        breaker.record("bug_scan", {"path": f"D:/p{i}"}, result)
    assert breaker.check("bug_scan", {"path": "D:/x"}) is None


def test_reset_clears_spin(monkeypatch):
    """trip 后 breaker_reset 解除，同工具重新放行。"""
    _spin_in(monkeypatch)
    result = {"ok": True, "result": {"issues": []}}
    for i in range(4):
        breaker.record("fs_read", {"path": f"D:/a{i}"}, result)
    assert breaker.check("fs_read", {"path": "D:/y"}) is not None
    breaker.reset("fs_read")
    assert breaker.check("fs_read", {"path": "D:/z"}) is None, "复位后空转闸应解除"


def test_cooldown_recovers_spin(monkeypatch):
    """冷却到期后空转态自动清空、恢复放行（不永久锁死）。"""
    clk = _Clock(1000.0)
    monkeypatch.setattr(breaker, "_now", lambda: clk.t)
    result = {"ok": True, "result": {"issues": []}}
    for i in range(4):
        breaker.record("grep", {"pat": f"p{i}"}, result)
    assert breaker.check("grep", {"pat": "zzz"}) is not None
    clk.t += 61                                                  # 过冷却 60s
    assert breaker.check("grep", {"pat": "still"}) is None, "冷却后应自动恢复"
