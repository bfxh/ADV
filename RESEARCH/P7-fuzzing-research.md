# P7 — Fuzzing 研究前沿论文扫视（2023–2026）

- 日期：2026-10-04；域：fuzzing 研究前沿（工具层之上的算法与实证）；主分支锚：adv-rewrite
- 方法：WebSearch 定向检索（"USENIX Security 2025 fuzzing"、"FuzzBench 2025"、"LLM fuzzing harness"等）+ 顶会收录页收割（S&P 2024 官方 accepted-papers 页全文收割成功）
- 条目：54（P0 6 / P1 11 / P2 21 / watch 16）；registry 见 `registry/P7.jsonl`
- 检索局限留档（本次实测）：搜索后端并发上限约 2 条/轮；`sp2023.ieee-security.org/accepted-papers.html` 返回 404；DBLP 被 Anubis 反爬拦截；USENIX Security 2024 议程页抓取截断。因此 USENIX'24 / CCS'24 / ICSE'25 全清单未收割，条目分布偏向已验证来源；标注「待验证」的条目只到标题+摘要快照级，未复核到 DOI/正式版。
- 口径：每篇必须能说出"可抄机制"才收录；结论均为检索快照级（snippet/abstract），不是精读——引用前应先取全文复核。

---

## P0 清单（≤6，读了能直接改哪个设计决策）

| # | 论文 | venue+年份 | 读了能直接改哪个设计决策 |
|---|------|-----------|------------------------|
| 1 | SoK: Prudent Evaluation Practices for Fuzzing | IEEE S&P 2024 | 改 wave-0 / 6h 深跑的对比验收协议：最少运行轮数、统计显著性检验、方差披露——防止"我的 fuzzer 更好"式自欺基准。 |
| 2 | LIBAFL: Framework for Building Scalable and Reusable Fuzzers | USENIX NSDI 2023 | 改 fuzz core 架构：Scheduler/Feedback/Executor 组件化可插拔，且是 Rust 同源框架——决定自研循环还是复用组件模型。 |
| 3 | Fuzz4All: Universal Fuzzing with Large Language Models | ICSE 2024 | 改 adv-parse 自测的输入生成策略：LLM"目标描述→prompt→种子/变异"，配差分 oracle——替代纯手写语法模板。 |
| 4 | AFGen: Whole-Function Fuzzing for Applications and Libraries | IEEE S&P 2024 | 改 scanner 结构感知 harness 生成策略：以整个函数为单位、目标库零修改产驱动——决定要不要自建 harness 生成器。 |
| 5 | FuzzBench（SOSP 2021 论文 + fuzzbench.com 2026 实时报告） | Google / SOSP 2021–今 | 改 6h 深跑内嵌哪些 fuzzer 组合与基线对比集：以平均秩+显著性报告为选型依据，而非论文自报数字。 |
| 6 | OSS-Fuzz-Gen（AI-powered fuzz target generation） | Google 项目, 2024– | 改 harness 自动化的边界：LLM 产 fuzz target 的真实成功率与人工兜底配比（Google 已部署的经验数据）。 |

---

## 0. 引擎架构底座

### 2. LIBAFL: Framework for Building Scalable and Reusable Fuzzers
- venue+年份：USENIX NSDI 2023（Fioraldi, Mantovani, Maier, Balzarotti）
- 一句话结论：把 fuzzer 拆成 Scheduler/Feedback/Executor/Observers 等可复用组件，进程内 fuzzing 与分布式扩展统一在一个框架里。
- 可抄机制：组件化 trait 抽象（尤其 Feedback/Energy Scheduler 的组合方式）；in-process 执行器与崩溃恢复（`InProcessExecutor` 的 forkserver/重置模式）。
- 模块映射：fuzz core 架构 → reference（Rust 同源，可直接评估部分复用）。
- 优先级：P0；链接：https://www.usenix.org/conference/nsdi23/presentation/fioraldi（高置信未复核）

## A. 理论与实证评测（SoK / 基准 / FuzzBench 现状）

