"""tools/guard.py —— 防幻觉域（2 工具）：hallucination_guard / capability_manifest

AI 声明事实核查（verified/refuted/unverifiable 三分级）——防 AI 编造
file:line / 符号 / 工具名。这是"工具代替智能体"里最关键的护栏。
"""
import difflib
import os
import re
import time

from registry import list_tools, tool
from tools.fs import _resolve as _fs_resolve

_CAPABILITIES = {
    "有": [
        "本地文件读写（沙盒内）", "静态 bug 扫描（多语言）", "工程标准检查",
        "代码定位/上下文/编辑", "语义检索", "防幻觉核查", "教训记忆",
        "真 LSP 语义查询（rust-analyzer/pylsp：定义/引用/hover/符号/诊断/重命名预案）",
        "工具使用统计（调用频率/耗时/时段）", "扫描日志与趋势", "项目健康度评分",
        "教训库统计", "每日备份", "游戏域检查",
        "白名单命令执行", "进程管理",
    ],
    "没有": [
        "联网搜索/网页抓取", "任意代码执行（白名单外）", "沙盒外路径访问",
        "本地模型推理（暂未接入）", "GitHub 写操作（需 git CLI 手动授权）",
    ],
}


# S161/F5：curated 意图簇——**确定性路由的主判据**。
# 为什么不靠模糊匹配：实测（2026-09-22）纯文本打分把「找出哪些地方调用了这个函数」路由到
# `ide_break`、「有哪些引用」路由到 `ide_dead_code`——因为"函数/引用"这类词在很多工具描述里
# 都出现。弱模型最怕的就是"选错工具还拿到看起来成功的答案"，所以主判据必须是手工对过的表；
# 模糊分只用来在表没命中时兜底（且排在人写的候选之后）。
_INTENTS = (
    (("调用", "引用", "谁用", "callers", "callgraph", "reference"),
     ("ide_callgraph", "ide_impact", "code_search")),
    (("影响", "改了会", "波及", "impact"), ("ide_impact", "ide_callgraph")),
    (("死代码", "死符号", "没用到", "没被用到", "没用上", "没人用", "无人引用", "unused",
      "dead code"), ("ide_dead_code",)),
    (("评审", "坏味道", "review", "补丁", "质量问题"), ("code_review",)),
    (("编译", "构建", "build", "cargo"), ("ide_build", "local_run")),
    (("测试", "test", "跑测"), ("ide_test",)),
    (("报错", "traceback", "异常", "定位", "panic"), ("bug_locate", "ide_diagnostics")),
    (("搜索", "找代码", "检索", "search", "语义"), ("code_search", "code_semantic", "ast_grep")),
    (("大纲", "符号表", "结构", "outline"), ("ide_outline", "repo_map")),
    (("重命名", "改名", "rename"), ("ide_rename",)),
    (("类型", "诊断", "lsp", "hover", "定义跳转"), ("ide_lsp", "ide_diagnostics")),
    (("依赖", "dependency", "谁依赖", "模块稳定"), ("dep_graph", "module_stability")),
    (("重复", "相似", "dupe", "拷贝"), ("near_dupes",)),
    (("找 bug", "缺陷", "扫描 bug", "bugscan"), ("bug_scan", "code_review")),
    (("漏洞", "安全", "密钥", "凭据", "secrets"), ("secrets_hunt", "vuln_knowledge", "file_scan")),
    (("读文件", "打开文件", "看文件", "cat "), ("fs_read",)),
    (("写文件", "保存", "改文件"), ("fs_write", "ide_edit_multi")),
    (("列目录", "有哪些文件", "目录内容"), ("fs_list", "fs_stat")),
    (("文件信息", "多大", "mtime", "存在吗"), ("fs_stat",)),
    (("体检", "健康", "doctor"), ("project_health", "ide_doctor", "ide_multi_check")),
    (("风险", "优先做", "排序"), ("ide_risk_rank",)),
    (("覆盖率", "coverage"), ("code_coverage",)),
    (("标准", "规范", "占位"), ("std_check",)),
    (("界面", "ui", "按钮", "空容器"), ("ui_check", "game_check")),
    (("性能", "慢", "耗时", "prof"), ("usage_stats", "session_burn", "sys_topology")),
    (("进程", "cpu", "线程", "核"), ("sys_procs", "sys_topology", "sys_threads")),
    (("备份", "回滚"), ("backup",)),
    (("教训", "经验"), ("lesson", "lesson_stats", "vuln_knowledge")),
    (("日志", "趋势"), ("scan_log", "project_health")),
    (("克隆", "审计应用"), ("app_clone", "app_audit")),
    (("防幻觉", "核查声明", "声明对不对"), ("hallucination_guard",)),
    (("有什么工具", "能力边界", "能不能做"), ("capability_manifest",)),
)


