# 04 · MCP 生态与 Agent 工具面设计（2025–2026 现状调研）

> 调研日期：2026-10-03 · 调研人：深度外部调研子任务（为《重写蓝图》供弹药）
> 方法：WebSearch + WebFetch 官方 spec / SDK 仓库 / 官方博客 / 实测基准文章。查不到的标「待验证」。
> 纪律：数字带锚（日期/版本/压测口径）；结论带适用范围；区分「营销话术」与「机制」。

---

## 0. 一页结论（先看这里）

| 问题 | 结论（判据见正文 §3） |
|---|---|
| ① rmcp 还是手写 stdio JSON-RPC？ | 不是二选一：**stdio 主路径自研（薄）、协议一致性用官方 conformance 套件验收、HTTP 形态或 spec 快跟需求出现时评估 rmcp 3.x 做壳**。2026-07-28 spec 转无状态后，手写成本显著下降（无握手、无会话）。性能不是决策变量——rmcp 修掉 SSE 硬编码后已达 4,845 RPS/10.9MB，手写在本地 stdio 场景拿不到有意义的边际收益。 |
| ② 工具面怎么设计？ | **三层渐进披露做成 server 端一等公民**：L0 轻目录（name+一句话+annotations）→ L1 按需 schema → L2 调用；输出侧用 outputSchema/structuredContent 让结果可被脚本消费。Anthropic 官方数据：defer_loading 方案省 85% token、准确率反升（Opus 4: 49%→74%）。 |
| ③ spec 新能力第一梯队？ | **annotations、structured tool output、server/discover** 三样（成本≈0、收益直接命中 token 痛点）。**绝不采用 sampling/roots**（2026-07-28 已 deprecated）。tasks/elicitation/MRTR 二梯队。 |

---

## 1. MCP 官方 Spec 演进

### 1.1 Spec 2025-06-18（当前多数 SDK 的稳定基线）

**一行定位**：MCP 第三个正式版，一次"安全与表达力"升级——elicitation、structured tool output、OAuth 资源服务器化都在这版落地。

**关键内容（机制级）**：
- **Elicitation**：`elicitation/create` 允许 server 在工具执行流程中向用户要结构化输入（message + requestedSchema）。
- **Structured tool output**：工具可带 `outputSchema`，返回 `structuredContent`；向后兼容要求同时给一份序列化 JSON 的 TextContent。spec 明确：有 outputSchema 时 server MUST 提供符合 schema 的结构化结果，client SHOULD 校验。
- **OAuth 硬化**：要求 RFC 8707 Resource Indicators；server 明确可作为 OAuth 资源服务器（token 校验职责）。（细节字段级内容待验证，见 [Cisco 博客] 链接）
- **JSON-RPC batching 移除**（对旧 agent 有破坏性）。
- **工具注解（annotations）**：`readOnlyHint / destructiveHint / idempotentHint / openWorldHint`（+ `title`）。语义为**提示而非保证**；spec 原文要求 client「MUST 视 annotations 为不可信，除非来自可信 server」——即注解是权限/确认 UI 的决策输入，不是安全边界。

**值得抄的机制**：
1. `outputSchema` + `structuredContent` 双通道输出（人读 text + 机器读 JSON）——本项目 89 个扫描类工具的输出治理地基。
2. annotations 四 hint 作为工具元数据必填项（进 L0 轻目录，成本≈0）。
3. `tools` capability 里 `listChanged` + 通知机制——目录变更时主动推。

**档位【吸收】**：这是 stdio 本地场景的主工作基线，全盘实现成本低且已是事实标准。
**第一步动作**：对照 spec 页给 89 工具补 outputSchema 与四 annotation，作为新项目工具定义格式的输入。
**许可证**：开放规范（spec 仓库 MIT；文档页许可待验证——内容本身可引用/实现，不受限）。
**链接**：https://modelcontextprotocol.io/specification/2025-06-18 · https://modelcontextprotocol.io/specification/2025-06-18/server/tools · [changelog](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-06-18/changelog.mdx)

### 1.2 Spec 2026-07-28（已正式发布的无状态大版本）

