"""hub_gate.py —— 平台层防骗判据机器门（S176，spec/HUB.md §6.3）。

判据（任一不达标即红；全部在临时数据根里跑，**不污染真实账本**）：
  ① 本仓 pipelines/ 无非法 manifest；
  ② 金丝雀三态：必绿（green）/ 必红（exit 3 → red）/ 含 SKIP 只算 green_with_skips；
  ③ **parity**：平台跑与直跑同 argv/cwd——退出码 + stdout 逐字节一致；
  ④ 运行指纹八项齐（缺一即红）；
  ⑤ 账本链校验 ok，且**篡改注入必红**（先记基线再验红）；
  ⑥ 复核计划（verify_plan，argv 列表形态）**真执行**：日志哈希重算一致 + 直跑退出码一致；
  ⑦ 授权矩阵：坏令牌 None / viewer 不可触发 / MCP 无 __authorized 必拒。

独立复核入口（供智能体/人**不信任平台地**复核）：
  python -X utf8 scripts/hub_gate.py --verify-chain
  python -X utf8 scripts/hub_gate.py --seal <run_id>
"""
# ruff: noqa: E402  # 本文件刻意先建 sys.path 再 import 仓内模块（scripts/ 直跑入口惯例）
from __future__ import annotations

import contextlib
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import hub_auth
import hub_core
import hub_live
import hub_runner
import server_web

PY = sys.executable
FP_KEYS = ("manifest_sha256", "git_head", "git_dirty", "argv0", "cwd",
           "env_keys_hash", "python", "cargo")


@contextlib.contextmanager
def _env(root: pathlib.Path, pipes: pathlib.Path):
    keys = ("UNIFIED_RX_HUB_ROOT", "UNIFIED_RX_HUB_PIPELINES")
    old = {k: os.environ.get(k) for k in keys}
    os.environ["UNIFIED_RX_HUB_ROOT"] = str(root)
    os.environ["UNIFIED_RX_HUB_PIPELINES"] = str(pipes)
    try:
        yield
    finally:
        for k, v in old.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def _canary_manifest(pid: str, code: str, resource_class: str = "shared") -> dict:
    return {"id": pid, "title": pid, "when": "gate", "resource_class": resource_class,
            "on": ["manual"],
            "steps": [{"step": "s", "cmd": [PY, "-X", "utf8", "-c", code], "timeout_s": 60}]}


def _write_canaries(d: pathlib.Path) -> None:
    cans = (("adv.canary-ok", "print('canary-ok')", "shared"),
            ("adv.canary-bad", "import sys; sys.exit(3)", "shared"),
            ("adv.canary-skip", "print('SKIP canary-line')", "shared"),
            ("adv.canary-live",
             "import time; print('live-1', flush=True); time.sleep(2.0);"
             " print('live-2', flush=True)", "shared"),
            ("adv.canary-sched", "import time; time.sleep(1.2); print('sched')", "shared"),
            ("adv.canary-excl", "import time; time.sleep(1.2); print('excl')", "exclusive"))
    for pid, code, rc in cans:
        (d / f"{pid}.json").write_text(
            json.dumps(_canary_manifest(pid, code, rc)), encoding="utf-8")


def _run_canaries() -> dict[str, dict]:
    pipes, invalid = hub_core.load_pipelines()
    if invalid:
        raise AssertionError(f"金丝雀 manifest 非法：{invalid}")
    return {pid: hub_runner.run_pipeline(pipes[pid], actor="gate", trigger="canary")
            for pid in ("adv.canary-ok", "adv.canary-bad", "adv.canary-skip")}


def _parity(res: dict) -> tuple[bool, str]:
    """平台跑 vs 直跑：退出码 + stdout 逐字节一致（机制①）。argv 列表，shell=False。"""
    s = res["steps"][0]
    cp = subprocess.run(s["argv"], cwd=str(hub_core.REPO_ROOT), capture_output=True,
                        shell=False, timeout=60)
    direct = hashlib.sha256(cp.stdout).hexdigest()
    ok = cp.returncode == s["exit"] and direct == s["out_sha256"]
    return ok, f"exit {cp.returncode}=={s['exit']} stdout {direct[:12]}=={s['out_sha256'][:12]}"