def _route(intent, tools, k=5):
    """确定性路由（无 LLM、无外部调用）：`intent` → 候选工具。

    为什么需要它（S161 模型适配 P1）：80 个工具、命名还挤（`ide×20`），**弱模型最大的失败
    不是"调错参数"，而是"根本选错工具"**——选错还会得到一个看起来像成功的答案。
    给一个"先问再调"的入口，比让弱模型在近义工具之间猜要可靠。

    两段式：① curated 意图簇（主判据，按表内优先级）；② 文本模糊分兜底（ASCII 命中工具名 ×3 /
    描述 ×2；中文 2-gram ×1）。同一 tier 内按表序/名字排序 ⇒ 输出稳定可测。
    """
    text = (intent or "").strip().lower()
    ranked, seen = [], set()

    def push(name, why):
        if name not in seen:
            seen.add(name)
            ranked.append((name, why))

    for keys, names in _INTENTS:                       # ① curated
        hit = [kw for kw in keys if kw in text]
        if hit:
            for n in names:
                push(n, "意图簇命中：" + "/".join(hit))

    by_name = {t["name"]: t for t in tools}
    ascii_tokens = [t for t in re.split(r"[^a-z0-9_]+", text) if len(t) >= 2]
    grams = set()
    for chunk in re.findall(r"[\u4e00-\u9fff]{2,}", text):
        grams.update(chunk[i:i + 2] for i in range(len(chunk) - 1))
    fuzzy = []
    for t in tools:                                    # ② 兜底
        if t["name"] in seen:
            continue
        name = t["name"]
        hay = (name.replace("_", " ") + " " + (t.get("description") or "")
               + " " + str(t.get("_group") or "")).lower()
        score, hits = 0, []
        for tok in ascii_tokens:
            if tok in name.lower().split("_"):
                score += 3
                hits.append(tok)
            elif tok in hay:
                score += 2
                hits.append(tok)
        for g in grams:
            if g in hay:
                score += 1
                hits.append(g)
        if score:
            fuzzy.append((score, name, sorted(set(hits))[:4]))
    fuzzy.sort(key=lambda x: (-x[0], x[1]))
    for score, name, hits in fuzzy:
        push(name, f"文本匹配 {'/'.join(hits)}（分 {score}）")

    out = []
    for name, why in ranked[:k]:
        t = by_name.get(name) or {}
        schema = t.get("inputSchema") or {}
        out.append({"工具": name, "为什么": why, "域": t.get("_group"),
                    "参数": sorted((schema.get("properties") or {}).keys()),
                    "必填": schema.get("required") or [],
                    "写操作": "__authorized" in (schema.get("required") or [])})
    return {
        "intent": intent,
        "候选": out,
        "提示": ("参数名照抄「参数」；「必填」一个都不能少（缺参会得到带 next 的结构化错误）。"
                 "写操作需 __authorized:true。若候选取空，改用不含 intent 的清单式调用。"),
    }