**一行定位**：MCP 从"双向有状态"翻转为"请求自描述的无状态协议"，配 Tasks 转正、MRTR 替代服务端发起请求、缓存语义、扩展框架——2026 年最重要的协议事实。

**关键内容（机制级，来自官方发布公告，2026-07-28 发布）**：
- **无状态核心**：`initialize/initialized` 握手与 `Mcp-Session-Id` 头退役（SEP-2575/2567）；每个请求自带协议版本、client 身份与能力（`_meta`）。任意请求可落在任意实例上，无需共享存储。
- **`server/discover`**：新的可选 RPC，client 一次性获知 server 能力。
- **MRTR（Multi Round-Trip Requests，SEP-2322）**：替代服务端发起的 `elicitation/create`、`sampling/createMessage`、`roots/list`——server 返回 `resultType: 'input_required'`，client 带 `inputResponses` 重试。
- **Tasks 转正为扩展**：`io.modelcontextprotocol/tasks`（SEP-2663），轮询式 `tasks/get` + 新 `tasks/update`；通知从 HTTP GET 端点改为单一 `subscriptions/listen` 流（按类型订阅）。
- **缓存语义（SEP-2549）**：`tools/list` 等列表响应携带 `ttlMs` 与 `cacheScope`。
- **废弃（SEP-2577）**：Roots、Sampling、Logging 至少保留 12 个月但**新实现不应采用**；HTTP+SSE 旧传输一年退场窗口；DCR 正式废弃，改用 Client ID Metadata Documents（CIMD）。
- **授权**：RFC 9207 issuer 校验（SEP-2468）；Streamable HTTP 请求须带 `Mcp-Method`、`Mcp-Name` 头（SEP-2243）。
- **扩展框架**：Tasks、MCP Apps、Enterprise Managed Authorization（EMA）作为扩展正式化。
- 采用面（官方口径）：约 5 亿月 SDK 下载；TS/Python SDK 各破 10 亿总下载；AWS Bedrock AgentCore / Cloudflare / Google Cloud / Microsoft Foundry 表态支持。⚠️ 这部分是生态热度叙事（营销面），机制面以上述协议变更为准。

**值得抄的机制**：
1. **无状态 + 请求自描述**的设计取向——对"零依赖手写"是重大利好：stdio 本地场景本就少用会话/OAuth，无状态把协议壳削到最薄。
2. `ttlMs/cacheScope` 的缓存注记——本项目 `tools/list` 目录可原样受益。
3. Extensions 命名空间（`io.modelcontextprotocol/tasks` 式反域名）——本项目自有扩展（如工具面分层加载）可按此格式声明，避免污染核心。

**档位【有界】**：机制全部认可，但 Tasks/MRTR/MCP Apps 不必首发实现——本地 CLI 形态用不上大半；**deprecated 的 sampling/roots 明确不抄**。
**第一步动作**：按 2026-07-28 的工具 schema 写新项目的工具定义；把「无状态、请求自描述」定为实现约束；给 `tools/list` 加 `ttlMs`。
**许可证**：开放规范（同上）。
**链接**：https://blog.modelcontextprotocol.io/posts/2026-07-28 · https://modelcontextprotocol.io/specification/2026-07-28

### 1.3 Spec 2025-11-25（中间版本，内容未查证）

**一行定位**：rmcp 3.x 声称兼容的中间 spec 版本，介于 2025-06-18 与 2026-07-28 之间。**内容待验证**（本次调研未展开，WorkOS 提到 URL-mode elicitation 可能在此版）。第一步动作：定协议壳策略时补查一次 changelog。

---

## 2. Rust 实现与性能证据

### 2.1 rmcp（官方 Rust SDK，modelcontextprotocol/rust-sdk）

**一行定位**：MCP 官方 Rust SDK，2026-07-28 随 spec 发出 RMCP 3.0.0，社区维护、性能顶级但"极简主义"，文档假定你懂 Rust async 与 MCP 内部。

