"""hub_impact.py —— 静态语义影响面（S181，spec/FRONTIER-CI.md 方向①）。

**与既有能力的分工（不重复造）**：
- `tools/tia.py` 是**动态**口径：要有"上次跑过测试"的文件访问集，且状态在进程内（跨重启丢）；
- `ide_impact` 是**符号级**引用（走 LSP/resolved/text 三级），面向"某个符号谁在用"；
- **本模块**补的是**变更驱动的静态口径**：只看 `git diff` + 仓内 import 图，回答
  "改了哪些文件 → 波及哪些模块 → 该跑哪些测试"，**不依赖历史运行**、可跨进程复算。

口径（保守，宁多跑不误跳）：
- 影响闭包 = 变更文件 ∪ **反向可达**（谁 import 了它，传递；有界节点数防爆）；
- 测试集 = `tests/` 下、其（传递）import 闭包与影响闭包相交者；**图上无记录的测试一律纳入**；
- 任何不确定（不是 git 仓 / 解析失败率过高 / 超界）→ `fallback="full"` + `reason`，**如实退化**。
"""
from __future__ import annotations

import ast
import pathlib

import hub_propose

_SKIP_DIRS = {".git", "__pycache__", "node_modules", "target", ".pytest_cache", "results"}
_MAX_FILES = 3000
_MAX_NODES = 3000
_PARSE_FAIL_TOLERANCE = 0.2          # 解析失败率超此值 ⇒ 退化全量（不可信就别说选过）


def repo_root() -> pathlib.Path:
    return hub_propose.repo_root()


def _py_files(root: pathlib.Path) -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    for p in root.rglob("*.py"):
        if any(part in _SKIP_DIRS for part in p.parts):
            continue
        out.append(p)
        if len(out) >= _MAX_FILES:
            break
    return out


def changed_files(root: pathlib.Path, base: str = "HEAD") -> list[str]:
    """变更集 = `git diff base..HEAD` ∪ 工作树未提交（相对路径，仅 .py，去重保序）。"""
    out: list[str] = []
    for args in (["diff", "--name-only", f"{base}..HEAD"], ["status", "--porcelain"]):
        rc, text, _err = hub_propose._git(root, args)
        if rc != 0:
            continue
        for line in text.splitlines():
            rel = line[3:] if args[0] == "status" else line
            rel = rel.strip().strip('"')
            if rel.endswith(".py") and rel not in out:
                out.append(rel)
    return out


def _resolve(name: str, level: int, cur: pathlib.Path, root: pathlib.Path) -> list[pathlib.Path]:
    """import 名 → 仓内文件（解析不出返回空；"未知"由保守策略兜底，不猜）。"""
    base = cur.parent
    for _ in range(max(0, level - 1)):
        base = base.parent
    rel = name.replace(".", "/")
    cands: list[pathlib.Path] = []
    if level:
        cands += [base / f"{rel}.py", base / rel / "__init__.py"] if rel else [base / "__init__.py"]
    else:
        cands += [root / f"{rel}.py", root / rel / "__init__.py"] if rel else []
    return [p for p in cands if p.is_file()]


def build_graph(root: pathlib.Path) -> tuple[dict[str, set[str]], dict[str, set[str]], int, int]:
    """→ (正向 import 边, 反向被 import 边, 文件数, 解析失败数)；键均为相对 posix 路径。"""
    forward: dict[str, set[str]] = {}
    reverse: dict[str, set[str]] = {}
    files = _py_files(root)
    failed = 0
    for p in files:
        rel = p.relative_to(root).as_posix()
        forward.setdefault(rel, set())
        try:
            tree = ast.parse(p.read_text(encoding="utf-8", errors="replace"))
        except (OSError, SyntaxError, ValueError):
            failed += 1
            continue
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                pairs = [(al.name, 0) for al in node.names]
            elif isinstance(node, ast.ImportFrom):
                pairs = [(node.module or "", node.level or 0)]
            else:
                continue
            for name, level in pairs:
                for tgt in _resolve(name, level, p, root):
                    tgt_rel = tgt.relative_to(root).as_posix()
                    forward[rel].add(tgt_rel)
                    reverse.setdefault(tgt_rel, set()).add(rel)
    return forward, reverse, len(files), failed


def _closure(seeds: list[str], reverse: dict[str, set[str]]) -> set[str]:
    """反向可达闭包（谁依赖我，传递）；有界。"""
    seen: set[str] = set()
    stack = list(seeds)
    while stack and len(seen) < _MAX_NODES:
        cur = stack.pop()
        for importer in reverse.get(cur, ()):
            if importer not in seen:
                seen.add(importer)
                stack.append(importer)
    return seen


def _selected_tests(forward: dict[str, set[str]], affected: set[str],
                    prefix: str) -> tuple[list[str], list[str], list[str]]:
    """影响闭包 → 建议测试集（保守口径的唯一实现：图上无记录的测试一律纳入）。"""
    tests = [f for f in forward if f.startswith(prefix)]
    hit = sorted(t for t in tests if t in affected or (forward.get(t, set()) & affected))
    unknown = sorted(t for t in tests if t not in forward)        # 图上无记录 ⇒ 保守纳入
    return sorted(set(hit) | set(unknown)), unknown, tests


def impact(base: str = "HEAD", prefix: str = "tests/") -> dict:
    """静态影响面：变更 → 波及文件 → 建议测试集（保守）+ 退化标记。"""
    root = repo_root()
    changed = changed_files(root, base)
    forward, reverse, n_files, n_failed = build_graph(root)
    if n_files == 0:
        return {"ok": False, "error": f"仓内无可解析的 .py（root={root}）"}
    if n_failed and n_failed / n_files > _PARSE_FAIL_TOLERANCE:
        return {"ok": True, "fallback": "full", "changed": changed,
                "reason": f"解析失败率 {n_failed}/{n_files} 过高——不可信就不选（退全量）",
                "impacted_tests": [], "impacted_files": []}
    if not changed:
        return {"ok": True, "fallback": "none", "changed": [],
                "reason": f"相对 {base} 无 .py 变更（无需选测）",
                "impacted_files": [], "impacted_tests": []}
    affected = set(changed) | _closure(changed, reverse)
    selected, unknown, tests = _selected_tests(forward, affected, prefix)
    return {
        "ok": True, "fallback": None, "base": base, "root": str(root),
        "changed": changed,
        "impacted_files": sorted(affected),
        "impacted_tests": selected,
        "conservative_included_unknown": unknown,
        "skipped_tests": sorted(set(tests) - set(selected)),
        "counts": {"files_scanned": n_files, "parse_failed": n_failed,
                   "changed": len(changed), "impacted": len(affected),
                   "tests_total": len(tests), "tests_selected": len(selected)},
        "how_to_verify": [
            "全量对拍: python -m pytest tests/ -q   # 结论应与只跑 impacted_tests 一致（除 flaky）",
            "反向复核: 任一 impacted_tests 去掉后重跑，若出现新红即说明漏选（本模块的假实现会被抓）",
        ],
    }
