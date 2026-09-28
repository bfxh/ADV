"""tests/test_s176_hub.py —— 平台层判据单测（spec/HUB.md §6.3）。

覆盖：manifest 严格校验（未知字段/命令白名单/步数）· 账本哈希链 + 篡改 · 跨进程护栏
（busy/陈旧接管）· 执行器三态判定（green/red/green_with_skips）· 指纹八项 · 封印重算 ·
verify_plan 可执行 · 授权矩阵（令牌/角色/MCP 侧 __authorized）· HTTP 面（读放行/写需令牌）。
"""
import hashlib
import json
import os
import pathlib
import sys
import threading
import time

import pytest

import hub_auth
import hub_core
import hub_runner

PY = sys.executable


@pytest.fixture()
def hub_root(tmp_path, monkeypatch):
    """隔离数据根 + 管线目录（不触真实 ~/.ADV/hub）。"""
    root = tmp_path / "hub"
    pipes = tmp_path / "pipes"
    pipes.mkdir(parents=True, exist_ok=True)
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(root))
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    return {"root": root, "pipes": pipes}


def _manifest(pid, code, **kw):
    m = {"id": pid, "title": pid, "when": "t", "resource_class": "shared",
         "on": ["manual"],
         "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", code], "timeout_s": 60}]}
    m.update(kw)
    return m


def _put(pipes: pathlib.Path, pid: str, code: str, **kw) -> None:
    (pipes / f"{pid}.json").write_text(json.dumps(_manifest(pid, code, **kw)),
                                       encoding="utf-8")


def _sha256_file(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


# ---------------- manifest 严格校验 ----------------

def test_manifest_rejects_unknown_field():
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.x.json",
                                   {**_manifest("adv.x", "print(1)"), "extra": 1})


def test_manifest_rejects_id_mismatch_and_bad_id():
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.y.json", _manifest("adv.x", "print(1)"))
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.y.json",
                                   {**_manifest("adv.y", "print(1)"), "id": "Bad_ID"})


def test_manifest_rejects_cmd_outside_whitelist():
    m = _manifest("adv.x", "print(1)")
    m["steps"][0]["cmd"] = ["rm", "-rf", "."]
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.x.json", m)


def test_manifest_rejects_step_limits():
    m = _manifest("adv.x", "print(1)")
    m["steps"] = [{"step": f"s{i}", "cmd": [PY, "-c", "print(1)"]} for i in range(17)]
    with pytest.raises(ValueError):
        hub_core.validate_manifest("adv.x.json", m)


def test_load_pipelines_lists_invalid(hub_root):
    pipes = hub_root["pipes"]
    _put(pipes, "adv.ok", "print('ok')")
    (pipes / "adv.bad.json").write_text("{not json", encoding="utf-8")
    valid, invalid = hub_core.load_pipelines()
    assert "adv.ok" in valid and "adv.bad" in invalid


# ---------------- 账本链 ----------------

def test_chain_append_verify_and_tamper(hub_root):
    hub_core.append_row({"id": "r1", "phase": "start"})
    hub_core.append_row({"id": "r1", "phase": "final", "verdict": "green"})
    assert hub_core.verify_chain()["ok"] is True
    p = hub_core.runs_path()
    text = p.read_text(encoding="utf-8")
    assert '"verdict":"green"' in text                    # 紧凑 JSON（账本口径）
    p.write_text(text.replace('"verdict":"green"', '"verdict":"gren"', 1), encoding="utf-8")
    v = hub_core.verify_chain()
    assert v["ok"] is False and v["broken_index"] == 1


def test_latest_runs_keeps_final_row(hub_root):
    hub_core.append_row({"id": "r1", "phase": "start"})
    hub_core.append_row({"id": "r1", "phase": "final", "verdict": "red"})
    hub_core.append_row({"id": "r2", "phase": "start"})
    by_id = {r["id"]: r for r in hub_core.latest_runs(5)}
    assert by_id["r1"]["verdict"] == "red" and by_id["r2"]["phase"] == "start"


# ---------------- 资源级准入（S178：shared 并行 / exclusive 互斥） ----------------

def test_admit_matrix_and_stale_cleanup(hub_root):
    """准入矩阵：shared 并行到上限、exclusive 互斥、未知资源级拒、陈旧自动清理。"""
    assert hub_core.admit("s1", "shared", os.getpid())["ok"] is True
    assert hub_core.admit("s2", "shared", os.getpid())["ok"] is True
    third = hub_core.admit("s3", "shared", os.getpid())
    assert third["ok"] is False and "上限" in third["reason"]
    excl = hub_core.admit("x1", "exclusive", os.getpid())
    assert excl["ok"] is False and "exclusive" in excl["reason"]
    assert hub_core.admit("z", "wild", os.getpid())["ok"] is False
    hub_core.release_active("s1")
    hub_core.release_active("s2")
    assert hub_core.read_active() == []
    assert hub_core.admit("x1", "exclusive", os.getpid())["ok"] is True   # 清空后可独占
    shared_now = hub_core.admit("s9", "shared", os.getpid())
    assert shared_now["ok"] is False and "exclusive" in shared_now["reason"]
    hub_core.release_active("x1")
    (hub_core.active_dir() / "dead.json").write_text(
        json.dumps({"run_id": "dead", "pid": 999999999, "resource_class": "exclusive"}),
        encoding="utf-8")
    assert hub_core.read_active() == []            # 死 pid ⇒ 清理，不阻塞后续运行


