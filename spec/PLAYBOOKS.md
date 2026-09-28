# PLAYBOOKS.md —— 场景族（playbook）设计：种子文档批判分析、开源对标与精选待办

> 来历（用户指令，2026-09-28）：拿《CI 平台「自我演进」战略与场景化 AI 提示词体系》
> （会话附件 `ci-self-evolution-prompts.md`，下称「种子文档」）当种子，分析怎么落到本仓。
> 用户原话要点：**种子只是个起点**；「提示词上面的想不一定就是很好，你是要分析分析的」；
> **UI 不做**；**DEMO 不需要**；**干什么必须有详细的（规格）**；**不能用上帝对象（定性条件）**；
> 「不能只看论文，还有开源项目，还要看这个东西（本仓），要思考」。
> 性质：与 [EXTERNAL-ALIGNMENT.md](EXTERNAL-ALIGNMENT.md) 同族——调研 → 对标 → 三档精选
> 待办，文档先行。选型处置口径 = LIBRARY-POLICY 三问 + 用户既有口径（名气 / 前沿性 /
> 理念契合 / 设计契合——先读这个库想成为什么）。

## 一、种子文档批判性分析（先判后用）

### 1.1 总判

种子文档的核心命题——「CI 里不存在一个通用的 CI 的 LLM，要把智能需求拆成垂直、窄口径、
可熔断、带质量门禁的场景族」——**成立，且与本仓同构**：本仓「少而准」（183→80 工具）
就是同一命题在工具面的版本。但两条主线方向相反：

| | 种子文档 | 本仓（ADV） |
|---|---|---|
| 出发点 | CI 平台里**加** LLM 场景族 | 智能体里**减** LLM 体力活（确定性工具） |
| 族的载体 | 提示词模板 + 最小后端（小模型/规则） | 工具 + 域 + 门（零 LLM） |
| 终态 | rule-first，模型调用率 <10% | **调用率 0**——种子文档的终态再往前一步 |

⇒ **可移植的不是提示词，是契约骨架与族纪律**。种子 §2 的 15 个标准字段正是本仓缺的
「一族一张机器可读契约」的形状；§0 六族与 §1.2 八方向是 CI 平台特性，大多 N/A；
小模型论证违本仓红线，不吸收（§1.5）。

### 1.2 字段逐条映射（种子 §2 → 本仓现状）

