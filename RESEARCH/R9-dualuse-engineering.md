# R9 · 红队/双用途工具工程架构与防御对照（只读思想）

> 域：R9（ADV 扩展调研 Wave 1 · 逆向/二进制/安全工程专线）
> 日期：2026-10-03 ｜ 产出：本文件 + `RESEARCH/registry/R9.jsonl`（27 条，全部登记）
> **定位（红线）**：只读思想。只分析公开的红队/双用途工具的**工程架构**与防御方文献，服务两个目的——
> （a）ADV 工程质量借鉴；（b）防御对照（威胁模型、供应链扫描规则反向启示）。
> 不写攻击教程；不提供利用/规避实现细节；涉及"可操作攻击能力"的内容一律止步于架构描述与**可检测特征**层面（边界见 §6）。
> 与相邻域的边界：ATT&CK/YARA/Sigma 规则生态在 R5 已部分覆盖，网络检测信号（DNS/JA3）在 R8 有邻接；本域聚焦"框架工程架构"与"防御对照转化"，重叠处按本域问题清单（§3.4）收口。

**档位图例**：【吸收】机制进 ADV 计划且有集成目标 ｜【有界】只吸收数据模型/架构思想，用途与实现不吸收 ｜【不吸收】明确排除 ｜【watch】暂不集成，留观察（含超出 ADV 当前能力面） ｜【reference】只作工程/防御对照，不进路线图。

---

## 0. 总览表（27 对象）

| # | 对象 | 类别 | 档位 | 集成目标 | 成熟度（2026-10-03 核） |
|---|------|------|------|----------|--------------------------|
| 1 | Mythic | C2 编排 | 有界 | adv-sandbox | 维护中（4.x，push 2026-09-27） |
| 2 | Sliver | C2 | reference | reference | 维护中（push 2026-10-02） |
| 3 | Havoc | C2 | reference | reference | **弃维护**（仓库 archived=True，末次 push 2025-12-18） |
| 4 | Metasploit Framework | 后渗透框架 | reference | reference | 维护中（push 2026-10-02） |
| 5 | Cobalt Strike | 商业 C2 | reference | reference | 专有·商业维护（仅防御文献） |
| 6 | Empire | 后渗透框架 | watch | watch | 维护中（低强度，push 2026-09-08） |
| 7 | Caldera | 对抗模拟 | 有界 | adv-rules | 维护中（Apache-2.0，push 2026-08-27） |
| 8 | Atomic Red Team | 测试库 | 有界 | adv-rules | 维护中（MIT，push 2026-09-28） |
| 9 | ATT&CK STIX 2.1 / TAXII 2.1 | 数据集 | 有界 | adv-rules | 维护中（版本化发布，push 2026-08-05） |
| 10 | Nuclei | 模板扫描器 | 有界 | adv-rules | 维护中（MIT，push 2026-10-02） |
| 11 | XZ Utils backdoor（CVE-2024-3094） | 供应链案例 | **吸收** | adv-sca | 案例（2024-03 披露，无在野确认利用） |
| 12 | SolarWinds SUNSPOT | 供应链案例 | 有界 | adv-sca | 案例（2020-12 披露） |
| 13 | SLSA / in-toto / Sigstore | 证明框架 | reference | docs-process | 维护中（标准与工具链活跃） |
| 14 | GoPhish | 钓鱼演练 | reference | reference | 维护中（低活跃，末次 push 2024-09-23） |
| 15 | Evilginx2 | 反代钓鱼 | reference | reference | 维护中（BSD-3-Clause，push 2026-06-10） |
| 16 | dnscat2（DNS 隧道代表） | 隐蔽传输 | reference | reference | 维护中（低活跃，末次 push 2024-03-14） |
| 17 | Malleable C2 profile 概念 | 规避概念 | reference | reference | 概念（防御文献充分） |
| 18 | Domain fronting（T1090.004） | 规避概念 | reference | reference | 概念（MITRE ATT&CK 公开条目） |
| 19 | JA3/JA4/JARM + c2detect | 指纹工程 | watch | watch | 维护中（c2detect 2025 活跃） |
| 20 | BOF / COFF loader | 扩展 ABI | reference | reference | 维护中（TrustedSec，push 2026-09-28） |
| 21 | Sliver Armory | 扩展分发 | watch | watch | 维护中（无 LICENSE 标注） |
| 22 | BloodHound CE | 攻击路径图 | reference | reference | 维护中（Apache-2.0，push 2026-10-02） |
| 23 | Impacket | 协议库 | reference | reference | 维护中（Apache 修改版，push 2026-10-01） |
| 24 | Sigma + pySigma | 检测规则标准 | 有界 | adv-rules | 维护中（push 2026-10-02） |
| 25 | Chainsaw / Hayabusa | Sigma 引擎 | reference | reference | 维护中（GPL/AGPL，push 2026-10） |
| 26 | Velociraptor / osquery | 端点 agent | watch | watch | 维护中（AGPL / 双许可） |
| 27 | Eris（Mythic Android agent，2025-08 信号） | 移动 agent | watch | watch | 实验（链接待验证） |

> 机制级（deep）15 条：#1–5、7–12、19–20、22、24；其余 12 条为扫视（sweep）。许可证与活跃度均按 GitHub API 于 2026-10-03 实查（`archived`/`pushed_at` 字段）标注。

---

## 1. C2/后渗透框架：工程架构解剖（对象 #1–#6）

### 1.1 Mythic（SpecterOps/its-a-feature）—— 多 agent 编排参考架构
- **定位**：开源 C2 平台，核心卖点是把"agent（payload type）、C2 profile、翻译层"都做成**可插拔容器**，本体只当编排中枢（文档：docs.mythic-c2.net，2025 版 4.x）。
- **可抄机制**：
  1. **插件=容器 + 注册契约**：payload type / C2 profile / translation / webhook / logger / eventing 六类扩展均以独立服务容器加入，`Mythic/InstalledServices/<name>/` 落位，Python 侧统一 `mythic_container` 包、Go 侧统一 base 镜像 ⇒ 插件与核心只有一套契约，核心不感知插件内部。
  2. **总线分层**：组件间走 RabbitMQ（Postgres 存操作数据、Nginx 反代、Hasura GraphQL 查询），但 **translation 容器改用 gRPC** 以压缩往返延迟 ⇒"通用消息总线 + 关键路径低延迟旁路"的混合拓扑。
  3. **任务/回调数据模型**：operation → task → callback 三级；4.0 增加 `agent-initiated RPC`（agent 可主动发起请求）与**可恢复的字节偏移文件传输**（断点续传语义进任务层）。
