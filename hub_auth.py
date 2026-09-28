"""hub_auth.py —— 平台层团队身份（S176，spec/HUB.md §三）：用户令牌 + 角色 + Web 会话。

本质：平台能力只有一份（registry 工具面）；身份的另一个来源是网页用户（本文件）。
角色三档：admin / operator（可触发运行）/ viewer。读面先绑回环，读会话鉴权挂 M1。

用户表 <root>/users.json 首跑自举 admin：令牌 secrets 生成、**只打 stderr 一次**，
文件只存 pbkdf2 哈希；比对走 hmac.compare_digest（恒时）。坏表 **fail closed**（不重建）。
"""
from __future__ import annotations

import hashlib
import hmac
import json
import os
import pathlib
import re
import secrets
import sys
import time

ROLES = ("admin", "operator", "viewer")
TRIGGER_ROLES = frozenset({"admin", "operator"})
_SESSION_TTL_S = 12 * 3600
_PBKDF2_ROUNDS = 120_000
_SESSIONS: dict[str, tuple[str, str, float]] = {}


def root() -> pathlib.Path:
    """数据根：env UNIFIED_RX_HUB_ROOT 优先（测试隔离），缺省 ~/.ADV/hub。"""
    raw = (os.environ.get("UNIFIED_RX_HUB_ROOT") or "").strip()
    return pathlib.Path(raw) if raw else pathlib.Path.home() / ".ADV" / "hub"


def users_path() -> pathlib.Path:
    return root() / "users.json"


def _hash(token: str, salt: str) -> str:
    return hashlib.pbkdf2_hmac(
        "sha256", token.encode("utf-8"), bytes.fromhex(salt), _PBKDF2_ROUNDS
    ).hex()


def _hashed(token: str) -> tuple[str, str]:
    salt = secrets.token_hex(16)
    return salt, _hash(token, salt)


def _valid_users(data: object) -> list[dict] | None:
    """形状校验；非法返回 None（fail closed，不静默重建）。"""
    if not isinstance(data, dict) or not isinstance(data.get("users"), list):
        return None
    out: list[dict] = []
    for u in data["users"]:
        if not isinstance(u, dict):
            return None
        if not isinstance(u.get("name"), str) or u.get("role") not in ROLES:
            return None
        if not isinstance(u.get("salt"), str) or not isinstance(u.get("token_hash"), str):
            return None
        out.append(u)
    return out


def _load() -> tuple[list[dict] | None, str | None]:
    p = users_path()
    if not p.exists():
        return [], None
    try:
        data: object = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return None, f"用户表不可读（fail closed，不重建）: {exc}"
    users = _valid_users(data)
    if users is None:
        return None, "用户表形状非法（fail closed，不重建）"
    return users, None


def bootstrap() -> dict:
    """加载/自举用户表。返回 {users, created, bootstrap_token, error}。"""
    users, err = _load()
    if err:
        return {"users": [], "created": False, "bootstrap_token": None, "error": err}
    if users:
        return {"users": [u["name"] for u in users], "created": False,
                "bootstrap_token": None, "error": None}
    token = secrets.token_urlsafe(24)
    salt, th = _hashed(token)
    doc = {"users": [{"name": "admin", "role": "admin", "salt": salt, "token_hash": th}]}
    p = users_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"[hub] 自举 admin 令牌（仅显示一次，请立即保存）: {token}", file=sys.stderr)
    return {"users": ["admin"], "created": True, "bootstrap_token": token, "error": None}


def verify(user: str, token: str) -> str | None:
    """校验用户令牌 → 角色；任何不匹配返回 None（不区分"无此人/令牌错"）。"""
    users, _err = _load()
    if users is None:
        return None
    for u in users:
        if u["name"] != user:
            continue
        if hmac.compare_digest(_hash(token, u["salt"]), u["token_hash"]):
            return str(u["role"])
        return None
    return None


