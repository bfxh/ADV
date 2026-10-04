# E5 — AI4Code 工程实验（编码代理 × 扫描器协作机制）

> 域定位：ADV 是"本地优先代码安全/质量平台"，本域不调研 Agent 产品，只调研**扫描器与 AI 代理协作的工程机制**：代理怎么读代码库（上下文）、怎么用工具面（ACI/MCP）、怎么形成修复验证闭环（仲裁权归谁）、离线档能承诺什么。
> 检索口径：WebSearch，2026-10-03；一手来源以官方文档/论文为主，社区口径已标注。标注（待核）= 未拿到一手确认。

## 0. 五个重点问题的结论（先答）

**① aider repo-map 机制拆解**（ADV repo 摘要→MCP 上下文的直接参照）
- 链路：tree-sitter 全仓解析 → AST 查询提取"定义/引用"标签（语言无关、增量快）→ 标签构成有向引用图（节点=定义，边=引用；私有/内部定义权重低、公开定义权重高）→ **Personalized PageRank** 排序，聊天中已打开/已编辑的文件作为个性化种子提权 → 在 token 预算内选**最优文件集**（覆盖排名最高的定义数最大化）→ 树形渲染（类→方法嵌套 + 行号），LLM 需要细节再自取。
- 预算与频率：地图 token 预算可配（`--map-tokens`；默认值未一手核实，待核）；每次发送前重算，靠 tree-sitter 的速度+标签缓存支撑该频率。官方文档只写"graph ranking algorithm"，社区考据（blog.longhopick.com 2026-08）确认为 PageRank 族但官方未点名——引用时按"aider 的图排序"表述更稳。
- 对 ADV：adv-index 已有符号层 → 把"标签图 + 图排序 + 预算内选文件集 + 树渲染"做成 repo 摘要的 MCP 返回形态，是成本最低的移植路径（算法零 LLM 参与，纯静态，符合 IRIS 边界）。

**② SWE-agent ACI 设计原则**（工具面怎么设计才好被代理用）
- 四原则（arXiv:2405.15793，2024）：(1) **Guardrails**：危险动作加护栏——编辑后自动 lint，语法错误即回滚，代理无法弄坏文件；(2) **紧凑信息密集**：搜索返回行号+片段，不倾倒全文；(3) **高效检索/浏览**：find+grep 类检索工具 + 文件查看器限 100 行，强制聚焦；(4) **即时反馈**：动作失败必须给出明确错误与原因，代理才能自纠。
- 实证：自定义 ACI 使 SWE-bench 全集 12.47% / Lite 18.00%（当时 SOTA，2024-05 口径）；消融显示同一强模型换差接口性能骤降。
- 对 ADV MCP 面：这正是 wave-0 MCP 面的机制级依据——工具结果预算化（=紧凑）、门禁失败返回结构化证据（=反馈）、写操作带守门（=guardrails）、渐进披露（=高效导航）。

**③ 代理上下文选择的实证结论**（什么粒度最有效）
- 多源一致（非单一实验，均为各论文自报口径）：一次性**文件级 RAG** 最弱——SWE-agent 论文中 non-interactive RAG 基线显著低于交互式 ACI；RepoCoder（arXiv:2303.12570）实证**迭代检索-生成**优于一次性检索（one-shot 常因上下文稀疏/词表错配漏检；RepoEval 1600 用例）；RepoGraph（arXiv:2410.14684）实证**仓库级代码图接地**省交互轮次并提升解决率。
- 2025–2026 主流信号：**图级+符号级混合、代理多轮迭代取用**，文件级只作装载单位不做检索单位。SWE-ContextBench（2026，编号待核）专门评代理上下文检索粒度，可作后续量具。
- 对 ADV：MCP 检索面应提供符号/图查询原语 + 支持代理多轮增量检索，不承诺"一次返回全部上下文"。