def test_runner_shared_parallel_and_exclusive_mutex(hub_root):
    """两个 shared 真共存（活跃集同时含两个）；across 期间 exclusive 被拒，清空后可跑。"""
    _put(hub_root["pipes"], "adv.p1", "import time; time.sleep(0.8); print('p1')")
    _put(hub_root["pipes"], "adv.p2", "import time; time.sleep(0.8); print('p2')")
    _put(hub_root["pipes"], "adv.x1", "print('x1')", resource_class="exclusive")
    pipes, _ = hub_core.load_pipelines()
    box: dict = {}
    th1 = threading.Thread(target=lambda: box.update(a=hub_runner.run_pipeline(pipes["adv.p1"])),
                           daemon=True)
    th2 = threading.Thread(target=lambda: box.update(b=hub_runner.run_pipeline(pipes["adv.p2"])),
                           daemon=True)
    th1.start()
    th2.start()
    coex = False
    for _ in range(120):
        if len(hub_core.read_active()) >= 2:
            coex = True
            break
        time.sleep(0.02)
    x = hub_runner.run_pipeline(pipes["adv.x1"])
    th1.join(30)
    th2.join(30)
    assert coex, "两个 shared 未同时进入活跃集"
    assert box["a"]["verdict"] == "green" and box["b"]["verdict"] == "green"
    assert x["busy"] is True and "exclusive" in str(x.get("error"))
    assert hub_runner.run_pipeline(pipes["adv.x1"])["ok"] is True      # 活跃清空后独占可跑


# ---------------- 执行器三态 + 指纹 + 封印 + 复核计划 ----------------

def test_runner_three_verdicts(hub_root):
    _put(hub_root["pipes"], "adv.ok", "print('ok')")
    _put(hub_root["pipes"], "adv.bad", "import sys; sys.exit(3)")
    _put(hub_root["pipes"], "adv.skip", "print('SKIP one')")
    pipes, _ = hub_core.load_pipelines()
    assert hub_runner.run_pipeline(pipes["adv.ok"])["verdict"] == "green"
    assert hub_runner.run_pipeline(pipes["adv.bad"])["verdict"] == "red"
    assert hub_runner.run_pipeline(pipes["adv.skip"])["verdict"] == "green_with_skips"


def test_runner_fingerprint_seal_and_verify_plan(hub_root):
    _put(hub_root["pipes"], "adv.ok", "print('ok')")
    pipes, _ = hub_core.load_pipelines()
    res = hub_runner.run_pipeline(pipes["adv.ok"])
    for k in ("manifest_sha256", "git_head", "git_dirty", "argv0", "cwd",
              "env_keys_hash", "python", "cargo"):
        assert k in res["fingerprint"]
    row = hub_core.latest_runs(1)[0]
    payload = {"pipeline": row["pipeline"],
               "manifest_sha256": row["fingerprint"]["manifest_sha256"],
               "git_head": row["fingerprint"]["git_head"],
               "git_dirty": row["fingerprint"]["git_dirty"],
               "verdict": row["verdict"],
               "steps": [{k: s[k] for k in ("step", "exit", "out_sha256", "err_sha256")}
                         for s in row["steps"]]}
    assert hub_runner.seal_of(payload) == row["seal"] == res["seal"]
    kinds = {e["kind"] for e in res["verify_plan"]}
    assert {"log_hash", "direct_rerun", "chain", "seal"} <= kinds
    entry = res["verify_plan"][0]
    assert _sha256_file(pathlib.Path(entry["path"])) == entry["expect_sha256"]
    assert any("封条重算" in line for line in res["how_to_verify"])


# ---------------- 身份与授权矩阵 ----------------

def test_auth_bootstrap_roles_and_sessions(hub_root, capsys):
    boot = hub_auth.bootstrap()
    assert boot["created"] is True and boot["bootstrap_token"]
    assert "仅显示一次" in capsys.readouterr().err
    tok = boot["bootstrap_token"]
    assert hub_auth.verify("admin", tok) == "admin"
    assert hub_auth.verify("admin", "wrong") is None
    assert hub_auth.verify("nobody", tok) is None
    assert hub_auth.can_trigger("viewer") is False
    assert hub_auth.can_trigger("operator") is True
    sid = hub_auth.new_session("admin", "admin")
    assert hub_auth.session_user(sid) == ("admin", "admin")
    assert hub_auth.count_users() == 1
    assert hub_auth.bootstrap()["created"] is False        # 幂等：不重建


def test_auth_bad_table_fails_closed(hub_root):
    p = hub_auth.users_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("{broken", encoding="utf-8")
    assert hub_auth.verify("admin", "x") is None
    assert hub_auth.bootstrap()["error"]


def test_mcp_side_requires_authorized():
    import registry
    denied = registry.call("hub_run", {"pipeline": "adv.gate-fast"})
    assert denied["ok"] is False


def test_hub_tools_registered_with_auth_schema():
    import registry
    tools = {t["name"]: t for t in registry.list_tools()}
    for name in ("hub_status", "hub_pipelines", "hub_runs", "hub_run"):
        assert name in tools, f"{name} 未注册"
    props = tools["hub_run"]["inputSchema"]["properties"]
    assert props["__authorized"]["type"] == "boolean"
    assert "hub_status" in tools and tools["hub_status"]["annotations"]["readOnlyHint"] is True


# ---------------- HTTP / RBAC 面已迁至 tests/test_s179_auth.py ----------------
# （S179 起：读面要会话、用户管理属 admin 面——按轮次分文件，断言在新文件里升级；
#   此前本文件曾因一次编辑吃掉函数头把两个测试合并，搬迁时一并修正。）