def _tamper() -> tuple[bool, str]:
    """篡改注入：改一行内容 → 链校验必红（金丝雀；先记基线再验红）。"""
    p = hub_core.runs_path()
    base = hub_core.verify_chain()
    if not base.get("ok"):
        return False, f"篡改前链已坏：{base.get('reason')}"
    text = p.read_text(encoding="utf-8")
    marker = '"verdict": "green"'
    if marker not in text:
        marker = '"verdict":"green"'
    if marker not in text:
        return False, "找不到可篡改的 verdict 字段（金丝雀失效即红）"
    p.write_text(text.replace(marker, marker.replace("green", "gren"), 1), encoding="utf-8")
    after = hub_core.verify_chain()
    ok = not after.get("ok")
    return ok, (f"篡改后 {after.get('reason')}" if ok else "篡改未被发现（仪器失效）")


def _verify_plan_check(res: dict) -> tuple[bool, str]:
    """复核计划真执行：日志哈希重算一致 + 直跑退出码一致（argv 列表，无 shell）。"""
    plan = res.get("verify_plan") or []
    logs = [e for e in plan if e.get("kind") == "log_hash"]
    reruns = [e for e in plan if e.get("kind") == "direct_rerun"]
    kinds = {e.get("kind") for e in plan}
    if not logs or not reruns or not {"chain", "seal"} <= kinds:
        return False, f"plan 不全 kinds={sorted(kinds)}"
    ok_log = all(
        hashlib.sha256(pathlib.Path(str(e["path"])).read_bytes()).hexdigest() == e["expect_sha256"]
        for e in logs)
    e0 = reruns[0]
    cp = subprocess.run(list(e0["argv"]), cwd=str(e0["cwd"]), capture_output=True,
                        shell=False, timeout=60)
    ok_run = cp.returncode == e0["expect_exit"]
    return ok_log and ok_run, f"log_hash={ok_log} rerun_exit={cp.returncode}=={e0['expect_exit']}"


def _auth_matrix() -> tuple[bool, str]:
    import registry
    bad_token_rejected = hub_auth.verify("admin", "wrong-token") is None
    roles = hub_auth.can_trigger("viewer") is False and hub_auth.can_trigger("operator") is True
    denied = registry.call("hub_run", {"pipeline": "adv.gate-fast"})
    no_auth = denied.get("ok") is False
    return (bad_token_rejected and roles and no_auth), \
        f"坏令牌拒={bad_token_rejected} 角色矩阵={roles} MCP 无授权拒={no_auth}"


def _has_final(run_id: str) -> bool:
    return any(r.get("phase") == "final" and r.get("id") == run_id
               for r in hub_core.read_rows())


def _live_check() -> tuple[bool, str]:
    """live 判据（S177）：日志须在**运行结束前**就可见——真流式，而非结束后一次性给。

    观测口径：起一个"先输出一行、再 sleep 2s"的金丝雀，主线程按 offset tail 日志文件；
    判据 = `收到 live-1 的时刻` 早于 `账本出现 final 行的时刻` 至少 0.3s（post-hoc 实现
    两者几乎同时 ⇒ 判红）。不依赖绝对时钟精度，CI 慢也不会假红。
    """
    pipes, _inv = hub_core.load_pipelines()
    if "adv.canary-live" not in pipes:
        return False, "缺 adv.canary-live 金丝雀"
    before = {r.get("id") for r in hub_core.read_rows()}
    box: dict = {}
    th = threading.Thread(
        target=lambda: box.update(hub_runner.run_pipeline(pipes["adv.canary-live"],
                                                          actor="gate", trigger="canary")),
        daemon=True)
    th.start()
    run_id: str | None = None
    early: float | None = None
    final: float | None = None
    t0 = time.monotonic()
    while time.monotonic() - t0 < 30:
        if run_id is None:
            for r in hub_core.read_rows():
                if r.get("phase") == "start" and r.get("id") not in before:
                    run_id = str(r.get("id"))
        if run_id:
            for p, _step, _stream in hub_live.log_files(run_id):
                text, _off = hub_live.read_from(p, 0)
                if "live-1" in text and early is None:
                    early = time.monotonic()
            if _has_final(run_id):
                final = time.monotonic()
                break
        time.sleep(0.05)
    th.join(20)
    ok = bool(early and final and (final - early) > 0.3)
    return ok, (f"early={early is not None} final={final is not None} "
                f"gap={'%.2fs' % (final - early) if early and final else '-'} "
                f"verdict={box.get('verdict')}")