- **档位**：【有界】——吸收"插件契约 + 消息总线 + 关键路径旁路"与任务队列语义到 ADV 插件沙箱/MCP 工具面的设计对照；不吸收其 C2 用途、agent 实现与载荷体系。
- **第一步动作**：读 `payload-type-development` 文档与一个最小 payload type 容器，抽出「注册元数据（builder 参数/C2 参数/命令清单）→ 任务 → 回调」字段表，写成 ADV 插件 manifest 的对照笔记（adv-sandbox 文档追加一节）。
- **许可证/成熟度**：BSD 3-Clause（LICENSE 全文含 "BSD 3-Clause License" 字样，2026-10-03 实读）｜【维护中】（push 2026-09-27，4.x 活跃）。
- **链接**：https://github.com/its-a-feature/Mythic ｜ 容器说明：https://docs.mythic-c2.net/installation/a-note-about-containers

### 1.2 Sliver（BishopFox）—— 传输抽象与统一封装
- **定位**：Go 写的跨平台对抗模拟框架；operator 走 gRPC+Protobuf（clientpb），植入体侧**不用 gRPC**，走自定义传输。
- **可抄机制**：
  1. **传输抽象层**：植入体侧 transport manager + 服务端 job-based listener；mTLS（Yamux 多路复用，preface `MUX/1`）、WireGuard（Noise 协议，UDP）、HTTP(S)、DNS（Base32/TXT 分片重组）可在消息层之下互换。
  2. **统一消息封装**：无论传输为何，消息一律包 `sliverpb.Envelope`（type ID + 序列化 protobuf）⇒ 新增传输不改应用层协议。会话密钥用 age 格式封装（植入体用内嵌公钥加密会话密钥），数据面 AES-256-GCM；每植入体独立非对称密钥 ⇒ 静态二进制签名不可复用（对检测方的启示：静态哈希检测会被"一植入一密钥"稀释）。
  3. **内置蓝队功能**：DNS 传输自带 **DNS canary**（刻意制造可被蓝队发现的特征）以及 WireGuard 用 EventBroker 动态更新 peer 配置 —— 红队框架主动提供"可被检测"旋钮，是防御方理解检测面的一手材料。
- **档位**：【reference】——架构分层（传输无关的消息封装、envelope 类型化）可作 ADV 多协议检索/多条 MCP 传输的工程对照；GPL-3.0 代码不内联、用途边界不吸收。
- **第一步动作**：画一张"Envelope(type id)→传输"分层图，与 ADV server 的消息/工具调用封装现状做差异清单（只记差异，不改代码）。
- **许可证/成熟度**：GPL-3.0 ｜【维护中】（push 2026-10-02；stars ≈11.9k）。
- **链接**：https://github.com/BishopFox/sliver

### 1.3 Havoc（C5pider）—— 三组件分层 + 模块/BOF 扩展（已归档）
- **定位**：Go teamserver（默认端口 40056、`.yaotl` profile）+ Qt/C++ 客户端 + C/ASM Demon agent 的三组件结构；Demon 流量 AES-256-CTR，支持 SMB pivoting；扩展面有 ExternalC2、模块与 **BOF**（见 #20）。
- **可抄机制**：
  1. 团队服务器/客户端/agent 权责切分清晰，profile 文件声明监听与流量外观（与 #17 同一概念谱系）。
  2. 扩展以 BOF/模块形式加载，"不换 agent 加能力"——插件 ABI 的一种轻量实现。
- **档位**：【reference】——**上游已归档（archived=True，末次 push 2025-12-18，2026-10-03 API 实查）⇒ 按弃维护处理**，只留工程对照价值，不进 watch 队列。
- **第一步动作**：仅在文档中记录"三组件分层 + profile 驱动"两个模式名，不投产评估。
- **许可证/成熟度**：GPL-3.0 ｜【弃维护】（仓库归档）。
- **链接**：https://github.com/HavocFramework/Havoc

### 1.4 Metasploit Framework（Rapid7）—— 模块注册表与组合式装配
- **定位**：Ruby 模块化渗透框架（仅拆工程层面：模块系统、注册机制、载荷架构；不涉具体利用代码）。
- **可抄机制**：
  1. **模块对象模型**：所有模块继承 `Msf::Module` 类型化基类；`Msf::ModuleManager` 管理 **module set = Hash（引用名→类）的懒加载工厂** ⇒ 大目录模块的注册与实例化解耦。
  2. **组合式装配**：payload 不是静态物，而是运行时把 stager+stage+handler **mixin 组合成匿名类**（`build_payload` 先 `Class.new(Payload)` 再 `include(*modules)`）；handler 亦以 mixin 注入（ReverseTcp 等）。Uberhandler 提案把 handler 从 payload 解耦为独立可追踪服务。
  3. **RPC 面**：msgpack/HTTP 的 RPC API 把"跑模块/管会话/导数据"暴露为远程自动化接口 —— 与 MCP 工具面同构的"外部编排入口"。
- **档位**：【reference】——"Hash 工厂 + mixin 组合 + 外部 RPC"三点是通用装配模式，ADV 插件/规则注册可对照；不吸收任何利用模块与载荷实现。
- **第一步动作**：把 ADV 现有插件/规则注册表与"hash 工厂 + 惰性实例化"逐项对照，记一行结论（是否已有等价物）。
- **许可证/成熟度**：Rapid7 自定义 BSD 类（GitHub NOASSERTION，LICENSE 为 Debian copyright 格式，2026-10-03 实读）｜【维护中】（push 2026-10-02，stars ≈39k）。
- **链接**：https://github.com/rapid7/metasploit-framework

### 1.5 Cobalt Strike（专有）—— 只取防御方分析面
- **定位**：商业红队平台；本文档仅引用**公开防御分析**：Malleable C2 profile（#17）、Beacon 配置 blob（XOR 混淆，2025 年 TrustedSec 有运行时改写研究）、DFR 延迟符号解析（BOF 声明 `LIBRARY$Function`，Beacon 侧上限 128 个函数，解析失败拒绝执行）。
- **可抄机制（防御侧）**：
  1. **配置提取器**作为取证范式：CobaltStrikeParser 类工具从内存/样本提取 beacon 配置 ⇒ 启发 ADV"扫描结论要能落成结构化证据（配置对象），不止一条命中"。
  2. **检测-规避竞争的时间线证据**：默认 URI/YARA 签名 → profile 整形使其失效；TLS 指纹 → 反代前置使 JARM 趋同（#19 有实测口径）⇒ 证明"静态特征检测的时效性衰减"，规则需带时效标签。
