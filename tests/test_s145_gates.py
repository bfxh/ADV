"""S145 契约：本地审核门（不依赖 GitHub/Linux）+ 协议协商与握手留痕。

四锁：
1. **本地门与 CI 同源不漂移**——core.yml 里出现的每道门脚本都必须在本仓
   `scripts/local_gate.py` 的步骤里（防"CI 加门、本地没加"或反向漂移）；
2. **钩子分档**——pre-commit 跑快门（--fast）、pre-push 跑全门（含双测/clippy）；
3. **真门验证**——UNIFIED_RX_GATE_FORCE_FAIL 注入必须让门判红（不是永远绿）；
4. **握手审计（B1）**——版本协商白名单语义 + 留痕落盘（谁/什么版本/协商结果）
   + 留痕失败永不阻断握手。
"""
import ast
import json
import os
import re
import subprocess
import sys

import server

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GATE = os.path.join(ROOT, "scripts", "local_gate.py")
CORE = os.path.join(ROOT, ".github", "workflows", "core.yml")
ADV = os.path.join(ROOT, ".github", "workflows", "adv.yml")


# 门步在 CI 的落点里有三类按**脚本名**查不到：CI 把 pytest 交给分片计划展开，
# cargo/clippy 是 cargo 子命令而非仓内脚本。显式登记而非靠名字子串蒙混——
# 子串匹配既会放过"CI 其实没跑这步"，也会因别处的同名提及而假绿。
CI_INDIRECT = {
    "pytest": "shard_plan.py --plan",
    "cargo-test": "cargo test",
    "clippy": "clippy",
}


def _steps() -> list[dict[str, str]]:
    """STEPS 的单一真值源是 local_gate.py 源码（ast 解析，与 claim-gate 的步数口径同形）。"""
    tree = ast.parse(_read(GATE))
    node = next((n for n in ast.walk(tree) if isinstance(n, ast.Assign)
                 and any(getattr(t, "id", "") == "STEPS" for t in n.targets)), None)
    if node is None or not isinstance(node.value, ast.List):
        raise AssertionError("local_gate.py 里读不到 STEPS 列表——门链被改名？")
    rows: list[dict[str, str]] = []
    for el in node.value.elts:
        if not isinstance(el, ast.Tuple) or len(el.elts) < 3:
            continue
        name, argv, tier = el.elts[0], el.elts[1], el.elts[2]
        if not (isinstance(name, ast.Constant) and isinstance(tier, ast.Constant)):
            continue
        scripts = [a.value for a in (argv.elts if isinstance(argv, ast.List) else [])
                   if isinstance(a, ast.Constant) and isinstance(a.value, str)
                   and a.value.endswith(".py")]
        rows.append({"name": str(name.value), "tier": str(tier.value),
                     "script": scripts[0] if scripts else ""})
    return rows


def _runs_in_ci(row: dict[str, str], core: str) -> bool:
    name = row["name"]
    if name in CI_INDIRECT:
        return CI_INDIRECT[name] in core
    return bool(row["script"]) and row["script"] in core