def _sched_check() -> tuple[bool, str]:
    """调度判据（S178/S179）：共享位共存 + 上限拒绝 + exclusive 互斥 + 清空后可独占。

    **确定性口径**：第 1 个 shared 位由本进程**手工持有**（不会自己消失），第 2 个走真实
    runner ⇒ "共存"不再依赖线程竞速（S179 CI 实锤：双线程互相等待会因慢机启动延迟而假红，
    `coexist=False`）。真并行的执行重叠由 runner 语义 + live-log 判据共同背书。
    """
    pipes, _inv = hub_core.load_pipelines()
    for pid in ("adv.canary-sched", "adv.canary-excl"):
        if pid not in pipes:
            return False, f"缺金丝雀 {pid}"
    t0 = time.monotonic()
    held = hub_core.admit("gate-hold", "shared", os.getpid())
    if not held.get("ok"):
        return False, f"手工占位失败：{held.get('reason')}"
    box: dict = {}
    th = threading.Thread(
        target=lambda: box.update(hub_runner.run_pipeline(pipes["adv.canary-sched"],
                                                          actor="gate", trigger="canary")),
        daemon=True)
    th.start()
    coexist = False
    for _ in range(240):
        if len(hub_core.read_active()) >= 2:
            coexist = True
            break
        time.sleep(0.05)
    third = hub_runner.run_pipeline(pipes["adv.canary-sched"], actor="gate", trigger="canary")
    excl_busy = hub_runner.run_pipeline(pipes["adv.canary-excl"], actor="gate", trigger="canary")
    th.join(60)
    hub_core.release_active("gate-hold")
    wall = time.monotonic() - t0
    excl_alone = hub_runner.run_pipeline(pipes["adv.canary-excl"], actor="gate", trigger="canary")
    ok_third = third.get("busy") is True and "上限" in str(third.get("error"))
    ok_excl_busy = excl_busy.get("busy") is True and "exclusive" in str(excl_busy.get("error"))
    ok_excl_alone = excl_alone.get("ok") is True
    ok = coexist and ok_third and ok_excl_busy and ok_excl_alone
    return ok, (f"coexist={coexist} third_busy_at_limit={ok_third} excl_mutex={ok_excl_busy} "
                f"excl_alone={ok_excl_alone} wall={wall:.2f}s verdict={box.get('verdict')}")


def _auth_session_check() -> tuple[bool, str]:
    """鉴权分层判据（S179）：status 摘要公开 / 读面无会话 401 / 坏令牌 401 / 登录后 200 /
    非 admin 管用户 403。只连本进程刚起的回环服务（host 字面量白名单，不出网）。"""
    import http.client as hc
    boot = hub_auth.bootstrap()
    srv = server_web.make_server(0)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    host, port = "127.0.0.1", int(srv.server_address[1])

    def _call(method: str, path: str, payload: dict | None = None,
              cookie: str | None = None) -> tuple[int, str, str | None]:
        conn = hc.HTTPConnection(host, port, timeout=20)
        try:
            body = json.dumps(payload).encode() if payload is not None else None
            headers = {"Content-Type": "application/json"} if body else {}
            if cookie:
                headers["Cookie"] = cookie
            conn.request(method, path, body=body, headers=headers)
            r = conn.getresponse()
            return r.status, r.read().decode("utf-8", "replace"), r.getheader("Set-Cookie")
        finally:
            conn.close()

    def _cookie_of(sc: str | None) -> str | None:
        return "hub_sid=" + sc.split("hub_sid=")[1].split(";")[0] if sc else None

    try:
        st_status = _call("GET", "/api/status")[0]
        st_anon = _call("GET", "/api/runs")[0]
        st_bad = _call("POST", "/api/login", {"user": "admin", "token": "nope"})[0]
        st_login, _t, sc = _call("POST", "/api/login",
                                 {"user": "admin", "token": boot["bootstrap_token"]})
        ck = _cookie_of(sc)
        st_read = _call("GET", "/api/runs", cookie=ck)[0] if ck else 0
        v = hub_auth.add_user("gateviewer", "viewer")
        _s2, _t2, sc2 = _call("POST", "/api/login",
                              {"user": "gateviewer", "token": v.get("token")})
        ck2 = _cookie_of(sc2)
        st_users = _call("GET", "/api/users", cookie=ck2)[0] if ck2 else 0
    finally:
        srv.shutdown()
        srv.server_close()
    ok = (st_status == 200 and st_anon == 401 and st_bad == 401
          and st_login == 200 and st_read == 200 and st_users == 403)
    return ok, (f"status={st_status} anon={st_anon} bad={st_bad} login={st_login} "
                f"read={st_read} viewer_users={st_users}")


