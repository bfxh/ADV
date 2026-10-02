"""S200 契约：会话交接卡门（`scripts/handoff_gate.py` + `spec/handoff.json`）。

由来（zcode 会话 049206af 实锤）：并行长会话靠「继续」链接续，交接上下文只活在
session sqlite/artifacts 里——重建一次意图要翻 423 条消息做考古。本门把交接变成
**仓内账**，锁四件事：
1. **真仓绿**：卡 round 已在 ROUNDLOG 记账、next 锚全可达（期望轮次取自卡，不钉字面值）；
2. **完整性红不可放行**：ghost 轮次 / ghost 锚——`--allow-stale` 之后依旧红
   （同 audit-ledger 的 S199 ghost commit 口径）；
3. **节奏黄可放行**：卡滞后 ROUNDLOG 最新轮 > MAX_BEHIND 判红，但 `--allow-stale`
   /`UNIFIED_RX_HANDOFF_MAX_BEHIND` 显式可调（"先推卡、随后补账"是合法节奏）；
4. **fail-closed**：卡缺失 / 坏 JSON / 缺字段 / next 为空 ⇒ 红（门被清空即失效）。
"""
import json
import os
import pathlib
import subprocess
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "handoff_gate.py"
PY = sys.executable


def _run(env_extra: dict[str, str] | None = None, *args: str) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if env_extra:
        env.update(env_extra)
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args], capture_output=True,
                          text=True, encoding="utf-8", errors="replace", cwd=str(ROOT),
                          shell=False, timeout=120, env=env)


def _fixture(tmp_path: pathlib.Path, card: Any, log_rounds: tuple[str, ...] = ("100", "180"),
             root_files: tuple[str, ...] = ()) -> dict[str, str]:
    root = tmp_path / "repo"
    (root / "spec").mkdir(parents=True, exist_ok=True)
    log = "".join(f"## S{r} —— 夹具\n" for r in log_rounds)
    (root / "spec" / "ROUNDLOG.md").write_text(log, encoding="utf-8")
    card_path = root / "spec" / "handoff.json"
    if isinstance(card, str):
        card_path.write_text(card, encoding="utf-8")
    elif card is not None:
        card_path.write_text(json.dumps(card, ensure_ascii=False), encoding="utf-8")
    for rel in root_files:
        f = root / rel
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text("x\n", encoding="utf-8")
    return {"UNIFIED_RX_HANDOFF_ROOT": str(root), "UNIFIED_RX_HANDOFF": str(card_path),
            "UNIFIED_RX_HANDOFF_LOG": str(root / "spec" / "ROUNDLOG.md")}


def _card(**over: Any) -> dict[str, Any]:
    base: dict[str, Any] = {"round": "S180", "as_of": "2026-10-01", "branch": "feat/x",
                            "next": ["给 scripts/real_gate.py 补 --json 出口"], "open": []}
    base.update(over)
    return base


# ---------- 真仓 ----------

def test_real_repo_green() -> None:
    card = json.loads((ROOT / "spec" / "handoff.json").read_text(encoding="utf-8"))
    got = _run()
    assert got.returncode == 0, got.stdout + got.stderr
    # 轮次取自卡本身：推进轮次是这张卡的正常用法，钉死字面值会让每次交接都必红
    assert f"HANDOFF-GATE OK round={card['round']}" in got.stdout, got.stdout


def test_real_card_shape() -> None:
    card = json.loads((ROOT / "spec" / "handoff.json").read_text(encoding="utf-8"))
    for key in ("round", "as_of", "branch", "next", "open"):
        assert key in card, key
    assert [x for x in card["next"] if str(x).strip()], "next 不许为空"


# ---------- 完整性红：--allow-stale 不豁免 ----------

def test_ghost_round_hard_red(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, _card(round="S999"))
    got = _run(env)
    assert got.returncode == 1 and "ghost 轮次" in got.stdout, got.stdout
    forced = _run(env, "--allow-stale")
    assert forced.returncode == 1 and "ghost 轮次" in forced.stdout, forced.stdout


def test_ghost_anchor_hard_red(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, _card(next=["把 scripts/nope_gate.py 接进门链"]),
                   root_files=("scripts/real_gate.py",))
    got = _run(env)
    assert got.returncode == 1 and "GHOST-ANCHOR" in got.stdout, got.stdout
    forced = _run(env, "--allow-stale")
    assert forced.returncode == 1 and "GHOST-ANCHOR" in forced.stdout, forced.stdout


def test_bare_filename_is_not_anchor(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, _card(next=["续写 handoff.json 的 open 账"]),
                   root_files=("scripts/real_gate.py",))
    got = _run(env)
    assert got.returncode == 0, got.stdout + got.stderr


# ---------- 节奏黄：可显式放行 ----------

def test_lag_red_but_bypassable(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, _card(round="S100"), log_rounds=("100", "120", "180"),
                   root_files=("scripts/real_gate.py",))
    got = _run(env)
    assert got.returncode == 1 and "[LAG]" in got.stdout, got.stdout
    forced = _run(env, "--allow-stale")
    assert forced.returncode == 0 and "WARN-STALE" in forced.stdout, forced.stdout


def test_max_behind_env_tunable(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, _card(round="S120"), log_rounds=("100", "120", "180"),
                   root_files=("scripts/real_gate.py",))
    assert _run(env).returncode == 1  # 滞后 60 > 默认 1
    loose = dict(env, UNIFIED_RX_HANDOFF_MAX_BEHIND="60")
    assert _run(loose).returncode == 0


# ---------- fail-closed ----------

def test_missing_card_red(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, None)
    got = _run(env)
    assert got.returncode == 1 and "交接卡缺失" in got.stdout, got.stdout


def test_bad_json_red(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, "{不是 JSON")
    got = _run(env)
    assert got.returncode == 1 and "坏 JSON" in got.stdout, got.stdout


def test_missing_field_and_empty_next_red(tmp_path: pathlib.Path) -> None:
    env = _fixture(tmp_path, {"round": "S180", "as_of": "2026-10-01",
                              "next": ["随便"], "open": []},
                   root_files=("scripts/real_gate.py",))
    got = _run(env)
    assert got.returncode == 1 and "缺字段 branch" in got.stdout, got.stdout
    env2 = _fixture(tmp_path / "b", _card(next=[]), root_files=("scripts/real_gate.py",))
    got2 = _run(env2)
    assert got2.returncode == 1 and "next 为空" in got2.stdout, got2.stdout