**机制与事实**：
- 版本轨迹：0.16 → 0.17.0（2026-02-27，合入 PR #683）→ 2.x → **3.0.0（2026-07-28）**；3.x 兼容 2025-11-25 及更早 spec。
- 传输：stdio、Streamable HTTP（`StreamableHttpServerConfig`，0.17.0 起支持 `json_response: bool`）、SSE。宏：`#[tool]` / `#[tool_router]` / `#[tool_handler]` + `ServiceExt`（宏名来自 README/社区文章，字段级签名待验证）。
- 实测性能（tmdevlab 基准 v2，口径：k6、50 VU、5 分钟、Streamable HTTP 无状态、3 轮 CV<2%）：修掉 0.16 硬编码 SSE（每请求 +40ms 传输开销）后 **4,845 RPS / 平均 5.09ms / P95 10.99ms / RAM 10.9MB**，为 15 个实现中 RPS 第一。
- 弱点（社区面）：被认为"过于极简"、OAuth 2.1 授权仍在追赶 2026 spec；有团队因此转第三方 SDK。

**值得抄的机制**：
1. 宏驱动的工具注册（`#[tool]` 描述即 schema）——自研时的 DX 参照，但注意它把 schema 生成绑进编译期。
2. PR #683 教训：**传输层不要对 Content-Type 做硬性假设**，响应格式（SSE vs 单行 JSON）应是配置——直接写进自研传输层的设计原则。
3. 把它当**conformance 对照物**而非依赖：其行为是官方语义的 Rust 参考实现。

**档位【有界】**：官方性 + 性能 + spec 跟进速度都成立；但重依赖栈（tokio/serde/HTTP 全家桶）与"零依赖"目标冲突，且本项目差异化在工具面引擎而非协议壳。判据触发才升级为依赖（见 §3.1）。
**第一步动作**：clone 仓库跑通 stdio 与 streamable-http 两个 example；抓官方 MCP conformance 套件清单，作为自研实现的验收门。
**许可证**：MIT OR Apache-2.0 双许可（仓库声明，待验证——双许可即允许闭源组合）。
**链接**：https://github.com/modelcontextprotocol/rust-sdk · https://github.com/modelcontextprotocol/rust-sdk/discussions/969

### 2.2 rust-mcp-stack/rust-mcp-sdk（第三方全功能 SDK）

**一行定位**：rust-mcp-stack 的异步 SDK/框架，声称**完全实现 2026-07-28 无状态协议并通过 100% 官方 conformance 测试（110/110 server、440/440 client）**——比 rmcp 更"电池全带"。

**值得抄的机制**：
1. **conformance 通过率当发布门**的工程实践——新项目应把「过官方 conformance 套件」写成 CI 里的一等门禁。
2. `rust-mcp-schema` 把 schema 对象类型化——自研时 Rust 类型即协议类型，避免手拼 JSON。

**档位【有界】**：作为依赖的优先级低于 rmcp（官方性弱、生态小）；其 conformance 跑法与 schema 拆包思路照抄。
**第一步动作**：读其 conformance 测试跑法（怎么接官方套件），抄进新项目 CI 设计。
**许可证**：开源（crate 在 crates.io 发布；具体许可待验证，读仓库 LICENSE 确认）。
**链接**：https://github.com/rust-mcp-stack/rust-mcp-sdk · https://crates.io/crates/rust-mcp-sdk

### 2.3 tmdevlab MCP Server 性能基准 v2（实证锚点）

**一行定位**：15 个实现、3990 万请求、3 轮零错误的跨语言压测——本项目"性能上限"讨论的**可复现外部锚**。

**关键数字（口径：I/O 负载 = Redis 7 + Go HTTP API（10 万商品），k6 50 VU，Streamable HTTP 无状态）**：
| 实现 | RPS | 均延迟 | RAM |
|---|---|---|---|
| Rust (rmcp) | 4,845 | 5.09ms | 10.9 MB |
| Quarkus | 4,739 | 4.04ms | 194.5 MB |
| Go | 3,616 | 6.87ms | 23.9 MB |
| Bun | 876 | 48.5ms | 540.8 MB |
| Python (FastMCP) | 259 | 251.6ms | 258.6 MB |