### 1. SoK: Prudent Evaluation Practices for Fuzzing
- venue+年份：IEEE S&P 2024（官方收录页今日已验证）
- 一句话结论：对 fuzzing 论文评测实践的系统化梳理，给出可执行的评测改进建议清单（统计口径、运行数、报基线）。
- 可抄机制：评测协议清单——固定种子数下限、报分布不只报均值、显著性检验先行。
- 模块映射：wave-0 验收/xtask-gates（6h 深跑与 PR 重放的结论判据）。
- 优先级：P0；链接：https://sp2024.ieee-security.org/accepted-papers.html（本页同时收录 AFGen/Titan/Chronos 等）

### 5. FuzzBench（开放 fuzzer 基准平台与 2025/2026 实时报告）
- venue+年份：SOSP 2021 论文；fuzzbench.com 报告持续更新（2026 现状为实时报告而非新论文）
- 一句话结论：Google 规模的可复现 fuzzer 对比服务，用平均秩+统计显著性报告横评，当前权威数据在实时报告里。
- 可抄机制：实验矩阵管理（fuzzer×benchmark×重复次数）、平均秩可视化、显著性标注；其 Docker 化 benchmark 定义可直接借鉴为 ADV 的回归基线。
- 模块映射：6h 深跑选型 + xtask-gates 基线对比。
- 优先级：P0；链接：https://www.fuzzbench.com ；文档 https://google.github.io/fuzzbench/reference/report ；代码 https://github.com/google/fuzzbench （三者今日已验证）

### 29. On the Reliability of Coverage-Based Fuzzer Benchmarking
- venue+年份：ISSTA 2022（临界，作背景）
- 一句话结论：覆盖更多代码的 fuzzer 确实倾向找更多 bug，但"哪个 fuzzer 更强"在覆盖口径下经常无法显著区分。
- 可抄机制：把"覆盖增长"与"bug 产出"分开报告；对覆盖指标的置信区间要求。
- 模块映射：输出 oracle 与验收指标设计 → reference。
- 优先级：P2；链接：待验证（检索快照，scispace 索引页）

### 30. Fuzzing: On Benchmarking Outcome as a Function of Experimental Variables
- venue+年份：arXiv 2212.09519（2022-12，v2 更新；临界）
- 一句话结论：fuzzing 基准结论随实验变量（时长/轮数/种子）系统性变化，单窗口结论不稳。
- 可抄机制：变量敏感性扫描设计——ADV 验收判据应对"跑多久才显著"给出锚点。
- 模块映射：xtask-gates（验收时序独立性）。
- 优先级：P2；链接：https://arxiv.org/html/2212.09519v2（今日已验证）

### 31. Green Fuzzer Benchmarking
- venue+年份：ISSTA 2023
- 一句话结论：典型 FuzzBench 实验≈11 fuzzer×20 程序×20 次，能耗/算力成本可测且可观。
- 可抄机制：把 fuzzing 预算（CPU 时/能耗）作为一阶指标报告——6h 深跑的预算管理依据。
- 模块映射：持续 fuzzing 预算管理 → reference。
- 优先级：P2；链接：待验证

### 51. FuzzingPaper（fengjixuchui 维护的近期 fuzzing 论文清单）
- venue+年份：GitHub 持续更新（meta 资源）
- 一句话结论：社区维护的 fuzzing 论文追踪仓库，可做 P7 域的增量补充源。
- 可抄机制：按 venue 分类的论文索引结构（直接可抄进 ADV 的 registry 更新流程）。
- 模块映射：registry 维护 → reference。
- 优先级：watch；链接：https://github.com/fengjixuchui/FuzzingPaper（今日已验证）

### 52. NDSS 2025 官方收录清单
- venue+年份：NDSS 2025（列表资源）
- 一句话结论：本次未完整收割（抓取失败），留链接待补收割；已知含 SYSYPHUZZ、GPU 驱动 fuzzing、FuzzUEr 等。
- 可抄机制：——（资源型条目）
- 模块映射：P7 增量扫描源 → reference。
- 优先级：watch；链接：https://www.ndss-symposium.org/ndss2025/accepted-papers/（URL 真实，本次抓取失败）