@tool("capability_manifest",
      "能力边界清单（有什么/没有什么，防能力幻觉）；**给 intent 即变为「该调哪个工具」的路由**",
      "guard",
      {"type": "object",
       "properties": {
           "intent": {"type": "string",
                      "description": "你想做的事（自然语言/中文/英文皆可）。给了它本工具从"
                                     "「清单」变成「路由」：返回候选工具 + 为什么 + 参数名 + 必填"},
       },
       "required": []})
def capability_manifest(intent=None):
    tools = list_tools()
    groups = {}
    for t in tools:
        g = t.get("_group", "misc")
        groups.setdefault(g, []).append(t["name"])
    # S75：高权限清单动态生成——list_tools 已按 requires_auth 注入 __authorized
    # 声明（S72b），此处反向读出，新挂门工具自动进清单，不再靠手写维护
    gated = sorted(t["name"] for t in tools
                   if "__authorized" in (t["inputSchema"].get("required") or []))
    out = {
        "定位": "工具箱，不是智能体；产出证据与事实，不替代 LLM 推理",
        "有": _CAPABILITIES["有"],
        "没有": _CAPABILITIES["没有"],
        "高权限": {
            "说明": "以下工具须调用方显式传 __authorized:true 授权确认（写/执行/隐私面）",
            "工具": gated,
        },
        "工具面": f"{len(tools)} 工具",
        "分组": groups,
    }
    if intent:
        # S161/F5：只回路由（清单对大模型是噪声）——弱模型要的就是"下一步调哪个"
        return {"定位": out["定位"], "路由": _route(intent, tools)}
    return out


def _tool_claim(name, sorted_tool_names):
    """反引号小写词按「在册闭集」判工具声明（确定性，无 LLM）。

    S196 设计修正——只保留两条能立住的判据：
    ① 精确在册 → verified（调用方处理）；
    ② 与在册名近似拼写（difflib ratio ≥0.8：改名/笔误类幻觉）→ refuted。
    形态规则（多段 snake_case 即疑似声明）**已退役**：328 份真实双臂答案实测，
    它误杀 `build_terrain_collider`/`map_width`/`named_pipe` 这类代码标识符 712 次、
    真工具幻觉命中 0 次——编码语境里反引号 snake_case 几乎总是被讨论的代码，
    名称本身不携带「这是 ADV 工具声明」的信息。其余词一律落「未核查」（skipped
    纪律：不静默，也不冤判）。已知边界：不在近邻半径内的凭空造名不判。
    """
    close = difflib.get_close_matches(name, sorted_tool_names, n=1, cutoff=0.8)
    return close[0] if close else None


def _check_tool_claims(text, tool_names, sorted_tool_names):
    """分支1：反引号小写词。在册→verified、近邻拼写→refuted、其余→未核查。"""
    results, unchecked = [], []
    for m in re.finditer(r"`([a-z][a-z0-9_]{2,})`", text):
        name = m.group(1)
        if name in tool_names:
            results.append({"decl": m.group(0), "kind": "tool", "status": "verified",
                            "detail": f"工具存在: {name}"})
            continue
        close = _tool_claim(name, sorted_tool_names)
        if close:
            results.append({"decl": m.group(0), "kind": "tool", "status": "refuted",
                            "detail": f"工具不存在: {name}（近似在册工具: {close}）"})
        else:
            unchecked.append(name)
    return results, unchecked


# P2：文件声明扩展名白名单。旧口径只有 12 种语言，md/json/toml/头文件等引用
# 直接消失（连 unverifiable 都不落）——「漏判 0」是在盲区上成立的。白名单外的
# `name.ext:NN` 引用现在如实计入「未核查」，不静默。
_CLAIM_EXTS = ("py", "rs", "go", "ts", "tsx", "js", "jsx", "gd", "cs", "dart", "java",
               "kt", "rb", "php", "cpp", "hpp", "cc", "c", "h", "md", "json", "toml",
               "yaml", "yml", "vue", "swift", "lua")