**④ 扫描结果→代理修复闭环的仲裁设计**
- 机制样本：SWE-agent 编辑守门（语法门自动判、不过即回滚）；aider `--test-cmd --auto-test`（每次编辑后自动跑测试，失败输出回喂 LLM，循环）；Self-Debug（arXiv:2304.05128）消融：测试反馈 > 解释反馈 > 简单反馈，且反馈越结构化越省 token。
- 结论（结合 wave-0 10 的 IRIS 边界"LLM 只产候选、确认权在静态引擎"）：**仲裁权=确定性门**——重扫结果 diff 判定（原告警消失且无新增告警 + 测试通过），LLM 自评与 LLM 复核只能做解释层/候选层，不做通过判定。这与 aider/SWE-agent 的工程实践一致：它们的循环里"通过/不通过"都由 linter/测试程序说了算。
- 对 ADV：adv-server 的门（gate）天然是这个仲裁者；MCP 面向代理返回的应该是"结构化门失败证据"而非自然语言结论。

**⑤ 本地 LLM 档的可行性边界**（离线场景 ADV 能承诺什么）
- 2026 现状：llama.cpp 是本地生态事实标准后端（第三方口径 118k+ stars），GGUF + OpenAI 兼容 API；Ollama 为其易用包装；llama.vim（ggml-org）是极简本地 FIM 补全插件，官方即提示编码任务需把默认 ~4K 上下文调大（社区文档口径）。
- 社区共识（r/LocalLLaMA 2026-04 线程口径）：本地小模型（7B–24B 量化，如 Devstral Small 1.1 24B）**补全/摘要/解释可用，端到端 agentic 修复仍落后云模型**。
- 对 ADV 离线档承诺：离线 = "离线解释器"——告警解释、摘要、候选排序可承诺；自动修复决策、复杂多步修复不承诺。产品形态锚：llama.vim 的克制（只做 FIM）而非全套代理。

## 1. 对象清单（定位 → 可抄机制 → 档位 → 第一步 → 许可证/成熟度 → 链接）

### 1.1 编码代理开源栈

**1) aider repo-map**
- 定位：非 agentic 编码助手的仓库摘要机制，本文档重点①的全部内容即其拆解。
- 可抄机制：(a) tree-sitter 标签→引用图→Personalized PageRank；(b) 预算内最优文件集选择；(c) 聊天文件作排序种子的动态提权。
- 档位：**吸收**。第一步：adv-index 原型一个 `repo_map(budget_tokens)` MCP 工具（tree-sitter Rust 绑定已属 D 域栈），对照 aider 渲染格式做 golden 测试。
- 许可证/成熟度：Apache-2.0，维护中（2026 仍活跃）。
- 链接：https://aider.chat/docs/repomap.html ｜ https://aider.chat/2023/10/22/repomap.html

**2) SWE-agent（ACI）**
- 定位：ACI 概念的出处与四原则（重点②），Princeton NLP。
- 可抄机制：(a) 编辑守门 lint-and-revert；(b) 检索工具返回"行号+片段"契约；(c) 100 行限查看器的聚焦强制；(d) 失败必带原因的错误返回。
- 档位：**吸收**（原则层，非代码层——它是 Python 学术栈）。第一步：把四原则写成 adv-server MCP 工具的验收判据（每工具一条"失败返回什么"契约）。
- 许可证/成熟度：MIT，维护中。
- 链接：https://arxiv.org/abs/2405.15793 ｜ https://swe-agent.com/

**3) OpenHands**
- 定位：最活跃的开源编码代理平台（2026 综述口径），架构参照物。
- 可抄机制：(a) **事件流架构**——一切动作/观察皆事件、可回放可审计（对 ADV 扫描会话的审计日志形态直接适用）；(b) CodeAct——动作=可执行代码而非 JSON 工具调用（表达力强但沙箱要求高）；(c) Docker 沙箱 runtime 隔离。
- 档位：**有界**。事件流思想吸收；CodeAct 不吸收（ADV 是静态引擎+MCP 面，不是通用代理平台）。
- 第一步：adv-server 的会话记录对齐"事件=不可变原子"模型。
- 许可证/成熟度：MIT，维护中。
- 链接：https://github.com/All-Hands-AI/OpenHands ｜ https://docs.all-hands.dev