- **档位**：【reference】——专有软件，只读其公开防御文献；任何实现/规避细节不吸收。
- **第一步动作**：把"配置提取→结构化证据"列为 ADV 报告格式的一条对照项（docs-process）。
- **许可证/成熟度**：专有（商业授权）｜商业维护；防御方文献持续更新（如 ngCERT 2025-09 公告）。
- **链接**：https://www.cobaltstrike.com/product/features （只引公开页面）

### 1.6 Empire（BC-Security）—— PowerShell/.NET 后渗透框架
- **定位**：老牌 PowerShell/.NET 后渗透框架，stager + module 插件结构；仓库 BSD-3-Clause 仍活跃（push 2026-09-08），但生态热度显著低于 Sliver/Mythic。
- **可抄机制**：几乎没有需吸收的东西；价值在"框架生命周期"样本——红队工具生态更替快（对照 Havoc 归档），watch 即可。
- **档位**：【watch】——不集成的理由：ADV 无对应能力面；留作"工具生态生命周期"观察项。
- **第一步动作**：无（保持 watch）。
- **许可证/成熟度**：BSD-3-Clause ｜【维护中】（低强度）。
- **链接**：https://github.com/BC-Security/Empire

---

## 2. 对抗模拟编排与数据模型（对象 #7–#10，ADV 威胁映射层的直接输入）

### 2.1 MITRE Caldera —— ability/fact/planner 数据模型
- **定位**：MITRE 的自动化对手模拟平台：异步 C2 服务器（REST + Web UI），插件以独立仓库"挂"在核心上（Stockpile 提供 abilities/ adversaries/ planners；Sandcat/Manx/Human 为 agent 类型；Emu 模拟 APT；Debrief 出报告图）。
- **可抄机制**：
  1. **五对象数据模型**：`Ability`（挂 ATT&CK technique id 的原子能力，可含多平台执行器）→ `Adversary`（按 tactic 阶段编排的 ability 序列）→ `Operation`（一次运行实例）→ `Fact`（前序发现的结构化变量，供后续 ability 复用）→ `Planner`（如 Look Ahead 决策器）。这层"能力目录 + 事实传播 + 规划器解耦"可直接映射到 ADV 的威胁映射与检测编排。
  2. **插件不改核心**：新增 TTP 库/UI/agent 都是独立仓库注册进核心 ⇒ 与 Mythic 同构的插件治理模式（两条独立证据）。
  3. **运行报告（Debrief）**：操作 → 图结构可视化 ⇒ 覆盖度报告的呈现范式。
- **档位**：【有界】——吸收数据模型与编排解耦到 adv-rules 威胁映射层；不吸收其任何 TTP 执行内容（abilities 的具体命令）。
- **第一步动作**：抄一张 `Ability/Fact/Adversary` 字段表进 adv-rules 设计稿，并用 3–5 个 T1xxx 手工样例验证"规则→technique→覆盖报告"数据流。
- **许可证/成熟度**：Apache-2.0 ｜【维护中】（push 2026-08-27；stars ≈7.3k）。
- **链接**：https://github.com/mitre/caldera

### 2.2 Atomic Red Team（Red Canary）—— 测试库数据模型（检测即测试）
- **定位**：ATT&CK 对齐的原子测试库；重点在 **YAML spec 与校验工程**，不在测试内容。
- **可抄机制**：
  1. **数据模型**：`atomics/T1234/T1234.yaml`，顶层 `attack_technique`/`display_name`/`atomic_tests[]`；每个测试含 `name`、`auto_generated_guid`（CI 生成）、`description`、`supported_platforms`（windows/macos/linux/office-365/azure-ad/google-workspace/…）、`input_arguments`（**类型化**：Path/Url/String/Integer/Float + default）、`dependencies`（prereq_command / get_prereq_command）、`executor`（name/command/cleanup_command/elevation_required）。
  2. **校验工程**：仓库自带 `bin/validate_atomics.rb`，2025 年维护者又发布了独立 **validation schema** ⇒ "测试条目必须过机器校验才准入库"的 CI 门。
  3. **清理语义**：`cleanup_command`/`elevation_required` 显式声明 ⇒ 可重复、可回滚的测试执行约定。
- **档位**：【有界】——吸收"测试条目 schema + 机器校验 + 清理语义"到 adv-rules/test-replay 夹具设计；测试内容本身不吸收。
- **第一步动作**：拿 `spec.yaml` + validation schema 各读一遍，抽出 ADV 检测夹具应具备的最小字段集（id/平台/输入类型化/依赖/清理/证据字段）。
- **许可证/成熟度**：MIT ｜【维护中】（push 2026-09-28；stars ≈12.6k）。
- **链接**：https://github.com/redcanaryco/atomic-red-team/blob/master/atomic_red_team/spec.yaml

### 2.3 MITRE ATT&CK STIX 2.1 / TAXII 2.1 —— 威胁映射层的数据面
- **定位**：ATT&CK 官方分发即 **STIX 2.1 数据集**；`attack-pattern` 对象对应 technique，`intrusion-set --uses--> attack-pattern` 等 Relationship 组成知识图。
- **可抄机制**：
  1. **图模型**：SDO（attack-pattern/malware/tool/campaign/identity…）+ SCO（observable）+ SRO（relationship/sighting）分层；technique 的 parent/child（sub-technique）与 tactic 关联均为图边 ⇒ ADV 威胁映射层按图存储比平表更贴源。
  2. **分发协议**：TAXII 2.1（discovery → api root → collections，可增量拉取；企业版 collection id `95ecc380-afe9-11e4-9b6c-751b66dd541e`）；官方快照仓库 `attack-stix-data` 带版本化发布 ⇒ ADV 宜"冻结快照 + 版本锚 + 离线消费"，不依赖在线服务。
  3. **口径锚**：检索口径（2025 资料整理）企业版为 14 tactics / 691 techniques（216+475）/ 44 mitigations / 172 groups——落地时以选定快照的实际数字为准，不引用二手数字。
- **档位**：【有界】——吸收 STIX 快照消费与"technique 图"到 adv-rules；不吸收攻击语义内容（不做执行能力）。
- **第一步动作**：冻结 `attack-stix-data` 一个版本进测试夹具；写一个校验器验证"规则条目引用的 Txxxx"在快照中存在（引用完整性门）。
- **许可证/成熟度**：MITRE 免版税授权（LICENSE.txt："non-exclusive, royalty-free license… for research, development, and commercial" 2026-10-03 实读；GitHub 标 NOASSERTION）｜【维护中】（版本化发布，push 2026-08-05）。
- **链接**：https://github.com/mitre-attack/attack-stix-data ；TAXII 服务：https://cti-taxii.mitre.org/taxii2/