## B. 覆盖度量与语义反馈（对输出 oracle 的价值）

### 7. Following Dragons: Code Review-Guided Fuzzing
- venue+年份：arXiv（2024–2025 快照）
- 一句话结论：用代码评审注解做语义反馈（annotation-aware fuzzing），补边缘覆盖分不清"语义有意义的程序状态"这一结构性缺陷。
- 可抄机制：把人工/静态分析产生的语义注解编译成覆盖之外的反馈信号——ADV 输出 oracle 的直接参考。
- 模块映射：输出 oracle → reference。
- 优先级：P1；链接：待验证（arXiv 索引快照）

### 28. BigMap: Future-proofing Fuzzers with Efficient Large Maps
- venue+年份：2024–2025（作者主页快照，A. Ahmed）
- 一句话结论：边缘覆盖 bitmap 小方差导致 fuzzer 间难区分，提出高效大容量覆盖图。
- 可抄机制：覆盖位图容量/冲突处理的可扩展设计（Rust 端数据结构可直接参考）。
- 模块映射：fuzz core 覆盖存储 → reference。
- 优先级：P2；链接：https://alifahmed.github.io（作者页，具体条目待验证）

### 53. SoK: The Good, the Bad, and the Unbalanced: Measuring Structural Limitations of Fuzzing Campaigns
- venue+年份：NDSS 2023
- 一句话结论：量化覆盖反馈的"不平衡"——某些区域系统性难达、覆盖增长滞后于实际探索，单次 campaign 的覆盖曲线有结构性偏差。
- 可抄机制：不平衡度量（分区域覆盖达成率）——ADV 报告层可借此区分"没覆盖"与"覆盖了但无反馈"。
- 模块映射：输出 oracle / 覆盖反馈设计 → reference。
- 优先级：P1；链接：待验证（NDSS paper 页 slug 未复核）

### 54. Beyond Edge Coverage: Per-Task Data-Flow Extraction at Scale（内核 fuzzing 语境）
- venue+年份：arXiv（2026-06 快照）
- 一句话结论：以 syzkaller 为例指出边缘覆盖是唯一反馈信号的不足，提出按任务粒度的数据流抽取作为补充反馈。
- 可抄机制：per-task 数据流特征提取（机制与内核无关，可移植到解析面自测）。
- 模块映射：输出 oracle / adv-parse → reference。
- 优先级：P2；链接：待验证（arXiv 快照，未取得编号）

## C. 语料管理与种子选择（能量调度/去重/最小化——工程实证）

### 9. Seed Selection for Successful Fuzzing
- venue+年份：EPFL（约 2023，检索快照）
- 一句话结论：初始语料构成（空文件/单种子/精简集）显著影响 fuzzer 评测结局。
- 可抄机制：语料初始化对照实验设计——ADV 各档（PR 重放/6h）的种子包应分层配置而非一份通用语料。
- 模块映射：语料管理 → test-replay。
- 优先级：P1；链接：待验证

### 10. Ensemble Fuzzing with Dynamic Resource Scheduling
- venue+年份：arXiv 2025（Y. Zhao，快照）
- 一句话结论：多 fuzzer 集成 + 动态资源再调度优于静态分配。
- 可抄机制：按实时收益（覆盖增速/新崩溃率）在 fuzzer 实例间迁移 CPU 预算——6h 深跑的多 fuzzer 预算分配。
- 模块映射：6h 深跑调度 → test-replay。
- 优先级：P1；链接：待验证

### 26. Learning Seed-Adaptive Mutation Strategies for Greybox Fuzzing
- venue+年份：约 2023（Korea Univ. PRL，高被引快照）
- 一句话结论：按种子学习自适应变异策略（多臂赌博机式）优于全局统一变异。
- 可抄机制：per-seed 变异算子权重的在线学习（Havoc 档位分配）。
- 模块映射：变异引擎 → reference。
- 优先级：P2；链接：https://prl.korea.ac.kr（实验室页，具体条目待验证）

