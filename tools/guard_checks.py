"""tools/guard_checks.py —— 防幻觉守卫的判定件（S195：自 tools/guard.py 拆出 + 精度修复）。

拆分理由：`tools/guard.py` 是工具面（注册/描述/路由），判定逻辑独立成件才好单测，也不撑爆
上帝对象门（同 `tools/svc.py` 先例：纯辅助件，不在 `tools/__init__.py` 里注册）。

本片修的是**外部审计（2026-09-30）实测复现的四条**（我在本机逐条复现，见 ROUNDLOG S195）：

- **P1「见了反引号就判」**：任何小写反引号词不在册即 `refuted` ⇒ `` `git` ``、`` `parser` `` 被判
  "工具不存在"，整段文本被扣上"存在幻觉，必须纠正后才能引用"。改为**疑似才判**（在册 /
  与在册名编辑距离 ≤2 / 命名形态=下划线分段），其余**不判**并计入 `skipped.not_a_tool_claim`
  ——**宁可漏判，不冤枉**：误杀会让守卫被整体忽略（"狼来了"）。
- **P2 扩展名盲区**：只认 12 种扩展名 ⇒ `README.md:12`、`config.toml:7` 这类声明**直接消失**
  （连 `unverifiable` 都不是），而"H2 漏判=0"正建立在这个盲区上。改为：代码/文档/配置扩展名
  一并**真查存在性**；表外扩展名落 `unverifiable` 并计入 `skipped.unchecked_ext`
  （本仓"能力缺席不静默"红线）。
- **P3 行号下界**：`int(lineno) <= n` ⇒ `server.py:0` 判 verified（行 0 不存在）
  ⇒ 改 `1 <= lineno <= n`。
- **P4 符号分支恒 unverifiable**（死分支，而编造 API/类名恰是幻觉最高发类）⇒ 接一次**有界全仓
  词边界检索**：命中 ⇒ verified；扫完未命中 ⇒ refuted；达上限 ⇒ unverifiable（扫描范围写进 detail）。

口径（写进工具描述与 skills/guard.md）：**判定只覆盖"可复核的声明形态"**；判不了的如实落
`unverifiable` 或计入 `skipped`，**永不静默丢声明**。
"""
import os
import re

from tools.fs import _resolve as _fs_resolve

# 真查存在性的扩展名（代码 + 文档 + 配置）。表外扩展名不判死，落 unverifiable 并计数。
CHECKED_EXT = frozenset([
    "py", "rs", "go", "ts", "tsx", "js", "jsx", "mjs", "cjs", "gd", "cs", "dart", "java",
    "kt", "rb", "php", "swift", "vue", "c", "cc", "cpp", "cxx", "h", "hpp", "hxx",
    "sh", "bash", "ps1", "sql", "html", "css", "md", "markdown", "txt",
    "toml", "json", "yaml", "yml", "ron", "lock", "ini", "cfg", "conf", "csv", "xml",
])

# 路径声明：`<路径>.<扩展名>[:行号]`；扩展名首字符必须字母 ⇒ 版本号（v2.90.0 / 1.2.3）不会误匹配
PATH_RE = re.compile(r"([A-Za-z0-9_./\\-]+\.[A-Za-z][A-Za-z0-9]{1,7})(?::(\d+))?")
TOOL_RE = re.compile(r"`([a-z][a-z0-9_]{2,})`")
SYM_RE = re.compile(r"`([A-Z][A-Za-z0-9_]{2,})`")

_SKIP_DIRS = frozenset({".git", "target", "node_modules", "__pycache__", ".venv", "venv",
                        "dist", "build", ".mypy_cache", ".ruff_cache", ".pytest_cache"})
_MAX_FILES = 2000          # 符号检索：最多看这么多文件
_MAX_BYTES = 2_000_000     # 单文件上限（跳过更大的）
_MAX_HITS = 3              # 够了就停（只要"存在"）

SANDBOX_OUTSIDE = "沙盒外路径，按纪律不读取不判定"


def edit_distance_le2(a: str, b: str) -> bool:
    """编辑距离 ≤2（长度差先剪枝）——工具名近似判据。"""
    if abs(len(a) - len(b)) > 2:
        return False
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1] <= 2


def tool_claim_status(name: str, tool_names: set[str]) -> str | None:
    """反引号小写词 → 'verified' / 'refuted' / None（None = **不是工具主张，不判**）。

    判据（"疑似才判"）：① 在册 ⇒ verified；② 命名形态（下划线分段，≥2 段且各段字母数字）
    ⇒ 疑似工具名 ⇒ refuted；③ 与某个在册名编辑距离 ≤2 ⇒ 疑似笔误 ⇒ refuted。
    三者都不满足 ⇒ 不是工具主张（命令 `git`、变量 `parser`、配置键 `log` 都在此列）。
    """
    if name in tool_names:
        return "verified"
    parts = name.split("_")
    if len(parts) >= 2 and all(p.isalnum() and p for p in parts):
        return "refuted"
    if any(edit_distance_le2(name, t) for t in tool_names):
        return "refuted"
    return None