| 种子字段 | 本仓等价物 | 状态 |
|---|---|---|
| id | 工具名（registry 唯一） | ✅ |
| 族 | 域（14 域） | ✅ 但见 §1.3——域≠族 |
| role / objective | toolmeta title + description（S144 瘦身纪律） | ✅ |
| trigger | 无机器载体（散文住在 skills/*.md） | ⚠️ 缺口 |
| inputs / outputs | 入参 schema + ACI 钳制 + model-fit 回包预算 | ✅ |
| method / backends | engine 字段（lsp/resolved/text/callgraph）+ 三档引擎如实上报 + skipped 降级 | ✅ |
| boundaries | 授权门（requires_auth）+ 沙盒 fail-closed + agent-boundaries + untrusted 前缀 + annotations | ✅ 最厚的一块 |
| fallback | 三级降级（S109）+ 引擎回落（rust/gpu/cpu）如实上报 | ✅ |
| circuit_breaker | tools/breaker.py（per-key 10 次/300s + 全局 QPM 3000 + 日量 10 万） | ✅ 但**只有工具级，无族级** |
| quality_gates | 门链 34+ 项（S160 记账口径，此后 lint/type/typos/gitleaks 等仍有增补）+ tool-evals 13 任务 | ✅ 但**任务没有按族归组** |
| evaluation | EVAL.md H1-H5 + bench/ 三管线 | ✅ |
| template | —— | N/A（零 LLM，无模板可言） |

**结论：15 字段里 13 个已有等价物、1 个 N/A。真缺口三个半：**
①**族没有被声明化**——契约散在 7 处（registry/toolmeta/skills/门脚本/bench/breaker/审计
账本），加一件东西要摸 7 处；②族级熔断；③族→评测映射；（半个）trigger 的机器载体。

### 1.3 六族怎么落：族=编排层，不是新分区

种子把 LLM 任务分六族；本仓 14 域是**工具归属**，不是**任务场景**。同一批工具
（fs+scan+guard）会被「审计一个仓」「体检一个项目」「挖一轮漏洞」三个场景反复手工组合
——硬编码编排器就是这些场景的事实存在，只是没声明化：

| 既有硬编码编排器 | 场景 | 现状 |
|---|---|---|
| `attack_cruise` | 全攻击面一键巡航 | 授权门自审+路径探针+输入模糊 串在代码里 |
| `ide_doctor` / `ide_multi_check` | 项目一键体检 / 多项目联动 | bug_scan+code_review+构建+测试 串在代码里 |
| `project_scan` / `project_health` | 三路扫描 / 健康评分 | 同上 |
| `scripts/vuln_hunt.py` | 克隆→三合一→封印报告 | 脚本化，不在工具面 |

⇒ 族的正确落点：**把编排从「写死在代码」升为「声明在数据」（manifest），经同一解释器
执行**；不是新开一套工具分区，更不是给六族各造后端。与 S149 profile_enable 的关系：
**族映射到域集合，不新增分区**（EXTERNAL-ALIGNMENT C 档预留的「域级 profile 可选」
正是这条线）。

### 1.4 八方向判定（种子 §1.2）

| 方向 | 判定 | 理由 |
|---|---|---|
| 时间旅行调试 | N/A | CI 平台特性；本仓无 Job/重放面 |
| 成本/碳感知调度 | N/A | 同上 |
| 交互式调试 | ✅ 已有 | `ide_debug`/`ide_break`（断点+locals+调用栈）即 attach 诊断 |
| 管道 DSL | N/A 且**反对** | 见 §2.1——S15 已砍 pipeline；发明 DSL 是噪音海的数据版 |
| CI 可观测性 | 部分 N/A | span/火焰图属平台面；本仓已有 usage_stats/scan_log/session_burn |
| 多租户 | N/A | 本地单用户 |
| 预测性缓存预热 | N/A | 同上 |
| 取证审计 | ✅ 已有且更厚 | stats.jsonl/scan_log/audit-ledger/clients.jsonl；hash-chain 见 B 档⑦ |

### 1.5 小模型论证（种子 §1.3）：不吸收，三理由

① 违红线（纯 stdlib / Cargo 恒空；LIBRARY-POLICY 三问过不了——嵌入模型不是「能力探测
薄壳」能包的形态）；② 现有 tf-idf（code_semantic）/BM25（code_search）/纯统计已覆盖
种子点名的任务（归因=规则、检索=tf-idf、flaky=统计）；③ **先量后改**：没有任何一条
usage_stats 证据显示「规则不够用、需要嵌入」——按纪律记 C 档（不追），需求出现再立项。

## 二、本仓自己的先例（要看这个东西，别绕开它）

### 2.1 S15 的 pipeline/parallel 之死——playbook 不是它的还魂

S15 以证据驱动砍掉 `pipeline`（步骤链 preset 配方）/`parallel`/`pure_*`（README 有案）。
**必须正面回答：playbook 与被砍的 pipeline 有何不同？**

| | pipeline（被砍） | playbook（本设计） |
|---|---|---|
| 语义 | **通用**步骤链执行器（任意组合） | **场景垂直**：一族对应一个真实重复场景 |
| 产出 | 中间结果透传，无证据聚合 | 逐族输出契约 + 证据聚合形状（门锁） |
| 边界 | 无 | 逐族声明（能力档/域/预算/熔断），门校验 |
| 评测 | 无 | 逐族挂 tool-evals 任务 |
| 准入 | 无门槛 | **用量证据或用户点名**（§4.9） |

教训吸收：通用编排面=噪音海的数据版。⇒ 声明面刻意最小（§4.4），**拒绝发明工作流 DSL**：
无循环、无内容分支、无嵌套——表达不了的场景继续留在硬编码（少而准）。

### 2.2 硬约束落到形态（用户定性条件）

- **不能用上帝对象**：manifest=数据（一族一文件）；解释器单文件 ≤260 行（函数硬阈 120
  在案，全仓已无超长函数）；加一族=加一个 JSON、**不改一行解释器**——这条本身做成
  机器断言进门（§4.7⑦）。
- **干什么必须有详细的**：§4 逐字段规格 + 逐条门校验项 + oracle 迁移程序。
- **UI 不做 / DEMO 不需要**：落地物只有 manifest + 解释器 + 门 + 测试；无界面、无示例工程。
- **价值主张必须可测**（H1 口径教训：散文自评不算数）：族层的判据=「弱模型完成同类
  场景的轮次数/选错工具次数/回包体量」对照，不写「更智能」。

## 三、开源对标（2026-09-28 检索；按 LIBRARY-POLICY 三档处置）

| 项目 | 形态 | 对本设计的可取处 | 处置 |
|---|---|---|---|
| **Agent Skills**（Anthropic 开放标准 agentskills.io；OpenHands 支持并扩关键词/路径触发） | 一族一目录 + SKILL.md 元数据；**三级渐进披露**（启动只载名字/描述 → 命中才载正文 → 附属文件按需） | ①「族=数据包」的业界标准形态；②三级披露正是工具面 token 经济的解法（对齐 EXTERNAL-ALIGNMENT §一.2 的 defer_loading 线） | **吸收理念**（manifest + 披露分级：list 常驻仅 id+title+when，详情按需，证据执行时才拉）；不搬 SKILL.md 格式——宿主侧已有 skills/*.md 通道，工具侧 manifest 不与之混 |
| **claude-code-action / Claude Code 权限档**（`allowed_tools` 白名单 + permission mode） | CI 自动化场景 = 工具白名单 + 权限档显式声明 | 「族=工具白名单」的直接先例；边界随族声明而非全局一把抓 | **吸收**（manifest 的 tools/capability 字段） |
| **PR-Agent（Qodo）** | 垂直命令族（/review /improve /describe /ask）各自配置；类别开关；产出数量预算；repo 级配置文件 | 逐族配置 + 产出预算正是族契约要管的两件事 | **吸收**（budget_chars/outputs 字段） |
| **mcp-scan**（Invariant：工具描述投毒、rug-pull 哈希钉扎、可进 CI）与 **mcp-scanner**（Cisco AI Defense 开源：YARA+LLM 扫 MCP server） | 扫第三方 MCP server 的工具定义 | ①宿主 config.json 里**其它** server 的工具定义投毒扫描 = appaudit 真实扩展位；②rug-pull 对照：selftest 已锁 schema/exe/version，**工具描述面未锁哈希**——缺口如实记 | **有界**（B 档⑤⑥） |
| NeMo Guardrails / OPA·Cedar | 声明式边界 / 策略即数据 | 「策略是数据不是代码」与 manifest 同构 | **吸收理念，不引依赖** |
| LangGraph 等工作流框架 | 重量级编排 | —— | **不吸收**（违零依赖；且本设计恰要「不发明 DSL」） |

（外部理念处置先例：S100 雷达 11 项全部按「oracle→双绿→文档」兑现；本文待办沿用同款。）

## 四、设计：playbook 层详细规格

### 4.1 定位与四条不变量

1. **工具箱不是智能体**：解释器只做确定性步骤执行与证据聚合；裁决（怎么解读证据、下一步
   干啥）永远在智能体。族内**禁止**任何内容语义分支。
2. **权限不升格**：族内每步照走 registry.call 单一裁决点（授权/沙盒/QPM/审计全走既账）；
   族声明的能力档是**上界预告**，不是旁路。
3. **声明面最小**：顺序步 + 占位符 + 跳过条件 + 预算，到此为止。
4. **数据与解释器分离**：加族=加 JSON；解释器改动须过 god/dupe 双门并走实施轮。

### 4.2 文件布局

```
playbooks/                 # 一族一文件（数据层）
  attack.cruise.json
  ide.doctor.json
  scan.project.json
tools/playbook.py          # 解释器 + 两个工具注册（≤260 行，进 god 基线）
scripts/playbook_gate.py   # 门（schema/引用/预算/解释器哈希锁/金丝雀）
```

### 4.3 manifest schema（逐字段）

| 字段 | 类型 | 必选 | 语义与校验 |
|---|---|---|---|
| `id` | str | ✅ | `域.场景`（如 `attack.cruise`）；全库唯一（门查重） |
| `title` | str | ✅ | 一行中文标题（进 list 输出；遵守 S144 描述瘦身纪律：削 provenance，保语义/判据/代价/when-not） |
| `when` | str | ✅ | **触发指引**（给智能体的意图匹配词）。诚实声明：本仓无宿主侧自动触发，这是选择辅助不是自动开关 |
| `domains` | [str] | ✅ | 涉及域集合（门校验为合法域名；执行前逐域核 profile，未启用 ⇒ 按 registry 既有文案如实报并指 `profile_enable`） |
| `capability` | enum | ✅ | `read`/`write`/`execute`——**族能力上界**；门校验=按成员工具 annotations 实算不得低于声明（全 readOnly 才许 read）；`playbook_run` 对 write/execute 档要求 `__authorized`（门控看能力，决策 #5） |
| `tools` | [str] | ✅ | **成员白名单**（claude-code-action allowed_tools 式）；门校验=步骤引用 ⊆ 白名单 ⊆ 真实注册名 |
| `inputs` | obj | ✅ | `{名: {type: str|int|bool|path, required, default?, desc}}`；未知键在执行入口即拒（默认严苛）；`path` 型过 `_fs_resolve` 沙盒（随步骤本身） |
| `steps` | [obj] | ✅ | 见 §4.4；≤16 步（门限） |
| `outputs` | obj | ✅ | 输出契约：每步数据键名 + 聚合摘要键（形状由门与测试锁） |
| `budget_chars` | int | — | 回包总预算（默认 32768，门限 ≤65536；超预算按 ACI 钳制保头保尾并如实标） |
| `fallback` | str | ✅ | 兜底路径文字（哪步能力缺席时退到什么；执行器不解释，给智能体读） |
| `breaker` | obj | — | `{max_fail_streak: int=3}` 族级失败连击阈值（§4.6） |
| `evals` | [str] | — | 关联 tool-evals 任务 id（门校验存在性；B 档⑥才端到端真跑） |

### 4.4 步骤语义（刻意最小）

```json
{"step": "s1", "tool": "path_probe",
 "args": {"path": "$target"},            // $name 绑定 inputs；未知占位符门即红
 "optional": false,                       // true=失败只记不连坐
 "skip_if": {"step": "s0", "ok": true}    // 唯一的条件形态：前置步 ok 才跳过
}
```

- 无循环、无嵌套、无对返回**内容**的分支（`skip_if` 只看 ok/error 布尔）。
- 族级 `on_error`：`stop`（默认）| `continue`；失败步如实进 `errors`。
- 每步经 `registry.call` ⇒ 授权/沙盒/QPM/审计（stats.jsonl 逐步留痕 + playbook_run 自身
  一行）全部既账，**零新增审计代码**。

### 4.5 输出契约（形状）

```json
{"ok": true, "playbook": "attack.cruise", "capability": "read",
 "steps": [{"step": "s1", "tool": "path_probe", "ok": true, "data": {...}},
           {"step": "s2", "tool": "input_fuzz", "ok": false, "skipped": "skip_if"}],
 "summary": {"...": "...outputs 声明的聚合键..."},
 "chars": 8123, "budget": 32768}
```

### 4.6 族级熔断（真缺口，小改）

tools/breaker.py 现有音量熔断（per-key 10 次/300s + 全局 QPM + 日量告警）。新增
**失败连击**维度：key=`playbook:<id>`，连续 ok:false ≥ manifest 阈值（默认 3）⇒ 族开路，
报错文案指向「修输入或 `breaker_reset`」。约 30 行 + 测试；与音量熔断并存、互不替代；
BREAKER.md 同步。

### 4.7 门（scripts/playbook_gate.py，进快门 + core.yml，形状锁同步）

校验项：①JSON 可解析 + 字段齐 + 类型对；②id 唯一；③tools/steps 引用的工具名全部真实
注册；④domains 合法；⑤capability 与成员工具 annotations 实算一致；⑥步骤数/预算/
占位符上限；⑦**解释器哈希锁**——门记录 tools/playbook.py 的 sha256 基线：manifest 集合
变化而哈希不变 ⇒ 绿（「加族不改码」的机器断言）；哈希变化 ⇒ 红，提示须走实施轮 +
god/dupe 复核；⑧evals 引用存在；⑨金丝雀 manifest（故意坏三处）必红。
工具面预算影响：+2 工具 ≈ +1.2K 字符（现 41,463 / 帽 45,000，帽内；超帽按既有门要求
记账抬帽或瘦身）。

### 4.8 oracle 迁移程序（换芯纪律，S80-S86 套路）

首批三族是**既有硬编码编排器的声明化**，不是新行为：① 写 manifest；② 解释器跑出的
步骤序列与输出**与原工具逐字节对拍**（同输入同语料 A/B）；③ 一致才把原工具改薄壳转调
manifest（原实现保留为 oracle 至退役轮）；④ 退役断言：内部路径名不得复活；⑤ god/dupe
基线随净账重记。

### 4.9 准入纪律（防噪音海在数据层复活）

新 manifest 的准入证据 = usage_stats 显示该场景手工组合近 30 天 ≥10 次，**或**用户点名。
低于门槛 ⇒ 记在本文档排队区，不做。排队区当前为空（如实）。

## 五、精选待办（三档，不堆）

### A 档（立刻，各一轮）
1. ~~本文档~~（本轮 S174）。
2. **manifest schema 定稿 + 解释器 + 门 + 首批三族**（§4 全部；三族按 §4.8 对拍换芯：
   attack.cruise / ide.doctor / scan.project）。
3. **族级失败连击熔断**（§4.6；breaker 扩维度 + 测试 + BREAKER.md 同步）。

### B 档（评估后做，有决策点）
4. **导航型步骤**（guide 模式：某步返回「检查点」让智能体判断再续）——弱模型场景实测
   有需求再上，先量后改。
5. **宿主其它 MCP server 的工具定义投毒扫描**（对标 mcp-scan/mcp-scanner 的检测项取
   正则子集，走 appaudit 域；YARA 与 LLM 分析都不引——零依赖红线）。
6. **族→evals 端到端真跑**（tool-evals 增族级任务 + usage_stats 增 playbook 维度）+
   selftest 补「工具描述面哈希」对账（mcp-scan 的 rug-pull 启发）。
7. **审计链 hash-chain**（audit-ledger 增 prev-hash 串联，append-only 取证加固；种子
   §4.8 取证方向的可取残段）。

### C 档（记录不追）
- 嵌入/小模型后端（§1.5）；八方向 CI 平台特性（§1.4）；提示词模板文本（宿主侧资产，
  不进工具面）；**UI / DEMO**（用户明示不做）；SKILL.md 格式照搬（§三）；工作流 DSL（§2.1）。

## 六、来源（2026-09-28 检索）

- 种子文档：《CI 平台「自我演进」战略与场景化 AI 提示词体系》（会话附件，未入仓）
- Agent Skills 开放标准：[agentskills.io](https://agentskills.io/home) ·
  [Anthropic 工程博客](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills) ·
  [平台文档](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview)
- claude-code-action（allowed_tools）：[anthropics/claude-code-action](https://github.com/anthropics/claude-code-action)
- PR-Agent（Qodo）：[Codium-ai/pr-agent](https://github.com/Codium-ai/pr-agent) · [docs.pr-agent.ai](https://docs.pr-agent.ai)
- mcp-scan（Invariant Labs）：[invariantlabs.ai](https://invariantlabs.ai) ·
  mcp-scanner（Cisco AI Defense）：[cisco-ai-defense.github.io](https://cisco-ai-defense.github.io) ·
  [GitHub](https://github.com/cisco-ai-defense/mcp-scanner)
- OpenHands Skills（关键词/路径触发）：[docs.openhands.dev/overview/skills](https://docs.openhands.dev/overview/skills)
- 仓内先例：README（S15 废物面清单）/ PANORAMA §三 决策账 / EXTERNAL-ALIGNMENT §一.2、C 档 /
  BREAKER.md / LIBRARY-POLICY.md / CD-PLATFORM.md