- 适用范围：**I/O 型工具负载**，不代表 CPU 密集扫描（本项目主场景）的结论；Python 瓶颈被归因于 FastMCP 会话开销而非 ASGI。
- 附带发现：native 镜像省 RAM 27–81% 但掉 20–36% RPS；Spring MVC 阻塞式在 50 VU 反超 WebFlux。

**值得抄的机制**：方法论本身（3 轮 + CV 报告 + 固定工作负载 + 上游 bug 回馈）——新项目性能门照此设计。

**档位【吸收（方法论）】**。
**第一步动作**：新项目定型后用同款 k6 脚本跑一次自测，落盘为基线（先量后改）。
**许可证**：博客文章（专有），方法与数字引用注明出处即可。
**链接**：https://www.tmdevlab.com/mcp-server-performance-benchmark-v2.html

> 注：未逐一调研更多 Rust MCP server 标杆实现（如 mcp-rs 社区 SDK 现状为「活跃维护，2026 初」二手信息）。**待验证**——定协议壳策略时若需要第三个参照物再补查。

---

## 3. 工具面设计与 token 经济（本项目的核心差异化区）

### 3.1 Anthropic「Advanced Tool Use」（tool search + PTC + examples）

**一行定位**：Anthropic 2025-11-20 beta（GA 说法来自社区，待验证）的三件套，直接定义了 2026 年"大工具面省 token"的官方答案。

**机制与数字（官方工程博客口径）**：
- **Tool Search Tool**：工具定义标 `defer_loading: true` 后不进上下文，模型只见一个 ≈500 token 的搜索工具；用 regex/BM25/自定义检索按需展开 3–5 个相关定义（≈3K token）。对比：50+ MCP 工具全量 ≈72K token → 总消耗 ≈8.7K token（官方称省 85%、保留 95% 上下文）。**准确率不降反升**：Opus 4 从 49%→74%，Opus 4.5 从 79.5%→88.1%（同套 MCP 评测）。建议阈值：定义总量 >10K token 或工具数 >10 时启用。prompt caching 不受影响（deferred 工具不在初始 prompt 里）。
- **Programmatic Tool Calling (PTC)**：模型写 Python 在托管沙箱里编排工具（工具经 `allowed_callers: ["code_execution_20250825"]` 选择性开放），中间结果不回灌上下文。官方数字：token 43,588→27,297（−37%，复杂研究任务）；一次代码块编排 20+ 调用省 19+ 轮推理。适用判据：3+ 个有依赖的调用、大中间结果聚合；单工具简单调用不值得。
- **Tool Use Examples**：`input_examples` 附样例调用，教 JSON schema 表达不了的约定。复杂参数处理准确率 72%→90%。

**值得抄的机制**：
1. **defer_loading 语义的 server 端等价物**：既然客户端 tool search 由 client 决定，server 应把目录做成"一层薄索引 + 按需展开"，两种形态都能喂。
2. 输出经脚本消费而非回灌（PTC 的本质是**工具结果的数据流设计**）——structuredContent/outputSchema 正是为它铺路。
3. `input_examples` 思想进工具定义格式（1–5 个真实样例，含最小/部分/完整三种）。

**档位【吸收（机制）/不吸收（托管沙箱）】**：沙箱 PTC 是 API 厂商能力，本地项目抄的是"输出数据流"思想。
**第一步动作**：把新项目工具定义格式定成：`name/title/one-liner/annotations`（L0）+ `inputSchema/outputSchema/input_examples`（L1）。
**许可证**：Anthropic 文档与博客专有——只抄思想，不抄文字。
**链接**：https://www.anthropic.com/engineering/advanced-tool-use · https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool

### 3.2 Claude Code / Agent SDK 的工具面管理

**一行定位**：官方 agent 形态对"几百上千工具"的答案同样是 tool search（按需动态发现加载），并叠加 profiles/allowlist 类机制。

**机制**：Agent SDK 提供 tool search 集成（官方文档《Scale to many tools with tool search》）；社区实测面：把一个 MCP server 从 85 工具裁到 9 个、以 tool search 模式重组织后省 69–85% 工具描述 token（社区样本，单点，非稳态结论）。Claude Code 另有 MCP 工具 allow/disable 配置面（具体参数名待验证）。

