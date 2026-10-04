# 02 — SAST / Secrets / SCA 供应链产品对标调研

> 调研日期：2026-10-03 · 供《ADV 重写蓝图》使用 · 信息来源为公开搜索快照（2025–2026 优先），未逐条读源码的机制细节已标「待验证」。
> 档位定义：**吸收**=可直接依赖或照搬实现（许可兼容）；**有界**=抄机制/思想，不抄代码；**不吸收**=对本地优先零依赖 Rust 项目无增量。
> 营销话术与机制已区分：厂商跑分（如 "5x faster"）一律标注为厂商口径，不作结论依据。

---

## 一、SAST

### 1. Semgrep（OSS 引擎 + Pro 专有引擎）

**一行定位**：规则驱动的轻量 SAST——OSS 引擎做单文件内模式匹配 + 同文件内跨函数/污点分析，跨文件（interfile）与全程序过程间分析锁在专有 Pro 引擎里。

**架构边界（2025 口径，来自 docs.semgrep.dev/semgrep-pro-vs-oss 与 Doyensec 白皮书）**：
- OSS：`pattern`/`metavariable` 结构匹配；taint mode 限**单文件**（可跨同文件内函数）；`join mode` 可粗糙模拟跨文件（社区文章，非常用路径）。
- Pro：跨文件/跨目录污点追踪、完整过程间数据流——专有（OCaml 实现，不开源）。
- 营销边界确认：**跨文件分析就是 Semgrep 的付费墙**。这对 ADV 是机会：本地工具能做好的跨文件污点分析是差异化点（但难度高，见 01 号调研的 IR 讨论）。

**值得抄的机制**：
1. **规则即 YAML、 metavariable 元变量系统**：`$X` 单节点、`$...MANY` 多节点，`pattern-inside`/`pattern-not-inside` 做上下文约束——误报控制主要靠「负模式」（说清哪里不该匹配），而非调阈值。规则里 `metadata.confidence: HIGH/MEDIUM/LOW` 显式携带置信度。
2. **规则测试协议**：`semgrep --test`——测试文件用 `ruleid:` 注释标注期望命中行，CI 里跑规则库全量校验；`--json` 输出含 `config_missing_fixtests`（缺测试的规则文件），可机器化门禁。规则发布前强制对真实代码评估（Testing Handbook）。
3. **规则分发优化**：Registry 规则从 YAML 预转换成 **JSON** 供 CLI 解析（CHANGELOG 佐证：为加快规则解析）——「预编译规则格式」是加速的便宜一招。

**档位【有界】**——OSS 引擎是 LGPL-2.1（可引用但绑定较麻烦），规则格式与测试协议可抄机制；Pro 引擎专有只看思想。
**第一步动作**：在 ADV 规则格式设计里定死「负模式 + confidence 元数据 + ruleid 注释测试」三件套；规则库配 `adv test --json` 门禁。
**许可证**：引擎 LGPL-2.1；Pro 专有（只抄思想）。**链接**：https://docs.semgrep.dev/semgrep-pro-vs-oss · https://github.com/semgrep/semgrep · Doyensec 白皮书 https://www.doyensec.com/resources/Comparing_Semgrep_Pro_and_Community_Whitepaper.pdf

### 2. CodeQL（GitHub）

**一行定位**：把代码编译成可查询的**关系数据库**，用声明式查询语言 QL 写安全查询；污点/数据流建在 `DataFlow`/`TaintTracking` 库上。

**值得抄的机制**：
1. **「代码→数据库」分离**：先建库（database extraction，每语言一个提取器），查询与建库解耦——增量扫描、多查询复用同一库成为自然架构（Tweag 2025-08 介绍文章佐证）。
2. **库建模（qlpack）作为版本化包**：框架的 source/sink/model 模型按 qlpack 发布、带版本与依赖关系——「语言模型可独立升级」是这个设计的核心价值。
3. Path query 概念：结果天然带污点路径（source→sink 的完整链），呈现给用户的是链不是孤点。

