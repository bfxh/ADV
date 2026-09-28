"""server_web.py —— 平台层第二协议通道（S176/S177/S179，spec/HUB.md §一/§三）：HTTP + 控制台。

只绑 **127.0.0.1**（本地优先，无远端暴露）。**鉴权分层**（S179 起）：
  开放   `/`、`/static/*`（控制台外壳与登录页要能加载）、`/api/status`（**摘要态**：
         实例/版本/链是否 OK——登录前也能看见平台是否降级）、`/api/login`
  会话   `/api/pipelines`、`/api/runs*`、`/api/stream`（Cookie `hub_sid`，HttpOnly+SameSite=Strict）
  管理   `/api/users*`（admin：列表/创建/改角色/删除；**不允许删或降最后一个 admin**）
  触发   `POST /api/runs`：会话（operator/admin）**或** Bearer（脚本/智能体兼容）

路由（与 MCP 工具面同一份能力层 hub_core/hub_runner/hub_auth，不是第二实现）：
  GET  /  /static/<f>  /api/status  /api/pipelines  /api/runs?limit=
  GET  /api/runs/<id>  /api/runs/<id>/log（?offset=&step=&stream=）  /api/stream?run=<id>（SSE）
  GET  /api/users（admin）      POST /api/login  /api/logout  /api/users（admin）
  POST /api/users/role（admin）  POST /api/runs（触发）  DELETE /api/users/<name>（admin）

用法：python -X utf8 server_web.py [--port N] [--no-open]
"""
from __future__ import annotations

import json
import os
import pathlib
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

import hub_auth
import hub_core
import hub_live
import hub_runner

CONSOLE_DIR = pathlib.Path(__file__).resolve().parent / "console"
_STATIC_EXT = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
               ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml", ".ico": "image/x-icon"}
_SSE_MAX_S = 1800          # 单条 SSE 连接最长时长（防挂死连接）
_COOKIE = "hub_sid"


def _instance() -> str:
    return (os.environ.get("UNIFIED_RX_HUB_INSTANCE") or "dev").strip().lower()


def default_port() -> int:
    raw = (os.environ.get("UNIFIED_RX_HUB_PORT") or "").strip()
    if raw.isdigit():
        return int(raw)
    return 7741 if _instance() == "stable" else 7742


def _status_payload(full: bool, who: tuple[str, str] | None = None) -> dict:
    """摘要态（未登录）：实例/版本/链是否 OK/降级原因；完整态另含用户数/活跃/数据根/身份。"""
    pipes, invalid = hub_core.load_pipelines()
    chain = hub_core.verify_chain()
    reasons: list[str] = []
    if not chain.get("ok"):
        reasons.append(f"账本链校验失败：{chain.get('reason')}")
    reasons.extend(f"manifest 非法 {k}: {v}" for k, v in sorted(invalid.items()))
    out = {"ok": True, "instance": _instance(), "version": hub_core.server_version(),
           "pipelines": len(pipes), "degraded": bool(reasons), "degraded_reasons": reasons,
           "chain": {"ok": chain.get("ok"), "count": chain.get("count")}}
    if full and who is not None:
        out.update({"users": hub_auth.count_users(), "invalid": invalid,
                    "user": who[0], "role": who[1],
                    "active": hub_core.read_active(), "max_shared": hub_core.max_shared(),
                    "data_root": str(hub_core.runs_path().parent)})
    return out


def _final_row(run_id: str) -> dict | None:
    rows = [r for r in hub_core.read_rows(limit=500) if r.get("id") == run_id]
    fin = [r for r in rows if r.get("phase") == "final"]
    return fin[-1] if fin else None


