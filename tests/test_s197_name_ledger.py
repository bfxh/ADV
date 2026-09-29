"""S197 契约：名字账门（`scripts/name_ledger_gate.py` + `spec/name-ledger.json`）。

由来：claim-gate 钉了**数字**、selftest 钉了 **skills/*.md 的名字**，但路由意图表
`_INTENTS`、能力清单 `_CAPABILITIES`、README 工具面章节三张镜像面无账——实测
（2026-09-30 探针）`capability_manifest` 路由第一候选 `risk_rank` 不在册。
判据两头：
1. **真仓绿**：默认参数直接跑真路径（在册闭集经子进程从 registry 取）；
2. **必红四向**：三张面各塞一个假名 ⇒ 红且点名；豁免理由占位未填 ⇒ 红；
3. **fail-closed**：被检文件缺失 / `_INTENTS` 提取数低于下限 / 豁免表缺失或坏 JSON ⇒ 红。
"""
import json
import pathlib
import shutil
import subprocess
import sys
from typing import Any

import registry  # conftest 已把仓根入 sys.path；导入即注册由 tools 副作用完成
import tools  # noqa: F401

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "name_ledger_gate.py"
PY = sys.executable


def _live_dump(tmp_path: pathlib.Path) -> str:
    names = [t["name"] for t in registry.list_tools()]
    p = tmp_path / "live.json"
    p.write_text(json.dumps(names), encoding="utf-8")
    return str(p)


def _tmp_root(tmp_path: pathlib.Path) -> pathlib.Path:
    root = tmp_path / "repo"
    (root / "tools").mkdir(parents=True, exist_ok=True)
    (root / "spec").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "tools" / "guard.py", root / "tools" / "guard.py")
    shutil.copyfile(ROOT / "README.md", root / "README.md")
    shutil.copyfile(ROOT / "spec" / "name-ledger.json", root / "spec" / "name-ledger.json")
    return root


def _run(root: pathlib.Path, live: str) -> subprocess.CompletedProcess:
    return subprocess.run([PY, "-X", "utf8", str(GATE), "--root", str(root),
                           "--live-names", live],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(ROOT), shell=False, timeout=300)


# ---------- 真仓自检 ----------

def test_real_repo_green() -> None:
    got = subprocess.run([PY, "-X", "utf8", str(GATE)], capture_output=True, text=True,
                         encoding="utf-8", errors="replace", cwd=str(ROOT),
                         shell=False, timeout=300)
    assert got.returncode == 0, got.stdout + got.stderr
    assert "NAME-LEDGER-GATE OK" in got.stdout and "surfaces=3" in got.stdout, got.stdout


def test_ledger_entries_have_real_why() -> None:
    """豁免表结构性契约：每条必须有 name+why（防"先塞名字进表再说"式的后门）。"""
    data: dict[str, Any] = json.loads((ROOT / "spec" / "name-ledger.json")
                                      .read_text(encoding="utf-8"))
    for ent in data["allow"]:
        assert ent.get("name") and len(str(ent.get("why", ""))) >= 10, ent


# ---------- 必红四向 ----------

def test_canary_intents_drift_red(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    src = (root / "tools" / "guard.py").read_text(encoding="utf-8")
    src = src.replace('("hallucination_guard",)', '("hallucination_guard", "zzz_ghost_route")')
    (root / "tools" / "guard.py").write_text(src, encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "[INTENTS] zzz_ghost_route" in got.stdout, got.stdout


def test_canary_readme_drift_red(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    readme = (root / "README.md").read_text(encoding="utf-8")
    idx = readme.index("## 工具面")
    readme = readme[:idx] + readme[idx:].replace("| 🧰 meta (3) |",
                                                 "| 🧰 meta (3) | `zzz_ghost_doc` |", 1)
    (root / "README.md").write_text(readme, encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "[README] zzz_ghost_doc" in got.stdout, got.stdout


def test_canary_caps_drift_red(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    src = (root / "tools" / "guard.py").read_text(encoding="utf-8")
    src = src.replace('"教训记忆"', '"教训记忆 zzz_ghost_cap"')
    (root / "tools" / "guard.py").write_text(src, encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "[CAPS] zzz_ghost_cap" in got.stdout, got.stdout


def test_canary_placeholder_exempt_red(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    data: dict[str, Any] = json.loads((root / "spec" / "name-ledger.json")
                                      .read_text(encoding="utf-8"))
    data["allow"].append({"name": "anything", "why": "TODO: 待填"})
    (root / "spec" / "name-ledger.json").write_text(
        json.dumps(data, ensure_ascii=False), encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "占位未填" in got.stdout, got.stdout


# ---------- fail-closed ----------

def test_fail_closed_missing_ledger(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    (root / "spec" / "name-ledger.json").unlink()
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "豁免表缺失" in got.stdout, got.stdout


def test_fail_closed_bad_json(tmp_path: pathlib.Path) -> None:
    root = _tmp_root(tmp_path)
    (root / "spec" / "name-ledger.json").write_text("{ nope", encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert got.returncode != 0 and "坏 JSON" in got.stdout, got.stdout


def test_fail_closed_empty_intents(tmp_path: pathlib.Path) -> None:
    """`_INTENTS` 提取数低于下限 ⇒ 红（"解析悄悄数 0"不许冒充绿）。"""
    root = _tmp_root(tmp_path)
    (root / "tools" / "guard.py").write_text(
        '_INTENTS = ()\n_CAPABILITIES = {"有": [], "没有": []}\n', encoding="utf-8")
    got = _run(root, _live_dump(tmp_path))
    assert (got.returncode != 0 and "低于下限" in got.stdout.replace("< 下限", "低于下限")) \
        or "下限" in got.stdout, got.stdout


def test_fail_closed_live_names_empty(tmp_path: pathlib.Path) -> None:
    live = tmp_path / "live.json"
    live.write_text("[]", encoding="utf-8")
    got = _run(_tmp_root(tmp_path), str(live))
    assert got.returncode != 0 and "在册闭集为空" in got.stdout, got.stdout