**档位【有界】**——CLI 专有（对 OSI 开源项目/研究免费，商业闭源需 GitHub Advanced Security 授权，见 codeql-cli-binaries/LICENSE.md 与 issue #21487）；标准库与查询（github/codeql）MIT——**查询与库可读可学，CLI 不可嵌入**。
**第一步动作**：读 github/codeql 里 Java 的 `DataFlow::ConfigSig` 用法，抽象出 ADV 自己的「source/sink 声明 + 数据流配置」接口形状。
**许可证**：库 MIT；CLI 专有（开源免费档）。**链接**：https://codeql.github.com · https://github.com/github/codeql

### 3. SonarQube

**一行定位**：质量+安全一体平台，核心资产不是分析器而是**工作流**：Quality Gate 策略门 + Security Hotspot 人工复核流。

**值得抄的机制**：
1. **Hotspot ≠ Issue 两级模型**：确定性规则出 Issue，可疑但不必然的模式出 Hotspot，强制人审「是否真漏洞」——避免把低置信发现混进同等告警流。
2. Quality Gate：策略即配置（新代码相对基线的通过条件），挂在 PR 上。
3. 安全污点分析、34 语言支持属其商业 editions（LGPL-3.0 的 Community 版不含完整 SAST 能力——以官方版本页为准，细节待验证）。

**档位【有界】**——机制层面（两级告警流）值得抄；分析器本体不针对 Rust。
**第一步动作**：ADV 报告模型里直接预留 `issue` vs `hotspot` 两个通道 + `confirmed/rejected` 复核态。
**许可证**：Community LGPL-3.0；商业版专有。**链接**：https://www.sonarsource.com/comparison/sonarqube-vs-snyk

### 4. JetBrains Qodana

**一行定位**：把 IDE（IntelliJ 系）inspection 引擎无头化跑 CI，附带 baseline 机制冻结存量问题、只卡新增。

**值得抄的机制**：**baseline 文件**（把已接受存量发现落盘指纹，扫描时 diff 出「新增」）——与 detect-secrets 的 baseline、ADV 已有的否证标签思路同构，可统一成一种「已知问题账本」格式。其引擎本身是 IDE inspections 的 CI 化（公开文档较薄，引擎细节待验证）。
**档位【不吸收】**——面向 JVM/PHP 等其 IDE 生态，对 Rust 本地工具无引擎增量；只吸收 baseline 概念（该概念从 gitleaks/detect-secrets 也能拿到）。
**第一步动作**：无（baseline 概念并入第 3 条 Snyk/detect-secrets 动作即可）。
**许可证**：专有（有 Community 免费档，条款以 JetBrains 官方为准）。**链接**：https://www.jetbrains.com/qodana/

### 5. Snyk Code（DeepCode）

**一行定位**：收购 DeepCode 得来的 SAST，符号引擎 + ML 混合，主打跨文件数据流与低误报。

**值得抄的机制**：
1. **数据流图上打污点标签做 source→sink**（safeguard.sh 解析文章描述其原理）；跨文件分析全档位开放（对比 Semgrep 的付费墙）。
2. ML 参与 FP 过滤与修复建议生成（DeepCode 血统）。
3. 厂商跑分「~5x SonarQube / ~14x LGTM」来自 snyk.io 自家博客——**记为厂商口径，不作设计依据**。

**档位【有界】**——产品闭源服务；「混合符号+ML」的思想与全档位开放污点分析的定价策略值得学。
**第一步动作**：蓝图里给「确定性引擎先行，ML/LLM 只做过滤与解释」的分层定调（与 Aikido 同一条，见前沿信号）。
**许可证**：专有。**链接**：https://snyk.io/blog/sast-tools-speed-comparison-snyk-code-sonarqube-lgtm · https://konvu.com/compare/snyk-vs-sonarqube

### 6. Aikido

**一行定位**：安全平台赛道新玩家，SAST 亮点是 **AutoTriage：先用可达性引擎过滤，再让 LLM 只审剩下的发现**。

**值得抄的机制**：
1. **两段式过滤顺序**：确定性可达性检查（「脆弱函数是否真的被调用链触达」）在前，LLM 读代码上下文在后——LLM 只处理「引擎确认为真链但可能误报」的存量，省成本且可审计（help.aikido.dev/sast-autotriage 官方文档描述）。
2. 「95% FP 消减」为厂商宣传口径，无第三方复测——**只取流水线顺序，不取数字**。