def _run_detail(run_id: str) -> dict:
    rows = [r for r in hub_core.read_rows(limit=500) if r.get("id") == run_id]
    if not rows:
        return {"ok": False, "error": f"无此运行 {run_id!r}"}
    final = [r for r in rows if r.get("phase") == "final"]
    row = final[-1] if final else rows[-1]
    steps = [s for s in row.get("steps") or [] if isinstance(s, dict)]
    how = hub_runner._how_to_verify(run_id, steps, str(hub_core.REPO_ROOT)) if steps else []
    return {"ok": True, "run": row, "how_to_verify": how, "rows": len(rows)}


def _static_bytes(name: str) -> tuple[bytes, str] | None:
    """只按 basename 取文件 + 目录内校验（防穿越）；扩展白名单。"""
    base = pathlib.Path(name).name
    ext = pathlib.Path(base).suffix.lower()
    if ext not in _STATIC_EXT:
        return None
    p = (CONSOLE_DIR / base).resolve()
    if p.parent != CONSOLE_DIR.resolve() or not p.is_file():
        return None
    return p.read_bytes(), _STATIC_EXT[ext]


def _actor_of(header: str | None) -> tuple[str, str] | None:
    """Authorization: Bearer user:token → (user, role)；不匹配返回 None（脚本/智能体面）。"""
    if not header or not header.startswith("Bearer "):
        return None
    blob = header[len("Bearer "):].strip()
    user, sep, token = blob.partition(":")
    if not sep or not user or not token:
        return None
    role = hub_auth.verify(user, token)
    return (user, role) if role else None


def _int_arg(q: dict, key: str, default: int) -> int:
    try:
        return int((q.get(key) or [str(default)])[0])
    except (TypeError, ValueError):
        return default


def _audit(action: str, actor: str, **fields) -> None:
    """管理动作入账本（kind=admin，链保护；无 id ⇒ 不进运行列表）。"""
    hub_core.append_row({"kind": "admin", "action": action, "actor": actor, **fields})


