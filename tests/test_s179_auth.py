"""tests/test_s179_auth.py —— 平台层 HTTP 鉴权与团队面（S179：读面会话 + RBAC 全量）。

覆盖：状态摘要态公开（未登录也能看见平台是否降级）/ 读面要会话 / 登录-登出循环 /
触发权限（会话 operator+，Bearer 兼容保留）/ 用户管理全 CRUD + 三处防锁死
（最后 admin 不可删/降、重名拒、坏名拒）/ 实时日志与增量端点在会话下工作。
"""
import http.client
import json
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
    root = tmp_path / "hub"
    pipes = tmp_path / "pipes"
    pipes.mkdir(parents=True, exist_ok=True)
    monkeypatch.setenv("UNIFIED_RX_HUB_ROOT", str(root))
    monkeypatch.setenv("UNIFIED_RX_HUB_PIPELINES", str(pipes))
    return {"root": root, "pipes": pipes}


def _put(pipes: pathlib.Path, pid: str, code: str, **kw) -> None:
    m = {"id": pid, "title": pid, "when": "t", "resource_class": "shared", "on": ["manual"],
         "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", code], "timeout_s": 60}]}
    m.update(kw)
    (pipes / f"{pid}.json").write_text(json.dumps(m), encoding="utf-8")


def _req_h(addr, method, path, payload=None, headers=None):
    """只连本进程刚起的回环服务：host 固定 127.0.0.1（白名单），端口来自进程内 server；
    强制 http、不跟随重定向、不出网。返回 (status, text, set_cookie)。"""
    host, port = addr
    if host != "127.0.0.1":
        raise ValueError(f"非回环目标拒绝: {host}")
    conn = http.client.HTTPConnection(host, port, timeout=40)
    try:
        body = json.dumps(payload).encode("utf-8") if payload is not None else None
        hdrs = dict(headers or {})
        if body is not None:
            hdrs["Content-Type"] = "application/json"
        conn.request(method, path, body=body, headers=hdrs)
        resp = conn.getresponse()
        return resp.status, resp.read().decode("utf-8", "replace"), resp.getheader("Set-Cookie")
    finally:
        conn.close()


def _req(addr, method, path, payload=None, headers=None):
    st, text, _sc = _req_h(addr, method, path, payload, headers)
    return st, text


def _auth(web, user="admin", token=None):
    """登录 → Cookie 头；失败即断言失败（测试夹具必须能拿到会话）。"""
    st, _t, sc = _req_h(web["addr"], "POST", "/api/login",
                        {"user": user, "token": token or web["token"]})
    assert st == 200, f"登录失败 {user}: {st}"
    sid = (sc or "").split("hub_sid=")[1].split(";")[0]
    return {"Cookie": f"hub_sid={sid}"}


@pytest.fixture()
def web(hub_root):
    _put(hub_root["pipes"], "adv.ok", "print('ok')")
    boot = hub_auth.bootstrap()
    srv = server_web.make_server(0)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield {"addr": (str(srv.server_address[0]), int(srv.server_address[1])),
           "token": boot["bootstrap_token"]}
    srv.shutdown()
    srv.server_close()


def _read_sse(addr, run_id, cookie, max_frames=60):
    host, port = addr
    if host != "127.0.0.1":
        raise ValueError(f"非回环目标拒绝: {host}")
    conn = http.client.HTTPConnection(host, port, timeout=40)
    conn.request("GET", f"/api/stream?run={run_id}", headers=cookie)
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


# ---------------- 分层：摘要态公开 / 读面要会话 ----------------

def test_status_summary_public_but_read_paths_need_session(web):
    st, text = _req(web["addr"], "GET", "/api/status")
    payload = json.loads(text)
    assert st == 200 and payload["ok"] is True
    assert payload["instance"] in ("dev", "stable") and "chain" in payload
    assert "users" not in payload and "data_root" not in payload      # 摘要态
    st, _ = _req(web["addr"], "GET", "/api/pipelines")
    assert st == 401
    st, _ = _req(web["addr"], "GET", "/api/runs")
    assert st == 401
    st, text = _req(web["addr"], "GET", "/")           # 外壳与登录页要能加载
    assert st == 200 and "<html" in text.lower()


def test_login_logout_cycle_and_full_status(web):
    st, _ = _req(web["addr"], "POST", "/api/login", {"user": "admin", "token": "wrong"})
    assert st == 401
    hdr = _auth(web)
    st, text = _req(web["addr"], "GET", "/api/status", headers=hdr)
    payload = json.loads(text)
    assert payload["user"] == "admin" and payload["role"] == "admin"
    assert "data_root" in payload
    st, text = _req(web["addr"], "GET", "/api/pipelines", headers=hdr)
    assert st == 200 and any(p["id"] == "adv.ok" for p in json.loads(text)["pipelines"])
    st, _ = _req(web["addr"], "POST", "/api/logout", {}, hdr)
    assert st == 200
    st, _ = _req(web["addr"], "GET", "/api/runs", headers=hdr)     # 旧会话已失效
    assert st == 401


# ---------------- 触发权限矩阵 ----------------