**档位【有界】**——闭源平台，机制值得抄。
**第一步动作**：在 ADV 的 triage 模块设计里固定：`引擎过滤 → 可达性 → LLM 复核 → 结果缓存为记忆` 的顺序（详见前沿信号一节）。
**许可证**：专有。**链接**：https://help.aikido.dev/aikido-agent/sast-autotriage

### 7–9. Checkmarx / Fortify / Veracode（只看架构思想）

- **Checkmarx**：**建图一次、查询任意**——CxQL 是跑在代码图上的 DSL，用户可自定义查询规则（docs.checkmarx.com Query Structure：「先构建代码元素与逻辑流的图，再对图跑查询」）。CxFLOW 是扫描编排/结果过滤层。**抄**：图上 DSL 的自定义规则思路。专有。
- **Fortify**：**translation（翻译成中间表示）→ scan → 报告** 三段，自定义规则以 rule pack 分发（SCA_Guide 23.1.0）。**抄**：翻译/扫描分离、规则包独立版本化。专有。
- **Veracode**：**扫编译产物而非源码**（源码不出企业边界）+ Pipeline Scan（快速档）与深度扫描（慢档）分层、策略治理。**抄**：快/慢双档扫描与「产物级清单」概念（后者与 cargo-auditable 呼应）。专有。
- 共同落地建议（搜索综合）：多扫描器结果归一到 SARIF 统一 triage——ADV 输出格式应原生支持 SARIF。
- **许可证**：三者皆专有，只抄思想。**链接**：https://docs.checkmarx.com/en/34965-46539-query-structure.html · https://www.microfocus.com/documentation/fortify-static-code-analyzer-and-tools/2310/SCA_Guide_23.1.0.pdf · https://docs.veracode.com/r/Pipeline_Scan

---

## 二、Secrets

### 10. Nosey Parker（本领域对 ADV 最值钱的单件）

**一行定位**：Rust 写的 secrets 扫描器（v0.10 起从 Haskell 重写为 Rust），以「按 secret 内容去重」和 GB/s 吞吐为核心卖点。

**高性能/低噪流水线拆解（综合 repo、RULES.md、Praetorian 博客、HN 讨论）**：
1. **扫描**：文本按块并行喂给 regex 引擎；内容分块以支持并行与增量。**注意：它不靠熵启发式**——Praetorian 博客明确其路线是「结构化 regex 规则 + ML 降噪器」，熵筛选是 gitleaks 一系的做法（两者路线差异要写进蓝图）。
2. **规则**：YAML，每条规则 = 单个 regex `pattern` + **至少一个捕获组**（捕获组隔离出 secret 本体，与周围上下文分开）+ name/examples/references 元数据（docs/RULES.md）。捕获组分离是去重得以成立的前提。
3. **去重**：匹配后按**捕获出的 secret 字节内容**分组——同一 secret 出现 1000 处 = 1 条 grouped finding + 全部位置列表。官方口径：review burden 降 10–1000x。
4. **数据存储**：本地 SQLite datastore（man page 与 discussion #83/#206 佐证），内容寻址记录已扫 blob；**增量扫描跳过已扫 blob**；多轮扫描共用一个 datastore，结果跨运行合并去重。
5. **降噪**：ML 分类器（如 logistic 回归类的轻量模型，daily.dev 文章记载其 ML 演进）在 regex 命中后过滤假阳性；具体模型细节在源码 `crates/` 内，公开文档未完整描述（待验证）。
6. **性能锚点**：官方 README 口径——笔记本上 ~5 分钟扫完 Linux kernel 100GB git 历史（GB/s 级）。

**档位【吸收】**——Apache-2.0 + Rust + 零依赖精神一致：可作为**库/子进程直接用**，或抄其 datastore/规则/去重设计。
**第一步动作**：clone 仓库，读 `crates/noseyparker-datastore` 的表结构与 `crates/noseyparker-rules` 的规则解析；评估「直接依赖 crate」vs「自研兼容实现」两条路的成本。
**许可证**：Apache-2.0。**链接**：https://github.com/praetorian-inc/noseyparker · https://github.com/praetorian-inc/noseyparker/blob/main/docs/RULES.md