### 27. Dissecting American Fuzzy Lop: A FuzzBench Evaluation
- venue+年份：ACM 2023（Fioraldi 等，快照）
- 一句话结论：在 FuzzBench 口径下解剖 AFL 各组件（变异/调度/反馈）的独立贡献。
- 可抄机制：组件消融实验方法——ADV 改任何一个 fuzz 组件前先做消融基线。
- 模块映射：fuzz core 调参 → reference。
- 优先级：P2；链接：待验证

### 32. GPU Driver Fuzzing by Recalling In-Vivo Execution States
- venue+年份：NDSS 2025
- 一句话结论：回放"体内"执行状态来引导 fuzzing 到达深层状态。
- 可抄机制：状态回放引导（从真实运行记录反哺种子生成）——PR 重放档可直接借鉴。
- 模块映射：语料管理/种子生成 → reference。
- 优先级：P2；链接：待验证

### 33. SYSYPHUZZ and the Pressure of More Coverage
- venue+年份：NDSS 2025
- 一句话结论：内核 syscall 序列 fuzzing 中"更多覆盖压力"的机制与代价分析。
- 可抄机制：覆盖压力与崩溃产出之间的权衡度量（避免为覆盖而覆盖）。
- 模块映射：6h 深跑终止判据 → reference。
- 优先级：P2；链接：https://kdsjzh.github.io（作者页，今日已验证存在）

## D. 结构感知 / 语法感知（grammar 挖掘 / 输入结构推断）

### 8. Quantifying the Limitations of Learning-Assisted Grammar-Based Fuzzing
- venue+年份：2024（Semantic Scholar 索引快照）
- 一句话结论：负结果向——语法学习辅助的 fuzzing 在可解析输入程序上的收益存在可量化的边界条件。
- 可抄机制：收益边界判据（何时值得投入 grammar 学习）——决定 ADV adv-parse 是否自建语法挖掘。
- 模块映射：adv-parse 投入决策 → reference。
- 优先级：P1；链接：待验证

### 16. UPFUZZ: Detecting Data Format Incompatibility Bugs
- venue+年份：USENIX Security 2025（ proceedings 收录，快照）
- 一句话结论：用 fuzzing 检测数据格式不兼容类 bug（同一格式多实现间行为分歧）。
- 可抄机制：跨实现格式分歧作 oracle——adv-parse 解析面自测可直接抄（多解析器差分）。
- 模块映射：adv-parse → test-replay。
- 优先级：P1；链接：https://www.usenix.org（主站已验证存在，论文页 slug 待验证）

### 22. Generating Inputs for Grammar Mining using Dynamic Symbolic Execution
- venue+年份：arXiv 2508.03832（2025）
- 一句话结论：用 DSE 生成高质量输入集喂给 grammar 挖掘，提升学得文法的覆盖。
- 可抄机制：DSE 语料构造→挖掘前置——比随机/真实语料喂挖掘器更系统。
- 模块映射：adv-parse 语法挖掘语料 → reference。
- 优先级：P2；链接：https://arxiv.org/abs/2508.03832（检索返回编号，今日已验证）

### 23. Inferring Attributed Grammars from Parser Implementations
- venue+年份：IEEE（约 2024，快照）
- 一句话结论：从解析器实现直接挖属性文法（含语义属性，不止 CFG）。
- 可抄机制：属性文法挖掘流程——带语义约束的输入生成，胜过裸 CFG。
- 模块映射：adv-parse 语法建模 → reference。
- 优先级：P2；链接：待验证（computer.org 索引）

### 24. Fuzzing-based Grammar Learning from a Minimal Set of Seed Inputs
- venue+年份：期刊（约 2024–2025，快照；Kaufmann 等）
- 一句话结论：小种子集上通过 fuzzing 反馈学文法（递归下降解析器场景）。
- 可抄机制：种子稀缺场景的增量文法学习循环。
- 模块映射：adv-parse 冷启动 → reference。
- 优先级：P2；链接：待验证