**4) Cline / Roo Code（上下文管理）**
- 定位：VSCode 代理类，上下文工程脏活样本（第三方 harness study 拆解口径）。
- 可抄机制：(a) 工具结果 ~48k 字符上限 + **头尾保留中段裁剪**（output-limits.ts，第三方拆解）；(b) Roo 3.3 对非缓存模型用滑窗丢最旧消息；(c) 教训：用 ~3 chars/token 字符估算触发压缩导致裁切不准（Cline changelog 2026-09）——ADV 有真实 tokenizer，别犯这个错。
- 档位：**吸收**（限幅机制对照旧仓"出口限幅"，业界一致）。第一步：MCP 工具返回统一走"预算化+头尾裁剪+落盘引用"。
- 许可证/成熟度：Apache-2.0，维护中。
- 链接：https://github.com/cline/cline ｜ https://github.com/RooCodeInc/Roo-Code

**5) Claude Code 工程分析（第三方拆解，非官方）**
- 定位：闭源标杆的公开工程考据：三层压缩——**microcompact**（旧工具结果优先逐出）/ **full compact**（阈值触发整段摘要）/ **session memory**；auto-compact 阈值每次迭代前评估。
- 可抄机制：(a) 逐出优先级=工具结果先于对话历史；(b) 压缩分档触发而非单一阈值。
- 档位：**reference**（闭源，只能考据不能抄码）。第一步：把"工具结果优先逐出"写进 ADV 上下文预算分配默认档。
- 许可证/成熟度：专有（Anthropic）；拆解属第三方分析，未经官方确认。
- 链接：https://oldeucryptoboi.com（How Claude Code Manages Infinite Conversations…）｜ https://wuu73.org（When the Window Fills Up）

**6) 《The Autonomous Coding Agent Landscape (2026)》（综述）**
- 定位：2026 代理栈景观综述，交叉验证各栈定位（如"OpenHands 最活跃 MIT 平台、CodeAct 直跑 Python 非 JSON 工具调用"）。
- 可抄机制：无单点机制，作 2026 前沿信号与交叉引用源。
- 档位：**reference**。第一步：无需动作。
- 许可证/成熟度：博客，时效口径 2026。
- 链接：https://taibui.dev

### 1.2 代理 × 代码理解

**7) RepoGraph**
- 定位：仓库级代码图作为代理的即插即用接地模块（定义/引用/语义邻接关系图），代理按需查图而非靠 LLM 脑补全仓结构。
- 可抄机制：(a) 静态图构造与代理解耦（图由静态解析产出，LLM 只消费——与 IRIS 边界同构）；(b) 图节点=代码实体、边=引用/邻接，代理的"定位-展开"动作直接落在图上。
- 档位：**吸收**。第一步：adv-index 的符号图对齐其节点/边模型，暴露 `graph_neighbors(symbol)` MCP 查询。
- 许可证/成熟度：论文 arXiv:2410.14684（2024-10 首发，2025 被广泛引用复现）；开源代码仓状态待核。
- 链接：https://arxiv.org/abs/2410.14684

**8) CodexGraph**
- 定位：LLM 代理 + 代码图数据库（Neo4j 持久图），代理动态发图查询导航仓库，对照"一次性 RAG"。
- 可抄机制：图库持久化 + 代理以工具调用方式增量查图（对应 ADV：adv-index 即图库，MCP 即查询面）。
- 档位：**有界**（思想吸收；Neo4j 引入不吸收——本地优先栈不需要外部图库进程）。第一步：验证 SQLite/自研索引能否承载同等查询原语。
- 许可证/成熟度：论文 2024（arXiv:2408.03910，编号待核），工业界团队（ModelScope 系）产出。
- 链接：https://arxiv.org/abs/2408.03910