### 11. TruffleHog

**一行定位**：Go 写的 secrets 扫描器，800+ 检测器映射到具体身份（AWS/Stripe/…），核心差异化是**对每个命中做在线验证**。

**值得抄的机制**：
1. **检测器 = 代码不是正则**：每个 detector 是 Go 模块（含解析、上下文提取、verify 方法），验证引擎对命中凭证调真实 provider API 确认真伪（trufflesecurity.com/blog/how-trufflehog-verifies-secrets）——「regex-only 扫不出真假」的直接解法；部分验证是多步流（待验证：具体哪些 detector 需要两段式）。
2. **多数据源抽象**：同一检测管线接 git / docker / S3 / slack 等源——源插件化。
3. 验证需要网络出站访问目标 provider——本地优先工具需把验证设计成**可选、可代理、可断网降级**。

**档位【有界】**——**AGPL-3.0（2023 年起），严禁代码复用**；验证优先思想与 detector=代码的组织方式值得抄。
**第一步动作**：设计 ADV 的「验证器 trait」：`verify(secret) -> Verified/Invalid/Unknown`，默认离线 Unknown，联网模式才启用——把 AGPL 边界与网络边界都写进蓝图。
**许可证**：AGPL-3.0（只抄思想）。**链接**：https://github.com/trufflesecurity/trufflehog · https://trufflesecurity.com/blog/how-trufflehog-verifies-secrets

### 12. gitleaks

**一行定位**：Go 单二进制、纯确定性 regex+熵 的 secrets 扫描器，速度与 CI/pre-commit 集成见长，无验证能力。

**值得抄的机制**：
1. **TOML 规则极简**：~180 条内置规则，自定义门槛极低；支持 allowlist（路径/正则）做 ignore。
2. **默认扫全量 commit 历史**——不只工作树，适合首次审计场景。
3. 学术对照锚点（arXiv 2307.00714，2023）：gitleaks recall ~88% 但 precision ~46%（GitHub Secret Scanning precision ~75% 对照）——说明**纯 regex+熵路线精度天花板有限**，佐证 ADV 走「规则+去重+ML/验证」而非「熵堆料」。

**档位【吸收】**——MIT，规则格式与 allowlist 机制可直接借鉴实现（机制简单，自研成本低于引入 Go 工具）。
**第一步动作**：把 gitleaks TOML 规则集当作 ADV 内置规则的「移植语料」清单来源（MIT 允许，保留其版权头）。
**许可证**：MIT。**链接**：https://github.com/gitleaks/gitleaks · https://arxiv.org/html/2307.00714v1

### 13. detect-secrets（Yelp）

**一行定位**：Python 插件化 secrets 扫描器，核心贡献是 **baseline 文件工作流**。

**值得抄的机制**：baseline = 已知/已接受发现落盘，后续扫描只报**新增**；配合人工 audit 记录形成渐进式清欠（Rafter 对比文）。插件架构（每类 secret 一个 plugin）组织清晰。
**档位【有界】**——Apache-2.0，代码是 Python 且插件机制简单；抄 baseline 工作流即可，不引依赖。
**第一步动作**：统一 ADV 的「已知问题账本」：secrets baseline、SAST hotspot 复核、否证时间戳共用一种账本格式。
**许可证**：Apache-2.0。**链接**：https://github.com/Yelp/detect-secrets

---

## 三、SCA / 供应链

### 14. osv-scanner（Google）

**一行定位**：围绕 **OSV.dev 数据库与 OSV schema** 的依赖漏洞扫描器，强项是精确的版本区间匹配与生态一致性。

**值得抄的机制**：
1. **OSV 数据格式当 interchange**：漏洞记录=机器可读 JSON，带「生态特定的规范化版本区间」——ADV 的 Rust 依赖告警可直接吃 OSV/RustSec 数据，避免自建库。
2. **离线数据库模式**（本地优先友好）。
3. 可达性集成（Go 生态接 govulncheck 做调用图过滤，把「有洞」收窄到「可达的洞」；Rust 侧对应物可参考 cargo call-stack / rustdoc JSON，实现待验证）。
**档位【吸收】**——Apache-2.0；格式与离线模式直接用。
**第一步动作**：设计 ADV 依赖扫描输出为 OSV 兼容 JSON + `reachable: true/false` 字段（可达性判定可后置）。
**许可证**：Apache-2.0。**链接**：https://github.com/google/osv-scanner

