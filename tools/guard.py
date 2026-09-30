"""tools/guard.py —— 防幻觉域（2 工具）：hallucination_guard / capability_manifest

AI 声明事实核查（verified/refuted/unverifiable 三分级）——防 AI 编造
file:line / 符号 / 工具名。这是"工具代替智能体"里最关键的护栏。
"""
import os
import re

from registry import list_tools, tool
from tools import guard_checks

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
    (("风险", "优先做", "排序"), ("risk_rank",)),
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


@tool("hallucination_guard", "声明核查：file:line/符号/工具名 → verified/refuted/unverifiable"
      "（疑似才判：非工具主张/未覆盖扩展名如实进 skipped——不冤判、不静默丢）", "guard",
      {"type": "object",
       "properties": {
           "text": {"type": "string", "description": "AI 声明文本（含 file:line / 反引号符号）"},
           "root": {"type": "string",
                    "description": "仓库根目录（相对路径解析基准）。**讨论别的项目必须给**——"
                                   "缺省用服务进程 cwd，会给出一整批假 refuted"},
       },
       "required": ["text"]})
def hallucination_guard(text, root=None):
    """三分支核查；判定件在 `tools/guard_checks.py`（S195 拆出并修四条精度缺陷：
    P1 非工具主张不判 · P2 未覆盖扩展名不静默 · P3 行号下界 1 · P4 符号真查）。"""
    root = root or os.getcwd()
    tool_names = {t["name"] for t in list_tools()}
    tool_rows, skipped_tool = guard_checks.tool_decls(text, tool_names)
    file_rows, unchecked_ext, outside = guard_checks.file_decls(text, root)
    results = tool_rows + file_rows + guard_checks.symbol_decls(text, root)
    verified = sum(1 for r in results if r["status"] == "verified")
    refuted = sum(1 for r in results if r["status"] == "refuted")
    unverifiable = sum(1 for r in results if r["status"] == "unverifiable")
    return {
        "total": len(results), "verified": verified, "refuted": refuted,
        "unverifiable": unverifiable,
        "skipped": {"not_a_tool_claim": skipped_tool, "unchecked_ext": unchecked_ext,
                    "sandbox_outside": outside},
        "结论": (f"存在被证伪声明 {refuted} 条（见 results 里 status=refuted），纠正后才能引用"
                 if refuted else "无被证伪声明"),
        "results": results[:50],
    }