**9) RepoCoder**
- 定位：迭代检索-生成闭环的奠基实证（2023，被引 600+）。
- 可抄机制：生成/读到的代码反哺下一轮检索 query——**检索不是一次性动作而是循环**；多粒度检索（行/块/文件）。
- 档位：**吸收**（设计原则）。第一步：MCP 检索工具支持代理回传"已读内容摘要"以 sharpen 下轮查询（或至少文档化多轮检索的推荐调用序列）。
- 许可证/成熟度：论文，arXiv:2303.12570，社区复现活跃。
- 链接：https://arxiv.org/abs/2303.12570

**10) SWE-ContextBench / 粒度消融线**
- 定位：2026 出现的代理上下文检索评测线（含 SWE Atlas 的 Codebase Q&A 任务），回答重点③的量具。
- 可抄机制：把"检索粒度"当被测变量（文件/符号/图）而非固定实现。
- 档位：**watch**。第一步：出基线后用其任务形态设计 ADV 检索面消融（先量后改）。
- 许可证/成熟度：新基准（2026，引用数少，编号待核），实验期。
- 链接：https://arxiv.org（SWE-ContextBench，检索标题）

### 1.3 代理验证闭环

**11) aider auto-test / lint-test 流程**
- 定位：编辑后自动验证的工程范式：`--test-cmd` 指定命令 + `--auto-test` 每次编辑后自动执行，失败输出回喂 LLM（重点④的仲裁样本；本条机制描述来自 aider 官方文档口径，具体开关名以文档为准，待核）。
- 可抄机制：循环中"通过判定"由外部程序（测试进程）做出，LLM 只消费失败证据。
- 档位：**吸收**。第一步：adv-server 定义 `scan_gate(findings, rescan_diff)` 仲裁原语。
- 许可证/成熟度：Apache-2.0，维护中。
- 链接：https://aider.chat/docs/usage/lint-test.html

**12) Self-Debug**
- 定位：自调试研究线（2023），反馈形式消融：单元测试反馈 > 解释反馈 > 简单反馈，且省 token。
- 可抄机制：结构化失败证据（测试名+断言+期望/实际）是修复循环里性价比最高的反馈形态。
- 档位：**吸收**（作为④的证据锚）。第一步：门失败输出统一为结构化 finding（复用 MCP 面契约），不产自然语言散文。
- 许可证/成熟度：论文 arXiv:2304.05128，引用充分。
- 链接：https://arxiv.org/abs/2304.05128

**13) 扫描→修复→重扫闭环（本域综合设计条目）**
- 定位：把 11/12 与 wave-0 的 IRIS 边界合成 ADV 的闭环：代理拿结构化告警→产候选修复→**重扫 diff 仲裁**（告警消失且无新增=过门）→门红时回喂结构化证据。LLM 复核只做解释层。
- 可抄机制：见上；仲裁者=adv-server 的确定性门。
- 档位：**吸收**。第一步：在 E5 域出一个最小实验设计（一个 CWE 类别 × 一个开源仓 × 有无门仲裁的对照）。
- 许可证/成熟度：内部设计，无 license。
- 链接：见本文档 0.④

### 1.4 本地代理运行时

**14) llama.cpp**
- 定位：本地推理事实标准后端（GGUF + llama-server OpenAI 兼容 API）。
- 可抄机制：无代码可抄（不同栈）；机制=离线档的承载进程：ADV 以子进程方式调 llama-server，LSP/索引仍在 ADV 侧。
- 档位：**有界**。第一步：离线档可行性实验：用 llama-server 跑告警解释 prompt，量延迟与质量下限。
- 许可证/成熟度：MIT，维护极活跃（第三方口径 118k+ stars，2026）。
- 链接：https://github.com/ggml-org/llama.cpp

**15) Ollama**
- 定位：llama.cpp 易用包装，一键运行模型。
- 可抄机制：模型清单/拉取/服务的 UX；对 ADV 用户本地模型接入的降门槛路径。
- 档位：**watch**。第一步：无需动作。
- 许可证/成熟度：MIT，维护中。
- 链接：https://github.com/ollama/ollama