### 2.4 Nuclei（ProjectDiscovery）—— 模板引擎架构（规则 DSL 工程参数）
- **定位**：YAML 模板驱动的扫描引擎；本文档只收**引擎工程**，不收其模板库内容（dual-use 边界：模板库属"不吸收"面）。
- **可抄机制**：
  1. **模板结构**：`id` + `info`（severity/tags/classification/CVSS）+ 按协议的请求段；matchers（status/word/dsl/regex/json）+ extractors + variables（`{{randstr}}` 等运行时函数）+ payloads 组合策略（batteringram/pitchfork/clusterbomb 三种命名策略是通用笛卡尔爆炸问题的现成命名）。
  2. **协议接口抽象**：每个协议实现同一接口——`Compile`（编译请求生成器与匹配器）/`Requests`（数量）/`ExecuteWithResults`（带回调执行）/`Match`/`Extract`/`MakeResultEvent` ⇒ "编译-执行-事件"三段式，便于增量加协议。
  3. **性能工程**：**请求聚类**（相同请求合并去重）、并发与速率限流、工作流（模板输出喂下一模板）⇒ ADV 扫描引擎可对照的三项参数。
- **档位**：【有界】——吸收引擎架构/DSL 设计/性能参数到 adv-rules 规则 DSL 设计；模板库与任何请求内容不吸收。
- **第一步动作**：读 `DESIGN.md` 的协议接口定义，列出 ADV 规则执行器要复用的三个方法语义（编译/执行/结果事件），写进 adv-rules 设计稿。
- **许可证/成熟度**：MIT ｜【维护中】（push 2026-10-02；stars ≈31.7k）。
- **链接**：https://github.com/projectdiscovery/nuclei ；模板结构：https://docs.projectdiscovery.io/templates/structure

---

## 3. 供应链攻击：反向规则启示（对象 #11–#13）

### 3.1 XZ Utils backdoor（CVE-2024-3094）—— 检测规则启示清单（★ 吸收）
- **定位**：2024-03-29 由 Andres Freund 发现（线索：SSH 登录约 500ms 延迟）；CVSS 10.0；**至今无在野确认利用、未进 CISA KEV**（口径：公开报道与 KEV 目录，2026-10 复核）；影响 xz 5.6.0/5.6.1。
- **机制拆解（为什么它绕过常规审查）**：
  1. **发布物 ≠ 仓库**：恶意宏 `m4/build-to-host.m4` 只存在于 release tarball，git 仓库干净；
  2. **伪测试夹具藏载荷**：`bad-*.xz` 等二进制"测试文件"内封装加密的构建期载荷，`configure` 阶段解密执行；
  3. **条件门控**：仅 x86-64 + glibc + deb/rpm 打包路径触发（规避沙箱/CI/手工审查）；
  4. **符号级劫持**：经 IFUNC 解析把 `RSA_public_decrypt` 重定向（sshd→libsystemd→liblzma 传递依赖链）；
  5. **社会工程长线**：Jia Tan 角色 2021 起两年积累信誉、sockpuppet 施压、获得 release 权（MSR 2025 论文 *Wolves in the Repository* 有工程化定量分析）。
- **→ adv-sca 规则启示清单（9 条；标"静态"=纯源码/制品静态可判，"元数据"=需 SCA 元数据，"流程"=需流程证据）**：
  1. **发布物↔仓库一致性**：tarball 独有的构建脚本文件（如 `m4/*.m4`）、文件数/哈希差异 ⇒ 一致性检查项【静态】
  2. **构建脚本内嵌 blob**：autoconf/CMake/Makefile 中的长 base64/hex 常量、`xxd/dd/printf` 提取链、`xz -d` 解密流【静态】
  3. **伪二进制夹具**：扩展名非可执行但内容含 ELF/PE 魔数、熵异常（>7 bits/byte）或与声明用途不符的压缩流【静态】
  4. **构建期写回**：configure 阶段修改生成文件（Makefile 注入、`--wrap`/符号重定向、version-script 干预）【静态】
  5. **发行版条件分支**：构建脚本按 distro/打包系统（deb/rpm）分支且差异巨大【静态·启发】
  6. **维护权/发布权变更事件**：新增 co-maintainer、发布者变更、仓库控制权转移 ⇒ SCA 元数据风险信号【元数据】
  7. **贡献模式异常**：长期低速贡献后集中触碰构建/发布关键文件（统计信号，**只作加权不作结论**——单点不是判据）【元数据·统计】
  8. **CI/模糊测试干扰**：与 fuzzer 配置相关的"抑制/豁免"补丁，出现即提升审计级别【静态·启发】
  9. **可复现性缺口**：release 无法从公开源重建 ⇒ 风险等级上调（与 #12 共享此条）【流程】
- **档位**：【吸收】——规则清单进 adv-sca 路线；不吸收攻击构造细节（载荷实现、宏的完整逻辑）。
- **第一步动作**：从 9 条中挑 **4–5 条纯静态可判**（1/2/3/4/9）写成 adv-sca 检测项草案，每条带"命中即报告 + 需人工复核"的表述（避免绝对化）。
- **许可证/成熟度**：案例研究（无代码吸收；引用 NVD/MSR 论文）｜事件为 2024 年，**结论有时效性**（"至今无在野利用"是 2026-10 口观察）。
- **链接**：https://nvd.nist.gov/vuln/detail/CVE-2024-3094 ；MSR 2025 论文："Wolves in the Repository: A Software Engineering Analysis of the XZ Utils Supply Chain Attack"

### 3.2 SolarWinds SUNSPOT —— 构建管道完整性启示
- **定位**：2020-12-13 披露的供应链事件；构建期注入器 SUNSPOT（taskhostsvc.exe）在构建服务器上监视 MSBuild 进程，替换目标源文件 `InventoryManager.cs` 编译后**还原原文件**（git 历史保持干净），产物用 SolarWinds 有效 Authenticode 证书签名；约 1.8 万组织收到带毒更新，其中 <100 家有后续人工操作（口径：FireEye/Mandiant 公开分析）。
- **可抄机制（防御向）**：
  1. **可复现/密封构建**：SolarWinds 事后自建三套并行临时构建环境做字节级比对 ⇒"构建产物必须能从审查过的源码重建"是最小反制。
  2. **签名事件基线**：签名操作日志化 + 与工单/预期比对（异常 DLL 体积、缺少审批单）⇒ ADV 侧对应"发布流程证据链"条目（docs-process）。
  3. **审查盲区教训**：源代码、CI 日志、签名各自单看都"干净"，缺失的是**跨层一致性**；同理，ADV 的 SCA 结论要标注"证据来自哪一层"。