def tool_decls(text: str, tool_names: set[str]) -> tuple[list[dict], int]:
    """工具名声明：返回 (results, 未判计数)。未判 = 不是工具主张（不冤判，但计数不静默）。"""
    out: list[dict] = []
    skipped = 0
    for m in TOOL_RE.finditer(text):
        name = m.group(1)
        status = tool_claim_status(name, tool_names)
        if status is None:
            skipped += 1
            continue
        detail = f"工具存在: {name}" if status == "verified" else f"工具不存在: {name}"
        out.append({"decl": m.group(0), "kind": "tool", "status": status, "detail": detail})
    return out, skipped


def file_decls(text: str, root: str) -> tuple[list[dict], int, int]:
    """文件声明：返回 (results, 未覆盖扩展名计数, 沙盒外计数)。"""
    out: list[dict] = []
    unchecked = outside = 0
    for m in PATH_RE.finditer(text):
        fpath, lineno = m.group(1), m.group(2)
        ext = fpath.rsplit(".", 1)[-1].lower()
        full = fpath if os.path.isabs(fpath) else os.path.join(root, fpath)
        try:
            full = _fs_resolve(full)
        except ValueError:
            outside += 1
            out.append({"decl": m.group(0), "kind": "file", "status": "unverifiable",
                        "detail": SANDBOX_OUTSIDE})
            continue
        if ext not in CHECKED_EXT:
            unchecked += 1
            out.append({"decl": m.group(0), "kind": "file", "status": "unverifiable",
                        "detail": f"扩展名 .{ext} 不在可查表（未检查，计入 skipped.unchecked_ext）"})
            continue
        if not os.path.isfile(full):
            out.append({"decl": m.group(0), "kind": "file", "status": "refuted",
                        "detail": f"文件不存在: {full}"})
            continue
        if not lineno:                       # S195 P3：行号是缺陷面 ⇒ 只做下界+上界同判
            out.append({"decl": m.group(0), "kind": "file", "status": "verified",
                        "detail": "文件存在"})
            continue
        with open(full, encoding="utf-8", errors="replace") as f:
            n = sum(1 for _ in f)
        ok = 1 <= int(lineno) <= n
        out.append({"decl": m.group(0), "kind": "file",
                    "status": "verified" if ok else "refuted",
                    "detail": (f"文件存在，行号在范围内（1..{n}）" if ok
                               else f"行号越界（文件 {n} 行；行号必须 ≥1）")})
    return out, unchecked, outside


def scan_symbol(sym: str, root: str) -> tuple[int, int, bool]:
    """有界全仓词边界检索 → (命中文件数, 扫描文件数, 是否达上限)。"""
    pat = re.compile(r"(?<![A-Za-z0-9_])" + re.escape(sym) + r"(?![A-Za-z0-9_])")
    hits = scanned = 0
    truncated = False
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in _SKIP_DIRS]
        for fn in sorted(filenames):
            if scanned >= _MAX_FILES:
                truncated = True
                break
            fp = os.path.join(dirpath, fn)
            try:
                if os.path.getsize(fp) > _MAX_BYTES:
                    continue
                with open(fp, encoding="utf-8", errors="ignore") as f:
                    body = f.read()
            except OSError:
                continue
            scanned += 1
            if pat.search(body):
                hits += 1
                if hits >= _MAX_HITS:
                    return hits, scanned, False
        if truncated:
            break
    return hits, scanned, truncated


def symbol_decls(text: str, root: str) -> list[dict]:
    """符号声明：真查（命中⇒verified / 扫完未命中⇒refuted / 达上限⇒unverifiable）。"""
    out: list[dict] = []
    try:
        scan_root = _fs_resolve(root)
    except ValueError:
        return [{"decl": m.group(0), "kind": "symbol", "status": "unverifiable",
                 "detail": SANDBOX_OUTSIDE} for m in SYM_RE.finditer(text)]
    cache: dict[str, tuple[int, int, bool]] = {}
    for m in SYM_RE.finditer(text):
        sym = m.group(1)
        if sym not in cache:
            cache[sym] = scan_symbol(sym, scan_root)
        hits, scanned, truncated = cache[sym]
        if hits:
            status, detail = "verified", f"全仓命中 {hits} 个文件（扫描 {scanned}）"
        elif truncated:
            status, detail = "unverifiable", f"达扫描上限 {_MAX_FILES} 文件，未判"
        else:
            status, detail = "refuted", f"全仓 {scanned} 文件未命中——疑似编造符号"
        out.append({"decl": m.group(0), "kind": "symbol", "status": status, "detail": detail})
    return out