### 25. MOCK: Optimizing Kernel Fuzzing Mutation with Context-aware Dependency
- venue+年份：2024（快照）
- 一句话结论：用上下文依赖关系约束变异，减少无效变异。
- 可抄机制：依赖图约束的变异裁剪（结构与内核无关，可移植）。
- 模块映射：变异引擎 → reference。
- 优先级：P2；链接：待验证

### 46. XVADA（黑盒上下文无关文法泛化推断）
- venue+年份：约 2024（快照）
- 一句话结论：从种子观测泛化终结符，黑盒推断更准的 CFG。
- 可抄机制：文法泛化/去特化迭代。
- 模块映射：adv-parse → reference。
- 优先级：watch；链接：待验证

### 47. FLAT: Formal Languages as Types
- venue+年份：2024（快照）
- 一句话结论：把输入文法当类型系统，用符号解析为递归下降解析器挖文法。
- 可抄机制：类型化文法挖掘（与 Rust 类型系统同思路）。
- 模块映射：adv-parse → reference。
- 优先级：watch；链接：待验证

## E. 差分 / 元变异 fuzzing（编译器/解析器面自测）

### 12. StaAgent: An Agentic Framework for Testing Static Analyzers
- venue+年份：arXiv（2025-07 快照）
- 一句话结论：用 agentic LLM 驱动的元变异测试找静态分析器自身的 bug。
- 可抄机制：元变异等价变换+LLM 判等代理——ADV 扫描器自测（对自身 SAST 面）可直接参考。
- 模块映射：adv-parse / 扫描器自测 → reference。
- 优先级：P1；链接：待验证（arXiv 快照）

### 13. WhiteFox: White-Box Compiler Testing Powered by Large Language Models
- venue+年份：OSDI 2023
- 一句话结论：把"触发某编译器优化 bug 的测试用例该满足什么条件"喂给 LLM 生成用例，白盒化编译器测试。
- 可抄机制：白盒触发条件→LLM 提示→用例生成闭环——解析面自测的 oracle 生成参考。
- 模块映射：adv-parse oracle 生成 → reference。
- 优先级：P1；链接：待验证（arXiv 编号未复核）

### 34. OpDiffer: LLM-assisted Industrial-Scale Differential Testing of EVM
- venue+年份：2024–2025（快照）
- 一句话结论：LLM+静态分析批量产测试输入做虚拟机大规模差分（工业部署）。
- 可抄机制：差分测试的规模化输入生产线（LLM 产种子+静态约束过滤）。
- 模块映射：差分 oracle → reference。
- 优先级：P2；链接：待验证

### 35. Fuzzing Processing Pipelines for Zero-Knowledge Circuits
- venue+年份：ACM（2024–2025 快照）
- 一句话结论：对 ZK 编译管线做系统化 fuzzing，用元变异 oracle 抓逻辑 bug——编译器形态系统的直接案例。
- 可抄机制：管线级元变异 oracle（对编译/转换步骤的等价性断言）。
- 模块映射：adv-parse/转换管线自测 → reference。
- 优先级：P2；链接：待验证

## F. API / 库级 fuzzing（无修改的 harness 生成）

### 4. AFGen: Whole-Function Fuzzing for Applications and Libraries
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：以整个函数为单元自动生成 fuzz harness，应用与库通用，目标代码零修改。
- 可抄机制：whole-function harness 构造（参数合成+调用序列），替代手写驱动。
- 模块映射：scanner 结构感知 harness → reference。
- 优先级：P0；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 21. SyzGen++: Dependency Inference for Augmenting Kernel Driver Fuzzing
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：静态推断驱动接口间依赖以增强 harness（内核场景，机制通用）。
- 可抄机制：API 依赖推断→harness 调用序列排序。
- 模块映射：harness 生成依赖建模 → reference。
- 优先级：P2；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 48. Speculate（静态分析+LLM 自动产 fuzz target）
- venue+年份：FSE（2024–2025 卷，快照；conference-publishing.com 索引）
- 一句话结论：轻量静态分析约束 LLM 生成 fuzz target，声称首个该组合。
- 可抄机制：静态分析作为 LLM 产 harness 的护栏（可编译性/可达性过滤）。
- 模块映射：harness 生成 → reference。
- 优先级：P2；链接：待验证

