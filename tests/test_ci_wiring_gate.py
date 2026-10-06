"""CI 接线完整性门（`scripts/ci_wiring_gate.py` + `spec/ci-wiring.json`）的契约。

这条门存在的理由是一条真实事故：设计债门被接进 `core.yml`，而 `core.yml` 的 `on.push` 只列
`main` ⇒ 在 `adv-rewrite` 上一次都没跑过，我却据此断言"带红推送 CI 会红"。所以本文件最重要
的一条用例是**复现那个形状**（W2/W3），而不是只测今天绿。
"""

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "ci_wiring_gate.py"
PY = sys.executable


def _load():
    import importlib.util

    spec = importlib.util.spec_from_file_location("ci_wiring_gate", GATE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


G = _load()

ADV_YML = """
on:
  push:
    branches: [adv-rewrite, main]
  pull_request:
jobs:
  core:
    steps:
      - uses: actions/checkout@abc # v7
      - name: gates
        run: cargo run -p xtask -- gate
      - name: Debt gate
        run: python -X utf8 scripts/debt_gate.py
"""

CORE_YML = """
on:
  push:
    branches:
      - main   # 旧项目 CI 只守 main
  workflow_dispatch:
jobs:
  core:
    steps:
      - name: Set up Python 3.14
        uses: actions/setup-python@x
      - name: Debt gate
        run: python -X utf8 scripts/debt_gate.py
"""

INDIRECT = {"xtask-gate": "xtask -- gate", "claim-gate": "scripts/claim_gate.py"}
STEPS = {"debt-gate", "claim-gate", "xtask-gate", "path-gate"}


def test_parse_branches_reads_all_three_shapes():
    """流式 `[a, b]`、块式 `- item`（带行内注释）、以及紧凑行内单值都要能读出来。"""
    assert G.parse_branches(ADV_YML) == {"adv-rewrite", "main"}
    assert G.parse_branches(CORE_YML) == {"main"}
    assert G.parse_branches("on:\n  push:\n    branches: dev\n") == {"dev"}


def test_w2_reproduces_the_real_incident_branch_not_triggered():
    """今天那条假账的形状：清单说 adv-rewrite 看 core.yml，而 core.yml 只触发 main。"""
    cfg = {"workflow": ".github/workflows/core.yml", "why": "x", "gates": ["debt-gate"]}
    got = G.check_branch("adv-rewrite", cfg, CORE_YML, INDIRECT, STEPS)
    assert any(p.startswith("W2") and "on.push.branches" in p for p in got), got
    assert any("['main']" in p for p in got), f"要顺手打出实际触发的分支，别让人猜：{got}"


def test_w3_gate_declared_but_not_actually_a_step():
    cfg = {"workflow": ".github/workflows/adv.yml", "why": "x", "gates": ["claim-gate"]}
    got = G.check_branch("adv-rewrite", cfg, ADV_YML, INDIRECT, STEPS)
    assert any(p.startswith("W3") and "claim-gate" in p for p in got), got
    # 反向对照：真接了的门不许误报。
    cfg_ok = {"workflow": ".github/workflows/adv.yml", "why": "x", "gates": ["debt-gate",
                                                                            "xtask-gate"]}
    assert G.check_branch("adv-rewrite", cfg_ok, ADV_YML, INDIRECT, STEPS) == []


def test_w4_orphan_script_step_is_named():
    """逐门认领的 workflow 里出现没人认领的 scripts 门步 ⇒ 红（"写了步骤但清单不知道"）。"""
    text = ADV_YML + "      - name: Lint gate\n        run: python scripts/lint_gate.py\n"
    got = G.check_orphans({".github/workflows/adv.yml": text}, {"debt-gate"}, INDIRECT)
    assert any(p.startswith("W4") and "lint_gate.py" in p for p in got), got
    assert G.check_orphans({".github/workflows/adv.yml": ADV_YML}, {"debt-gate"}, INDIRECT) == []


def test_w5_missing_why_and_drifted_gate_name():
    cfg = {"workflow": ".github/workflows/adv.yml", "why": "   ", "gates": ["no-such-gate"]}
    got = G.check_branch("adv-rewrite", cfg, ADV_YML, INDIRECT, STEPS)
    assert any(p.startswith("W5") and "why" in p for p in got), got
    assert any("no-such-gate" in p for p in got), got


def test_w1_missing_workflow_file_is_red_not_silence():
    cfg = {"workflow": ".github/workflows/absent.yml", "why": "x", "gates": ["debt-gate"]}
    got = G.check_branch("main", cfg, None, INDIRECT, STEPS)
    assert got and got[0].startswith("W1"), got


def test_real_repository_wiring_is_green_via_subprocess():
    """真仓自检：今天这两条分支的清单必须真跑得过，且绿得有内容（不是空清单蒙过）。"""
    cp = subprocess.run([PY, "-X", "utf8", str(GATE)], cwd=str(ROOT),
                        capture_output=True, text=True)
    assert cp.returncode == 0, cp.stdout + cp.stderr
    assert "CI-WIRING OK" in cp.stdout, cp.stdout
    reg = cp.stdout
    assert "adv-rewrite" in reg or "adv.yml" in reg, reg          # 两条分支都被读出来
    assert "core.yml" in cp.stdout and "['main']" in cp.stdout, cp.stdout
