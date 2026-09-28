"""hub_core.py —— 平台层数据（S176，spec/HUB.md §四/§六）：manifest 校验 + 运行账本 + 护栏。

manifest 与 spec/PLAYBOOKS.md 族骨架同源；步骤语义刻意最小（顺序 + 固定 argv + 超时，
无循环/无内容分支/无嵌套）。**命令首段白名单**（python/cargo/git/node）——平台不提供
任意命令面（与 local_run 白名单同精神），manifest 本体住在受版本控制的仓内目录。

账本 runs.jsonl：append-only + sha256 链（prev_hash/hash，篡改即链断）；跨进程单写者
由锁文件保证。active.json（O_CREAT|O_EXCL + pid 存活探测）=「同时至多一个运行」护栏。
"""
from __future__ import annotations

import contextlib
import ctypes
import hashlib
import json
import os
import pathlib
import re
import secrets
import time

try:
    import msvcrt  # type: ignore[import-not-found]  # Windows 账本锁
    _HAVE_MSVCRT = True
except ImportError:
    _HAVE_MSVCRT = False

REPO_ROOT = pathlib.Path(__file__).resolve().parent
RESOURCE_CLASSES = frozenset({"shared", "exclusive"})
ALLOWED_HEADS = frozenset({"python", "python3", "py", "cargo", "git", "node"})
MAX_STEPS, MAX_CMD_SEGMENTS, MAX_TIMEOUT_S = 16, 32, 7200
GENESIS = "0" * 64
_ID_RE = re.compile(r"[a-z][a-z0-9]*(\.[a-z0-9-]+)+")


def pipelines_dir() -> pathlib.Path:
    raw = (os.environ.get("UNIFIED_RX_HUB_PIPELINES") or "").strip()
    return pathlib.Path(raw) if raw else REPO_ROOT / "pipelines"


def _data_root() -> pathlib.Path:
    raw = (os.environ.get("UNIFIED_RX_HUB_ROOT") or "").strip()
    return pathlib.Path(raw) if raw else pathlib.Path.home() / ".ADV" / "hub"


def runs_path() -> pathlib.Path:
    return _data_root() / "runs.jsonl"


def logs_dir() -> pathlib.Path:
    return _data_root() / "logs"


def server_version() -> str:
    """直读 server.py 的 SERVER_VERSION（正则，不 import——避免协议层耦合）。"""
    m = re.search(r'SERVER_VERSION = "([\d.]+)"',
                  (REPO_ROOT / "server.py").read_text(encoding="utf-8"))
    return m.group(1) if m else "unknown"


def _check_id(data: dict, name: str) -> str:
    pid = data.get("id")
    if not isinstance(pid, str) or not _ID_RE.fullmatch(pid):
        raise ValueError(f"{name}: id 形如 域.场景（小写点分）")
    if pid != name[:-5]:
        raise ValueError(f"{name}: id 与文件名不一致")
    return pid


def _check_step(pid: str, st: object, seen: set) -> None:
    if not isinstance(st, dict):
        raise ValueError(f"{pid}: 步骤必须是对象")
    unknown = sorted(set(st) - {"step", "cmd", "timeout_s", "optional"})
    if unknown:
        raise ValueError(f"{pid}: 步骤未知字段 {unknown}")
    sn = st.get("step")
    if not isinstance(sn, str) or not sn or sn in seen:
        raise ValueError(f"{pid}: 步骤名缺失或重复")
    seen.add(sn)
    cmd = st.get("cmd")
    if (not isinstance(cmd, list) or not cmd
            or not all(isinstance(x, str) and x for x in cmd)
            or len(cmd) > MAX_CMD_SEGMENTS):
        raise ValueError(f"{pid}: 步骤 {sn!r} cmd 必须 1..{MAX_CMD_SEGMENTS} 段非空字符串")
    head = pathlib.PurePath(cmd[0]).stem.lower()
    if head not in ALLOWED_HEADS:
        raise ValueError(f"{pid}: 步骤 {sn!r} 命令首段 {cmd[0]!r} 不在白名单 {sorted(ALLOWED_HEADS)}")
    ts = st.get("timeout_s", 1800)
    if not isinstance(ts, int) or not 1 <= ts <= MAX_TIMEOUT_S:
        raise ValueError(f"{pid}: 步骤 {sn!r} timeout_s 必须 1..{MAX_TIMEOUT_S}")
    if not isinstance(st.get("optional", False), bool):
        raise ValueError(f"{pid}: 步骤 {sn!r} optional 必须是布尔")


