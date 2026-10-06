"""CI 接线完整性门（DD-0010 的处置）：钉住"**哪条分支真跑哪几道门**"。

为什么要有它（2026-10-06 一天内兑现三次的失败形状）：门被接进了不守这条分支的 workflow
（Debt gate 写进 `core.yml`，而 `core.yml` 的 `on.push` 只列 `main`）、门被接进 CI 却没配
runner 需要的前置（浅克隆让 SHA 全查不到 ⇒ 14 条假红盖住真信号）、钩子文档写"已装"而实际
没装。三类都同一个根因：**"门存在"与"门在这条分支上真会跑"之间没有任何机器核对**。

判据（命中即红，全部纯函数可单测）：
  W1 在册分支的 workflow 文件必须存在且非空；
  W2 该分支必须真被那个 workflow 触发（`on.push.branches` 里点名到它）；
  W3 清单里每道门都必须能在该 workflow 的步骤（name/run/uses）里找到——找不到就是"以为接了"；
  W4 反向：workflow 步骤里出现的仓内 `scripts/*.py` 必须被**某条**分支的清单认领（防漂移与无主步骤）；
  W5 清单里的门名必须都在 local_gate 的 STEPS 或登记的 indirect 里（防改名漂移）。

不依赖 PyYAML：CI 的 `.github/ci-requirements.txt` 里没有它（新增依赖要走红线），而且 YAML
会把 `on:` 解析成布尔 `True` —— 这里只按行取需要的那几个字段，形状写死在测试里。
"""

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "spec" / "ci-wiring.json"
_STEP_NAME = re.compile(r"^\s*-\s+(?:name|uses):\s*(.+?)\s*$")
_RUN = re.compile(r"^\s*run:\s*(.+?)\s*$")
_LIST_ITEM = re.compile(r"^\s*-\s+([A-Za-z0-9_./-]+)\s*(?:#.*)?$")
_FLOW_BRANCHES = re.compile(r"^\s*branches:\s*\[([^\]]*)\]")
_INLINE_BRANCHES = re.compile(r"^\s*branches:\s*([A-Za-z0-9_./-]+)\s*(?:#.*)?$")
_SCRIPT_REF = re.compile(r"scripts/[A-Za-z0-9_]+\.py")


def _flow_items(line: str):
    """取 `branches: [a, b]` / `branches: dev` 这两种紧凑写法的分支名。"""
    s = line.split("#", 1)[0].rstrip()
    m = _FLOW_BRANCHES.match(s) or _INLINE_BRANCHES.match(s)
    if not m:
        return []
    return [p.strip() for p in m.group(1).split(",") if p.strip()]


def parse_branches(text: str) -> set:
    """取 `on: push: branches:` 的分支名，支持流式 `[a, b]`、块式 `- item`、行内单值。"""
    out: set = set()
    in_push = False
    for ln in text.splitlines():
        s = ln.split("#", 1)[0].rstrip()
        if not s.strip():
            continue
        if re.match(r"^\s{2}push:\s*$", s):
            in_push = True
            continue
        if not in_push:
            continue
        items = _flow_items(s)
        if items:
            out |= set(items)
            in_push = False
            continue
        m = _LIST_ITEM.match(s)
        if m:
            out.add(m.group(1))
        elif not re.match(r"^\s{4,}", s):
            in_push = False
    if out:
        return out
    for ln in text.splitlines():  # 紧凑写法没缩进进 push: 块时的兜底
        out |= set(_flow_items(ln))
    return out


def parse_step_blobs(text: str) -> list:
    """每个步骤的可搜文本：name + uses + run（多行 run 合并）。"""
    blobs: list = []
    current = None
    for ln in text.splitlines():
        if re.match(r"^\s*-\s+(?:name|uses):", ln):
            if current is not None:
                blobs.append(current)
            m = _STEP_NAME.match(ln)
            current = m.group(1) if m else ""
            continue
        m = _RUN.match(ln)
        if m and current is not None:
            current += " " + m.group(1)
    if current is not None:
        blobs.append(current)
    return blobs


