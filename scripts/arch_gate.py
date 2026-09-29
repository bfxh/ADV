"""arch_gate.py —— 架构守卫（S188，spec/FRONTIER-CI.md §5.3 第 10 项，方向十二第一片）。

把"某层不许依赖某层"从文档变成**数据声明式规则 + 机器判定**：`spec/arch-rules.json` 里每条规则
声明 `from`（谁）→ `import`（不许依赖什么），违规**定位到 import 行**。判定是集合运算——
依赖图不在这里造（`dep_graph` / `ide_callgraph` 已有），这里只做"规则 × 边"。

**为什么自带提取器而不复用 `tools.metrics._imports_of`**：① 它不返回行号（守卫要定位到行）；
② 它在 `tools.metrics` 里，模块级拖着 `registry`/`tools.fs`——**门不依赖工具面**本身就是
本守卫要钉的纪律（R1/R3 一族），门自己不能先破。此处提取器是"带行号的另一能力"，不是复制。

**语言面（S192 起两门）**：
- Python：`ast` 真解析（`import`/`from ... import`，相对导入就地解析）；
- Rust：**`use` 边**（`mod` 声明只构成模块树，不是层间依赖，不取）；分组 `a::{b, c::d}`、
  嵌套分组、glob `a::*`、别名 `a as b` 都会展开；`crate::`/`self::`/`super::` 就地解析成
  **crate 限定名**（`crate::sim::step` → `<本文件所属 crate>.sim.step`），使"同 crate 内"与
  "跨 crate"两种规则写法统一；crate 名取自**最近的 `Cargo.toml`**（向上找，不越过后根），
  模块名 = crate 名下 `src/` 里的路径（`lib.rs`/`main.rs`/`mod.rs` 折叠掉）。
  **连字符与下划线都对**：crate 名常写作 `vxl-phys-core` 而 Rust 路径写作 `vxl_phys_core`
  ——候选里两种形态都给（规则写哪个都命中）。
  `scope: toplevel` 的 Rust 语义 = **不在任何 `fn` 体内**（函数内 `use` 是延迟导入，与 Python 同）。
  **已知边界**：不做 `#[cfg(...)]` 求值、不做宏展开（`macro_rules!` 里生成的 `use` 看不见）、
  字符串/字符字面量不做掩码（`use` 语句本身不含字面量，风险为零；注释已剥离）。

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
import re
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
    """全仓 .py/.rs → [((模块, 行号, 触及模块, 是否顶层))]；坏文件**计数**进摘要（不静默）。"""
    edges: list[tuple[str, int, str, bool]] = []
    skipped = 0
    count = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in filenames:
            is_py, is_rs = fn.endswith(".py"), fn.endswith(".rs")
            if not (is_py or is_rs):
                continue
            fp = pathlib.Path(dirpath) / fn
            count += 1
            if count > MAX_FILES:
                break
            if is_rs:
                try:
                    edges.extend(_rust_edges(fp, root))
                except OSError:
                    skipped += 1
                continue
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


# ---------- Rust 语言面（S192）----------

# `use` 语句起点：可带 `pub`/`pub(crate)`，可带前导空白（模块级与函数内都算）
_USE_HEAD = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?use\s")
_FN_HEAD = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+|async\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*fn\s")


def _strip_comments(text: str) -> str:
    """剥 `//` 行注释与 `/* */` 块注释（**不做字符串感知**：`use` 语句里不含字面量，
    数据流层面无风险；文档注释 `///` 同属行注释一并剥掉）。"""
    out, i, n = [], 0, len(text)
    while i < n:
        two = text[i:i + 2]
        if two == "//":
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif two == "/*":
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def _crate_of(fp: pathlib.Path, root: pathlib.Path) -> tuple[str, pathlib.Path]:
    """向上找最近的 `Cargo.toml`（不越过后根）→ (crate 名, crate 目录)；没有则 ("", root)。"""
    d = fp.parent
    while True:
        manifest = d / "Cargo.toml"
        if manifest.is_file():
            try:
                txt = manifest.read_text(encoding="utf-8", errors="replace")
            except OSError:
                return "", root
            m = re.search(r'^\s*name\s*=\s*"([^"]+)"', txt, re.MULTILINE)
            return (m.group(1) if m else d.name), d
        if d == root or d.parent == d:
            return "", root
        d = d.parent


def _rust_module(fp: pathlib.Path, root: pathlib.Path) -> tuple[str, str]:
    """→ (crate 名, 模块名)；模块名 = 在 `src/` 下的路径（`lib`/`main`/`mod` 折叠、`bin/` 去掉）。"""
    crate, cdir = _crate_of(fp, root)
    try:
        rel = fp.relative_to(cdir)
    except ValueError:
        rel = fp.relative_to(root)
    parts = list(rel.with_suffix("").parts)
    if parts and parts[0] == "src":
        parts = parts[1:]
    if parts and parts[-1] in ("lib", "main", "mod"):
        parts = parts[:-1]
    if parts[:1] == ["bin"]:
        parts = parts[1:]
    mod = ".".join(parts)
    if crate:
        return crate, f"{crate}.{mod}" if mod else crate
    return crate, mod


def _split_top_level(text: str) -> list[str]:
    """按**顶层**逗号切分（花括号内的逗号不切）。"""
    items, cur, depth = [], "", 0
    for ch in text:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            items.append(cur)
            cur = ""
            continue
        cur += ch
    items.append(cur)
    return items


def _walk_use(prefix: str, text: str, out: list[str]) -> None:
    for raw in _split_top_level(text):
        item = raw.strip()
        if not item:
            continue
        if "{" in item:
            head, _, rest = item.partition("{")
            head = head.strip().rstrip(":")
            _walk_use(f"{prefix}{head}::" if head else prefix, rest.rstrip("}"), out)
        elif item == "self":
            out.append(prefix.rstrip(":"))      # `use a::{self, b}`：self 指模块 a 本身
        else:
            out.append(prefix + item.split(" as ")[0].strip().removesuffix("::*"))


def _split_use_tree(body: str) -> list[str]:
    """`a::{b, c::{d, e}, f as g, h::*}` → ['a::b','a::c::d','a::c::e','a::f','a::h']。"""
    out: list[str] = []
    _walk_use("", body, out)
    return [p for p in (x.strip().rstrip(":") for x in out) if p]


def _resolve_rust_path(path: str, mod: str) -> str:
    """`crate::`/`self::`/`super::` 就地解析成 crate 限定名；其余原样（外部 crate）。"""
    parts = [p for p in path.split("::") if p and p != "*"]
    if not parts:
        return ""
    head = parts[0]
    if head == "crate":
        base, rest = ([mod.split(".")[0]] if mod else []), parts[1:]
    elif head in ("self", "super"):
        base, rest = (mod.split(".") if mod else []), list(parts)
        while rest and rest[0] in ("self", "super"):
            if rest[0] == "super" and base:
                base.pop()
            rest = rest[1:]
    elif head in ("std", "core", "alloc"):
        return ".".join(parts)          # 标准库：不参与层间规则，如实归一
    else:
        base, rest = [], parts          # 外部 crate（bevy/serde/…）或 fixture 无 crate 名
    return ".".join(base + rest) if rest else ".".join(base)


def _rust_candidates(resolved: str) -> list[str]:
    """点前缀候选 + **每个前缀的首段连字符形态**（crate 名常写 `vxl-phys-core`，
    Rust 路径写 `vxl_phys_core`——规则写哪种形态都要命中）。"""
    if not resolved:
        return []
    parts = resolved.split(".")
    out = {".".join(parts[:i]) for i in range(1, len(parts) + 1)}
    out |= {".".join([parts[0].replace("_", "-"), *parts[1:i]])
            for i in range(1, len(parts) + 1)}
    return sorted(out)


def _rust_edges(fp: pathlib.Path, root: pathlib.Path) -> list[tuple[str, int, str, bool]]:
    """单文件 `use` 边；`mod` 声明不取（模块树不是依赖）。顶层 = 不在任何 `fn` 体内。"""
    _crate, mod = _rust_module(fp, root)

    lines = _strip_comments(fp.read_text(encoding="utf-8", errors="replace")).split("\n")
    out: list[tuple[str, int, str, bool]] = []
    stack: list[int] = []          # 进入 fn 体时的花括号深度
    depth = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if _USE_HEAD.match(line):
            stmt, j = line, i
            while ";" not in stmt and j + 1 < len(lines):
                j += 1
                stmt += " " + lines[j]
            body = stmt[stmt.index("use") + 3:stmt.find(";", stmt.index("use"))]
            top = not stack
            for path in _split_use_tree(body):
                out.extend((mod, i + 1, cand, top)
                           for cand in _rust_candidates(_resolve_rust_path(path, mod)))
            i = j + 1
            continue
        if _FN_HEAD.match(line) and "{" in line:
            stack.append(depth)
        depth += line.count("{") - line.count("}")
        while stack and stack[-1] >= depth:
            stack.pop()
        i += 1
    return out


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