def _propose_check() -> tuple[bool, str]:
    """提案隔离判据（S180，G1）：真跑一次提案——**主树逐位不变** + 隔离树无残留 +
    结构完整（patch/apply_hint/never_merged）。G1 的机器版本：平台永不改主树、永不合并。"""
    import hub_propose
    root = hub_propose.repo_root()
    _rc, head0, _e = hub_propose._git(root, ["rev-parse", "HEAD"])
    _rc, st0, _e = hub_propose._git(root, ["status", "--porcelain"])
    res = hub_propose.propose(["ruff-fix"])
    _rc, head1, _e = hub_propose._git(root, ["rev-parse", "HEAD"])
    _rc, st1, _e = hub_propose._git(root, ["status", "--porcelain"])
    _rc, wtl, _e = hub_propose._git(root, ["worktree", "list"])
    leaked = [ln for ln in wtl.splitlines() if "adv-propose-" in ln]
    ok = (res.get("ok") is True and res.get("unchanged_main") is True
          and head0 == head1 and st0 == st1 and not leaked
          and isinstance(res.get("patch"), str) and bool(res.get("apply_hint"))
          and res.get("never_merged") is True)
    return ok, (f"unchanged_main={res.get('unchanged_main')} head_stable={head0 == head1} "
                f"status_stable={st0 == st1} worktree_leak={len(leaked)} "
                f"patch_bytes={len(res.get('patch') or '')} files={len(res.get('files') or [])} "
                f"error={res.get('error') or '-'}")


def _impact_check() -> tuple[bool, str]:
    """静态影响面判据（S181，方向①）：① 本仓真跑一次（只验契约诚实：不炸、退化带理由、
    选中项都是真文件）；② **金丝雀小仓**：造一次真变更，断言"间接依赖不漏"（传递闭包）。

    为什么带金丝雀：shallow 检出下本仓可能"无可比基线"（合法状态），只跑本仓会假绿。
    """
    import hub_impact
    import hub_propose
    res = hub_impact.impact(base="HEAD")
    root = hub_impact.repo_root()
    sel = res.get("impacted_tests") or []
    honest = (res.get("ok") is True
              and (res.get("fallback") in ("none", "full") or bool(sel))
              and all((root / t).is_file() and t.startswith("tests/") for t in sel)
              and (res.get("fallback") is None or bool(res.get("reason"))))
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-impact-"))
    caught = False
    try:
        (tmp / "pkg").mkdir()
        (tmp / "tests").mkdir()
        (tmp / "pkg" / "__init__.py").write_text("", encoding="utf-8")
        (tmp / "pkg" / "b.py").write_text("def h():\n    return 1\n", encoding="utf-8")
        (tmp / "pkg" / "a.py").write_text("from pkg.b import h\n\n\ndef r():\n    return h()\n",
                                          encoding="utf-8")
        (tmp / "tests" / "test_a.py").write_text(
            "from pkg.a import r\n\n\ndef test_r():\n    assert r()\n", encoding="utf-8")
        hub_propose._git(tmp, ["init", "-q"])
        for args in (["add", "-A"], ["commit", "-qm", "i"]):
            hub_propose._git(tmp, ["-c", "user.name=g", "-c", "user.email=g@g", *args])
        (tmp / "pkg" / "b.py").write_text("def h():\n    return 2\n", encoding="utf-8")
        saved = os.environ.get("UNIFIED_RX_HUB_REPO")
        os.environ["UNIFIED_RX_HUB_REPO"] = str(tmp)
        try:
            res2 = hub_impact.impact(base="HEAD")
        finally:
            if saved is None:
                os.environ.pop("UNIFIED_RX_HUB_REPO", None)
            else:
                os.environ["UNIFIED_RX_HUB_REPO"] = saved
        caught = "tests/test_a.py" in (res2.get("impacted_tests") or [])
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return honest and caught, (f"repo_honest={honest} canary_transitive={caught} "
                               f"fallback={res.get('fallback')} selected={len(sel)}")


def _stat_check() -> tuple[bool, str]:
    """统计判定判据（S182，方向⑩）：三态正确（分离/真超阈/噪声带）+ **确定性**
    （同输入两次逐位相同）+ 小样本不假装判过；噪声带样本用**独占机器实采数据**。"""
    import stat_judge
    noise = [0.898, 1.034, 0.787, 0.862, 0.815, 0.798, 1.272]      # 实采（semantic 7 轮）
    v1 = stat_judge.judge([0.50, 0.52, 0.48, 0.51, 0.49], 0.85)
    v2 = stat_judge.judge([1.10, 1.15, 1.20, 1.12, 1.18], 0.85)
    v3 = stat_judge.judge(noise, 0.95)
    det = stat_judge.judge(noise, 0.95) == v3
    weak = stat_judge.judge([0.9, 0.8], 0.85)
    ok = (v1["verdict"] == "pass" and v2["verdict"] == "fail"
          and v3["verdict"] in ("pass", "inconclusive")
          and det and weak["verdict"] == "inconclusive" and weak["weak"] is True)
    return ok, (f"sep={v1['verdict']} over={v2['verdict']} noise={v3['verdict']} "
                f"det={det} small={weak['verdict']}/{weak['weak']}")