def _read(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


def _py(*args, cwd=ROOT):
    return subprocess.run([sys.executable, "-X", "utf8", *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=cwd, shell=False, timeout=600)


def test_local_gate_covers_every_ci_gate_script():
    core = _read(CORE)
    ci_scripts = set(re.findall(r"(?:scripts/[a-z0-9_]+\.py|bench/tool_evals\.py)", core))
    step_scripts = {r["script"] for r in _steps() if r["script"]}
    missing = {s for s in ci_scripts if s not in step_scripts}
    assert not missing, f"CI 有而本地门没有（漂移）: {sorted(missing)}"


def _ci_union():
    """两份 workflow 的并集文本。

    2026-10-06 的实测教训（DD-0010）：core.yml 的 on.push 只列 main，本分支由 adv.yml 守。
    只看 core.yml 会把"接在 adv.yml 上的门"判成没落点，而"接在 core.yml 上的门"对本分支
    其实一步都没跑——两个方向都错。逐分支的精确口径由 scripts/ci_wiring_gate.py 管，
    这里只保证"每道门至少在某条 CI 上真跑"。
    """
    return _read(CORE) + "\n" + _read(ADV)


def test_every_gate_step_has_a_ci_landing():
    """反向锁：STEPS 的每一步都必须在 CI 有落点——「本地加门、CI 不跑」同样是漂移。"""
    never = [r["name"] for r in _steps() if not _runs_in_ci(r, _ci_union())]
    assert not never, f"这些门步在两份 workflow 里都找不到落点: {sorted(never)}"


def test_reverse_lock_has_teeth() -> None:
    """反假绿：把 CI 里某一步的落点抹掉，反向锁必须点名到那一步（否则它是个装饰）。"""
    victim = next(r for r in _steps() if r["script"] and r["name"] not in CI_INDIRECT)
    here = _read(CORE)
    needle = victim["script"].split("/")[-1]
    src = CORE if needle in here else ADV
    assert needle in _read(src), f"{victim['name']} 的落点在两份 workflow 里都找不到，测了个空"
    other = _read(ADV) if src == CORE else _read(CORE)
    tampered = "\n".join(ln for ln in _read(src).splitlines() if needle not in ln) + "\n" + other
    caught = [r["name"] for r in _steps() if not _runs_in_ci(r, tampered)]
    assert victim["name"] in caught, f"抹掉 {victim['name']} 的 CI 落点却没判红: {caught}"


def test_hooks_split_fast_and_full():
    pre_commit = _read(os.path.join(ROOT, ".githooks", "pre-commit"))
    pre_push = _read(os.path.join(ROOT, ".githooks", "pre-push"))
    assert "local_gate.py --fast" in pre_commit, "pre-commit 必须只跑快门"
    assert "local_gate.py" in pre_push and "--fast" not in pre_push, \
        "pre-push 必须跑全门（双绿红线）"


def test_local_gate_force_fail_is_a_real_gate():
    env = dict(os.environ, UNIFIED_RX_GATE_FORCE_FAIL="toolface")
    cp = subprocess.run([sys.executable, "-X", "utf8", GATE, "--only", "toolface"],
                        capture_output=True, text=True, encoding="utf-8",
                        errors="replace", env=env, cwd=ROOT, shell=False, timeout=300)
    assert cp.returncode != 0, "注入失败还绿——假门"
    assert "LOCAL-GATE FAIL" in cp.stdout


def test_local_gate_list_shows_all_steps():
    cp = _py(GATE, "--list")
    assert cp.returncode == 0
    rows = _steps()
    listed = [ln for ln in cp.stdout.splitlines() if ln.strip()]
    assert len(listed) == len(rows), f"--list 行数 {len(listed)} ≠ STEPS 条数 {len(rows)}"
    for row in rows:
        assert f"{row['tier']} {row['name']}" in cp.stdout, \
            f"门步 {row['name']}（档位 {row['tier']}）没按档位进 --list"


def test_negotiate_version_semantics():
    assert server._negotiate_version("2025-03-26") == "2025-03-26"
    assert server._negotiate_version("2026-07-28") == server.PROTOCOL_VERSION
    assert server._negotiate_version(None) == server.PROTOCOL_VERSION
    assert server._negotiate_version(20250326) == server.PROTOCOL_VERSION


def test_handshake_recorded_and_reply_negotiated(tmp_path):
    p = tmp_path / "clients.jsonl"
    os.environ["UNIFIED_RX_CLIENTS_LOG"] = str(p)
    try:
        r = server._handle({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                            "params": {"protocolVersion": "2026-07-28",
                                       "clientInfo": {"name": "zcode",
                                                      "version": "9.9"}}})
        assert r["result"]["protocolVersion"] == server.PROTOCOL_VERSION
        line = json.loads(p.read_text(encoding="utf-8").strip())
        assert line["requested"] == "2026-07-28"
        assert line["negotiated"] == server.PROTOCOL_VERSION
        assert line["client"] == "zcode" and line["client_version"] == "9.9"
    finally:
        os.environ.pop("UNIFIED_RX_CLIENTS_LOG", None)


def test_handshake_record_failure_never_breaks(tmp_path):
    d = tmp_path / "as_dir"          # 落点是个目录 → 写入必失败
    d.mkdir()
    os.environ["UNIFIED_RX_CLIENTS_LOG"] = str(d)
    try:
        r = server._handle({"jsonrpc": "2.0", "id": 2, "method": "initialize",
                            "params": {"protocolVersion": "2025-03-26"}})
        assert r["result"]["protocolVersion"] == "2025-03-26"
    finally:
        os.environ.pop("UNIFIED_RX_CLIENTS_LOG", None)


def test_audit_copy_dirty_guard(tmp_path):
    dest = str(tmp_path / "audit-copy")
    script = os.path.join(ROOT, "scripts", "audit_copy.py")
    ok = _py(script, dest, "--allow-dirty")
    assert ok.returncode == 0 and "AUDIT-COPY OK" in ok.stdout, ok.stderr
    st = subprocess.run(["git", "-C", ROOT, "status", "--porcelain"],
                        capture_output=True, text=True, timeout=60)
    if (st.stdout or "").strip():        # 开发中必然是脏树 → 不带标志必须拒
        cp2 = _py(script, dest + "2")
        assert cp2.returncode != 0 and "未提交" in cp2.stderr