**值得抄的机制**：
1. 「裁剪 + 搜索」两条腿并行——unified-rx 的 profile 是 opt-in，新项目应**默认薄目录**，profile 变成"预设的 L0 子集"而非开关。
2. 域（domain）划分与工具名前缀天然对齐命名空间。

**档位【吸收】**。
**第一步动作**：profile 设计从"opt-in 开关"改为"默认 L0 索引 + 显式展开"，保留旧 profile 作为预设包。
**许可证**：文档专有，抄机制。
**链接**：https://code.claude.com/docs/en/agent-sdk/tool-search

### 3.3 Synaptic Labs「Meta-Tool Pattern / Bounded Context Packs」

**一行定位**：只注册 2 个元工具（list-packs / load-pack），把大工具面组织成"上下文包"，按需展开——客户端无关的渐进披露方案。

**值得抄的机制**：
1. **元工具对**（`discover` + `invoke`）模式：对不支持 tool search 的客户端零成本兼容——这正是 unified-rx profile 的"可移植化"。
2. 包边界 = token 预算单元：每个 pack 自带尺寸声明。

**档位【吸收】**：与 server 端三层设计完全同构，直接采纳为默认交互形态之一。
**第一步动作**：新工具面 API 草案里加 `adv.discover(packs)` / `adv.load(pack_id)` 一对元工具。
**许可证**：博客专有，抄思想。
**链接**：https://blog.synapticlabs.ai/bounded-context-packs-meta-tool-pattern

### 3.4 claude-code-router《Progressive Disclosure of Agent Tools》（CLI 视角批评）

**一行定位**：从 CLI 工具哲学批评 MCP「一函数一工具」——CLI 用 flag/子命令组合表达力，MCP 用 N 个 schema 烧 N 份 token。

**值得抄的机制**：
1. **合并粗粒度工具 + 参数区分动作**（如 `adv.scan(action=...)`）作为 L0 兜底形态——89 个工具不必是 89 个 MCP tool。
2. 粗粒度化的代价（schema 复杂、校验弱化）要点名：与 structured output/outputSchema 组合可部分抵消。

**档位【有界】**：方向对但不走极端——高频细工具保留独立 schema（模型对扁平 schema 的调用准确率仍更高），低频长尾并入 action 参数。
**第一步动作**：盘点 89 工具，按调用频次/域分出「保留独立 / 合并 action / 降级为 L0-only」三档（判据：月调用频次 + schema 尺寸）。
**许可证**：开源项目博文（MIT 项目；博文许可未标，引述即可）。
**链接**：https://github.com/musistudio/claude-code-router/blob/main/blog/en/progressive-disclosure-of-agent-tools-from-the-perspective-of-cli-tool-style.md

---

## 4. MCP 安全

### 4.1 Invariant Labs：tool poisoning / mcp-scan / toxic flows

**一行定位**：MCP 攻击面命名的源头团队——tool poisoning（工具描述里藏恶意指令）、tool shadowing（跨 server 同名遮蔽）、rug pull（装完改行为）、toxic flows（两条无害数据流的组合泄漏）。

**机制**：
- tool poisoning：恶意指令藏在 `description` 等模型可见字段，**人看着无害、模型读了中毒**——防御不在"扫代码"而在"扫元数据"。
- mcp-scan：本地扫描配置与 server 元数据，检测上述模式（开源工具；许可待验证）。
- toxic flow：单工具无害、A→B 组合泄密（如 GitHub MCP 私仓越权读取案例）——威胁模型必须看**工具间的数据流**，不是单个工具。

**值得抄的机制**：
1. 自带一份 `security-scan` 式自检：对自家 89 工具的 description 做注入模式自审（本项目已有 attack_cruise/breaker，天然适配）。
2. 工具输出标注 `audience`（spec 的 content annotations）——区分「给人看」与「给模型看」，是断 toxic flow 的第一刀。
3. annotations 不可信原则（spec 原文）写进 server 文档：hint 是 UX 输入不是安全边界。