### 15. Syft + Grype（Anchore）——「清单与匹配分离」的范本

**一行定位**：Syft 只生成 SBOM（每生态一个 cataloger），Grype 只拿 SBOM 做漏洞匹配（NVD+厂商 feed）——两个工具一个接口。

**值得抄的机制**：
1. **清单/匹配分离**：同一份 SBOM 可随数据库更新反复重扫，不用重新盘点（safeguard.sh 对比文指出这是其最干净的架构优点）。
2. **cataloger/matcher 插件结构**：每种生态一个盘点器、每类漏洞源一个匹配器，互不耦合。
3. **反面教训（重要）**：Manifest Cyber 实测——用 Trivy 生成的 SBOM 喂 Grype，与 Syft 原配相比结果差异 ~66%；另有研究（LinkedIn 转载，数万容器镜像样本，2025-2026）称三工具间 66.8% 的结果分歧。**结论（带口径）：SCA 输出高度依赖生成器与扫描器的配对**——ADV 必须自持清单格式，不假装能对齐所有第三方 SBOM。

**档位【吸收】**——双 Apache-2.0；架构范式直接采纳。
**第一步动作**：ADV 内部定型 `Inventory（清单）→ Matcher（匹配）→ Report` 三段接口，清单可序列化落盘复扫。
**许可证**：Apache-2.0。**链接**：https://safeguard.sh/resources/blog/trivy-vs-grype · https://www.manifestcyber.com/blog/the-hidden-chaos-behind-vulnerability-scan-results

### 16. Trivy（Aqua）

**一行定位**：all-in-one 扫描器（漏洞/secret/IaC/license/SBOM 一个二进制），本地嵌入数据库是其快的原因。
**值得抄的机制**：单二进制+嵌入式本地 DB（首次下载后离线可用）——与「本地优先」完全同构；但一体化架构的代价是单工具结果与专精工具分歧（见上）。Grype 纯漏洞匹配比 Trivy 全家桶快 30–40%（devsecops.ae 2026 对比口径，样本为一类容器镜像扫描，非普适结论）。
**档位【有界】**——Apache-2.0；理念（本地 DB、单二进制）ADV 已有共识，无需引代码。
**第一步动作**：无独立动作；把「本地嵌入式漏洞库 + 断网可用」列为验收判据。
**许可证**：Apache-2.0。**链接**：https://github.com/aquasecurity/trivy · https://devsecops.ae/trivy-vs-grype

### 17–21. cargo 供应链六件套（Rust 原生，全部 Apache-2.0/MIT 双许可）

- **cargo-audit**【吸收】：吃 **RustSec Advisory Database（JSON）** 做响应式告警——RustSec 数据库本身是 ADV 的第一数据源。动作：订阅 RustSec 源，做成「变更时增量告警」。https://github.com/rustsec/rustsec
- **cargo-deny**【吸收】：**策略引擎**——license/ban/duplicate/advisory 四类规则一份 TOML 配置过依赖图；与 cargo-audit 的分工见 EmbarkStudios issue #386（audit 管告警面窄，deny 管策略面宽）。动作：ADV 的「项目门禁」配置模型抄这个四段式。https://github.com/EmbarkStudios/cargo-deny
- **cargo-vet**（Mozilla）【有界】：**前瞻式信任**——审计记录（vetted/exemptions/imports）是数据文件（supply-chain.toml），把「人审过哪些 crate」变成可验证、可导入（trust 聚合）的账本。思想值得抄给 ADV 的自身供应链声明；机制重（需要维护成本），不照搬。https://mozilla.github.io/cargo-vet/how-it-works.html
- **cargo-auditable**【吸收】：**把依赖清单嵌进编译产物**（JSON 段入二进制）——产物自带 SBOM，运行时资产可被事后扫描。动作：ADV 自身发布即带 auditable 段；扫描能力上「识别产物内嵌清单」可做成特性。https://github.com/rust-secure-code/cargo-auditable
- **cargo-semver-checks**【有界】：用 **rustdoc JSON 语义 diff** 抓破坏性 API 变更（非文本 diff）——思想对 ADV 的「规则升级后 API 兼容性自检」有用；其已知局限（宏生成的 API、trait 方法默认实现的边角，predr.ag 四难题文）同样适用于任何 rustdoc-JSON 方案。https://github.com/obi1kenobi/cargo-semver-checks · https://predr.ag/blog/four-challenges-cargo-semver-checks-has-yet-to-tackle

