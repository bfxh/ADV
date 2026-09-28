"""hub_runner.py —— 平台层执行器（S176 建立 / S177 流式改造）：执行 + 指纹 + 封印。

判据（防骗体系，逐条落在这里）：
- **退出码取自 OS**（Popen 返回值），不经任何中间解释层；
- **原始产物落盘 + sha256 入账**：stdout 与 stderr **分开两个文件**（`{run}.{step}.out/err.log`）
  ——stdout 由读线程流式搬运、stderr 由 OS 直写 ⇒ **两个流各自确定**，避免双线程交错写入
  破坏"同输入同封印"；智能体可用自己的工具独立重算哈希；
- **SKIP 显式化且不算绿**：green = 全部必跑步 exit 0 **且** skipped_lines == 0；
  有 SKIP 只算 green_with_skips（诚实降级，不是绿）；
- **运行指纹八项**（manifest sha256 / git HEAD / dirty 位 / argv / cwd / env 键哈希 /
  python / cargo）；
- **判定封印** seal（同输入同封印，跨机可复核）；
- **verify_plan**（argv 列表形态，不经 shell/字符串解析）+ **how_to_verify**（人读指引）。
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
import threading
import time

import hub_core

_SKIP_RE = re.compile(r"^\s*(SKIP\b|\[SKIP\])")
_VERDICT_RE = re.compile(r"^\s*(?:OK|FAIL|SKIP)\s+\S+|LOCAL-GATE\s+(?:OK|FAIL)")
_MAX_VERDICT_LINES = 40


def _git(args: list[str]) -> str | None:
    """注意：清掉 git 钩子注入的 GIT_* 定位变量（否则指纹可能采到错误的仓状态）。"""
    strip = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_PREFIX",
             "GIT_OBJECT_DIRECTORY", "GIT_COMMON_DIR", "GIT_ALTERNATE_OBJECT_DIRECTORIES")
    env = {k: v for k, v in os.environ.items() if k not in strip}
    try:
        cp = subprocess.run(["git", "-C", str(hub_core.REPO_ROOT), *args],
                            capture_output=True, timeout=20, env=env)
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
    status = _git(["status", "--porcelain"])
    return {
        "manifest_sha256": hub_core.manifest_fingerprint(manifest),
        "git_head": _git(["rev-parse", "HEAD"]),
        "git_dirty": None if status is None else bool(status),
        "argv0": manifest["steps"][0]["cmd"][0],
        "cwd": str(hub_core.REPO_ROOT),
        "env_keys_hash": hashlib.sha256(env_blob.encode("utf-8")).hexdigest(),
        "python": _toolchain()["python"],
        "cargo": _toolchain()["cargo"],
    }


def seal_of(payload: dict) -> str:
    blob = json.dumps(payload, sort_keys=True, ensure_ascii=False,
                      separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def _sha256_file(p: pathlib.Path) -> str:
    try:
        return hashlib.sha256(p.read_bytes()).hexdigest()
    except OSError:
        return ""


def _pump(stream, path: pathlib.Path, digest, counter: list[int]) -> None:
    """读线程：stdout → 文件（逐块 flush，网页面因此能实时 tail）+ 累计哈希。

    **必须用 read1 而不是 read(n)**：`BufferedReader.read(n)` 会阻塞到读满 n 字节或 EOF
    ——那等于"结束时一次性写出"，实测会把 live 判据打成 `gap=0.00s`（S177 首跑实锤）。
    `read1` 有数据即返回，才是真流式。
    """
    try:
        with path.open("wb") as fh:
            while True:
                chunk = stream.read1(4096)
                if not chunk:
                    break
                digest.update(chunk)
                counter.append(len(chunk))
                fh.write(chunk)
                fh.flush()
    except OSError:
        return


def _run_step(run_id: str, st: dict) -> dict:
    """跑一步：stdout 流式落盘 + stderr OS 直写；双向哈希、SKIP 计数、判定行抽取。"""
    cmd = list(st["cmd"])
    t0 = time.time()
    logdir = hub_core.logs_dir()
    logdir.mkdir(parents=True, exist_ok=True)
    out_path = logdir / f"{run_id}.{st['step']}.out.log"
    err_path = logdir / f"{run_id}.{st['step']}.err.log"
    digest = hashlib.sha256()
    counter: list[int] = []
    exit_code: int | None = None
    timed_out = False
    with err_path.open("wb") as err_fh:
        try:
            proc = subprocess.Popen(cmd, cwd=str(hub_core.REPO_ROOT),
                                    stdout=subprocess.PIPE, stderr=err_fh, shell=False)
        except OSError as exc:
            err_fh.write(str(exc).encode("utf-8", "replace"))
            proc = None
        if proc is not None:
            if proc.stdout is not None:
                th = threading.Thread(target=_pump, args=(proc.stdout, out_path, digest, counter),
                                      daemon=True)
                th.start()
            else:
                th = None
            try:
                exit_code = proc.wait(timeout=st["timeout_s"])
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
                timed_out = True
            if th is not None:
                th.join(15)
    out_bytes = sum(counter)
    err_bytes = err_path.stat().st_size if err_path.exists() else 0
    text = (out_path.read_text(encoding="utf-8", errors="replace") if out_path.exists() else "") \
        + (err_path.read_text(encoding="utf-8", errors="replace") if err_path.exists() else "")
    lines = text.splitlines()
    return {
        "step": st["step"], "argv": cmd, "exit": exit_code, "timed_out": timed_out,
        "ok": exit_code == 0, "optional": bool(st.get("optional", False)),
        "ms": int((time.time() - t0) * 1000),
        "out": str(out_path), "err": str(err_path),
        "out_sha256": digest.hexdigest(), "out_bytes": out_bytes,
        "err_sha256": _sha256_file(err_path), "err_bytes": err_bytes,
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
        for stream in ("out", "err"):
            log = pathlib.Path(str(s.get(stream) or "")).as_posix()
            out.append(f"日志独立复核({stream}): 重算 sha256({log}) 与记录比对"
                       f"  # 期望 {s.get(stream + '_sha256')}")
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
        plan.append({"kind": "log_hash", "stream": "out", "path": s.get("out"),
                     "expect_sha256": s.get("out_sha256")})
        plan.append({"kind": "log_hash", "stream": "err", "path": s.get("err"),
                     "expect_sha256": s.get("err_sha256")})
        plan.append({"kind": "direct_rerun", "argv": list(s["argv"]), "cwd": cwd,
                     "expect_exit": s["exit"]})
    plan.append({"kind": "chain", "argv": [sys.executable, "-X", "utf8", gate, "--verify-chain"]})
    plan.append({"kind": "seal", "argv": [sys.executable, "-X", "utf8", gate,
                                          "--seal", run_id]})
    return plan


_STEP_KEYS = ("step", "argv", "exit", "ok", "optional", "ms", "out", "err", "out_sha256",
              "out_bytes", "err_sha256", "err_bytes", "skipped_lines")


def run_pipeline(manifest: dict, actor: str = "local", trigger: str = "manual") -> dict:
    """独占护栏内顺序执行；start/final 两行入账（final 带封印与判定）。"""
    run_id = hub_core.new_run_id()
    adm = hub_core.admit(run_id, manifest["resource_class"], os.getpid())
    if not adm.get("ok"):
        return {"ok": False, "busy": True, "run_id": run_id,
                "resource_class": manifest["resource_class"],
                "error": f"准入被拒：{adm.get('reason')}——等活跃运行结束或改时机（查 hub_runs）",
                "active": adm.get("active")}
    fp = fingerprint(manifest)
    hub_core.append_row({"id": run_id, "phase": "start", "pipeline": manifest["id"],
                         "actor": actor, "trigger": trigger, "fingerprint": fp})
    try:
        steps = [_run_step(run_id, st) for st in manifest["steps"]]
        verdict = _verdict(steps)
        payload = {"pipeline": manifest["id"], "manifest_sha256": fp["manifest_sha256"],
                   "git_head": fp["git_head"], "git_dirty": fp["git_dirty"],
                   "verdict": verdict,
                   "steps": [{k: s[k] for k in ("step", "exit", "out_sha256", "err_sha256")}
                             for s in steps]}
        seal = seal_of(payload)
        row = hub_core.append_row({"id": run_id, "phase": "final", "pipeline": manifest["id"],
                                   "actor": actor, "trigger": trigger, "verdict": verdict,
                                   "seal": seal, "fingerprint": fp,
                                   "steps": [{k: s[k] for k in _STEP_KEYS} for s in steps]})
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