- **档位**：【有界】——吸收构建一致性检测与证据分层思想到 adv-sca/docs-process；事件细节（注入器实现）不吸收。
- **第一步动作**：把"发布物无法从源码重建即风险"写成 adv-sca 的一条可报告项（与 3.1 第 9 条合并）。
- **许可证/成熟度**：案例研究（引用 Mandiant 公开分析）｜2020 年事件，防御结论仍有效但需注明来源年份。
- **链接**：https://www.mandiant.com/resources/blog/sunburst-additional-technical-details

### 3.3 SLSA / in-toto / Sigstore —— 证明面参照
- **定位**：构建来源证明（provenance）、证明链（in-toto）、签名透明日志（Sigstore Rekor）三条互补的供应链证据标准/工具。
- **可抄机制**：provenance 的字段结构（builder/材料/产物摘要）可作 ADV 供应链检查项的分类骨架；透明日志"可对外核验"的思路对应发布物验证路径。
- **档位**：【reference】——标准/工具链不是扫描规则本体，只作 adv-sca 检查项的术语与分类参照（集成目标 docs-process）。
- **第一步动作**：把 provenance 三字段（builder/materials/artifact digest）加进 adv-sca 检查项备忘。
- **许可证/成熟度**：SLSA 规范与 Sigstore 均开放许可（Sigstore Apache-2.0）｜【维护中】。
- **链接**：https://slsa.dev/ ；https://in-toto.io/ ；https://www.sigstore.dev/

---

## 4. 钓鱼/社工模拟框架（防御训练向，对象 #14–#15）

### 4.1 GoPhish —— 演练编排模型
- **定位**：自托管邮件钓鱼**演练**平台：sending profile（SMTP）/邮件模板/落地页/目标组/campaign 五对象 + REST API + 事件上报（webhook/IMAP）。
- **可抄机制**：五对象 + 事件回流的编排模型是"训练活动管理"的清晰范式（对照 ADV 若做安全培训素材管理可参考其对象切分）；其 email 模板与 landing page 属**不吸收面**（可操作的钓鱼件）。
- **档位**：【reference】——防御训练场景的架构对照；低活跃（末次 push 2024-09-23）。
- **第一步动作**：无（仅登记参照）。
- **许可证/成熟度**：自定义（GitHub NOASSERTION）｜【维护中】但低活跃（近两年无推送，2026-10-03 实查）。
- **链接**：https://github.com/gophish/gophish

### 4.2 Evilginx2 —— 反向代理型钓鱼的防御对照
- **定位**：中间人式反代钓鱼框架，声明式 `phishlet` 描述目标站点适配 + 会话令牌捕获。
- **可抄机制**：防御侧价值=明确"带凭据转发"类攻击的检测面（证书与域不匹配、异常反代链路、条件访问/令牌绑定对策）；对 ADV 而言只作威胁模型条目。
- **档位**：【reference】——不吸收任何 phishlet/配置；仅威胁模型引用。
- **第一步动作**：无。
- **许可证/成熟度**：BSD-3-Clause ｜【维护中】（push 2026-06-10）。
- **链接**：https://github.com/kgretzky/evilginx2

---

## 5. 隐蔽传输工程与检测面（对象 #16–#19；防御检测视角）

### 5.1 dnscat2 —— DNS 隧道传输抽象（含碘 iodine 同类）
- **定位**：把 C2 会话封装进 DNS 查询/应答的经典实现（客户端+服务端，加密会话层、分片重组、多记录类型）。
- **可抄机制（检测向）**：隧道检测的三个稳定观测量——**子域标签长度/熵、查询量分布、记录类型与应答尺寸异常**；这些是网络面特征，ADV（源码/供应链平台）当前面外，登记为 watch/reference 供未来网络面复用。
- **档位**：【reference】——不集成（面外）；特征清单留档。
- **第一步动作**：无。
- **许可证/成熟度**：BSD-3-Clause ｜【维护中】（低活跃，末次 push 2024-03-14）。
- **链接**：https://github.com/iagox86/dnscat2

### 5.2 Malleable C2 profile 概念 —— "流量可整形"的防御含义
- **定位**：C2 profile 允许操作者声明 URI/header/body 编码/元数据编码（Base64/Base64URL/NetBIOS/NetBIOSU/Mask 等列表见公开文档）与睡眠/抖动节奏 ⇒ 网络层的静态签名（默认 URI、UA、固定结构）在整形后被稀释。
- **可抄机制（防御向）**：① 检测重心从"签名"移向"行为统计"（心跳间隔/抖动分布，2025 年有 Varonis "Jitter-Trap" 类方法）；② 配置提取（parser 类）仍是高价值取证手段；③ 任何静态规则须标注"时效性"（对应 1.5）。
- **档位**：【reference】——概念级防御理解；不吸收 profile 实现与规避技巧。
- **第一步动作**：无（已在 ADV 威胁模型备忘登记）。
- **许可证/成熟度**：概念（引用官方用户指南公开页）｜公开文档长期存在。
- **链接**：https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/malleable-c2-main.htm

### 5.3 Domain fronting（T1090.004）—— 借用域检测
- **定位**：TLS SNI 与 HTTP Host 指向不同域、借 CDN 边缘落地的规避手法（ATT&CK 正式技术条目）。
- **可抄机制（检测向）**：核心检测量=**SNI/Host 不匹配**；对策含 CDN 允许清单细化、TLS 检查点；ECH（加密 ClientHello）削弱 SNI 侧检测 ⇒ 检测组合需降级到指纹/行为面（#19）。
- **档位**：【reference】——概念级；ADV 面外。
- **第一步动作**：无。
- **许可证/成熟度**：概念（MITRE ATT&CK 公开条目）。
- **链接**：https://attack.mitre.org/techniques/T1090/004/

### 5.4 JA3/JA4/JARM + c2detect —— TLS 指纹与基础设施聚类的工程模型
- **定位**：JA3/JA3S 为客户端/服务端 hello 字段拼接哈希；JA4 家族（JA4/JA4S/JA4X 证书结构等）为其改良谱系；JARM 为**主动**服务端指纹（一组特殊 ClientHello 的响应哈希）。
- **可抄机制**：
  1. **聚类枢轴模型**：c2detect 项目利用"基础设施（域名/IP）轮换快、TLS 监听栈与证书栈轮换慢"的不对称，用 JARM/JA4X/JA4S/证书序列号等做**跨主机关联枢轴**，并为每个枢轴定义权重（"两独立主机同值"的罕见度 vs "单主机上的诊断力"）⇒ 这是"弱信号聚合"的通用工程范例，可直接借到 ADV 的规则打分/证据聚合设计（adv-rules 方法论层）。
  2. **不绝对化的实测锚**：公开实测（Piraeus 学位论文口径）显示，在 C2 前加 Nginx 反代后，Cobalt Strike 与 Sliver 的 JARM 指纹**趋同为 Nginx 特征**⇒ 指纹反映"暴露端点"而非后端框架，检测结论必须写清证据边界。
