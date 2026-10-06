"""门链按工作线分组（DD-0007 的处置）的契约：`spec/gate-lines.json` + `local_gate --line`。

立这条门的理由不是"想少跑几步"，而是快档长期红 13/30 步之后，红就成了背景噪声——
两条工作线都不认为那是自己的账，于是人人学会跳过它。分组只有在**同时**满足两件事时
才算处置而不是洗白：
1. 默认档（不带 `--line`）仍跑全表 ⇒ 没有任何一步因为分组而变得不判；
2. 每一步必须有归属 ⇒ 新增步骤忘了登记就红（"门看着在其实不在"的第三种形状）。
"""

import importlib.util
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GATE = ROOT / "scripts" / "local_gate.py"
REGISTRY = ROOT / "spec" / "gate-lines.json"
PY = sys.executable


def _load():
    spec = importlib.util.spec_from_file_location("local_gate", GATE)
    assert spec is not None and spec.loader is not None, "加载 local_gate 失败"
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _run(*args):
    return subprocess.run([PY, "-X", "utf8", str(GATE), *args],
                          cwd=str(ROOT), capture_output=True, text=True)


LG = _load()
STEP_NAMES = {name for name, *_ in LG.STEPS}


def test_registry_covers_every_step():
    """真仓自检：每一步都有归属，表里也没有漂出去的名字。"""
    lines, problems = LG.load_gate_lines(REGISTRY)
    assert problems == [], f"归属表不完整：{problems}"
    assert set(lines) == {"adv-m2", "umbrella-py"}, sorted(lines)
    for name, entry in lines.items():
        assert (entry.get("why") or "").strip(), f"{name} 没写凭什么管这些步"


def test_orphan_step_is_red():
    """新增步骤忘了登记 ⇒ 红。这条是"分组 ≠ 洗白"的牙齿。"""
    tmp = ROOT / "target" / "gate-lines-orphan.json"
    data = json.loads(REGISTRY.read_text(encoding="utf-8"))
    for entry in data["lines"].values():
        if "pytest" in entry["steps"]:
            entry["steps"].remove("pytest")
    tmp.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    try:
        _lines, problems = LG.load_gate_lines(tmp)
        assert any("步骤无归属：pytest" in p for p in problems), problems
    finally:
        tmp.unlink()


def test_dangling_step_name_in_registry_is_red():
    """表里写了不存在的步骤名 ⇒ 红（改了 STEPS 的名字没同步，静默漂移比缺步更常见）。"""
    tmp = ROOT / "target" / "gate-lines-dangling.json"
    data = json.loads(REGISTRY.read_text(encoding="utf-8"))
    data["lines"]["adv-m2"]["steps"].append("no-such-gate")
    tmp.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    try:
        _lines, problems = LG.load_gate_lines(tmp)
        assert any("no-such-gate" in p for p in problems), problems
    finally:
        tmp.unlink()


def test_missing_registry_is_red_not_silence():
    """表不存在 = 门链裸奔 ⇒ 判红。缺账不等于没债。"""
    _lines, problems = LG.load_gate_lines(ROOT / "target" / "definitely-absent.json")
    assert problems, "缺归属表却返回空违规"
    assert any("gate-lines.json" in p for p in problems), problems


def test_unknown_line_name_is_red_on_the_real_path():
    cp = _run("--line", "no-such-line", "--only", "path-gate")
    assert cp.returncode == 1, cp.stdout + cp.stderr
    assert "未知工作线" in cp.stdout, cp.stdout
    assert "adv-m2" in cp.stdout, "要把在册的线名打出来，别让人猜"


def test_line_filter_is_a_filter_not_a_weaken():
    """`_skip_why`：给了 line_steps 时，不在集合内 ⇒ **纯过滤**（note=None，不进 skipped 名单）。

    单独测这条，是因为"红变少"必须来自归属，不能来自把失败悄悄塞进 skipped 计数——
    那样 LOCAL-GATE 的 `failed=[...]` 会变小而没人看得出原因。
    """
    assert LG._skip_why("lint-gate", "fast", False, False, False, False, None,
                        {"path-gate"}) == ("skip", None)
    assert LG._skip_why("path-gate", "fast", False, False, False, False, None,
                        {"path-gate"}) == ("run", None)
    # 不给 line_steps（默认档）⇒ 全表照判，一步不少。
    assert LG._skip_why("lint-gate", "fast", False, False, False, False, None,
                        None) == ("run", None)


def test_default_tier_still_runs_the_whole_table():
    """默认档不因为分组少跑：`--list` 打出的每一步都必须在归属表里出现且只被过滤一次。"""
    lines, problems = LG.load_gate_lines(REGISTRY)
    assert problems == [], problems
    owned = set()
    for entry in lines.values():
        owned |= set(entry["steps"])
    assert owned == STEP_NAMES, f"归属集与 STEPS 不等：缺 {owned - STEP_NAMES} 多 {STEP_NAMES - owned}"
    cp = _run("--line", "adv-m2", "--only", "path-gate")
    # 给了线名又用 --only 缩到一步：那一步属于 adv-m2 ⇒ 必须真跑（不是被过滤掉）
    assert "path-gate" in cp.stdout, cp.stdout + cp.stderr
