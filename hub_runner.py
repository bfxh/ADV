"""hub_runner.py —— 平台层执行器（S176，spec/HUB.md §四/§六）：跑步骤 + 指纹 + 封印。

判据（防骗体系，逐条落在这里）：
- **退出码取自 OS**（Popen 返回值），不经任何中间解释层；
- **原始日志落盘 + sha256 入账**——智能体可用自己的工具独立重算；
- **SKIP 显式化且不算绿**：green = 全部必跑步 exit 0 **且** skipped_lines == 0；
  有 SKIP 只算 green_with_skips（诚实降级，不是绿）；
- **运行指纹八项**（manifest sha256 / git HEAD / dirty 位 / argv / cwd / env 键哈希 /
  python / cargo）——"平台跑的就是我以为的那个门"；
- **判定封印** seal：判定相关字段的再哈希（同输入同封印，跨机可复核）；
- **how_to_verify**：每条结果附带独立复核命令（防骗不依赖平台自觉）。
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import platform
import re
import subprocess
import sys
import time

import hub_core

_SKIP_RE = re.compile(r"^\s*(SKIP\b|\[SKIP\])")
_VERDICT_RE = re.compile(r"^\s*(?:OK|FAIL|SKIP)\s+\S+|LOCAL-GATE\s+(?:OK|FAIL)")
_MAX_VERDICT_LINES = 40


def _git(args: list[str]) -> str | None:
    try:
        cp = subprocess.run(["git", "-C", str(hub_core.REPO_ROOT), *args],
                            capture_output=True, timeout=20)
    except (OSError, subprocess.SubprocessError):
        return None
    if cp.returncode != 0:
        return None
    return cp.stdout.decode("utf-8", "replace").strip()


def _toolchain() -> dict[str, str | None]:
    cargo: str | None = None
    try:
        cp = subprocess.run(["cargo", "--version"], capture_output=True, timeout=30)
        if cp.returncode == 0:
            cargo = cp.stdout.decode("utf-8", "replace").strip()
    except (OSError, subprocess.SubprocessError):
        cargo = None
    return {"python": platform.python_version(), "cargo": cargo}


def fingerprint(manifest: dict) -> dict:
    """运行指纹八项：判"跑的是不是我以为的那个东西"；缺失如实置 None（不编造）。"""
    env_keys = sorted(k for k in os.environ
                      if k.startswith("UNIFIED_RX_") or k in ("PYTHONUTF8", "PYTHONHASHSEED"))
    env_blob = "\n".join(f"{k}={os.environ.get(k)}" for k in env_keys)
    first_cmd = manifest["steps"][0]["cmd"]
    status = _git(["status", "--porcelain"])
    return {
        "manifest_sha256": hub_core.manifest_fingerprint(manifest),
        "git_head": _git(["rev-parse", "HEAD"]),
        "git_dirty": None if status is None else bool(status),
        "argv0": first_cmd[0],
        "cwd": str(hub_core.REPO_ROOT),
        "env_keys_hash": hashlib.sha256(env_blob.encode("utf-8")).hexdigest(),
        "python": _toolchain()["python"],
        "cargo": _toolchain()["cargo"],
    }


def seal_of(payload: dict) -> str:
    blob = json.dumps(payload, sort_keys=True, ensure_ascii=False,
                      separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def _run_step(run_id: str, st: dict) -> dict:
    """跑一步：原始日志落盘 + 双向哈希 + SKIP 计数 + 判定行抽取。"""
    cmd = list(st["cmd"])
    t0 = time.time()
    exit_code: int | None
    timed_out = False
    try:
        cp = subprocess.run(cmd, cwd=str(hub_core.REPO_ROOT), timeout=st["timeout_s"],
                            capture_output=True, shell=False)
        out, err, exit_code = cp.stdout, cp.stderr, cp.returncode
    except subprocess.TimeoutExpired as exc:
        out = exc.stdout or b""
        err = exc.stderr or b""
        exit_code, timed_out = None, True
    except OSError as exc:
        out, err, exit_code = b"", str(exc).encode("utf-8", "replace"), None
    log = out + (b"\n[stderr]\n" + err if err else b"")
    log_path = hub_core.logs_dir() / f"{run_id}.{st['step']}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_bytes(log)
    text = log.decode("utf-8", "replace")
    lines = text.splitlines()
    return {
        "step": st["step"], "argv": cmd, "exit": exit_code, "timed_out": timed_out,
        "ok": exit_code == 0, "optional": bool(st.get("optional", False)),
        "ms": int((time.time() - t0) * 1000),
        "log": str(log_path),
        "log_sha256": hashlib.sha256(log).hexdigest(), "log_bytes": len(log),
        "stdout_sha256": hashlib.sha256(out).hexdigest(),
        "stderr_sha256": hashlib.sha256(err).hexdigest(),
        "skipped_lines": sum(1 for ln in lines if _SKIP_RE.match(ln)),
        "verdict_lines": [ln.strip() for ln in lines if _VERDICT_RE.search(ln)][:_MAX_VERDICT_LINES],
    }


def _verdict(steps: list[dict]) -> str:
    """绿的定义（收紧）：必跑步全 exit 0 且无 SKIP；有 SKIP 只算 green_with_skips。"""
    if any(not s["ok"] and not s["optional"] for s in steps):
        return "red"
    if sum(s["skipped_lines"] for s in steps) > 0:
        return "green_with_skips"
    return "green"


def _how_to_verify(run_id: str, steps: list[dict], cwd: str) -> list[str]:
    """人类/智能体可读的独立复核指引（路径 posix 形态便于复制）；可用账本行重建。"""
    out = ["账本链校验: python -X utf8 scripts/hub_gate.py --verify-chain"]
    for s in steps:
        log = pathlib.Path(str(s.get("log") or "")).as_posix()
        out.append(f"日志独立复核: 重算 sha256({log}) 与记录比对  # 期望 {s.get('log_sha256')}")
        out.append(f"独立直跑: cwd={pathlib.Path(cwd).as_posix()} "
                   f"argv={json.dumps(s.get('argv') or [], ensure_ascii=False)}"
                   f"  # 期望 exit={s.get('exit')}")
    out.append(f"封条重算: python -X utf8 scripts/hub_gate.py --seal {run_id}")
    return out


def _verify_plan(run_id: str, steps: list[dict], cwd: str) -> list[dict]:
    """机器可执行的复核计划（argv 列表，绝不经 shell/字符串解析）。"""
    gate = str(hub_core.REPO_ROOT / "scripts" / "hub_gate.py")
    plan: list[dict] = []
    for s in steps:
        plan.append({"kind": "log_hash", "path": str(s["log"]),
                     "expect_sha256": s["log_sha256"]})
        plan.append({"kind": "direct_rerun", "argv": list(s["argv"]), "cwd": cwd,
                     "expect_exit": s["exit"]})
    plan.append({"kind": "chain", "argv": [sys.executable, "-X", "utf8", gate, "--verify-chain"]})
    plan.append({"kind": "seal", "argv": [sys.executable, "-X", "utf8", gate,
                                          "--seal", run_id]})
    return plan


def run_pipeline(manifest: dict, actor: str = "local", trigger: str = "manual") -> dict:
    """独占护栏内顺序执行；start/final 两行入账（final 带封印与判定）。"""
    run_id = hub_core.new_run_id()
    if not hub_core.acquire_active(run_id, os.getpid()):
        return {"ok": False, "busy": True, "run_id": run_id,
                "active": hub_core.read_active(),
                "error": "已有运行进行中（护栏：同时至多一个）——先等它结束或查 hub_runs"}
    fp = fingerprint(manifest)
    hub_core.append_row({"id": run_id, "phase": "start", "pipeline": manifest["id"],
                         "actor": actor, "trigger": trigger, "fingerprint": fp})
    try:
        steps = [_run_step(run_id, st) for st in manifest["steps"]]
        verdict = _verdict(steps)
        payload = {"pipeline": manifest["id"], "manifest_sha256": fp["manifest_sha256"],
                   "git_head": fp["git_head"], "git_dirty": fp["git_dirty"],
                   "verdict": verdict,
                   "steps": [{k: s[k] for k in ("step", "exit", "log_sha256")} for s in steps]}
        seal = seal_of(payload)
        row = hub_core.append_row({"id": run_id, "phase": "final", "pipeline": manifest["id"],
                                   "actor": actor, "trigger": trigger, "verdict": verdict,
                                   "seal": seal, "fingerprint": fp,
                                   "steps": [{k: s[k] for k in
                                              ("step", "argv", "exit", "ok", "optional", "ms",
                                               "log", "log_sha256", "log_bytes", "skipped_lines")}
                                             for s in steps]})
        return {"ok": verdict != "red", "run_id": run_id, "verdict": verdict, "seal": seal,
                "steps": steps, "fingerprint": fp, "ledger_hash": row.get("hash"),
                "how_to_verify": _how_to_verify(run_id, steps, str(hub_core.REPO_ROOT)),
                "verify_plan": _verify_plan(run_id, steps, str(hub_core.REPO_ROOT))}
    finally:
        hub_core.release_active(run_id)


def main(argv: list[str]) -> int:
    """命令行入口：python -X utf8 hub_runner.py <pipeline-id>（平台外自跑，同一实现）。"""
    pid = argv[0] if argv else ""
    pipes, invalid = hub_core.load_pipelines()
    if not pid or pid not in pipes:
        print(f"未知管线 {pid!r}；可选 {sorted(pipes)}；非法 {invalid}", file=sys.stderr)
        return 2
    res = run_pipeline(pipes[pid], actor="cli", trigger="manual")
    print(json.dumps({k: res[k] for k in ("ok", "run_id", "verdict", "seal") if k in res},
                     ensure_ascii=False))
    return 0 if res.get("ok") else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