## G. LLM×fuzzing（只收有实测的）

### 3. Fuzz4All: Universal Fuzzing with Large Language Models
- venue+年份：ICSE 2024（Wang 等，Georgia Tech/清华；检索已验证存在）
- 一句话结论：首个"通用"fuzzer——LLM 两阶段：目标描述自动转 prompt（配置阶段），LLM 生成/变异输入（fuzzing 循环），在 C/C++/Go/SMT2/Java 等多语言上实测。
- 可抄机制：配置阶段产 prompt + LLM 变异循环 + 差分/崩溃 oracle 的三件套。
- 模块映射：adv-parse 自测输入生成 → reference。
- 优先级：P0；链接：https://arxiv.org/abs/2308.04748（高置信，未复核到 arXiv 页）

### 11. SyzHarness: Patch-Based Kernel Bug Reproduction with LLM Reasoning and Coverage-Guided Fuzzing
- venue+年份：arXiv（2026 快照）
- 一句话结论：LLM 推理+覆盖制导 fuzzing 做 patch 基的内核 bug 重放。
- 可抄机制：patch 差异→可测输入的推导——wave-0 PR 重放档的直接参考。
- 模块映射：test-replay。
- 优先级：P1；链接：待验证（arXiv 快照）

### 14. TitanFuzz（Large Language Models are Zero-Shot Fuzzers）
- venue+年份：ISSTA 2023
- 一句话结论：LLM 零样本生成+变异深度学习库输入，无需人工种子。
- 可抄机制：生成式种子生产线（生成模型产输入→过滤→进语料）。
- 模块映射：种子生成 → reference。
- 优先级：P1；链接：待验证（arXiv 编号未复核）

### 43. BUGSTONE: LLMs for Large-Scale Bug Discovery
- venue+年份：2024–2025（OpenReview 快照）
- 一句话结论：神经符号框架（静态分析+LLM 推理）检测重复出现的 bug 模式并辅助分诊。
- 可抄机制：重复 bug 模式聚类——报告层去重的语义化思路。
- 模块映射：报告层 → reference。
- 优先级：watch；链接：待验证（openreview 快照）

### 44. STITCH（LLM 驱动的自动化 fuzzing：配置/规格/崩溃分诊）
- venue+年份：2025（ResearchGate 快照）
- 一句话结论：LLM 处理项目配置、规格合成与自动崩溃分诊的端到端 fuzzing。
- 可抄机制：配置合成+崩溃自动分诊的 LLM 编排。
- 模块映射：harness 生成/报告层 → reference。
- 优先级：watch；链接：待验证

### 45. SyzMutateX（LLM 优化内核 fuzz 变异）
- venue+年份：2025（Semantic Scholar 快照）
- 一句话结论：LLM 驱动优化 syzkaller 变异策略。
- 可抄机制：LLM 在环的变异策略调优（非每输入调用 LLM，而是调策略）。
- 模块映射：变异引擎 → reference。
- 优先级：watch；链接：待验证

## H. 崩溃分诊（去重/根因/严重度——对报告层）

### 15. How Well Industry-Level Cause Bisection Works in Real World
- venue+年份：ACM 2024（快照）
- 一句话结论：业界级根因二分在真实场景成功率有限——失败重放不稳与分诊不可靠是主因（快照称 80%+ 失败与之相关）。
- 可抄机制：根因聚类 vs 栈去重的对比判据——merge 去重档不能只按栈哈希。
- 模块映射：merge 去重/报告层 → test-replay。
- 优先级：P1；链接：待验证