**档位【吸收】**。
**第一步动作**：把「工具元数据注入自检」加进新项目的发布门；输出 content 默认带 audience 注解。
**许可证**：博客专有（抄思想）；mcp-scan 工具开源（许可待验证）。
**链接**：https://invariantlabs.ai/blog/mcp-security-notification-tool-poisoning-attacks · https://invariantlabs.ai/blog/introducing-mcp-scan · https://invariantlabs.ai/blog/toxic-flow-analysis

### 4.2 Microsoft：MCP 间接注入缓解指南

**一行定位**：平台厂商的系统化缓解清单——把 prompt injection 当作输入消毒与权限边界问题处理。
**值得抄的机制**：最小权限工具面（只暴露需要的工具）+ 输出走校验层 + 人审敏感操作——与本项目"默认薄目录 + annotations 驱动确认"同向。
**档位【吸收（原则）】**。第一步动作：权限模型文档化时引用该清单做对照。
**许可证**：专有，抄原则。链接：https://developer.microsoft.com/blog/protecting-against-indirect-injection-attacks-mcp

---

## 5. MCP Registry

**一行定位**：官方注册表 2025-09-08 preview 上线，REST API 于 2025-10-24 冻结 v0.1；`server.json` 描述符 + 命名空间规则；生态 10,000+ server（官方/社区口径，单点统计）。

**值得抄的机制**：
1. `server.json` 描述符格式——新项目发布时按此格式产出，一次发布多端可达。
2. 命名空间规则（防抢注）——新工具名前缀（如 `adv.` / `urx.`）要现在定死，与 server 名一致。

**档位【有界】**：本地工具不依赖 registry 分发；发布面采纳。
**第一步动作**：写 `server.json` 生成脚本，纳入构建产物。
**许可证**：registry 代码开源（MIT，待验证）；schema 开放。
**链接**：https://blog.modelcontextprotocol.io/posts/2025-09-08-mcp-registry-preview · https://github.com/modelcontextprotocol/registry

---

## 6. 2025–2026 前沿信号

1. **协议重心从"有状态双向"翻到"无状态请求自描述"**（2026-07-28 正式发布，SEP-2575/2567）——手写协议壳的成本结构被根本改变； Roots/Sampling/Logging 进废弃通道，**新实现采纳即负资产**。
2. **Tasks 从实验转正为扩展**（`io.modelcontextprotocol/tasks`，轮询式）——异步长任务的官方答案落地，但走扩展而非核心：跟随成本可控、也可先自研 job 抽象。
3. **客户端侧 token 经济已产品化**：tool search（−85% token、准确率反升 25 个百分点）与 PTC（−37%）从 Anthropic API 层给出答案——server 端的工具面设计必须主动配合（薄目录、structuredContent），否则红利拿不到。
4. **Rust 实现性能天花板被官方 SDK 自己摸到**：rmcp 0.17.0 修复后 4,845 RPS / 10.9MB（I/O 负载），"手写更快"的空间主要剩 CPU 密集路径——而那部分本来就在工具实现里，与协议壳无关。
5. **conformance 测试成为 SDK 竞争货币**（rust-mcp-stack 拿 110/110 当卖点）——自研实现的验收门现成可用。
6. **安全威胁模型已标准化**（poisoning/shadowing/rug pull/toxic flow），Microsoft 等平台方给出缓解基线——工具元数据是攻击面，发布门要扫它。
7. **生态规模叙事**（≈5 亿月 SDK 下载、10K+ server、registry v0.1）是营销面信号，机制面才是立项依据；但它确认 MCP 不会是短期风尚，协议跟进值得投资。
8. **待验证清单**：2025-11-25 spec 内容；rmcp 宏签名细节与许可证声明；rust-mcp-stack 许可证；URL-mode elicitation 归属版本；Anthropic 三件套 GA 确切时间。

---

## 7. 三个关键问题的判据（供蓝图引用）

### 7.1 rmcp vs 手写 stdio JSON-RPC

不站队，给判据。两条路径的**依赖两端**要点名：手写 = 「本项目工具面引擎」对「协议语义」的完全控制权；rmcp = 「本项目发布节奏」对「官方 spec 跟进速度」的依赖。

