"""S199 契约：账本可达性（ghost 不可放行）+ 门链失败的一键复现/CI 注解。

由来（两轮实锤）：并行会话 rebase 后 audit-ledger 引用的 head 变 ghost，三个 CI 步
同一根因转红，且旧口径把 ghost 当"过期"——`--allow-stale` 能把它一并放行（账本
完整性问题被当成节奏问题）。同时 `cmd | tail` 管道吞退出码/截细节的假绿连踩两次。
判据：
1. 真仓绿（既有账本每条 head 可达）；
2. 篡 head ⇒ 红且点名"不可达"，**加 --allow-stale 仍红**（完整性不可豁免）；
3. local_gate 红步 ⇒ 打印一键复现命令（--only 逗号列）；
4. GITHUB_ACTIONS=1 时 ⇒ 每个失败步输出 ::error:: 注解。
"""
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PY = sys.executable


def _py(script: str, *args: str, env_extra: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env.update(env_extra or {})
    return subprocess.run([PY, "-X", "utf8", str(ROOT / "scripts" / script), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(ROOT), env=env, shell=False,
                          timeout=600)


def _ledger_copy(tmp_path: pathlib.Path, mutate) -> pathlib.Path:
    doc = json.loads((ROOT / "spec" / "audit-ledger.json").read_text(encoding="utf-8"))
    mutate(doc)
    p = tmp_path / "ledger.json"
    p.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return p


def _audit_env(ledger: pathlib.Path) -> dict[str, str]:
    return {"UNIFIED_RX_AUDIT_LEDGER": str(ledger),
            "UNIFIED_RX_AUDIT_TABLE": str(ROOT / "spec" / "HARDENING.md")}


def test_real_repo_green() -> None:
    got = _py("audit_ledger.py")
    assert got.returncode == 0, got.stdout + got.stderr
    assert "AUDIT-LEDGER OK" in got.stdout


def test_ghost_head_hard_red_not_bypassable(tmp_path: pathlib.Path) -> None:
    def poison(doc: dict) -> None:
        doc["entries"][-1]["head"] = "0123456789abcdef0123456789abcdef01234567"
    ledger = _ledger_copy(tmp_path, poison)
    env = _audit_env(ledger)
    plain = _py("audit_ledger.py", env_extra=env)
    assert plain.returncode == 1 and "不可达" in plain.stdout, plain.stdout
    allowed = _py("audit_ledger.py", "--allow-stale", env_extra=env)
    assert allowed.returncode == 1, "ghost 被 --allow-stale 放行 = 完整性问题被当节奏问题"
    assert "形状/对账不过" in allowed.stdout + allowed.stderr  # sys.exit 文案走 stderr


def test_headless_entries_grandfathered() -> None:
    """早期无 head 的条目不算 ghost（祖父化口径：只查登了 head 的）。"""
    doc = json.loads((ROOT / "spec" / "audit-ledger.json").read_text(encoding="utf-8"))
    headless = [e for e in doc["entries"] if not (e.get("head") or "").strip()]
    assert headless, "账本已全员有 head——本例改为删除末条 head 验证同一路径"
    got = _py("audit_ledger.py")
    assert got.returncode == 0, got.stdout


def test_local_gate_prints_repro_command(tmp_path: pathlib.Path) -> None:
    env = {"UNIFIED_RX_GATE_FORCE_FAIL": "toolface"}
    got = _py("local_gate.py", "--only", "toolface", env_extra=env)
    assert got.returncode == 1, got.stdout
    assert "复现" in got.stdout and "--only toolface" in got.stdout, got.stdout


def test_local_gate_gh_annotations() -> None:
    env = {"UNIFIED_RX_GATE_FORCE_FAIL": "toolface", "GITHUB_ACTIONS": "1"}
    got = _py("local_gate.py", "--only", "toolface", env_extra=env)
    assert got.returncode == 1
    assert "::error::local-gate step failed: toolface" in got.stdout, got.stdout
