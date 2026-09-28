"""tests/test_s176_hub.py —— 平台层判据单测（spec/HUB.md §6.3）。

覆盖：manifest 严格校验（未知字段/命令白名单/步数）· 账本哈希链 + 篡改 · 跨进程护栏
（busy/陈旧接管）· 执行器三态判定（green/red/green_with_skips）· 指纹八项 · 封印重算 ·
verify_plan 可执行 · 授权矩阵（令牌/角色/MCP 侧 __authorized）· HTTP 面（读放行/写需令牌）。
"""
import hashlib
import http.client
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
import server_web

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


# ---------------- 跨进程护栏 ----------------

def test_active_guard_busy_and_stale_takeover(hub_root):
    assert hub_core.acquire_active("r1", os.getpid()) is True
    assert hub_core.acquire_active("r2", os.getpid()) is False        # 活持有者 → busy
    hub_core.release_active("r1")
    assert hub_core.read_active() is None
    hub_core.active_path().write_text(json.dumps({"run_id": "old", "pid": 999999999}),
                                      encoding="utf-8")
    assert hub_core.acquire_active("r3", os.getpid()) is True          # 陈旧 → 接管


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


# ---------------- HTTP 面（读放行 / 写需令牌） ----------------

@pytest.fixture()
def web(hub_root):
    _put(hub_root["pipes"], "adv.ok", "print('ok')")
    srv = server_web.make_server(0)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield (str(srv.server_address[0]), int(srv.server_address[1]))
    srv.shutdown()
    srv.server_close()


def _req(addr, method, path, payload=None, headers=None):
    """只连本进程刚起的回环服务：host 固定 127.0.0.1（白名单，拒绝非回环），
    端口来自进程内 server 对象（非用户输入）；强制 http、不跟随重定向、不出网。"""
    host, port = addr
    if host != "127.0.0.1":
        raise ValueError(f"非回环目标拒绝: {host}")
    conn = http.client.HTTPConnection(host, port, timeout=30)
    try:
        body = json.dumps(payload).encode("utf-8") if payload is not None else None
        hdrs = dict(headers or {})
        if body is not None:
            hdrs["Content-Type"] = "application/json"
        conn.request(method, path, body=body, headers=hdrs)
        resp = conn.getresponse()
        return resp.status, resp.read().decode("utf-8", "replace")
    finally:
        conn.close()


def _json_of(text: str):
    return json.loads(text)


def test_web_read_paths_open_on_loopback(web):
    st, text = _req(web, "GET", "/api/status")
    payload = _json_of(text)
    assert st == 200 and payload["ok"] is True and payload["instance"] in ("dev", "stable")
    st, text = _req(web, "GET", "/api/pipelines")
    assert st == 200 and any(p["id"] == "adv.ok" for p in _json_of(text)["pipelines"])
    st, text = _req(web, "GET", "/")
    assert st == 200 and "<html" in text.lower()


def _read_sse(addr, run_id, max_frames=60):
    """读 SSE 帧 [(event, payload_text)...]；只在回环地址上（见 _req 的白名单口径）。"""
    host, port = addr
    if host != "127.0.0.1":
        raise ValueError(f"非回环目标拒绝: {host}")
    conn = http.client.HTTPConnection(host, port, timeout=40)
    conn.request("GET", f"/api/stream?run={run_id}")
    resp = conn.getresponse()
    events = []
    try:
        while len(events) < max_frames:
            line = resp.fp.readline()
            if not line:
                break
            head = line.decode("utf-8", "replace").strip()
            if not head.startswith("event: "):
                continue
            body = resp.fp.readline().decode("utf-8", "replace").strip()
            payload = body[len("data: "):] if body.startswith("data: ") else "{}"
            events.append((head[len("event: "):], payload))
            if events[-1][0] == "done":
                break
    finally:
        conn.close()
    return events


def test_web_stream_live_log_before_final(web, hub_root):
    """live 硬判据：首条日志事件到达时，该 run 在账本里**还没有 final 行**（真流式）。"""
    _put(hub_root["pipes"], "adv.live",
         "import time; print('live-1', flush=True); time.sleep(1.5); print('live-2', flush=True)")
    pipes, _ = hub_core.load_pipelines()
    box: dict = {}
    th = threading.Thread(target=lambda: box.update(hub_runner.run_pipeline(pipes["adv.live"])),
                          daemon=True)
    th.start()
    rid = None
    for _ in range(100):
        rows = hub_core.read_rows()
        if rows:
            rid = rows[-1]["id"]
            break
        time.sleep(0.05)
    assert rid, "run 未启动"
    events = _read_sse(web, rid)
    kinds = [k for k, _ in events]
    assert "log" in kinds and kinds[-1] == "done", f"事件序列异常: {kinds}"
    text = "".join(json.loads(p)["text"] for k, p in events if k == "log")
    assert "live-1" in text and "live-2" in text, text
    th.join(20)
    assert box.get("verdict") == "green"


def test_web_log_incremental_offset(web, hub_root):
    """增量端点：offset 推进后不再重复返回已读内容；done 标志在结束后为真。"""
    _put(hub_root["pipes"], "adv.ok", "print('inc-1')")
    pipes, _ = hub_core.load_pipelines()
    res = hub_runner.run_pipeline(pipes["adv.ok"])
    st, text = _req(web, "GET", f"/api/runs/{res['run_id']}/log?step=s&stream=out")
    payload = _json_of(text)
    assert st == 200 and "inc-1" in payload["text"] and payload["done"] is True
    st, text = _req(web, "GET",
                    f"/api/runs/{res['run_id']}/log?step=s&stream=out&offset={payload['offset']}")
    assert st == 200 and _json_of(text)["text"] == ""
    st, text = _req(web, "POST", "/api/runs", {"pipeline": "adv.ok"})
    assert st == 401, text
    boot = hub_auth.bootstrap()
    users = json.loads(hub_auth.users_path().read_text(encoding="utf-8"))
    users["users"].append({"name": "viewer1", "role": "viewer",
                           "salt": users["users"][0]["salt"],
                           "token_hash": users["users"][0]["token_hash"]})
    hub_auth.users_path().write_text(json.dumps(users), encoding="utf-8")
    st, _ = _req(web, "POST", "/api/runs", {"pipeline": "adv.ok"},
                 {"Authorization": "Bearer viewer1:whatever"})
    assert st == 401                      # 坏令牌即使名字存在也拒
    auth = {"Authorization": "Bearer admin:" + boot["bootstrap_token"]}
    st, _ = _req(web, "POST", "/api/runs", {"pipeline": "nope"}, auth)
    assert st == 404
    st, text = _req(web, "POST", "/api/runs", {"pipeline": "adv.ok"}, auth)
    payload = _json_of(text)
    assert st == 202 and payload["verdict"] == "green"
    st, text = _req(web, "GET", f"/api/runs/{payload['run_id']}")
    assert st == 200 and _json_of(text)["how_to_verify"]