### 20. Chronos: Finding Timeout Bugs in Practical Distributed Systems by Deep-Priority Fuzzing with Transient Delay
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：瞬态延迟注入+深优先调度找超时类 bug——罕见事件类 bug 的代表。
- 可抄机制：罕见事件（超时/hang）的优先级调度与 oracle 分类——报告层严重度分级参考。
- 模块映射：报告层/hang 类分诊 → test-replay。
- 优先级：P2；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 50. Kirenenko（执行前缀等价的崩溃去重）
- venue+年份：ACM CCS 2022（范围外临界，作背景）
- 一句话结论：按执行前缀等价类做崩溃去重与根因定位，在大规模 fuzzing 分诊上实测有效。
- 可抄机制：执行前缀哈希做崩溃聚类（比栈哈希细、比全执行轨迹省）——merge 去重直接可抄。
- 模块映射：merge 去重 → test-replay。
- 优先级：watch（2022 临界）；链接：待验证

## I. 定向 fuzzing 与多目标

### 17. Titan: Efficient Multi-target Directed Greybox Fuzzing
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：多目标定向 fuzzing 的目标间调度优化。
- 可抄机制：多目标距离计算的增量维护——PR 触及函数集作定向目标的直接参考。
- 模块映射：test-replay（PR 定向回归）→ reference。
- 优先级：P2；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 18. Predecessor-aware Directed Greybox Fuzzing
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：利用目标点前驱约束提升定向到达率。
- 可抄机制：前驱路径约束的定向引导。
- 模块映射：test-replay → reference。
- 优先级：P2；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 19. Everything is Good for Something: Counterexample-Guided Directed Fuzzing via Likely Invariant Inference
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：用似然不变式推断做反例引导的定向 fuzzing。
- 可抄机制：不变式作为定向与 oracle 的双重信号。
- 模块映射：输出 oracle → reference。
- 优先级：P2；链接：https://sp2024.ieee-security.org/accepted-papers.html

### 41. To Boldly Go Where No Fuzzer Has Gone Before（VirtIO 设备通道进内核无线栈）
- venue+年份：IEEE S&P 2024（官方页已验证）
- 一句话结论：借虚拟设备通道把 fuzzing 带进原本难达的内核子系统。
- 可抄机制：合成设备/通道作为 fuzzing 入口（难达面的一般化手段）。
- 模块映射：难达代码面 → reference。
- 优先级：watch；链接：https://sp2024.ieee-security.org/accepted-papers.html

## J. 持续 fuzzing 工程化（Oss-Fuzz 系经验）

### 6. OSS-Fuzz-Gen（AI-powered fuzz target generation）
- venue+年份：Google 项目，2024–（论文版链接待验证）
- 一句话结论：LLM 自动为 Oss-Fuzz 项目生成 fuzz harness 并已有真实部署流水线与成功率数据。
- 可抄机制：harness 生成的部署流水线（生成→构建→覆盖反馈筛选→人工兜底），及 Google 公布的成功率锚点。
- 模块映射：scanner harness 自动化边界 → reference。
- 优先级：P0；链接：https://github.com/google/oss-fuzz-gen（高置信）

### 49. SiliFuzz（Google CPU quirk 持续 fuzzing）
- venue+年份：ICSE-SEIP 2023（快照）
- 一句话结论：Google 对 CPU 层缺陷的持续 fuzzing 工程实践（快照/重放/ Canary 语料管理）。
- 可抄机制：快照+重放的持续 fuzzing 循环与金丝雀语料管理——三档制的长期运行参考。
- 模块映射：test-replay / 语料管理 → reference。
- 优先级：watch；链接：待验证

## K. watch 堆场（机制可借鉴，不进近期路线）

### 36. Software Testing With Large Language Models（综述）
- venue+年份：ACM（2024，约 1171 引，快照）——LLM 测试生成的全景背景。
- 可抄机制：taxonomy 分层（生成/判分/编排）用于 ADV 内部讨论对齐。
- 模块映射：全域背景 → reference。优先级：watch；链接：待验证（computer.org 索引）

### 37. FuzzUEr: Enabling Fuzzing of UEFI Interfaces
- venue+年份：NDSS 2025——固件接口 fuzzing；机制：接口层 harness 合成。→ reference；watch；待验证。

