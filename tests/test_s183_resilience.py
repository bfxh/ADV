"""tests/test_s183_resilience.py —— 对平台自身注入写故障（方向⑥ 混沌）。

命题：**存储坏了，平台必须如实失败**——不静默、不锁死、不报假绿。

复现证据（修复前）：账本不可写时 `run_pipeline` 抛异常且**留下一条 exclusive 活跃位**
（pid 活着）⇒ 平台被自己锁死、且无账可查；日志目录不可写时异常直接冒泡。
"""
import json
import os
import sys

import pytest

import hub_core
import hub_runner

PY = sys.executable


@pytest.fixture()
def env(tmp_path, monkeypatch):
    root = tmp_path / "hub"
    pipes = tmp_path / "pipes"
    pipes.mkdir()
    (pipes / "adv.ok.json").write_text(json.dumps(
        {"id": "adv.ok", "title": "t", "when": "t", "resource_class": "exclusive",
         "on": ["manual"],
         "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", "print(1)"],
                    "timeout_s": 60}]}), encoding="utf-8")
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(root))
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    return {"root": root, "pipes": pipes}


def _run():
    pipes, _ = hub_core.load_pipelines()
    return hub_runner.run_pipeline(pipes["adv.ok"], actor="test", trigger="chaos")


def test_ledger_unwritable_rejected_without_leak(env):
    """账本位置被目录占位 ⇒ 如实拒绝 + **活跃位不泄漏** + 链不报假绿（三件都要）。"""
    env["root"].mkdir(parents=True, exist_ok=True)
    (env["root"] / "runs.jsonl").mkdir()
    res = _run()
    assert res["ok"] is False and "账本不可写" in str(res.get("error")), res
    assert hub_core.read_active() == []                    # 关键①：不锁死平台
    assert hub_core.verify_chain()["ok"] is False          # 关键②：不报"链 OK"假绿
    assert "不可读" in str(hub_core.verify_chain().get("reason"))


def test_logs_unwritable_is_red_and_releases(env):
    """日志目录被文件占位 ⇒ 如实报失败（red）+ 活跃位不泄漏。"""
    env["root"].mkdir(parents=True, exist_ok=True)
    (env["root"] / "logs").write_text("occupied", encoding="utf-8")
    res = _run()
    assert res["ok"] is False and res.get("verdict") == "red", res
    assert hub_core.read_active() == []


def test_status_reports_degraded_when_root_unwritable(env):
    """数据根不可写 ⇒ storage_writable=False 且 degraded 原因可读（不假装健康）。"""
    env["root"].parent.mkdir(parents=True, exist_ok=True)
    env["root"].write_text("occupied", encoding="utf-8")
    assert hub_core.storage_writable() is False
    import tools.hub as hub_tools
    st = hub_tools.hub_status()
    assert st["degraded"] is True
    assert any("不可写" in r for r in st["degraded_reasons"])
    assert st["storage_writable"] is False


def test_read_path_does_not_crash_on_bad_ledger(env):
    """读路径容错：账本被目录占位时返回空表而不抛（"盘坏了"要可诊断，不是异常）。"""
    env["root"].mkdir(parents=True, exist_ok=True)
    (env["root"] / "runs.jsonl").mkdir()
    assert hub_core.read_rows() == []
    assert hub_core.latest_runs(5) == []


def test_happy_path_still_green(env):
    """对照：正常环境仍绿——混沌注入不得把好路径一起判死。"""
    res = _run()
    assert res["ok"] is True and res["verdict"] == "green", res
    assert hub_core.read_active() == []
    assert hub_core.verify_chain()["ok"] is True
    assert hub_core.storage_writable() is True


def test_env_restored_after_fixture(tmp_path):
    """防御性：确保测试不污染真实数据根（UNIFIED_RX_HUB_ROOT 未泄漏到全局）。"""
    assert os.environ.get("UNIFIED_RX_HUB_ROOT") is None or "pytest" in (
        os.environ.get("UNIFIED_RX_HUB_ROOT") or "")