def _check_header(data: dict, name: str) -> str:
    """头部字段（未知字段即拒 / 必填 / 类型 / id / 资源级 / 触发集）→ pid。"""
    unknown = sorted(set(data) - {"id", "title", "when", "resource_class", "on", "steps"})
    if unknown:
        raise ValueError(f"{name}: 未知字段 {unknown}（默认严苛）")
    for k in ("id", "title", "when", "resource_class", "on", "steps"):
        if k not in data:
            raise ValueError(f"{name}: 缺字段 {k}")
    pid = _check_id(data, name)
    if data["resource_class"] not in RESOURCE_CLASSES:
        raise ValueError(f"{pid}: resource_class 必须是 {sorted(RESOURCE_CLASSES)}")
    for k in ("title", "when"):
        if not isinstance(data[k], str) or not data[k].strip():
            raise ValueError(f"{pid}: {k} 必须是非空字符串")
    on = data["on"]
    if not isinstance(on, list) or not on or not all(isinstance(x, str) for x in on):
        raise ValueError(f"{pid}: on 必须是非空字符串列表")
    return pid


def validate_manifest(name: str, data: object) -> dict:
    """严格校验（未知字段即拒）+ 命令白名单；通过返回原 manifest，否则 ValueError。"""
    if not isinstance(data, dict):
        raise ValueError(f"{name}: manifest 必须是对象")
    pid = _check_header(data, name)
    steps = data["steps"]
    if not isinstance(steps, list) or not 1 <= len(steps) <= MAX_STEPS:
        raise ValueError(f"{pid}: steps 必须 1..{MAX_STEPS} 步")
    seen: set[str] = set()
    for st in steps:
        _check_step(pid, st, seen)
    return data


def load_pipelines() -> tuple[dict[str, dict], dict[str, str]]:
    """加载管线目录 → (有效表 id→manifest, 非法表 stem→错误)；坏文件如实列出。"""
    d = pipelines_dir()
    valid: dict[str, dict] = {}
    invalid: dict[str, str] = {}
    if not d.is_dir():
        return valid, {"(目录缺失)": str(d)}
    for p in sorted(d.glob("*.json")):
        try:
            m = validate_manifest(p.name, json.loads(p.read_text(encoding="utf-8")))
        except ValueError as exc:
            invalid[p.stem] = str(exc)
            continue
        except (OSError, json.JSONDecodeError) as exc:
            invalid[p.stem] = f"不可读: {exc}"
            continue
        if m["id"] in valid:
            invalid[p.stem] = f"id 重复: {m['id']}"
            continue
        valid[m["id"]] = m
    return valid, invalid