- **档位**：【watch】——指纹网络面目前超出 ADV（本地代码/供应链平台）；聚类权重方法论可作 reference 借入（若 adv-rules 做证据聚合）。
- **第一步动作**：把"枢轴罕见度权重"两句话记入 adv-rules 证据聚合备忘（不建网络嗅探面）。
- **许可证/成熟度**：c2detect 仓库许可待核；JA3/JA4 规范公开｜【维护中】（2025 年活跃）。
- **链接**：https://github.com/cognis-digital/c2detect

---

## 6. 扩展 ABI 与生态工程（对象 #20–#23）

### 6.1 BOF / COFF loader —— 轻量内存模块 ABI
- **定位**：Beacon Object Files——以 COFF 目标文件为分发格式的进程内扩展，被 Cobalt Strike/Havoc/Sliver/Outflank 等多框架共同支持。
- **可抄机制（工程参数级）**：
  1. **加载器三段**：COFF 解析（section 处理，含 `.bss` 零初始化 / symbol 解析 / relocation 应用——多数加载器只实现重定位子集，兼容性差异即源于此）；参考实现 TrustedSec `COFFLoader` 提供 4-pass 流程：尺寸计算（页对齐、安全整数运算）→ 段映射 → 符号与 AMD64/i386 重定位 → 内存保护（RX/RW/R 分段）后执行。
  2. **延迟符号解析（DFR）**：模块用 `LIBRARY$Function` 声明导入，宿主执行前解析（Beacon 侧上限 128 个）；失败则拒绝执行并报未解析符号 ⇒ "白名单式导入 + 运行前检查"的 ABI 安全模式。
  3. **质量门槛工具化**：boflint 类 linter 检查重定位类型/入口点/导入/栈变量/异常处理；`beacon_compatibility` 提供宿主 API 兼容层 + `beacon_generate` 参数打包 ⇒ 插件生态 = ABI + 兼容层 + linter + 打包器四件套。
- **档位**：【reference】——ADV 若做沙箱插件 ABI，可对照"轻量可移植格式 + 延迟解析 + linter 门"三点；COFF 加载实现本身不吸收（无对应需求）。
- **第一步动作**：读 boflint 的检查项清单，对齐 ADV 插件/sandbox 静态校验门现有条目做差异表（docs-process）。
- **许可证/成熟度**：TrustedSec 仓库 GitHub 标 NOASSERTION（许可待核）｜【维护中】（push 2026-09-28）。
- **链接**：https://github.com/trustedsec/COFFLoader

### 6.2 Sliver Armory —— 植入扩展的分发模型
- **定位**：Sliver 的扩展包安装器（第三方 BOF/扩展的一键分发）；工程价值="插件生态的分发与签名"对照。
- **档位**：【watch】——ADV 无植入体生态；仅观察其包分发/签名流程。仓库**无 LICENSE 标注**（GitHub license=None），引用时需注意。
- **链接**：https://github.com/sliverarmory/armory

### 6.3 BloodHound CE —— 攻击路径图模型
- **定位**：SpecterOps 的图谱化权限分析平台（CE 开源）：Go 后端 + React + Neo4j（graph-db）+ PostgreSQL（app-db）；采集器分离（SharpHound/.NET 采集方法 ACL/Session/LoggedOn/…、AzureHound/Go、BloodHound.py）。
- **可抄机制**：
  1. **节点/边枚举即威胁原语**：User/Group/Computer/GPO/OU/CertTemplate… 与 ACL 边（GenericAll/WriteDacl/DCSync/AllowedToAct…）构成"权限原语目录"——**枚举式建模**：把可分析的关系类型穷举化、命名化，查询（Cypher）最短路径到目标。
  2. **OpenGraph（v8）**：把 AD/Entra 之外的任意系统塞进同一图，分 generic（探索）与 structured（schema 化，享受路径查找等全功能）两档 ⇒ "schema 化的通用图扩展"思路可对照 ADV 的代码资产图（未来面）。
  3. **接口面**：REST（`/api/v2/graphs/cypher`）+ 2025 年出现的 MCP server 暴露给 LLM 做自然语言路径分析 ⇒ 图数据面与智能体面的桥接先例。
- **档位**：【reference】——图模型/枚举式边目录是可复用建模思想；AD 域内容不吸收。
- **第一步动作**：把"边类型枚举成命名原语"模式记入 ADV 图模型（若启）设计备忘。
- **许可证/成熟度**：Apache-2.0 ｜【维护中】（push 2026-10-02）。
- **链接**：https://github.com/SpecterOps/BloodHound

### 6.4 Impacket（Fortra）—— 协议库工程结构
- **定位**：Windows 协议（SMB/LDAP/Kerberos/…）的 Python 实现库 + examples 分离。
- **可抄机制**：库/示例分离的仓库结构（协议类库稳定，示例随用随弃）+ 统一的 NDR 序列化层 ⇒ 对 ADV 的"解析库与工具层分离"是可对照的整洁结构样本。
- **档位**：【reference】——仅结构对照；协议利用内容不吸收（该库 dual-use 程度高，明确不入 ADV）。
- **许可证/成熟度**：Apache 修改版（LICENSE 为 "slightly modified version of the Apache Software License"，2026-10-03 实读；GitHub NOASSERTION）｜【维护中】（push 2026-10-01）。
- **链接**：https://github.com/fortra/impacket

---

## 7. 检测规则工程与端点侧（对象 #24–#26）

### 7.1 Sigma + pySigma —— 单源规则 + 多后端转译管线（★ 有界吸收）
- **定位**：跨 SIEM 的通用检测规则标准（YAML）+ pySigma 转译器（pipeline 适配、correlation 规则）。
- **可抄机制**：
  1. **单一源 → 多后端**编译管线：规则 YAML 一次书写，经 pipeline（字段映射/过滤）转译为各后端查询 ⇒ ADV 规则（若面向多执行器/多语言）可对照该分层：规则对象 / 字段映射层 / 后端 emit。
  2. **correlation 规则**：把多条检测按时间窗口聚合成事件 ⇒ 与 5.4 的"弱信号聚合"同题。
  3. **许可分离**：规范与规则库（DRL 1.1）/ 工具（LGPL-2.1）许可不同 ⇒ 内联规则文本需看 DRL，代码借用需看 LGPL（ADV 只抄思想则不受影响）。