class _Handler(BaseHTTPRequestHandler):
    server_version = "ADV-HUB"
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt: str, *args) -> None:   # 静默：审计走账本，不刷屏
        pass

    # ---------------- 基础设施 ----------------

    def _send(self, code: int, body: bytes, ctype: str, extra: dict | None = None) -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("X-Hub-Instance", _instance())
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def _json(self, code: int, payload: dict, extra: dict | None = None) -> None:
        self._send(code, json.dumps(payload, ensure_ascii=False).encode("utf-8"),
                   "application/json; charset=utf-8", extra)

    def _session(self) -> tuple[str, str] | None:
        for part in (self.headers.get("Cookie") or "").split(";"):
            k, _, v = part.strip().partition("=")
            if k == _COOKIE and v:
                return hub_auth.session_user(v)
        return None

    def _body(self) -> dict | None:
        length = int(self.headers.get("Content-Length") or 0)
        if length <= 0 or length > 4096:
            return None
        try:
            data = json.loads(self.rfile.read(length).decode("utf-8"))
        except ValueError:
            return None
        return data if isinstance(data, dict) else None

    # ---------------- 实时日志（SSE） ----------------

    def _sse(self, run_id: str) -> None:
        """SSE：按 offset tail 该 run 的日志文件（文件即总线——跨进程且就是账上那份）。"""
        files = hub_live.log_files(run_id)
        if not files and _final_row(run_id) is None:
            self._json(404, {"ok": False, "error": f"无此运行 {run_id!r}"})
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True
        offsets: dict[str, int] = {}
        deadline = time.monotonic() + _SSE_MAX_S
        try:
            while time.monotonic() < deadline:
                for path, step, stream in hub_live.log_files(run_id):
                    text, off = hub_live.read_from(path, offsets.get(str(path), 0))
                    if text:
                        offsets[str(path)] = off
                        self.wfile.write(hub_live.sse_frame(
                            "log", {"step": step, "stream": stream, "text": text}))
                fin = _final_row(run_id)
                if fin is not None:
                    self.wfile.write(hub_live.sse_frame(
                        "done", {"verdict": fin.get("verdict"), "seal": fin.get("seal")}))
                    self.wfile.flush()
                    return
                self.wfile.flush()
                time.sleep(0.25)
        except (BrokenPipeError, ConnectionResetError, OSError):
            return                                  # 客户端断开：静默收尾（不污染审计）

    def _log_tail(self, query: str, run_id: str) -> None:
        q = parse_qs(query)
        offset = max(0, _int_arg(q, "offset", 0))
        step = (q.get("step") or [""])[0]
        stream = (q.get("stream") or ["out"])[0]
        files = hub_live.log_files(run_id)
        picked = [f for f in files if f[1] == step and f[2] == stream] if step else files
        if not picked:
            self._json(404, {"ok": False,
                             "error": f"无日志文件 run={run_id!r} step={step!r} stream={stream!r}"})
            return
        target = picked[-1][0]
        text, new_off = hub_live.read_from(target, offset)
        size = target.stat().st_size if target.exists() else 0
        self._json(200, {"ok": True, "path": str(target), "offset": new_off, "size": size,
                         "text": text, "done": _final_row(run_id) is not None,
                         "files": [{"step": s, "stream": st, "path": str(p),
                                    "size": p.stat().st_size} for p, s, st in files]})

    # ---------------- 读面 ----------------

    def _api_get(self, who: tuple[str, str], path: str, parsed) -> None:
        """会话内的读面路由（与静态/status 分离——god 门复杂度实测 12>10 故拆）。"""
        if path == "/api/pipelines":
            pipes, invalid = hub_core.load_pipelines()
            self._json(200, {"ok": True, "pipelines": list(pipes.values()), "invalid": invalid})
            return
        if path == "/api/runs":
            q = parse_qs(parsed.query)
            self._json(200, {"ok": True,
                             "runs": hub_core.latest_runs(max(1, _int_arg(q, "limit", 20))),
                             "chain": hub_core.verify_chain()})
            return
        if path == "/api/stream":
            q = parse_qs(parsed.query)
            run_id = (q.get("run") or [""])[0]
            (self._sse(run_id) if run_id else self._json(400, {"error": "缺 run 参数"}))
            return
        if path.startswith("/api/runs/") and path.endswith("/log"):
            self._log_tail(parsed.query, path[len("/api/runs/"):-len("/log")])
            return
        if path.startswith("/api/runs/"):
            self._json(200, _run_detail(path[len("/api/runs/"):]))
            return
        if path == "/api/users":
            if who[1] != "admin":
                self._json(403, {"error": "需要 admin 角色"})
                return
            self._json(200, {"ok": True, "users": hub_auth.list_users()})
            return
        self._json(404, {"error": "not found"})

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        path = parsed.path
        if path == "/":
            item = _static_bytes("index.html")
            (self._send(200, item[0], item[1]) if item else self._json(500, {"error": "console 缺失"}))
            return
        if path.startswith("/static/"):
            item = _static_bytes(path[len("/static/"):])
            (self._send(200, item[0], item[1]) if item else self._json(404, {"error": "not found"}))
            return
        who = self._session()
        if path == "/api/status":
            self._json(200, _status_payload(full=who is not None, who=who))
            return
        if who is None:
            self._json(401, {"error": "需要会话：先 POST /api/login（user + token）"})
            return
        self._api_get(who, path, parsed)

    # ---------------- 写面 ----------------

    def _login(self) -> None:
        body = self._body()
        user, token = (body or {}).get("user"), (body or {}).get("token")
        role = hub_auth.verify(str(user), str(token)) if user and token else None
        if role is None:
            self._json(401, {"error": "用户名或令牌不正确"})
            return
        sid = hub_auth.new_session(str(user), role)
        _audit("login", str(user), role=role)
        self._json(200, {"ok": True, "user": user, "role": role},
                   {"Set-Cookie": f"{_COOKIE}={sid}; HttpOnly; SameSite=Strict; Path=/"})

    def _logout(self) -> None:
        raw = (self.headers.get("Cookie") or "")
        for part in raw.split(";"):
            k, _, v = part.strip().partition("=")
            if k == _COOKIE and v:
                hub_auth.drop_session(v)
        self._json(200, {"ok": True}, {"Set-Cookie": f"{_COOKIE}=; Path=/; Max-Age=0"})

    def _users_write(self, who: tuple[str, str], path: str) -> None:
        if who[1] != "admin":
            self._json(403, {"error": "需要 admin 角色"})
            return
        body = self._body() or {}
        if path == "/api/users":
            res = hub_auth.add_user(str(body.get("name") or ""), str(body.get("role") or ""))
            if res.get("ok"):
                _audit("user-add", who[0], target=res["name"], role=res["role"])
            self._json(200 if res.get("ok") else 400, res)
            return
        if path == "/api/users/role":
            res = hub_auth.set_role(str(body.get("name") or ""), str(body.get("role") or ""))
            if res.get("ok"):
                _audit("user-role", who[0], target=res["name"], role=res["role"])
            self._json(200 if res.get("ok") else 400, res)
            return
        self._json(404, {"error": "not found"})

    def do_POST(self) -> None:
        path = urlparse(self.path).path
        if path == "/api/login":
            self._login()
            return
        if path == "/api/logout":
            self._logout()
            return
        if path.startswith("/api/users"):
            who = self._session()
            if who is None:
                self._json(401, {"error": "需要会话"})
                return
            self._users_write(who, path)
            return
        if path != "/api/runs":
            self._json(404, {"error": "not found"})
            return
        who = self._session() or _actor_of(self.headers.get("Authorization"))
        if who is None:
            self._json(401, {"error": "需要会话（Cookie）或 Bearer 令牌（user:token）"})
            return
        user, role = who
        if not hub_auth.can_trigger(role):
            self._json(403, {"error": f"角色 {role} 不能触发运行（需 operator/admin）"})
            return
        body = self._body()
        pid = (body or {}).get("pipeline")
        pipes, invalid = hub_core.load_pipelines()
        if not isinstance(pid, str) or pid not in pipes:
            self._json(404, {"error": f"未知管线 {pid!r}", "available": sorted(pipes),
                             "invalid": invalid})
            return
        res = hub_runner.run_pipeline(pipes[pid], actor=f"web:{user}", trigger="manual")
        self._json(202 if res.get("ok") else 500, res)

    def do_DELETE(self) -> None:
        path = urlparse(self.path).path
        if not path.startswith("/api/users/"):
            self._json(404, {"error": "not found"})
            return
        who = self._session()
        if who is None:
            self._json(401, {"error": "需要会话"})
            return
        if who[1] != "admin":
            self._json(403, {"error": "需要 admin 角色"})
            return
        target = path[len("/api/users/"):]
        res = hub_auth.delete_user(target)
        if res.get("ok"):
            _audit("user-delete", who[0], target=target)
        self._json(200 if res.get("ok") else 400, res)


def make_server(port: int = 0) -> ThreadingHTTPServer:
    """建服（测试用 port=0 取随机端口）；只绑回环。"""
    return ThreadingHTTPServer(("127.0.0.1", port), _Handler)


def main(argv: list[str]) -> int:
    port = default_port()
    if "--port" in argv:
        port = int(argv[argv.index("--port") + 1])
    boot = hub_auth.bootstrap()
    if boot.get("error"):
        print(f"[hub] {boot['error']}", file=sys.stderr)
    srv = make_server(port)
    url = f"http://127.0.0.1:{port}/"
    print(f"[hub] {_instance()} 实例 · v{hub_core.server_version()} · {url}"
          f"（数据根 {hub_core.runs_path().parent}）", file=sys.stderr)
    if "--no-open" not in argv:
        threading.Timer(0.4, lambda: webbrowser.open(url)).start()
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        srv.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