# 盘符前缀 `(?:[A-Za-z]:)?`：不带它时 "D:/x/y.py:12" 会被切成 "/x/y.py:12"
# （冒号不在字符类里），Windows 绝对路径声明永远对不上根（金丝雀语料实测）。
_FILE_DECL_RE = re.compile(
    r"(?:[A-Za-z]:)?([A-Za-z0-9_./\\-]+\.(?:"
    + "|".join(sorted(_CLAIM_EXTS, key=len, reverse=True)) + r"))(?::(\d+))?\b")
_ANY_CITE_RE = re.compile(r"(?:[A-Za-z]:)?[A-Za-z0-9_./\\-]+\.([A-Za-z][A-Za-z0-9]{0,6}):\d+")

_SYM_CAP = 30          # 单次调用最多真查的符号数
_SYM_SCAN_BUDGET_S = 2.0
_SYM_SKIP_DIRS = {".git", "node_modules", "__pycache__", "target", "dist", "build",
                  ".venv", ".pytest_cache", ".mypy_cache", ".ruff_cache"}


def _iter_text_files(root_full):
    """扫描用生成器：白名单扩展名、≤512KB、跳垃圾目录，坏文件静默跳过。"""
    for dirpath, dirs, files in os.walk(root_full):
        dirs[:] = [d for d in dirs if d not in _SYM_SKIP_DIRS]
        for fn in sorted(files):
            if not any(fn.endswith("." + e) for e in _CLAIM_EXTS):
                continue
            p = os.path.join(dirpath, fn)
            try:
                if os.path.getsize(p) > 512 * 1024:
                    continue
                with open(p, encoding="utf-8", errors="replace") as f:
                    yield p, f.read()
            except OSError:
                continue


def _scan_symbols(root_full, symbols):
    """在 root（已过沙盒钳制）内做限定扫描：返回 {符号: 首次出现 loc 或 None}、扫描数、是否穷尽。"""
    occ = dict.fromkeys(symbols)
    scanned, complete = 0, True
    deadline = time.monotonic() + _SYM_SCAN_BUDGET_S
    for p, content in _iter_text_files(root_full):
        scanned += 1
        for s, cur in occ.items():
            if cur:
                continue
            m = re.search(r"\b" + re.escape(s) + r"\b", content)
            if m:
                line = content.count("\n", 0, m.start()) + 1
                occ[s] = f"{os.path.relpath(p, root_full)}:{line}"
        if time.monotonic() > deadline:
            complete = False
            break
    return occ, scanned, complete


def _verdict_file(full, lineno):
    if not os.path.isfile(full):
        return "refuted", f"文件不存在: {full}"
    if not lineno:
        return "verified", "文件存在"
    try:
        with open(full, encoding="utf-8", errors="replace") as f:
            n = sum(1 for _ in f)
    except ValueError:
        return "unverifiable", "行号无法解析"
    # P3：行号从 1 起——旧口径 `:0` 因 0<=n 被假 verified
    if 1 <= int(lineno) <= n:
        return "verified", "文件存在，行号 在范围内"
    return "refuted", f"文件存在，行号 越界（文件 {n} 行，行号须 1..{n}）"


def _check_file_claims(text, root):
    """分支2：file:line 声明。沙盒外不读不判（S97 fail-closed 口径不变）。"""
    results = []
    for m in _FILE_DECL_RE.finditer(text):
        fpath, lineno = m.group(1), m.group(2)
        full = fpath if os.path.isabs(fpath) else os.path.join(root, fpath)
        try:
            full = _fs_resolve(full)
        except ValueError:
            results.append({"decl": m.group(0), "kind": "file", "status": "unverifiable",
                            "detail": "沙盒外路径，按纪律不读取不判定"})
            continue
        status, detail = _verdict_file(full, lineno)
        results.append({"decl": m.group(0), "kind": "file", "status": status, "detail": detail})
    return results