- **档位**：【有界】→ adv-rules（管线分层与 correlation 模型）；规则文本不内联、实现不复制（LGPL/DRL 边界）。
- **第一步动作**：读 pySigma 一个 backend 的接口，抽出"规则对象→pipeline→查询"三层的接口签名对照 ADV 规则执行器。
- **许可证/成熟度**：规则库 DRL 1.1 + pySigma LGPL-2.1（2026-10-03 实读）｜【维护中】（push 2026-10-02）。
- **链接**：https://github.com/SigmaHQ/sigma ；https://github.com/SigmaHQ/pySigma

### 7.2 Chainsaw / Hayabusa —— Sigma 引擎的 Rust 实现（性能对照）
- **定位**：事件日志（evtx）快速取证扫描器，内嵌 Sigma 规则引擎；Chainsaw（GPL-3.0）/ Hayabusa（AGPL-3.0）。
- **可抄机制**：规则编译 + 多线程扫描 + evtx 解析的工程组合（"规则引擎要快"的现成案例）；ADV 的 Rust 规则引擎性能基线可对照其公开口径。
- **档位**：【reference】——GPL/AGPL 只抄思想；不集成（日志面外）。
- **第一步动作**：无。
- **许可证/成熟度**：GPL-3.0 / AGPL-3.0 ｜【维护中】（push 2026-09/10）。
- **链接**：https://github.com/WithSecureLabs/chainsaw ；https://github.com/Yamato-Security/hayabusa

### 7.3 Velociraptor / osquery —— 端点 agent 架构（面外）
- **定位**：端点可见性 agent：Velociraptor（VQL 查询语言、离线采集、client-server）、osquery（SQL 化端点表）。
- **可抄机制**：query-language-as-API 的 agent 设计（把能力暴露为查询语言而非固定命令集）——ADV 的 MCP 工具面同样是"能力暴露"问题，可作对照；但两者面外且 AGPL，仅 watch。
- **档位**：【watch】——不集成理由：ADV 无端点 agent 面。
- **链接**：https://github.com/Velocidex/velociraptor ；https://github.com/osquery/osquery

---

## 8. 重点回答

### 8.1 ① C2 框架模块化/插件/多 agent 编排：值得 ADV 借鉴的工程做法
| 来源（两端点名） | 机制 | 可去处 |
|---|---|---|
| Mythic → ADV 插件沙箱 | 插件=独立容器 + 统一注册契约（六类扩展同构） | adv-sandbox 插件 manifest 字段表 |
| Mythic → ADV 任务面 | operation→task→callback 三级 + 可恢复字节偏移传输（中断续传语义） | MCP 工具面长任务语义 |
| Mythic → ADV 传输分层 | RabbitMQ 总线 + translation 容器 gRPC 旁路（按关键路径选传输） | server 内部模块通信（若启） |
| Sliver → ADV 消息层 | 传输可换而消息封装不变（Envelope: type id + protobuf） | 多协议/多工具面封装对照 |
| Metasploit → ADV 注册表 | module set = hash 工厂 + 惰性实例化；mixin 组合装配 | 插件/规则注册表设计对照 |
| Caldera → ADV 编排数据模型 | Ability/Fact/Operation/Planner 分层，事实在能力间传播 | adv-rules 威胁映射与检测编排 |
| BOF → ADV 插件 ABI | 轻量可移植格式 + 延迟符号解析（白名单导入）+ linter 门 | docs-process 插件校验清单 |

原则：只借**结构化模式**（契约/队列/封装/注册/校验），不借任何攻击实现；落入 adv-sandbox 与 docs-process 前先做"差异表"再定取舍。

### 8.2 ② ATT&CK/Caldera 数据模型怎么喂进 ADV 威胁映射层
1. **数据面（STIX 快照）**：冻结 `attack-stix-data` 版本 → 解析 `attack-pattern`（technique/sub-technique）与 tactic 关系 → 建"technique 图"；用引用完整性门校验一切规则中的 Txxxx（未知 id 拒绝入库）。
2. **能力面（Caldera 形态）**：ADV 的检测规则按 (technique, platform, evidence) 三元组登记，等价于 Caldera `Ability` 的裁剪版；"Fact"对应扫描结果里的结构化证据对象（供后续规则/报告复用）。
3. **验证面（ART 形态）**：每条规则挂最小夹具（输入→期望证据），过机器校验（schema + 清理语义），形成"检测即测试"的 test-replay 门。
4. **报告面（Debrief 形态）**：按 tactic 出覆盖度矩阵，标 BLIND_SPOT / NOT_TESTED（缺口分类命名可借 2025 年社区实践），数字全部锚到快照版本。
- 第一步：见 2.3 的第一步动作（冻结快照 + 校验器），这是整条链的地基。

### 8.3 ③ XZ backdoor 对 ADV 供应链扫描的规则启示清单
见 §3.1 的 9 条清单；落地优先级：**先做 5 条纯静态**（发布物↔仓库一致性 / 构建脚本内嵌 blob / 伪二进制夹具 / 构建期写回 / 可复现性缺口），元数据类（维护权变更、贡献模式）只作**加权信号**并明确"非单点判据"；所有条目输出"命中 + 人工复核"语义，不自动定性恶意。

### 8.4 ④ "红队工具可检测特征"→ ADV 检测/扫描思路 的转化表
| 可检测特征（防御文献口径） | 检测工程信号 | 强度/边界 | ADV 对应 |
|---|---|---|---|
| 静态字符串/URI/配置常量（默认 profile） | 快速筛 | 弱（可整形衰减，见 5.2） | adv-rules 仅作初筛 |
| 工件配置 blob（结构固定的混淆配置） | 内存/样本取证 parser | 中（需工件） | watch（面外） |
| TLS 指纹（JA3/JA4/JARM） | 客户端异常 + 基础设施聚类 | 中（反代前置趋同，5.4 实测） | watch |
| 证书特征（复用序列/CN/签发者） | 聚类枢轴 | 中 | watch |
| 心跳时序（interval/jitter） | 行为统计（Jitter-Trap 类） | 中 | watch |
| SNI/Host 不匹配（域前置） | 网络检查点 | 中（ECH 削弱） | watch |
| DNS 隧道（长标签/熵/查询分布） | 流量统计 | 中 | watch |
| 无文件内存模块（BOF） | 内存扫描/线程异常 | 中 | watch |
| **构建期异常（XZ/SolarWinds）** | **静态差异 + 流程证据** | **强（本域可直接吸收）** | **adv-sca** |
| **依赖/维护权变更事件** | SCA 元数据加权 | 弱-中（组合信号） | adv-sca |

转化原则：网络/主机行为特征（前三行群）在 ADV 当前面外 → watch 留档；**构建/依赖层静态特征 → 吸收进 adv-sca**；数据模型（#8/#9/#24）→ 吸收进 adv-rules。检测强度一律带边界说明，不做绝对化表述。