def _checks() -> list[tuple[str, bool, str]]:
    rows: list[tuple[str, bool, str]] = []
    pipes, invalid = hub_core.load_pipelines()
    rows.append(("canary-manifests", not invalid, f"ok={len(pipes)} invalid={invalid}"))
    can = _run_canaries()
    rows.append(("canary-green", can["adv.canary-ok"]["verdict"] == "green",
                 can["adv.canary-ok"]["verdict"]))
    rows.append(("canary-red", can["adv.canary-bad"]["verdict"] == "red",
                 can["adv.canary-bad"]["verdict"]))
    rows.append(("canary-skip-not-green",
                 can["adv.canary-skip"]["verdict"] == "green_with_skips",
                 can["adv.canary-skip"]["verdict"]))
    ok, detail = _parity(can["adv.canary-ok"])
    rows.append(("parity", ok, detail))
    miss = [k for k in FP_KEYS if k not in can["adv.canary-ok"]["fingerprint"]]
    rows.append(("fingerprint-8", not miss, f"missing={miss}"))
    ok, detail = _tamper()
    rows.append(("tamper-detect", ok, detail))
    ok, detail = _verify_plan_check(can["adv.canary-ok"])
    rows.append(("verify-plan", ok, detail))
    ok, detail = _auth_matrix()
    rows.append(("auth-matrix", ok, detail))
    ok, detail = _live_check()
    rows.append(("live-log", ok, detail))
    ok, detail = _sched_check()
    rows.append(("sched-resource-class", ok, detail))
    ok, detail = _auth_session_check()
    rows.append(("auth-session-layers", ok, detail))
    ok, detail = _propose_check()
    rows.append(("propose-isolation", ok, detail))
    ok, detail = _impact_check()
    rows.append(("impact-static", ok, detail))
    ok, detail = _stat_check()
    rows.append(("stat-judge", ok, detail))
    return rows


def _seal_check(run_id: str) -> int:
    for row in hub_core.read_rows():
        if row.get("id") == run_id and row.get("phase") == "final":
            fp = row.get("fingerprint") or {}
            payload = {"pipeline": row.get("pipeline"),
                       "manifest_sha256": fp.get("manifest_sha256"),
                       "git_head": fp.get("git_head"), "git_dirty": fp.get("git_dirty"),
                       "verdict": row.get("verdict"),
                       "steps": [{k: s.get(k) for k in ("step", "exit", "log_sha256")}
                                 for s in row.get("steps") or []]}
            got = hub_runner.seal_of(payload)
            same = got == row.get("seal")
            print(f"SEAL {run_id} recorded={row.get('seal')} recomputed={got} "
                  f"{'OK' if same else 'MISMATCH'}")
            return 0 if same else 1
    print(f"SEAL {run_id} NOT_FOUND")
    return 1


def main(argv: list[str]) -> int:
    if "--verify-chain" in argv:
        v = hub_core.verify_chain()
        print(f"CHAIN ok={v.get('ok')} count={v.get('count')} reason={v.get('reason')}")
        return 0 if v.get("ok") else 1
    if "--seal" in argv:
        return _seal_check(argv[argv.index("--seal") + 1])
    root = pathlib.Path(tempfile.mkdtemp(prefix="adv-hub-gate-"))
    pipes = root / "pipelines"
    pipes.mkdir(parents=True, exist_ok=True)
    _write_canaries(pipes)
    repo_pipes, repo_invalid = hub_core.load_pipelines()   # 真实 env 下的本仓管线
    try:
        with _env(root, pipes):
            rows = _checks()
    finally:
        shutil.rmtree(root, ignore_errors=True)
    rows.insert(0, ("repo-manifests", not repo_invalid,
                    f"ok={len(repo_pipes)} invalid={repo_invalid}"))
    for name, ok, detail in rows:
        print(f"{'OK  ' if ok else 'FAIL'} {name:22s} {detail}")
    bad = [n for n, ok, _d in rows if not ok]
    if bad:
        print(f"HUB-GATE FAIL: {bad}")
        return 1
    print("HUB-GATE OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
