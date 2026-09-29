"""arch_gate.py —— 架构守卫（S188，spec/FRONTIER-CI.md §5.3 第 10 项，方向十二第一片）。

把"某层不许依赖某层"从文档变成**数据声明式规则 + 机器判定**：`spec/arch-rules.json` 里每条规则
声明 `from`（谁）→ `import`（不许依赖什么），违规**定位到 import 行**。判定是集合运算——
依赖图不在这里造（`dep_graph` / `ide_callgraph` 已有），这里只做"规则 × 边"。

**为什么自带提取器而不复用 `tools.metrics._imports_of`**：① 它不返回行号（守卫要定位到行）；
② 它在 `tools.metrics` 里，模块级拖着 `registry`/`tools.fs`——**门不依赖工具面**本身就是
本守卫要钉的纪律（R1/R3 一族），门自己不能先破。此处提取器是"带行号的另一能力"，不是复制。

fail-closed：规则文件缺失 / 坏 JSON / 空规则 / 规则缺字段 ⇒ **判红**（守卫被清空即失效，
"占位即红"与快照棘轮同纪律）。`warn: true` 的规则只报告不计红（渐进启用）。

用法：
  python -X utf8 scripts/arch_gate.py                     # 扫本仓（默认规则）
  python -X utf8 scripts/arch_gate.py --root <dir> --rules <json>   # 夹具/判据用
"""
from __future__ import annotations

import ast
import fnmatch
import json
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", "node_modules", "target", "__pycache__", "dist", "build",
             ".venv", "venv", "manual_snaps", "results"}
MAX_FILES = 4000


def _module_name(rel: str) -> str:
    """相对路径 → 模块名（tools/metrics.py → tools.metrics；根下 a.py → a）。"""
    return rel[:-3].replace("\\", "/").replace("/", ".")


def _import_candidates(node: ast.ImportFrom | ast.Import, pkg: str) -> list[str]:
    """一个 import 语句触及的模块名（含点前缀，供规则按任意层级匹配；相对导入就地解析）。"""
    names = [a.name for a in node.names] if isinstance(node, ast.Import) \
        else ([node.module or ""] if node.module else [""])
    out: list[str] = []
    for name in names:
        if isinstance(node, ast.ImportFrom) and node.level:
            parts = [p for p in pkg.split(".") if p]
            up = max(0, len(parts) - (node.level - 1))
            name = ".".join(parts[:up] + ([name] if name else []))
        if not name:
            continue
        pieces = name.split(".")
        out.extend(".".join(pieces[:i]) for i in range(1, len(pieces) + 1))
    return sorted(set(out))


def _walk_scoped(tree: ast.AST):
    """yield (节点, 是否顶层)。顶层 = 最近 enclosing scope 是模块——**延迟导入**（函数内）
    是本仓的明文设计（hub_lesson"避免加载期耦合"/registry 分发器按需加载），
    规则据此可区分"加载期依赖"与"按需依赖"。"""
    def rec(node: ast.AST, top: bool):
        for child in ast.iter_child_nodes(node):
            if isinstance(child, (ast.Import, ast.ImportFrom)):
                yield child, top
            yield from rec(child, top and not isinstance(
                child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)))
    yield from rec(tree, True)


def scan(root: pathlib.Path) -> tuple[list[tuple[str, int, str, bool]], int]:
    """全仓 .py → [((模块, 行号, 触及模块, 是否顶层))]；坏文件**计数**进摘要（不静默）。"""
    edges: list[tuple[str, int, str, bool]] = []
    skipped = 0
    count = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in filenames:
            if not fn.endswith(".py"):
                continue
            fp = pathlib.Path(dirpath) / fn
            count += 1
            if count > MAX_FILES:
                break
            try:
                tree = ast.parse(fp.read_text(encoding="utf-8", errors="replace"))
            except (SyntaxError, OSError):
                skipped += 1
                continue
            mod = _module_name(str(fp.relative_to(root)))
            pkg = mod.rpartition(".")[0]
            for node, top in _walk_scoped(tree):
                edges.extend((mod, node.lineno, cand, top)
                             for cand in _import_candidates(node, pkg))
    return edges, skipped


def load_rules(path: pathlib.Path) -> list[dict]:
    """规则文件 → 规则表；**任何不健康都抛 ValueError**（fail-closed 由调用方判红）。"""
    if not path.is_file():
        raise ValueError(f"规则文件不存在: {path}")
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ValueError(f"规则文件不可读/坏 JSON: {exc}") from exc
    rules = doc.get("rules") if isinstance(doc, dict) else None
    if not isinstance(rules, list) or not rules:
        raise ValueError("rules 必须是非空数组——守卫被清空即失效（占位即红）")
    for r in rules:
        if not isinstance(r, dict) or not r.get("id") \
                or not isinstance(r.get("from"), list) or not r["from"] \
                or not isinstance(r.get("import"), list) or not r["import"]:
            raise ValueError(f"规则缺字段（id/from/import 必填）: {r!r}")
    return rules


def violations(edges: list[tuple[str, int, str, bool]],
               rules: list[dict]) -> tuple[list, list]:
    """规则 × 边 → (硬违规, warn 违规)；allow 按文件模块名精确豁免；scope 缺省 any。"""
    hard: list[tuple[str, int, str, dict]] = []
    warns: list[tuple[str, int, str, dict]] = []
    for mod, line, cand, top in edges:
        for r in rules:
            if r.get("scope") == "toplevel" and not top:
                continue
            if not any(fnmatch.fnmatch(mod, pat) for pat in r["from"]):
                continue
            if not any(fnmatch.fnmatch(cand, pat) for pat in r["import"]):
                continue
            if str(mod) in (r.get("allow") or []):
                continue
            (warns if r.get("warn") else hard).append((mod, line, cand, r))
    return hard, warns


def main(argv: list[str]) -> int:
    root = pathlib.Path(argv[argv.index("--root") + 1]).resolve() if "--root" in argv else ROOT
    rules_path = pathlib.Path(argv[argv.index("--rules") + 1]).resolve() \
        if "--rules" in argv else ROOT / "spec" / "arch-rules.json"
    try:
        rules = load_rules(rules_path)
    except ValueError as exc:
        print(f"ARCH-GATE FAIL 规则不健康：{exc}")
        return 1
    if not root.is_dir():
        print(f"ARCH-GATE FAIL 扫描根不存在: {root}")
        return 1
    edges, skipped = scan(root)
    hard, warns = violations(edges, rules)
    for mod, line, cand, r in hard:
        print(f"  ✗ {mod}:{line} import {cand}（{r['id']}：{r.get('why', '')}）")
    for mod, line, cand, r in warns:
        print(f"  ⚠ WARN {mod}:{line} import {cand}（{r['id']}：{r.get('why', '')}）")
    print(f"ARCH-GATE {'OK' if not hard else 'FAIL'} rules={len(rules)} "
          f"edges={len(edges)} violations={len(hard)} warns={len(warns)} "
          f"unparsed={skipped}")
    return 1 if hard else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