### 38. SATURN: Host-Gadget Synergistic USB Driver Fuzzing
- venue+年份：IEEE S&P 2024——主机/设备协同制导；机制：双端协同状态机。→ reference；watch；sp2024 页已验证。

### 39. LLMIF: Augmented Large Language Model for Fuzzing IoT Devices
- venue+年份：IEEE S&P 2024——LLM 增强 IoT fuzzing；机制：LLM 产报文语义变异。→ reference；watch；sp2024 页已验证。

### 40. LABRADOR: Response Guided Directed Fuzzing for Black-box IoT Devices
- venue+年份：IEEE S&P 2024——黑盒响应制导定向；机制：响应反馈做距离度量。→ reference；watch；sp2024 页已验证。

### 42. Towards Smart Contract Fuzzing on GPU
- venue+年份：IEEE S&P 2024——GPU 并行 fuzzing；机制：千级并发实例的调度与去重。→ reference；watch；sp2024 页已验证。

---

## 与 ADV 三档制的落点小结

- PR 重放档：SyzHarness（patch→输入）、定向三篇（Titan/Predecessor-aware/反例引导）、GPU in-vivo 状态回放、Seed Selection 实证。
- 6h 深跑档：Ensemble 动态资源调度、Green Fuzzer Benchmarking（预算）、SoK Prudent（验收协议）、FuzzBench 选型、LibAFL 组件化。
- merge 去重/报告层：Industry Cause Bisection（判据）、Kirenenko（执行前缀）、BUGSTONE（模式聚类）、Chronos（hang/超时分类）。
- adv-parse 自测：Fuzz4All、AFGen、UPFUZZ（跨实现格式差分）、StaAgent/WhiteFox（oracle 生成）、grammar 挖掘四篇 + 负结果一篇（Quantifying Limitations）。
- 注意：以上均为检索快照级结论，落地前须取全文复核并按 AGENTS.md 口径补锚（venue/S 编号/复算方式）。

## USENIX Sec 25 / NDSS 25 补扫（2026-10-04）

与 P6 同源抓取，取 fuzzing 相关新增条目 P7-055–P7-065 共 11 条，link 为官方 presentation/PDF 页。CCS 方向 fuzzing 论文归入 P1 补扫章节（一论文只入一域，避免重复）。

- `P7-055` NASS: Fuzzing All Native Android System Services with Interface Awareness and Coverage（USENIX Sec 2025） — P1（OA-USENIX）
- `P7-056` ELFuzz: Efficient Input Generation via LLM-driven Synthesis Over Fuzzer Space（USENIX Sec 2025） — P1（OA-USENIX）
- `P7-057` IDFuzz: Intelligent Directed Grey-box Fuzzing（USENIX Sec 2025） — P2（OA-USENIX）
- `P7-058` AidFuzzer: Adaptive Interrupt-Driven Firmware Fuzzing via Run-Time State Recognition（USENIX Sec 2025） — P2（OA-USENIX）
- `P7-059` Fuzzing the PHP Interpreter via Dataflow Fusion（USENIX Sec 2025） — P2（OA-USENIX）
- `P7-060` ChainFuzz: Exploiting Upstream Vulnerabilities in Open-Source Supply Chains（USENIX Sec 2025） — P1（OA-USENIX）
- `P7-061` Low-Cost and Comprehensive Non-textual Input Fuzzing with LLM-Synthesized Input Generators（USENIX Sec 2025） — P2（OA-USENIX）
- `P7-062` Encarsia: Evaluating CPU Fuzzers via Automatic Bug Injection（USENIX Sec 2025） — P2（OA-USENIX）
- `P7-063` Automatic Library Fuzzing through API Relation Evolvement（NDSS 2025） — P1（open-access）
- `P7-064` Truman: Constructing Device Behavior Models from OS Drivers to Fuzz Virtual Devices（NDSS 2025） — P2（open-access）
- `P7-065` ICSQuartz: Scan Cycle-Aware and Vendor-Agnostic Fuzzing for Industrial Control Systems（NDSS 2025） — P2（open-access）

