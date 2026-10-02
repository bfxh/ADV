"""S143 契约：工具面注解（annotations/title）+ 体量仪表门。

背景（EXTERNAL-ALIGNMENT A1/A2，2026-09-14 联网对标轮产出）：annotations 是
MCP 规范 2025-03-26（我们钉的版本）即有的字段，此前一直漏发；tools/list 是
会话开场的固定摊派，此前无仪表、只会无声膨胀。

五锁：
1. 注解全覆盖——每个工具都有非空 annotations.title，且 toolmeta 与注册表
   **双向一致**（改名/退役工具不同步即红）；
2. 注解映射授权三档（HARDENING §七 同口径）——① ② 档（不挂门）
   readOnlyHint + idempotentHint；③ 档（requires_auth）readOnlyHint=false +
   destructiveHint=true（显式写出，不押注宿主实现规范默认值）；
3. 体量仪表真跑绿——剥掉沙盒 env 自给自足（同 secrets 门 S134 纪律），且三条帽
   （全量 / core 绝对帽 / **S209 首屏占比**）各自压到极小都必须 FAIL（不是"永远绿"）；
4. 占比算术不许是手写的漂亮数字——用门自己打印的 core/full 反算，必须与它报的
   百分比一致，且帽必须低于"两边同时逼近绝对帽"算出的 65.2%（否则这条门形同虚设）；
5. 体量门是真门——抬帽 env 压到 1 必须 FAIL。
"""
import os
import subprocess
import sys

import registry
import toolmeta
import tools  # noqa: F401

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "scripts", "toolface_budget.py")


def _run_budget(extra_env=None):
    env = {k: v for k, v in os.environ.items() if k != "UNIFIED_RX_SANDBOX"}
    env.update(extra_env or {})
    return subprocess.run([sys.executable, "-X", "utf8", SCRIPT],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", env=env, cwd=ROOT, shell=False)


def test_annotations_title_full_coverage_and_bidirectional():
    live = set(registry._TOOLS)
    assert set(toolmeta.TOOL_TITLES) == live, (
        f"toolmeta 与注册表不一致: 缺 {live - set(toolmeta.TOOL_TITLES)}, "
        f"陈旧 {set(toolmeta.TOOL_TITLES) - live}")
    titles = []
    for t in registry.list_tools():
        ann = t.get("annotations") or {}
        title = ann.get("title")
        assert isinstance(title, str) and title.strip(), f"{t['name']} 缺 title"
        titles.append(title)
    assert len(set(titles)) == len(titles), "标题必须唯一（宿主靠它区分工具）"


def test_annotations_mirror_auth_tiers():
    for t in registry.list_tools():
        ann = t.get("annotations") or {}
        if registry._TOOLS[t["name"]].get("requires_auth"):
            assert ann.get("readOnlyHint") is False, t["name"]
            assert ann.get("destructiveHint") is True, t["name"]
        else:
            assert ann.get("readOnlyHint") is True, t["name"]
            assert ann.get("idempotentHint") is True, t["name"]


def test_toolface_budget_gate_runs_green():
    cp = _run_budget()
    assert cp.returncode == 0, f"体量门红: {cp.stdout}\n{cp.stderr}"
    assert "TOOLFACE-GATE OK" in cp.stdout, cp.stdout
    assert "total_chars=" in cp.stdout and "est_tokens=" in cp.stdout, cp.stdout
    assert "TOOLFACE-DISCLOSURE core/full=" in cp.stdout, f"没报首屏占比: {cp.stdout}"


def test_toolface_budget_gate_is_a_real_gate():
    cp = _run_budget({"UNIFIED_RX_TOOLFACE_CAP": "1"})
    assert cp.returncode != 0, "压到 1 字符还绿——这是假门"
    assert "TOOLFACE-GATE FAIL" in (cp.stdout + cp.stderr)


def test_core_cap_is_a_real_gate():
    cp = _run_budget({"UNIFIED_RX_TOOLFACE_CORE_CAP": "1"})
    assert cp.returncode != 0, "core 帽压到 1 还绿——这是假门"
    assert "TOOLFACE-CORE FAIL" in (cp.stdout + cp.stderr)


def test_disclosure_ratio_is_a_real_gate():
    """S209：渐进披露比例必须是一道会判红的门，不是注释里的愿望。"""
    cp = _run_budget({"UNIFIED_RX_TOOLFACE_RATIO_CAP": "0.01"})
    assert cp.returncode != 0, "比例帽压到 1% 还绿——渐进披露门是装饰"
    assert "TOOLFACE-DISCLOSURE FAIL" in (cp.stdout + cp.stderr), cp.stdout


def test_disclosure_ratio_arithmetic_matches_printed_measures():
    """占比不许是手写的漂亮数字：拿门自己打印的 core/full 反算，必须与它报的百分比一致。

    同时钉住"这条判据真有牙齿"的边界：现状 63.8% 若被放到 ≥65.2%（两边同时逼近绝对帽
    的结果），门就再也拦不住退化——所以帽必须低于那个数。
    """
    out = _run_budget().stdout
    full = int(out.split("total_chars=")[1].split()[0])
    core = int(out.split("TOOLFACE-CORE")[1].split("total_chars=")[1].split()[0])
    disc = out.split("TOOLFACE-DISCLOSURE")[1].splitlines()[0]
    printed = float(disc.split("core/full=")[1].split("%")[0]) / 100.0
    assert abs(printed - core / full) < 0.005, f"打印占比 {printed:.3%} ≠ core/full={core/full:.3%}"
    cap = float(disc.split("cap=")[1].split("%")[0]) / 100.0
    assert cap <= 0.65, f"帽放到 {cap:.0%} 就挡不住「两边同时逼近绝对帽」的 65.2% 退化"