def check_branch(branch, cfg, workflow_text, indirect, step_names):
    """单条分支的 W1–W3、W5。返回违规文案。"""
    problems = []
    wf = cfg.get("workflow")
    if not wf:
        return [f"W1 分支 {branch} 没写 workflow 落点"]
    if workflow_text is None:
        return [f"W1 分支 {branch} 的 workflow 文件不存在或为空：{wf}"]
    if not cfg.get("why", "").strip():
        problems.append(f"W5 分支 {branch} 没写 why（这条分支凭什么看这几道门）")
    triggered = parse_branches(workflow_text)
    if branch not in triggered:
        problems.append(
            f"W2 分支 {branch} 不在 {wf} 的 on.push.branches 里 ⇒ 这张清单上的门一次都不会跑"
            f"（实际触发：{sorted(triggered) or '读不出来'}）"
        )
    blobs = " || ".join(parse_step_blobs(workflow_text))
    for gate in cfg.get("gates", []):
        needle = indirect.get(gate, f"scripts/{gate.replace('-', '_')}.py")
        if needle not in blobs:
            problems.append(f"W3 {branch}：门 {gate} 在 {wf} 的步骤里找不到（找的是 {needle!r}）")
        if gate not in step_names and gate not in indirect:
            problems.append(f"W5 {branch}：清单里的 {gate} 既不在 local_gate 的 STEPS 也不是登记过的 indirect")
    return problems


def check_orphans(workflows, owned, indirect, root=ROOT):
    """W4：workflow 步骤里调用的 `scripts/*.py` 必须被某条分支认领。"""
    problems = []
    needles = set()
    for gate in owned:
        needles.add(indirect.get(gate, f"scripts/{gate.replace('-', '_')}.py"))
    for wf, text in workflows.items():
        if text is None:
            continue
        blobs = " || ".join(parse_step_blobs(text))
        unowned = [
            ref
            for ref in sorted(set(_SCRIPT_REF.findall(blobs)))
            if f"scripts/{ref}" not in needles and not any(n.endswith(ref) for n in needles)
        ]
        problems += [
            f"W4 {wf}：步骤调用了 {ref}，但没有任何分支清单认领它" for ref in unowned
        ]
    return problems


def _read(path: pathlib.Path):
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    return text if text.strip() else None


def _step_names(root: pathlib.Path) -> set:
    """STEPS 的名字集，从 local_gate.py 源码取（与 test_s145 的口径同形：ast 解析，不子串蒙混）。"""
    import ast

    src = _read(root / "scripts" / "local_gate.py")
    if not src:
        return set()
    names = set()
    for node in ast.walk(ast.parse(src)):
        if isinstance(node, ast.Assign) and any(
            getattr(t, "id", "") == "STEPS" for t in node.targets
        ):
            for el in getattr(node.value, "elts", []):
                if isinstance(el, ast.Tuple) and el.elts and isinstance(el.elts[0], ast.Constant):
                    names.add(el.elts[0].value)
    return names


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    a = ap.parse_args()
    root = pathlib.Path(a.root).resolve()
    reg_path = root / "spec" / "ci-wiring.json"
    if not reg_path.is_file():
        print(f"CI-WIRING FAIL: 缺 {reg_path.relative_to(root)}（缺账不等于没接线）")
        return 1
    reg = json.loads(reg_path.read_text(encoding="utf-8"))
    branches = reg.get("branches") or {}
    indirect = {**reg.get("indirect", {})}
    names = _step_names(root)

    workflows, texts = {}, {}
    for cfg in branches.values():
        wf = cfg.get("workflow")
        if wf and wf not in texts:
            texts[wf] = _read(root / wf)
            workflows[wf] = texts[wf]

    problems = []
    for branch, cfg in branches.items():
        problems += check_branch(branch, cfg, texts.get(cfg.get("workflow")), indirect, names)
    owned = {g for cfg in branches.values() for g in cfg.get("gates", [])}
    # W4 只对登记为"逐门认领"的 workflow 生效：core.yml 是旧伞仓的 20+ 步清单，
    # 一次要求全认领会把本片淹在抄表里；adv.yml 是新工作区的门面，必须一步不漏。
    checked = {
        wf: text for wf, text in workflows.items() if wf in set(reg.get("orphan_checked", []))
    }
    problems += check_orphans(checked, owned, indirect, root)
    if not branches:
        problems.append("W1 branches 清单为空（没有一条分支被声明要看哪些门）")

    for wf, text in sorted(workflows.items()):
        state = "存在" if text else "缺失/为空"
        print(f"  workflow {wf}：{state}，触发分支 {sorted(parse_branches(text)) if text else '—'}")
    for p in problems[:20]:
        print(f"  ❌ {p}")
    total = sum(len(cfg.get("gates", [])) for cfg in branches.values())
    if problems:
        print(f"CI-WIRING FAIL: {len(problems)} 处（口径见本脚本顶部注释）")
        return 1
    print(
        f"CI-WIRING OK（{len(branches)} 条分支 × 共 {total} 次门声明；"
        f"每个 scripts 门步都有归属）"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