**许可证汇总（可直接复用白名单）**：Nosey Parker、osv-scanner、Syft/Grype、Trivy、gitleaks（MIT）、detect-secrets、cargo-* 六件套、CodeQL 查询库（MIT，仅查询单元）——均 Apache-2.0/MIT 兼容。**不可复用代码**：TruffleHog（AGPL-3.0）、Semgrep Pro 引擎、CodeQL CLI、全部商业平台（Sonar 高级版/Qodana/Snyk/Aikido/Checkmarx/Fortify/Veracode）——只抄思想。

---

## 四、2025–2026 前沿信号

1. **LLM triage 已进入生产，且都长成同一个形状**：Semgrep 官方博客（2025）称 Assistant 自动 triage 达 60%（含 AI memory 复用历史 triage 决策，20% 场景）；Aikido AutoTriage = 可达性引擎 → LLM 只看剩余。两家的公共结构：**确定性过滤在前、LLM 在后、LLM 结论缓存成可复用记忆**。成本估计（$0.001–0.12/告警，LinkedIn 分析文）口径不明，仅作量级参考。
2. **跨文件分析 = 全行业的付费墙**：Semgrep Pro、Snyk Code 的商业核心都是 interfile 数据流；OSS 侧普遍缺位——本地 Rust 工具若做好「目录级污点分析 + 精确度可解释」，即是空白带（难度与建库成本见 01 号调研）。
3. **SCA 结果分歧实证化**：66–67% 量级的工具间结果分歧（2025–2026 多源实测，容器镜像样本）——「SCA 答案取决于工具配对」从经验变成了数据；自持清单+自持匹配器、对外诚实标注覆盖口径，是本地工具的生存位。
4. **secrets 三条路线分化清晰**：验证优先（TruffleHog，AGPL，联网）、去重+ML 降噪优先（Nosey Parker，Apache，离线）、极简 regex 优先（gitleaks，MIT，快）。学术对照（arXiv 2307.00714）证明纯 regex+熵的 precision 天花板。ADV 取第二路 + 可选第一路的验证器接口，是离线优先约束下的最优组合（待实现验证）。
5. **供应链从「查已知洞」转向「管信任账本」**：cargo-vet（审计记录即数据）、cargo-auditable（产物自描述）、OSV schema 成为事实 interchange；arXiv 2026 Cargo Scan 论文显示学界也在向 Rust 审计倾斜。

---

## 五、Top-3（对新项目最值钱的 3 条）

1. **Secrets 引擎直接吸收 Nosey Parker（Apache-2.0, Rust）**：内容寻址 datastore + 跨运行增量（按 blob hash 跳过）+ 单捕获组 YAML 规则 + 按 secret 内容去重（10–1000x review 降噪）——这是一套许可干净、语言一致、已验证（GB/s 实测口径）的完整方案；先评估依赖，后评估内化。
2. **SCA 采用「清单/匹配分离」架构并锚定 OSV/RustSec（均 Apache/MIT）**：Inventory→Matcher→Report 三段接口 + 本地嵌入式库 + `reachable` 字段——把 66% 分歧教训转化为「自持清单、明示口径」的产品判据。
3. **误报控制做成四层流水线，顺序不可倒**：规则置信度与负模式（Semgrep 式）→ baseline/已知问题账本（detect-secrets/gitleaks 式）→ 可达性/去重确定性过滤（Aikido 式）→ LLM 复核且结论缓存成记忆（Semgrep Assistant 式）——每层独立可关、可审计，LLM 永不前置。
