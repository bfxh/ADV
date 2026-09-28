"""tests/test_s185_quota.py —— per-actor 并发配额（方向⑨）：共享机器上的公平性最小兑现。

口径：默认一个 actor 同时最多 1 个 `shared`（`UNIFIED_RX_HUB_MAX_PER_ACTOR` 可调）；
`exclusive` 不受此限（它受全局互斥）；空 actor（既有测试/手工占位路径）不计入——不改旧语义。
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
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(tmp_path / "hub"))
    pipes = tmp_path / "pipes"
    pipes.mkdir()
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    monkeypatch.delenv("UNIFIED_RX_HUB_MAX_PER_ACTOR", raising=False)
    return {"pipes": pipes}


def test_same_actor_second_shared_rejected(env):
    pid = os.getpid()
    assert hub_core.admit("a1", "shared", pid, actor="alice")["ok"] is True
    r = hub_core.admit("a2", "shared", pid, actor="alice")
    assert r["ok"] is False and "上限" in r["reason"] and "alice" in r["reason"]


def test_other_actor_allowed(env):
    pid = os.getpid()
    hub_core.admit("b1", "shared", pid, actor="alice")
    assert hub_core.admit("b2", "shared", pid, actor="bob")["ok"] is True


def test_blank_actor_not_counted_backward_compatible(env):
    """空 actor 不计入（既有测试/手工占位走这条）——配额不改变旧语义。"""
    pid = os.getpid()
    assert hub_core.admit("c1", "shared", pid)["ok"] is True
    assert hub_core.admit("c2", "shared", pid)["ok"] is True


def test_quota_env_override(env, monkeypatch):
    monkeypatch.setenv("UNIFIED_RX_HUB_MAX_PER_ACTOR", "2")
    pid = os.getpid()
    assert hub_core.admit("d1", "shared", pid, actor="alice")["ok"] is True
    assert hub_core.admit("d2", "shared", pid, actor="alice")["ok"] is True
    assert hub_core.admit("d3", "shared", pid, actor="alice")["ok"] is False


def test_exclusive_not_limited_by_actor_quota(env):
    """exclusive 被拒的理由必须是**全局互斥**，不能被配额挡（两条语义分开）。"""
    pid = os.getpid()
    assert hub_core.admit("e1", "exclusive", pid, actor="alice")["ok"] is True
    r = hub_core.admit("e2", "exclusive", pid, actor="alice")
    assert r["ok"] is False and "独占" in r["reason"]


def test_status_exposes_actor_counts(env):
    pid = os.getpid()
    hub_core.admit("f1", "shared", pid, actor="alice")
    hub_core.admit("f2", "shared", pid, actor="bob")
    import tools.hub as hub_tools
    st = hub_tools.hub_status()
    assert st["active_by_actor"] == {"alice": 1, "bob": 1}
    assert st["max_per_actor"] == 1


def test_run_pipeline_passes_actor_to_admit(env):
    """端到端：`run_pipeline` 的 actor 真的进准入（同 actor 第二次被拒）。"""
    (env["pipes"] / "adv.p.json").write_text(json.dumps(
        {"id": "adv.p", "title": "t", "when": "t", "resource_class": "shared",
         "on": ["manual"],
         "steps": [{"step": "s",
                    "cmd": [PY, "-X", "utf8", "-c", "import time; time.sleep(0.4)"],
                    "timeout_s": 60}]}), encoding="utf-8")
    pipes, _ = hub_core.load_pipelines()
    hub_core.admit("hold", "shared", os.getpid(), actor="mcp")
    res = hub_runner.run_pipeline(pipes["adv.p"], actor="mcp", trigger="manual")
    assert res.get("busy") is True and "上限" in str(res.get("error")), res