### 8.5 ⑤ 明确不吸收清单（本域产出边界）
1. **载荷/loader/规避实现**：shellcode、crypter、sleep mask、间接系统调用、堆栈欺骗、AMSI/ETW 绕过等一切实现细节（只在"可检测特征"层引用名称，不给做法）。
2. **C2 服务端/agent 实现代码**：Sliver/Havoc/Mythic 代码一律不内联（GPL/用途双重边界）；仅借架构思想，且 Mythic 条目限"插件契约/队列语义"两点进 adv-sandbox 对照。
3. **钓鱼可操作件**：GoPhish 模板/落地页、Evilginx2 phishlet/配置、凭据捕获流程。
4. **攻击载荷库**：Nuclei 模板库、Metasploit 模块、Caldera abilities 的具体命令内容。
5. **IOC 数据内置**：任何需要持续更新的 IOC/威胁情报列表不内置（ATT&CK 快照除外——公开标准且版本化）。
6. **供应链攻击构造细节**：XZ 载荷构造步骤、SUNSPOT 注入器实现——只进检测规则设计，不进攻击复现。
7. **改变默认档的行为**：任何吸收项不得触碰 ADV 默认档冻结值与哈希（呼应工作区质量红线 8）。
8. **弃维护组件**：Havoc（归档）代码与生态不进入评估队列（仅文档对照）。

---

## 9. 2025–2026 前沿信号（带时间锚）
- **2025-08** Mythic 生态：SpecterOps 刊文"Browser Scripting / Consuming Containers"，把 Mythic 讲成操作中枢；同月 Android agent **Eris**（多 profile：HTTPS/DNS covert/FCM/WebSocket，依赖注入）出现——移动 agent 模块化是 C2 生态新热点（**链接待验证**）。
- **2025（Mythic 4.0）**：scoped opaque API token、agent-initiated RPC、可恢复字节偏移文件传输、operation chat/AI 容器、eventing 审批 ⇒ 编排中枢化与"审批流"进 C2 产品化。
- **2025-03** 学位论文级工程：DNS C2 agent 以 Python DNS 服务端 + RabbitMQ/RPC 桥接 Mythic ⇒ agent 与 C2 核心的"协议翻译容器"模式被独立复现（与 1.1 机制互证）。
- **2025-09** ngCERT 发布 Cobalt Strike Beacon 防御公告（EDR 行为检测、DNS/SMB 监控清单）⇒ 防御方对商业 C2 的标准响应模板。
- **2025** 规避-检测对抗向前沿：TeamT5 报告 CDN/Cloudflare Workers **serverless 反代**隐藏 C2；Varonis "Jitter-Trap" 用抖动统计反制；奇安信多模态（统计+序列+字节）检测加密 CS 流量 ⇒ 检测重心继续从签名移向行为/统计。
- **2024-03 → 2025** XZ 事件工程化复盘（MSR 2025 论文 *Wolves in the Repository*）；**截至 2026-10 仍无在野确认利用、未进 KEV**（口径复核）——供应链"未爆弹"性质使其成为规则设计的理想样本。
- **2026-10（本期实查）**：Sliver/Mythic/Caldera/ART/Nuclei/Sigma/BloodHound 均活跃；**Havoc 归档（2025-12 末次推送）**；GoPhish 停更超两年（末次 push 2024-09-23）⇒ 工具生态更替快，任何"生态现状"结论都要带查询日期。
- **2025** BloodHound OpenGraph v8 通用图 + MCP server 化 ⇒ 图数据面给 LLM/智能体消费成常规做法。

## 10. Top-3（深挖优先级）
1. **Mythic 插件契约 + 任务/回调模型 → adv-sandbox**（机制最完整、两条独立互证：Mythic 容器注册 与 Caldera 插件独立仓库；产出对照笔记后定 ADV 插件 manifest 字段）。
2. **ATT&CK STIX 快照 + Caldera/ART 数据模型 → adv-rules 威胁映射层**（地基工作：冻结快照 + 引用完整性校验器；一切后续威胁映射与覆盖报告都挂在这层）。
3. **XZ 供应链规则启示清单（5 条纯静态先行）→ adv-sca**（直接可落地，与 SolarWinds 第 9 条合并；输出"命中+人工复核"语义）。
- **近榜**：Nuclei 模板引擎三件套（编译-执行-结果事件 + 请求聚类）与 Sigma 单源多后端管线 → adv-rules 规则执行器设计的两条工程对照线。

## 11. 参考链接汇总（按对象序）
1. https://github.com/its-a-feature/Mythic ｜ https://docs.mythic-c2.net/installation/a-note-about-containers
2. https://github.com/BishopFox/sliver
3. https://github.com/HavocFramework/Havoc
4. https://github.com/rapid7/metasploit-framework
5. https://www.cobaltstrike.com/product/features ；ngCERT 2025-09 公告（防御文献）
6. https://github.com/BC-Security/Empire
7. https://github.com/mitre/caldera
8. https://github.com/redcanaryco/atomic-red-team/blob/master/atomic_red_team/spec.yaml
9. https://github.com/mitre-attack/attack-stix-data ；https://cti-taxii.mitre.org/taxii2/
10. https://github.com/projectdiscovery/nuclei ；https://docs.projectdiscovery.io/templates/structure
11. https://nvd.nist.gov/vuln/detail/CVE-2024-3094 ；MSR 2025（Wolves in the Repository）
12. https://www.mandiant.com/resources/blog/sunburst-additional-technical-details
13. https://slsa.dev/ ；https://in-toto.io/ ；https://www.sigstore.dev/
14. https://github.com/gophish/gophish
15. https://github.com/kgretzky/evilginx2
16. https://github.com/iagox86/dnscat2
17. https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/malleable-c2-main.htm
18. https://attack.mitre.org/techniques/T1090/004/
19. https://github.com/cognis-digital/c2detect
20. https://github.com/trustedsec/COFFLoader
21. https://github.com/sliverarmory/armory
22. https://github.com/SpecterOps/BloodHound
23. https://github.com/fortra/impacket
24. https://github.com/SigmaHQ/sigma ；https://github.com/SigmaHQ/pySigma
25. https://github.com/WithSecureLabs/chainsaw ；https://github.com/Yamato-Security/hayabusa
26. https://github.com/Velocidex/velociraptor ；https://github.com/osquery/osquery
27. https://docs.mythic-c2.net （Eris 条目链接待验证）

---
*登记：`RESEARCH/registry/R9.jsonl`（27 条，2026-10-03，python json.dumps 追加）。许可证/活跃度均为该日 GitHub API 实查值；所有数字带来源与日期口径，未在本文档中出现绝对化表述。*