| 判据 | 走手写 | 走 rmcp |
|---|---|---|
| spec 覆盖目标 | 只做 stdio + tools/prompts/resources 核心（无 OAuth/远程） | 需要远程 Streamable HTTP + OAuth + tasks 等 2026 能力首发 |
| 差异化所在 | 工具面引擎（分层加载/搜索/元数据）是产品本体 | 协议合规本身是产品卖点 |
| 依赖预算 | 零依赖是硬指标（审计面/供应链） | 接受 tokio/serde/http 栈 |
| 验收方式 | 官方 conformance 套件过门（抄 rust-mcp-stack 跑法） | SDK 自带语义 |
| 性能证据 | stdio 本地场景延迟在 IPC 而非协议层，bench 不构成变量 | 同左；HTTP 形态有 4.8K RPS 现成锚 |

**建议形态（有界混合）**：stdio 主路径自研（薄、无状态取向、按 2026-07-28 语义），conformance 套件当验收门；当「远程 HTTP 形态」成为需求时再评估 rmcp 3.x 做传输壳——用判据触发，不用信仰。适用范围：本地 CLI/单机工具服务器；若产品形态转向远程多租户服务，判据整体重算。

### 7.2 工具面三层设计（把 unified-rx 的 opt-in profile 变成默认机制）

- **L0 轻目录**：每工具 `name/title/一句话/四 annotation/pack 归属`，目标 ≤10 token/工具 → 89 工具 ≈1K token 冷启动（锚：Anthropic 搜索工具本体 ≈500 token；本目标为设计目标，非实测）。支持 `ttlMs` 缓存注记。
- **L1 按需展开**：元工具对 `discover/load`（Synaptic 模式）+ 兼容客户端 tool search 的 defer 语义；schema 拆 required-first。
- **L2 调用与输出**：outputSchema/structuredContent 双通道 + `input_examples`；大输出走 resource 引用/分页而非回灌。
- **粗细粒度规则**：高频细工具保独立 schema；低频长尾并入 action 参数；判据 = 调用频次 × schema 尺寸。
- **安全默认**：description 进发布门扫描；audience 注解；annotations 只做 UX 输入。

### 7.3 spec 能力梯队

| 梯队 | 能力 | 价值 | 成本 | 备注 |
|---|---|---|---|---|
| 一 | tool annotations | 权限/确认决策输入，进 L0 目录 | ≈0 | spec：client 须视为不可信 |
| 一 | structured tool output | 省 token 的数据流地基 | 低 | 2025-06-18 已稳定 |
| 一 | server/discover | 一次请求报能力，替代握手 | 低 | 2026-07-28 |
| 二 | tasks extension | 扫描类长任务官方语义 | 中 | 先自研 job 抽象，扩展稳定后对齐 |
| 二 | ttlMs/cacheScope | 目录缓存 | 低 | 顺手实现 |
| 二 | MRTR/elicitation | 本地用户在场场景交互 | 中 | 旧 elicitation/create 更简单 |
| 观望 | MCP Apps / EMA | CLI 形态用不上 | 高 | 不做 |
| **禁用** | sampling / roots / logging | 已 deprecated（SEP-2577） | — | 新实现不采纳 |

---

## 8. Top-3

1. **协议壳策略定成"判据触发的混合制"**：stdio 主路径自研（按 2026-07-28 无状态语义，成本已降到历史最低），官方 conformance 套件当 CI 验收门（抄 rust-mcp-stack 跑法），远程 HTTP + OAuth 需求出现时才评估 rmcp 3.x 做壳。
2. **工具面三层渐进披露是一等公民，不是配置项**：L0 轻目录默认（89 工具 ≈1K token 目标）+ 元工具对按需展开 + outputSchema/structuredContent 输出通道——unified-rx 的 opt-in profile 直接升格为默认机制，对齐 Anthropic tool search（−85% token、准确率反升）的 server 端形态。
3. **spec 能力只进三样 + 一条禁令**：annotations、structured output、server/discover 首发即做；sampling/roots/logging（2026-07-28 已废弃）明确永不采纳；tasks 走二梯队对齐 `io.modelcontextprotocol/tasks` 扩展。