def _symbol_scan_context(root, syms):
    """分支3 前置：root 钳制 + 限定扫描。返回 ({符号: loc 或 None}, 扫描说明)。"""
    if not syms:
        return {}, None
    try:
        root_full = _fs_resolve(root)
    except ValueError:
        return {}, "root 在沙盒外，按纪律不扫描，符号不判定"
    if not os.path.isdir(root_full):
        return {}, f"root 不是目录（{root}），符号仅登记不扫描"
    occ, scanned, complete = _scan_symbols(root_full, syms[:_SYM_CAP])
    note = f"符号扫描 {scanned} 文件，" + ("已穷尽" if complete else "未穷尽（限时 2s）")
    return occ, note


def _check_symbol_claims(text, root):
    """分支3：符号声明——真扫描给证据；缺席/外部符号不冤判，如实带扫描范围。"""
    sym_matches = list(re.finditer(r"`([A-Z][A-Za-z0-9_]+)`", text))
    syms = list(dict.fromkeys(m.group(1) for m in sym_matches))
    occ, scan_note = _symbol_scan_context(root, syms)
    results = []
    for m in sym_matches:
        sym = m.group(1)
        if not occ:
            status, detail = "unverifiable", f"符号 '{sym}' 未扫描（{scan_note or '无可用 root'}）"
        elif occ.get(sym):
            status, detail = "verified", f"符号字符串在本仓出现（提及≠定义）: {occ[sym]}"
        elif sym in occ:
            status = "unverifiable"
            detail = (f"本仓扫描未出现（{scan_note}）——外部符号属正常，"
                      "若文本声称是本仓符号则疑似编造")
        else:
            status, detail = "unverifiable", f"符号 '{sym}' 未核查（超出单次上限 {_SYM_CAP}）"
        results.append({"decl": m.group(0), "kind": "symbol", "status": status, "detail": detail})
    return results, scan_note


@tool("hallucination_guard", "声明核查：file:line/符号/工具名 → verified/refuted/unverifiable", "guard",
      {"type": "object",
       "properties": {
           "text": {"type": "string", "description": "AI 声明文本（含 file:line / 反引号符号）"},
           "root": {"type": "string", "description": "仓库根目录（相对路径解析基准；缺省=服务进程 cwd，跨仓必传）"},
       },
       "required": ["text"]})
def hallucination_guard(text, root=None):
    root = root or os.getcwd()
    tool_names = {t["name"] for t in list_tools()}
    results, unchecked_terms = _check_tool_claims(text, tool_names, sorted(tool_names))
    results += _check_file_claims(text, root)
    # 白名单外扩展名的带行号引用——不判，但如实计数（P2）
    unchecked_cites = [m.group(0) for m in _ANY_CITE_RE.finditer(text)
                       if m.group(1).lower() not in _CLAIM_EXTS]
    sym_results, scan_note = _check_symbol_claims(text, root)
    results += sym_results

    verified = sum(1 for r in results if r["status"] == "verified")
    refuted = sum(1 for r in results if r["status"] == "refuted")
    unverifiable = sum(1 for r in results if r["status"] == "unverifiable")
    unchecked_n = len(set(unchecked_terms)) + len(set(unchecked_cites))
    conclusion = "存在被证伪声明（幻觉），必须纠正后才能引用" if refuted else "无被证伪声明"
    if unchecked_n:
        conclusion += f"；另有 {unchecked_n} 条不在核查面（见「未核查」，勿当作已验证）"
    out = {
        "total": len(results), "verified": verified, "refuted": refuted,
        "unverifiable": unverifiable,
        "结论": conclusion,
        "results": results[:50],
        # P6：root 回显——旧版不报解析基准，模型漏传 root 时整片误 refuted 无从发现
        "根目录": root,
    }
    if scan_note:
        out["扫描"] = scan_note
    if unchecked_terms or unchecked_cites:
        out["未核查"] = {
            "非声明小写词": {"条数": len(set(unchecked_terms)),
                            "样例": sorted(set(unchecked_terms))[:10]},
            "白名单外扩展名引用": {"条数": len(set(unchecked_cites)),
                                  "样例": sorted(set(unchecked_cites))[:10]},
        }
    return out