def manifest_fingerprint(manifest: dict) -> str:
    """manifest 内容指纹（rug-pull 防护：改了 manifest 就是新指纹）。"""
    blob = json.dumps(manifest, sort_keys=True, ensure_ascii=False,
                      separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def new_run_id() -> str:
    return f"r-{time.strftime('%Y%m%d-%H%M%S')}-{secrets.token_hex(2)}"


# ---------------- 账本（append-only + sha256 链） ----------------

def _canon(row: dict) -> str:
    return json.dumps(row, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def _row_hash(prev_hash: str, row: dict) -> str:
    return hashlib.sha256((prev_hash + _canon(row)).encode("utf-8")).hexdigest()


def _last_hash() -> str:
    rows = read_rows()
    for row in reversed(rows):
        h = row.get("hash")
        if isinstance(h, str):
            return h
    return GENESIS


def _locked_append(line: str) -> None:
    p = runs_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    lock = p.parent / ".runs.lock"
    with lock.open("a+b") as lf:
        if _HAVE_MSVCRT:
            lf.seek(0)
            msvcrt.locking(lf.fileno(), msvcrt.LK_LOCK, 1)
        try:
            with p.open("a", encoding="utf-8") as fh:
                fh.write(line + "\n")
        finally:
            if _HAVE_MSVCRT:
                lf.seek(0)
                msvcrt.locking(lf.fileno(), msvcrt.LK_UNLCK, 1)


def append_row(row: dict) -> dict:
    """补 ts/链字段并落账本；返回带链字段的完整行。"""
    out = dict(row)
    out["ts"] = int(time.time())
    prev = _last_hash()
    out["prev_hash"] = prev
    out["hash"] = _row_hash(prev, out)
    _locked_append(_canon(out))
    return out


def read_rows(limit: int = 0) -> list[dict]:
    """读账本（limit>0 取尾部 N 行）；坏行跳过（完整性由 verify_chain 如实报）。

    **读路径容错**（S183 混沌）：账本不可读（权限/被目录占位/盘满）时返回空表而**不崩**
    ——"盘坏了"必须表现为"可诊断的降级"，不能让工具直接异常。
    """
    p = runs_path()
    if not p.exists():
        return []
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    rows: list[dict] = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows[-limit:] if limit > 0 else rows


def storage_writable() -> bool:
    """写探测（S183 混沌）：数据根能否建/删探针文件——"平台现在能不能干活"一眼可见。

    只探**数据根**（账本+日志所在）；失败即如实返回 False，由 hub_status 报 degraded。
    """
    d = _data_root()
    try:
        d.mkdir(parents=True, exist_ok=True)
        probe = d / ".write-probe"
        probe.write_text("1", encoding="utf-8")
        probe.unlink()
        return True
    except OSError:
        return False


def latest_runs(limit: int = 20) -> list[dict]:
    """按 run id 取每 run 最新一行（final 覆盖 start），ts 降序。"""
    agg: dict[str, dict] = {}
    for row in read_rows():
        rid = row.get("id")
        if isinstance(rid, str):
            agg[rid] = row
    return sorted(agg.values(), key=lambda r: int(r.get("ts", 0)),
                  reverse=True)[: max(1, limit)]


def _ledger_readable() -> bool:
    """账本可读性（S183 混沌）：存在但打不开（权限/被目录占位/盘满）时，**不能报"链 OK"**。"""
    p = runs_path()
    if not p.exists():
        return True                       # 空账本合法
    try:
        with p.open("rb") as fh:
            fh.read(1)
        return True
    except OSError:
        return False


def verify_chain() -> dict:
    """全链复核：prev_hash 连续 + 每行哈希可重算（篡改即红）。

    S183：账本**存在但不可读** ⇒ 直接判 `ok=False`（空表会让链"看起来 OK"——那是假绿）。
    """
    if not _ledger_readable():
        return {"ok": False, "count": 0, "broken_index": None,
                "reason": "账本存在但不可读（权限/占位/盘满）——不得报'链 OK'"}
    prev = GENESIS
    for i, row in enumerate(read_rows()):
        if row.get("prev_hash") != prev:
            return {"ok": False, "count": i, "broken_index": i, "reason": "prev_hash 不连续"}
        expect = row.get("hash")
        probe = {k: v for k, v in row.items() if k != "hash"}
        if not isinstance(expect, str) or _row_hash(prev, probe) != expect:
            return {"ok": False, "count": i, "broken_index": i, "reason": "行哈希不匹配（内容被篡改）"}
        prev = expect
    return {"ok": True, "count": len(read_rows()), "broken_index": None, "reason": None}


# ---------------- 资源级准入（S178：shared 并行 / exclusive 互斥） ----------------
# manifest.resource_class 生效：exclusive 与**任何**活跃运行互斥；shared 之间可并行
# （上限 max_shared()），且不得与 exclusive 并存。护栏 = 活跃目录"一运行一文件"
# （O_CREAT|O_EXCL 原子创建 + pid 存活探测 + 陈旧清理），**检查与创建在同一临界区内**
# 完成（目录锁，避免 TOCTOU 超卖），跨进程成立。
# 为什么不做阻塞式排队：本机作业粒度是分钟级，拒绝式（busy）+ 活跃可见已够用，且免掉
# 跨进程唤醒与公平性的复杂度——"先量后改"，有真实排队需求再做（spec/HUB.md §七 M1 余项）。

def _pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    if os.name == "nt":
        k32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
        h = k32.OpenProcess(0x1000, False, pid)   # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return False
        k32.CloseHandle(h)
        return True
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def active_dir() -> pathlib.Path:
    return _data_root() / "runs_active"


def max_shared() -> int:
    raw = (os.environ.get("UNIFIED_RX_HUB_MAX_SHARED") or "").strip()
    return int(raw) if raw.isdigit() and int(raw) > 0 else 2


@contextlib.contextmanager
def _active_lock():
    """准入临界区锁（活跃目录内 .lock；Windows msvcrt / POSIX fcntl 双实现）。"""
    d = active_dir()
    d.mkdir(parents=True, exist_ok=True)
    with (d / ".lock").open("a+b") as lf:
        locked = False
        if _HAVE_MSVCRT:
            lf.seek(0)
            msvcrt.locking(lf.fileno(), msvcrt.LK_LOCK, 1)
            locked = True
        else:
            try:
                import fcntl
                fcntl.flock(lf.fileno(), fcntl.LOCK_EX)
                locked = True
            except ImportError:
                locked = False
        try:
            yield
        finally:
            if locked and _HAVE_MSVCRT:
                lf.seek(0)
                msvcrt.locking(lf.fileno(), msvcrt.LK_UNLCK, 1)


def _active_file(run_id: str) -> pathlib.Path:
    return active_dir() / f"{run_id}.json"


def read_active() -> list[dict]:
    """活跃运行清单（清理陈旧：pid 已死 / 坏文件）。

    **新文件宽容**：`admit` 的"创建 → 写入"之间有极小窗口，此刻读到会解析失败——
    直接当陈旧删掉会**静默放开准入上限**（S178 实锤：第三个 shared 被放行、exclusive
    互斥失效，表现为自家调度判据 `third_busy_at_limit=False`）。故解析失败/无 pid 时，
    文件够旧（>60s）才删，否则本轮只跳过。
    """
    d = active_dir()
    if not d.is_dir():
        return []
    out: list[dict] = []
    for p in sorted(d.glob("*.json")):
        try:
            data: object = json.loads(p.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            data = None
        pid = int(data.get("pid") or 0) if isinstance(data, dict) else 0
        if isinstance(data, dict) and _pid_alive(pid):
            out.append(data)
            continue
        try:
            age = time.time() - p.stat().st_mtime
        except OSError:
            continue
        if age > 60:
            p.unlink(missing_ok=True)
    return sorted(out, key=lambda r: float(r.get("ts") or 0))


def admit(run_id: str, resource_class: str, pid: int) -> dict:
    """准入判定 + 占位（同一临界区内）；被拒返回原因与当前活跃集（可诊断）。"""
    if resource_class not in RESOURCE_CLASSES:
        return {"ok": False, "reason": f"未知资源级 {resource_class!r}"}
    with _active_lock():
        active = read_active()
        excl = [a for a in active if a.get("resource_class") == "exclusive"]
        shared = [a for a in active if a.get("resource_class") != "exclusive"]
        if resource_class == "exclusive" and active:
            return {"ok": False, "reason": "已有运行在进行（exclusive 需整机独占）",
                    "active": active}
        if resource_class == "shared" and excl:
            return {"ok": False, "reason": "有 exclusive 运行在进行（shared 不得与其并存）",
                    "active": active}
        if resource_class == "shared" and len(shared) >= max_shared():
            return {"ok": False, "reason": f"shared 并行已达上限 {max_shared()}",
                    "active": active}
        p = _active_file(run_id)
        try:
            fd = os.open(str(p), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError:
            return {"ok": False, "reason": f"运行位已被占用 {run_id}", "active": active}
        try:                       # 单次 write 落整份内容（缩小"创建→可见"窗口）
            os.write(fd, json.dumps({"run_id": run_id, "pid": pid,
                                     "resource_class": resource_class,
                                     "ts": time.time()}).encode())
        finally:
            os.close(fd)
        return {"ok": True, "active": read_active()}


def release_active(run_id: str) -> None:
    _active_file(run_id).unlink(missing_ok=True)
