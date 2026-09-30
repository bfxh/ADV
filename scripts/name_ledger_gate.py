"""name_ledger_gate.py —— 名字账门（S197）：镜像面上的工具名 ↔ 在册闭集。

**为什么是设计缺口而不是又一次笔误**：claim_gate 把文档里的**数字**锚上了真值源，
selftest 把 **skills/*.md** 的工具名锚上了注册表；但代码里手写的路由意图表
`_INTENTS`、能力清单 `_CAPABILITIES`、README 工具面章节同样是「名字镜像面」，
无人绑账。实测（2026-09-30 探针）：`capability_manifest` 的 curated 路由第一候选
`risk_rank` 不在册（真名 `ide_risk_rank`）——防幻觉工具箱的路由功能自己在推荐假名。
本门就是 claim_gate 的名字版：**不在册且无豁免 ⇒ 红**。

镜像面（三张，全走 ast/正则静态提取，不 import 被检文件）：
  1. `tools/guard.py` `_INTENTS` 各意图簇的候选工具名（min 提取数防"解析悄悄数 0"）；
  2. `tools/guard.py` `_CAPABILITIES` 里形似工具名的下划线词元（允许为空）；
  3. `README.md` "## 工具面" 章节内的反引号小写词元（排除下一章节）。

fail-closed：被检文件缺失 / 面提取为空（INTENTS、README 有下限）/ 豁免表缺失或坏
JSON / 豁免理由占位未填 ⇒ 红。豁免表 `spec/name-ledger.json`——入册必须写 why
（同 taint-baseline 纪律：`hard` 这类合法非工具名才走得通，假名永远走不通）。

用法：
  python -X utf8 scripts/name_ledger_gate.py
  python -X utf8 scripts/name_ledger_gate.py --root <dir> --live-names <json>  # 夹具/判据用
"""
from __future__ import annotations

import argparse
import ast
import json
import pathlib
import re
import subprocess
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEDGER_REL = "spec/name-ledger.json"
INTENTS_MIN = 10
README_MIN = 40
_TOKEN_RE = re.compile(r"\b[a-z][a-z0-9]*_[a-z0-9_]+\b")


_LIVE_SNIPPET = ("import json,sys; sys.path.insert(0, sys.argv[1]); "
                 "import registry, tools; "
                 "print(json.dumps([t['name'] for t in registry.list_tools()]))")


def _live_names(root: pathlib.Path, override: str | None) -> set[str]:
    """真值源=在册闭集，走真路径子进程（与 server 同一注册表模块，不依赖门内 import）。"""
    if override:
        return {str(n) for n in json.loads(pathlib.Path(override).read_text(encoding="utf-8"))}
    p = subprocess.run([sys.executable, "-X", "utf8", "-c", _LIVE_SNIPPET, str(root)],
                       cwd=str(root), shell=False, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", timeout=120)
    if p.returncode != 0:
        raise RuntimeError(f"真值子进程退出码 {p.returncode}: {(p.stderr or p.stdout)[-200:]}")
    return {str(n) for n in json.loads(p.stdout)}


def _guard_surfaces(root: pathlib.Path, problems: list[str]) -> tuple[set[str], set[str]]:
    p = root / "tools" / "guard.py"
    if not p.is_file():
        problems.append(f"被检文件缺失：{p}")
        return set(), set()
    tree = ast.parse(p.read_text(encoding="utf-8"))
    assigns = {n.targets[0].id: n for n in tree.body
               if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name)}
    intent_names: set[str] = set()
    if "_INTENTS" not in assigns:
        problems.append("guard.py 提取不到 _INTENTS（改名/结构漂移）")
    else:
        for pair in ast.literal_eval(assigns["_INTENTS"].value):
            intent_names |= set(pair[1])
        if len(intent_names) < INTENTS_MIN:
            problems.append(f"_INTENTS 候选名提取数 {len(intent_names)} < 下限 {INTENTS_MIN}"
                            "（疑似解析失效，不许悄悄数 0）")
    caps: set[str] = set()
    if "_CAPABILITIES" in assigns:
        caps = set(_TOKEN_RE.findall(str(ast.literal_eval(assigns["_CAPABILITIES"].value))))
    return intent_names, caps


def _readme_surface(root: pathlib.Path, problems: list[str]) -> set[str]:
    p = root / "README.md"
    if not p.is_file():
        problems.append(f"被检文件缺失：{p}")
        return set()
    text = p.read_text(encoding="utf-8")
    if "## 工具面" not in text:
        problems.append("README 提取不到「## 工具面」章节（结构漂移）")
        return set()
    sec = text.split("## 工具面", 1)[1].split("\n## ", 1)[0]
    names = set(re.findall(r"`([a-z][a-z0-9_]{2,})`", sec))
    if len(names) < README_MIN:
        problems.append(f"README 工具面词元提取数 {len(names)} < 下限 {README_MIN}（疑似解析失效）")
    return names


def _allow(root: pathlib.Path, problems: list[str]) -> dict[str, str]:
    p = root / LEDGER_REL
    if not p.is_file():
        problems.append(f"豁免表缺失：{LEDGER_REL}（登记表缺失=门失效，fail-closed）")
        return {}
    try:
        data: dict[str, Any] = json.loads(p.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        problems.append(f"豁免表坏 JSON：{e}")
        return {}
    allow: dict[str, str] = {}
    for ent in data.get("allow", []):
        name, why = str(ent.get("name", "")), str(ent.get("why", ""))
        if not name or not why or why.startswith("TODO") or "占位" in why:
            problems.append(f"豁免条目非法（缺名或理由占位未填）：{ent}")
            continue
        allow[name] = why
    return allow


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    ap.add_argument("--live-names", default=None)
    a = ap.parse_args(argv)
    root = pathlib.Path(a.root).resolve()
    problems: list[str] = []
    try:
        live = _live_names(root, a.live_names)
    except (RuntimeError, OSError, ValueError,
            subprocess.SubprocessError) as e:  # 真值源起不来=红，不静默（fail-closed）
        print(f"  ✗ 真值源不可用：{e}")
        print("NAME-LEDGER-GATE FAIL 原因=1")
        return 1
    if not live:
        problems.append("在册闭集为空（注册表没起来——全判不在册不是本门的判法）")
    intents, caps = _guard_surfaces(root, problems)
    readme = _readme_surface(root, problems)
    allow = _allow(root, problems)
    surfaces = {"INTENTS": intents, "CAPS": caps, "README": readme}
    for label, names in surfaces.items():
        problems.extend(f"[{label}] {n} 不在册且无豁免"
                        for n in sorted(names - live - set(allow)))
    if problems:
        for p in problems:
            print(f"  ✗ {p}")
        print(f"NAME-LEDGER-GATE FAIL 原因={len(problems)}")
        return 1
    total = len(intents) + len(caps) + len(readme)
    print(f"NAME-LEDGER-GATE OK surfaces=3 names={total} 在册={len(live)} 豁免={len(allow)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