**16) llama.vim**
- 定位：本地 FIM 补全的极简集成（Vim/Neovim + llama.cpp server），隐私优先。
- 可抄机制：产品形态的克制——离线只承诺补全层；编辑器内上下文（光标前后窗口）够补全用。
- 档位：**reference**。第一步：作为⑤离线档承诺边界的写作用例。
- 许可证/成熟度：MIT，维护中（ggml-org 官方线）。
- 链接：https://github.com/ggml-org/llama.vim

**17) 本地小模型可行性结论条目**
- 定位：⑤的证据条：7B–24B 量化模型补全/解释可用（Devstral Small 1.1 24B 为社区常提的 agentic 下限档），端到端复杂修复仍落后云模型（r/LocalLLaMA 2026-04 线程共识口径，样本=社区自报，非受控实验）。
- 可抄机制：无；承诺边界：离线档=解释/摘要/候选排序，不承诺修复决策。
- 档位：**watch**。第一步：列入 07-performance 的本地推理预算核算。
- 许可证/成熟度：社区口径，时效 2026-04。
- 链接：https://www.reddit.com/r/LocalLLaMA/comments/1sknx6n/best_local_llms_apr_2026

### 1.5 上下文工程

**18) 上下文裁剪策略谱系**
- 定位：业界裁剪策略四类对照（搜索归纳）：最旧先丢滑窗 / 钉住系统+滑窗 / **工具结果优先逐出** / 阈值自动压缩。
- 可抄机制：工具结果先于历史逐出（Claude Code 考据 + Cline 限幅 + Roo 滑窗三源一致）；真实 tokenizer 计数（Cline 的 3 chars/token 教训反证）。
- 档位：**吸收**。第一步：adv-server 会话装配器实现"钉住规则→限幅工具结果→压历史"三段预算。
- 许可证/成熟度：多源综述，锚见各条。
- 链接：见 1.1 第 4/5 条

**19) token 预算分配（系统/工具结果/历史权重）**
- 定位：2025–2026 工程实践合流：repo 摘要占**固定小预算常驻**（aider map 预算）；工具结果按调用预算化（SWE-agent 紧凑原则 + Cline 48k 上限）；历史走分档压缩（Claude Code 三层）。
- 可抄机制：预算三段式默认档 + 每段独立可配。
- 档位：**吸收**。第一步：定默认值实验（预算比对：1k/4k/8k map 预算下检索命中率）。
- 许可证/成熟度：实践综合。
- 链接：https://aider.chat/docs/repomap.html

## 2. 2025–2026 前沿信号
- **图接地成为主流**：RepoGraph（2024→2025 大量引用）、CodexGraph、SWE-ContextBench（2026）把"代理上下文"从纯文本 RAG 推向符号图+迭代查询；与 ADV 的 adv-index 符号图路线同向。
- **上下文工程独立成域**：三层压缩（micro/full/session-memory）、工具结果优先逐出、真实 tokenizer 计数成为 2025–2026 代理栈标配考据点。
- **评测转向粒度与上下文**：SWE-ContextBench / SWE Atlas（Codebase Q&A）出现，"什么检索粒度对代理有效"开始有专门量具。
- **本地栈收敛**：llama.cpp 后端 + GGUF + OpenAI 兼容 API 已是事实标准；小模型定位收敛于补全/解释层（2026-04 社区口径）。
- **验证闭环的仲裁权持续归确定性程序**：SWE-agent lint-revert、aider auto-test、Self-Debug 消融三个独立来源都把"通过判定"交给测试/lint 进程而非 LLM 自评——与 wave-0 的 IRIS 边界互证。

## 3. Top-3
1. **aider repo-map**（吸收 → adv-index）：树摘要+图排序+预算选文件的完整算法，零 LLM 参与即符合 IRIS 边界，是 ADV repo 摘要 MCP 工具的最短移植路径。
2. **SWE-agent ACI 四原则**（吸收 → adv-server）：guardrails / 紧凑输出 / 高效导航 / 即时反馈，直接当 MCP 工具面验收判据。
3. **RepoGraph 仓库级代码图**（吸收 → adv-index×adv-server 桥）：静态图接地代理定位，省轮次省 token，且"静态引擎产图、代理只消费"与现有边界设计同构。