def test_trigger_matrix_session_and_bearer(web):
    st, _ = _req(web["addr"], "POST", "/api/runs", {"pipeline": "adv.ok"})
    assert st == 401
    bearer = {"Authorization": "Bearer admin:" + web["token"]}      # 脚本/智能体兼容面
    st, _ = _req(web["addr"], "POST", "/api/runs", {"pipeline": "nope"}, bearer)
    assert st == 404
    st, text = _req(web["addr"], "POST", "/api/runs", {"pipeline": "adv.ok"}, bearer)
    assert st == 202 and json.loads(text)["verdict"] == "green"
    viewer = hub_auth.add_user("viewer1", "viewer")
    operator = hub_auth.add_user("op1", "operator")
    vh = _auth(web, "viewer1", viewer["token"])
    st, _ = _req(web["addr"], "POST", "/api/runs", {"pipeline": "adv.ok"}, vh)
    assert st == 403                                                 # viewer 不能触发
    oh = _auth(web, "op1", operator["token"])
    st, _ = _req(web["addr"], "POST", "/api/runs", {"pipeline": "adv.ok"}, oh)
    assert st == 202


# ---------------- 用户管理（admin）与防锁死 ----------------

def test_users_admin_crud_and_lockout_protection(web):
    viewer = hub_auth.add_user("viewer1", "viewer")
    vh = _auth(web, "viewer1", viewer["token"])
    st, _ = _req(web["addr"], "GET", "/api/users", headers=vh)
    assert st == 403                                                 # 非 admin 不可管用户
    ah = _auth(web)
    st, text = _req(web["addr"], "POST", "/api/users", {"name": "member1", "role": "operator"}, ah)
    payload = json.loads(text)
    assert st == 200 and payload["ok"] and payload["token"]
    st, _ = _req(web["addr"], "POST", "/api/login",
                 {"user": "member1", "token": payload["token"]})
    assert st == 200                                                 # 一次性令牌可用
    st, _ = _req(web["addr"], "POST", "/api/users", {"name": "member1", "role": "viewer"}, ah)
    assert st == 400                                                 # 重名
    st, _ = _req(web["addr"], "POST", "/api/users", {"name": "Bad Name", "role": "viewer"}, ah)
    assert st == 400                                                 # 坏名
    st, _ = _req(web["addr"], "POST", "/api/users/role", {"name": "admin", "role": "viewer"}, ah)
    assert st == 400                                                 # 不能降最后 admin
    st, _ = _req(web["addr"], "DELETE", "/api/users/admin", None, ah)
    assert st == 400                                                 # 不能删最后 admin
    st, text = _req(web["addr"], "POST", "/api/users", {"name": "admin2", "role": "admin"}, ah)
    assert st == 200
    st, _ = _req(web["addr"], "DELETE", "/api/users/member1", None, ah)
    assert st == 200
    assert hub_auth.verify("member1", "x") is None                   # 删了即失效
    st, _ = _req(web["addr"], "DELETE", "/api/users/member1", None, ah)
    assert st == 400                                                 # 再删报"无此用户"
    st, text = _req(web["addr"], "GET", "/api/users", headers=ah)
    names = {u["name"] for u in json.loads(text)["users"]}
    assert names == {"admin", "admin2", "viewer1"}


# ---------------- 会话下的实时面 ----------------

def test_stream_live_log_before_final_with_session(web, hub_root):
    _put(hub_root["pipes"], "adv.live",
         "import time; print('live-1', flush=True); time.sleep(1.5); print('live-2', flush=True)")
    hdr = _auth(web)
    pipes, _ = hub_core.load_pipelines()
    box: dict = {}
    th = threading.Thread(target=lambda: box.update(hub_runner.run_pipeline(pipes["adv.live"])),
                          daemon=True)
    th.start()
    rid = None
    for _ in range(100):
        runs = hub_core.latest_runs(1)          # 账本还含 admin 审计行（无 id）⇒ 按运行取
        if runs:
            rid = runs[0]["id"]
            break
        time.sleep(0.05)
    assert rid, "run 未启动"
    events = _read_sse(web["addr"], rid, hdr)
    kinds = [k for k, _ in events]
    assert "log" in kinds and kinds[-1] == "done", f"事件序列异常: {kinds}"
    text = "".join(json.loads(p)["text"] for k, p in events if k == "log")
    assert "live-1" in text and "live-2" in text
    th.join(20)
    assert box.get("verdict") == "green"


def test_log_incremental_offset_with_session(web):
    hdr = _auth(web)
    pipes, _ = hub_core.load_pipelines()
    res = hub_runner.run_pipeline(pipes["adv.ok"])
    st, text = _req(web["addr"], "GET", f"/api/runs/{res['run_id']}/log?step=s&stream=out",
                    None, hdr)
    payload = json.loads(text)
    assert st == 200 and "ok" in payload["text"] and payload["done"] is True
    st, text = _req(web["addr"], "GET",
                    f"/api/runs/{res['run_id']}/log?step=s&stream=out&offset={payload['offset']}",
                    None, hdr)
    assert st == 200 and json.loads(text)["text"] == ""