def can_trigger(role: str | None) -> bool:
    return role is not None and role in TRIGGER_ROLES


def new_session(user: str, role: str) -> str:
    sid = secrets.token_urlsafe(24)
    _prune()
    _SESSIONS[sid] = (user, role, time.time() + _SESSION_TTL_S)
    return sid


def session_user(sid: str) -> tuple[str, str] | None:
    ent = _SESSIONS.get(sid)
    if ent is None:
        return None
    user, role, exp = ent
    if time.time() > exp:
        _SESSIONS.pop(sid, None)
        return None
    return (user, role)


def _prune() -> None:
    now = time.time()
    for sid in [k for k, v in _SESSIONS.items() if v[2] <= now]:
        _SESSIONS.pop(sid, None)


def count_users() -> int:
    users, _err = _load()
    return len(users) if users else 0


# ---------------- 用户管理（S179：RBAC 全量） ----------------
# 纪律：令牌只此一次返回（表内只存 pbkdf2 哈希）；改表原子写（tmp + os.replace）；
# **拒绝删/降最后一个 admin**（防把自己锁在门外）；坏表 fail closed。

_NAME_RE = re.compile(r"[a-z0-9][a-z0-9_-]{1,31}")


def list_users() -> list[dict]:
    """用户清单（不含盐/哈希）；表不可读时如实空表。"""
    users, _err = _load()
    return [{"name": u["name"], "role": u["role"]} for u in (users or [])]


def _save(users: list[dict]) -> None:
    p = users_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_name(p.name + ".tmp")
    tmp.write_text(json.dumps({"users": users}, ensure_ascii=False, indent=2), encoding="utf-8")
    os.replace(tmp, p)


def _admin_count(users: list[dict]) -> int:
    return sum(1 for u in users if u.get("role") == "admin")


def add_user(name: str, role: str) -> dict:
    """创建用户 → 一次性令牌（只此一次返回；此后无法找回，只能重置）。"""
    if not isinstance(name, str) or not _NAME_RE.fullmatch(name):
        return {"ok": False, "error": "用户名须 2..32 位小写字母/数字/下划线/连字符"}
    if role not in ROLES:
        return {"ok": False, "error": f"角色须为 {sorted(ROLES)}"}
    users, err = _load()
    if users is None:
        return {"ok": False, "error": err or "用户表不可读"}
    if any(u["name"] == name for u in users):
        return {"ok": False, "error": f"用户已存在：{name}"}
    token = secrets.token_urlsafe(24)
    salt, th = _hashed(token)
    users.append({"name": name, "role": role, "salt": salt, "token_hash": th})
    _save(users)
    return {"ok": True, "name": name, "role": role, "token": token}


def set_role(name: str, role: str) -> dict:
    if role not in ROLES:
        return {"ok": False, "error": f"角色须为 {sorted(ROLES)}"}
    users, err = _load()
    if users is None:
        return {"ok": False, "error": err or "用户表不可读"}
    for u in users:
        if u["name"] != name:
            continue
        if u.get("role") == "admin" and role != "admin" and _admin_count(users) <= 1:
            return {"ok": False, "error": "不能降级最后一个 admin"}
        u["role"] = role
        _save(users)
        return {"ok": True, "name": name, "role": role}
    return {"ok": False, "error": f"无此用户：{name}"}


def delete_user(name: str) -> dict:
    users, err = _load()
    if users is None:
        return {"ok": False, "error": err or "用户表不可读"}
    target = next((u for u in users if u["name"] == name), None)
    if target is None:
        return {"ok": False, "error": f"无此用户：{name}"}
    if target.get("role") == "admin" and _admin_count(users) <= 1:
        return {"ok": False, "error": "不能删最后一个 admin"}
    _save([u for u in users if u["name"] != name])
    return {"ok": True, "name": name}


def drop_session(sid: str) -> None:
    _SESSIONS.pop(sid, None)
