# P2 · 软件工程五大顶会论文扫视（ICSE / FSE / ASE / ISSTA / TOSEM，2023–2026）

- 域负责人口径：工程导向扫视，服务 ADV（本地优先代码安全/质量平台）的模块设计决策。
- 检索日期：2026-10-03。方法：Semantic Scholar API（ topical 检索，venue 串短语归会场）+ Crossref API（ISSN/卷期精确判 FSE/ISSTA、TOSEM；按 DOI 前缀锚定 ICSE/FSE 各届）。
- 覆盖注记（防以偏概全）：DBLP 直连被反爬拦截（Anubis），ICSE 2023/2024 与 ISSTA 2023 的覆盖偏薄（只收单源可验证条目）；FSE/ISSTA 2024–2026 的场次由 Crossref `issue` 字段判定（PACMSE = Proc. ACM Softw. Eng.，Vol.1≈2024、Vol.2≈2025、Vol.3≈2026）；ASE 2025（DOI 前缀 10.1109/ASE63991）与 ICSE 2025（10.1109/icse55347）已双源确认；TOSEM 条目为在线首发年（2025–2026 混排，标注以 Crossref `issued` 为准）。所有链接为 DOI（来自 API 返回，非手写）。查不到确切信息的字段标「待验证」。
- 优先级：P0（≤8，读了直接改设计决策）> P1（机制清晰、建议排期精读）> P2（扫视留档）> watch（方向存疑或暂不落地）。

## P0 清单（读了能直接改哪个设计决策）

| # | 论文 | 直接改哪个设计决策 |
|---|------|--------------------|
| 1 | CodaMosa（ICSE 2023） | ADV fuzz 门遇到覆盖平台期时切换"LLM 生成 API 定向种子"再回 fuzz，而不是无脑加时长 |
| 2 | AutoCodeRover（ISSTA 2024） | ADV 修复模块采用"结构感知定位优先 + 测试验证闭环"，不做全文 LLM 改写 |
| 3 | RepairAgent（ICSE 2025） | 本地修复 runner 设计成显式状态机（诊断→定位→生成→验证），每个阶段配独立工具与提示 |
| 4 | Evaluating Agent-Based Program Repair at Google（ICSE-SEIP 2025） | 用工业界 APR 的采纳率/成本数据定 ADV 修复模块的目标：先做"定位+证据"，全自动合入另立项 |
| 5 | An Extensive Empirical Study of Nondeterministic Behavior in Static Analysis Tools（ICSE 2025） | ADV 扫描必须做可复现工程：固定分析环境、缓存键含工具/配置哈希、结果带"可复现性说明" |
| 6 | EvoTaint（TOSEM 2026） | adv-taint 的增量策略：哪些变化触发重分析、哪些用保守失效，直接抄其失效模式清单 |
| 7 | Hybrid Regression Test Selection by Integrating File and Method Dependences（ASE 2024） | test-replay 的回归选择在文件/方法两级依赖上做混合，别只做单层 |
| 8 | Aligning the Objective of LLM-Based Program Repair（ICSE 2025） | 修复验收判据：不能只看测试通过，需对齐"真实修复"目标防过拟合补丁 |

---

## ICSE 2023

1. **CodaMosa: Escaping Coverage Plateaus in Test Generation with Pre-trained Large Language Models**（ICSE 2023）· P0
   一句话：覆盖引导 fuzz 在平台期时用 LLM 生成针对目标 API 的输入作种子，再交回 fuzz 继续。
   可抄机制：覆盖率停滞检测 → LLM 种子注入 → 回到传统 fuzz 的混合回环；种子质量校验。
   模块映射：xtask-gates（fuzz 门）。链接：https://doi.org/10.1109/ICSE48619.2023.00085

2. **FixEval: Execution-based Evaluation of Program Fixes for Programming Problems**（ICSE 2023 · APR workshop）· watch
   一句话：以执行为基础评估补丁正确性的基准与指标（区别于 pass@k 的细粒度分级）。
   可抄机制：补丁分级评估维度（编译/部分通过/全通过/等价性），可作 xtask-gates 修复门的判据参考。
   模块映射：xtask-gates。链接：https://doi.org/10.1109/apr59189.2023.00009

## ICSE 2024

3. **GPTScan: Detecting Logic Vulnerabilities in Smart Contracts by Combining GPT with Program Analysis**（ICSE 2024）· P1
   一句话：LLM 初筛逻辑漏洞候选 + 程序分析确认调用序列与上下文，压假阳性。
   可抄机制：LLM 候选 → 静态确认的两段式；"候选-验证"解耦可直接用于 adv-rules 的高开销规则。
   模块映射：adv-rules / adv-taint。链接：https://doi.org/10.1145/3597503.3639117

## ICSE 2025（研究轨）

4. **Static Analysis of Remote Procedure Call in Java Programs**（ICSE 2025）· P2
   一句话：针对 RPC 边界的跨进程数据流建模的静态分析。
   可抄机制：把 RPC 桩/接口定义当作额外数据流边源，可推广到 ADV 对 IPC/FFI 边界的规则建模。
   模块映射：adv-taint。链接：https://doi.org/10.1109/icse55347.2025.00151

5. **An Extensive Empirical Study of Nondeterministic Behavior in Static Analysis Tools**（ICSE 2025）· P0
   一句话：实测多款静态分析工具存在非确定行为（同输入不同输出），并给出成因分类。
   可抄机制：可复现性工程清单（固定运行环境、顺序化输出、缓存键绑定工具版本与配置哈希）。
   模块映射：xtask-gates / ci-ops。链接：https://doi.org/10.1109/icse55347.2025.00125

6. **RepairAgent: An Autonomous, LLM-Based Agent for Program Repair**（ICSE 2025）· P0
   一句话：把修复拆成一组可复用工具（诊断、定位、编辑、验证），由 LLM 按状态机调度。
   可抄机制：修复 agent 的工具面划分与状态机（诊断→定位→生成→验证循环）。
   模块映射：xtask-gates / test-replay。链接：https://doi.org/10.1109/icse55347.2025.00157

7. **Aligning the Objective of LLM-Based Program Repair**（ICSE 2025）· P0
   一句话：论证以"测试通过"为目标的修复训练/评估与真实修复目标错位，提出对齐目标。
   可抄机制：修复验收判据分层（过拟合补丁识别：删测试检测、补丁泛化检查）。
   模块映射：xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00169

8. **Knowledge-Enhanced Program Repair for Data Science Code**（ICSE 2025）· P2
   一句话：为数据科学代码修复注入库语义知识。
   可抄机制：领域知识库（API 语义卡）注入修复提示的做法可迁移到 Rust 生态规则修复。
   模块映射：adv-rules。链接：https://doi.org/10.1109/icse55347.2025.00246

9. **Template-Guided Program Repair in the Era of Large Language Models**（ICSE 2025）· P1
   一句话：把传统模板修复与 LLM 结合，模板约束生成空间。
   可抄机制：规则库修复模式 → LLM 实例化的"模板引导生成"，与 adv-rules 的修复建议格式兼容。
   模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00030

10. **Test Intention Guided LLM-Based Unit Test Generation**（ICSE 2025）· P1
    一句话：先抽测试意图再让 LLM 按意图生成，提升可读性与多样性。
    可抄机制：意图抽取→分意图生成的两段式，可用作 ADV 测试建议的输出结构。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse55347.2025.00243

11. **Automated Test Generation For Smart Contracts via On-Chain Test Case Augmentation**（ICSE 2025）· P2
    一句话：用链上历史交易增广智能合约测试用例。
    可抄机制："真实历史输入当种子/语料"的思路可用于 ADV 对仓库历史流量的回归输入回放。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse55347.2025.00096

12. **TOGLL: Correct and Strong Test Oracle Generation with LLMs**（ICSE 2025）· P1
    一句话：LLM 生成 oracle 时以可执行断言 + 违例复检保证正确性与强度。
    可抄机制：oracle 生成后自动"反向验证"（构造应失败的变体）防弱断言。
    模块映射：test-replay / xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00098

13. **Feature-Driven End-to-End Test Generation**（ICSE 2025）· P2
    一句话：按功能特征组织端到端测试生成。
    可抄机制：以功能清单驱动用例矩阵，可对应 ADV 的规则→用例映射表。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse55347.2025.00141

14. **Rug: Turbo LLM for Rust Unit Test Generation**（ICSE 2025）· P1
    一句话：面向 Rust 的 LLM 单元测试生成（处理所有权/借用等约束）。
    可抄机制：Rust 特有约束在测试生成中的处理方式，直接服务 ADV 自身的 Rust 测试建议。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse55347.2025.00097

15. **Automated Generation of Accessibility Test Reports from Recorded User Transcripts**（ICSE 2025）· P2
    一句话：从录制交互生成可访问性测试报告。
    可抄机制：会话录制→可复验报告的流水线，可类比 ADV CLI 交互的快照测试。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse55347.2025.00043

16. **Software Model Evolution with Large Language Models**（ICSE 2025）· watch
    一句话：LLM 辅助软件模型演化的受控实验。
    可抄机制：暂无直接可抄工程机制，关注其受控实验设计。
    模块映射：reference。链接：https://doi.org/10.1109/icse55347.2025.00112

17. **SOEN-101: Code Generation by Emulating Software Process Models Using LLMs**（ICSE 2025）· watch
    一句话：用软件过程模型（评审/测试/修复循环）编排 LLM 代码生成。
    可抄机制：过程级编排（生成→自评审→测试）与 ADV 的门禁流水线理念同构。
    模块映射：reference。链接：https://doi.org/10.1109/icse55347.2025.00140

18. **Planning a Large Language Model for Static Detection of Runtime Errors in Code Snippets**（ICSE 2025）· P2
    一句话：用规划分解让 LLM 做片段级运行时错误静态检测。
    可抄机制：把"运行时错误"翻译成静态可查模式的规划提示结构。
    模块映射：adv-rules。链接：https://doi.org/10.1109/icse55347.2025.00102

19. **Invivo Fuzzing by Amplifying Actual Executions**（ICSE 2025）· P1
    一句话：放大系统真实执行（in-vivo）生成 fuzz 输入，替代脱离环境的合成输入。
    可抄机制：以真实执行为母本做放大变异，适合 ADV 对本地服务的无侵入 fuzz 门。
    模块映射：xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00172

20. **ROSA: Finding Backdoors with Fuzzing**（ICSE 2025）· P1
    一句话：面向后门触发的定向 fuzz（触发条件搜索）。
    可抄机制：把"可疑触发条件"转成 fuzz 目标谓词，可接入 adv-rules 的可疑模式核查。
    模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00183

21. **Sand: Decoupling Sanitization from Fuzzing for Low Overhead**（ICSE 2025）· P1
    一句话：把净化检查与 fuzz 执行解耦以降开销。
    可抄机制：采样式/解耦式净化策略，用于 ADV fuzz 门的低开销模式。
    模块映射：xtask-gates。链接：https://doi.org/10.1109/icse55347.2025.00187

## ICSE-SEIP 2025（工业轨）

22. **Towards Better Static Analysis Bug Reports in the Clang Static Analyzer**（ICSE-SEIP 2025）· P1
    一句话：改进 Clang 静态分析器报告质量（路径/解释/可操作性）的工业实践。
    可抄机制：报告内容要素清单（触发路径、置信度、修复建议、噪声控制）。
    模块映射：adv-rules / adv-taint（报告层）。链接：https://doi.org/10.1109/icse-seip66354.2025.00021

23. **ArkAnalyzer: The Static Analysis Framework for OpenHarmony**（ICSE-SEIP 2025）· P2
    一句话：OpenHarmony 应用的大规模静态分析框架（调用图/数据流）。
    可抄机制：多语言前端到统一 IR 的工程组织方式。
    模块映射：adv-taint / reference。链接：https://doi.org/10.1109/icse-seip66354.2025.00018

24. **Evaluating Agent-Based Program Repair at Google**（ICSE-SEIP 2025）· P0
    一句话：谷歌内部 agent 式修复的大规模评估：采纳率、成本与人类评审角色。
    可抄机制：APR 落地度量集（采纳率、修复时间、每补丁成本、评审负担），为 ADV 修复模块定目标。
    模块映射：xtask-gates / ci-ops。链接：https://doi.org/10.1109/icse-seip66354.2025.00038

25. **ASTER: Natural and Multi-Language Unit Test Generation with LLMs**（ICSE-SEIP 2025）· P1
    一句话：多语言自然风格单测生成的工业框架。
    可抄机制：跨语言测试生成的统一中间表示与"自然可读"约束。
    模块映射：test-replay。链接：https://doi.org/10.1109/icse-seip66354.2025.00042

## FSE 2023（ESEC/FSE）

26. **Program Repair Guided by Datalog-Defined Static Analysis**（FSE 2023）· P1
    一句话：用 Datalog 规则定义的静态分析结果引导补丁生成。
    可抄机制：分析规则（Datalog）→ 修复提示的结构化通道；与 adv-rules 若采用规则引擎可复用。
    模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1145/3611643.3616363

27. **ViaLin: Path-Aware Dynamic Taint Analysis for Android**（FSE 2023）· P1
    一句话：路径感知的动态污点追踪，降低漏报并控制开销。
    可抄机制：动态污点的路径感知剪枝，可用于 ADV 沙箱内动态校验静态污点结果。
    模块映射：adv-taint。链接：https://doi.org/10.1145/3611643.3616330

28. **NaNofuzz: A Usable Tool for Automatic Test Generation**（FSE 2023）· P2
    一句话：低配置门槛的自动测试生成工具（可用性导向）。
    可抄机制：零配置默认策略设计，服务 ADV 的开箱即用目标。
    模块映射：test-replay。链接：https://doi.org/10.1145/3611643.3616327

## FSE 2024

29. **SmartAxe: Detecting Cross-Chain Vulnerabilities in Bridge Smart Contracts via Fine-Grained Static Analysis**（FSE 2024）· P2
    一句话：跨链桥合约的细粒度静态漏洞检测。
    可抄机制：跨信任边界组件的分离分析再合并，类比 ADV 对多 crate/服务边界。
    模块映射：adv-taint。链接：https://doi.org/10.1145/3643738

30. **ProveNFix: Temporal Property-Guided Program Repair**（FSE 2024）· P2
    一句话：以时序性质引导的自动修复。
    可抄机制：性质（断言）→ 修复目标的机器可读通道。
    模块映射：xtask-gates。链接：https://doi.org/10.1145/3643737

31. **Understanding and Detecting Annotation-Induced Faults of Static Analyzers**（FSE 2024）· P1
    一句话：研究静态分析器的注解（源/汇/净化声明）本身出错导致的漏报误报并检测之。
    可抄机制：规则元数据自检（源-汇-净化一致性校验），adv-taint 规则库的质量门。
    模块映射：adv-taint / xtask-gates。链接：https://doi.org/10.1145/3643759

32. **BRF: Fuzzing the eBPF Runtime**（FSE 2024）· P2
    一句话：对 eBPF 运行时的结构化 fuzz。
    可抄机制：目标运行时（解释器/校验器）的结构化输入模型构造。
    模块映射：xtask-gates。链接：https://doi.org/10.1145/3643778

33. **COSTELLO: Contrastive Testing for Embedding-Based LLM as a Service Embedding Models**（FSE 2024）· watch
    一句话：对嵌入模型服务的对照测试。
    可抄机制：嵌入一致性测试思路可复用（若 adv-index 使用本地嵌入）。
    模块映射：adv-index / watch。链接：https://doi.org/10.1145/3643767

34. **LintQ: A Static Analysis Framework for Quantum Programs**（FSE 2024）· P2
    一句话：量子程序 lint 框架（领域规则静态化）。
    可抄机制：领域专用 lint 规则的构造与文档化方式。
    模块映射：adv-rules。链接：https://doi.org/10.1145/3660802

35. **Evaluating and Improving ChatGPT for Unit Test Generation**（FSE 2024）· P2
    一句话：系统评估 LLM 生成单测的质量维度并改进。
    可抄机制：测试质量维度清单（覆盖/可读/可维护/断言强度）。
    模块映射：test-replay。链接：https://doi.org/10.1145/3660783

## FSE 2025

36. **An Empirical Study of Suppressed Static Analysis Warnings**（FSE 2025）· P1
    一句话：实测被抑制（注释/配置压制）的告警中存在真实缺陷，并分析抑制原因。
    可抄机制：对"用户压制行为"建模：ADV 棘轮允许压制但要求结构化理由（可再评审）。
    模块映射：xtask-gates / adv-rules。链接：https://doi.org/10.1145/3715729

37. **CoverUp: Effective High Coverage Test Generation for Python**（FSE 2025）· P1
    一句话：以覆盖反馈驱动 LLM 迭代生成高覆盖 Python 测试。
    可抄机制：覆盖差值反馈→下一轮生成提示的闭环，适配 ADV test-replay。
    模块映射：test-replay。链接：https://doi.org/10.1145/3729398

38. **LLMDroid: Enhancing Automated Mobile App GUI Testing Coverage with LLM Guidance**（FSE 2025）· P2
    一句话：LLM 引导 GUI 测试探索提升覆盖。
    可抄机制：把 LLM 作为探索策略层而非执行层（与脚本执行解耦）。
    模块映射：test-replay。链接：https://doi.org/10.1145/3715763

39. **A Knowledge Enhanced Large Language Model for Bug Localization**（FSE 2025）· P2
    一句话：知识增强的 LLM 缺陷定位。
    可抄机制：仓库知识（历史缺陷/结构）注入定位提示的组织方式。
    模块映射：adv-index。链接：https://doi.org/10.1145/3729356

40. **Automated Unit Test Refactoring**（FSE 2025）· P2
    一句话：自动重构单元测试以改善结构质量。
    可抄机制：测试代码坏味清单与重构规则，可并入 ADV 质量门对测试代码的要求。
    模块映射：test-replay。链接：https://doi.org/10.1145/3715750

41. **Less Is More: On the Importance of Data Quality for Unit Test Generation**（FSE 2025）· P2
    一句话：证明测试生成训练/示例数据质量比数量更关键。
    可抄机制：少而精的示例集（few-shot 池）治理方式。
    模块映射：test-replay。链接：https://doi.org/10.1145/3715778

## FSE 2026（场次由 Crossref issue 判定）

42. **CodeCureAgent: Automatic Classification and Repair of Static Analysis Warnings**（FSE 2026）· P1
    一句话：对静态分析告警做真伪分类并自动修复的 agent。
    可抄机制：告警三分类（真缺陷/误报/需上下文）+ 分支处置，即 ADV 告警分诊流水线。
    模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1145/3808140

43. **NESA: Relational Neuro-Symbolic Static Program Analysis**（FSE 2026）· P2
    一句话：神经-符号结合的关系型静态分析。
    可抄机制：LLM 输出作为符号分析的关系约束来源。
    模块映射：adv-taint / adv-rules。链接：https://doi.org/10.1145/3808161

44. **Semantics-Guided Control-Flow Reconstruction for Firmware Binaries via Static Analysis**（FSE 2026）· P2
    一句话：固件二进制的语义引导 CFG 重建。
    可抄机制：语义提示引导传统分析（先猜后证）的混合范式。
    模块映射：adv-taint。链接：https://doi.org/10.1145/3797130

45. **ExpeRepair: Dual-Memory Enhanced LLM-Based Repository-Level Program Repair**（FSE 2026）· P2
    一句话：双记忆（经验库+仓库上下文）增强的仓库级修复。
    可抄机制：修复经验缓存（症状→修法）按项目积累，适配 ADV 本地优先存储。
    模块映射：xtask-gates / adv-index。链接：https://doi.org/10.1145/3808181

46. **VulKey: Automated Vulnerability Repair Guided by Domain-Specific Repair Patterns**（FSE 2026）· P2
    一句话：领域修复模式库引导的漏洞修复。
    可抄机制：漏洞类型→修复模式的结构化映射表。
    模块映射：adv-rules。链接：https://doi.org/10.1145/3808117

47. **OCPPuzz: Specification-Driven Fuzzing of Charging Station Management Systems with LLMs**（FSE 2026）· watch
    一句话：规范驱动的协议实现 fuzz。
    可抄机制：规范→fuzz 语法的自动编译，若有 RFC/规范输入可复用。
    模块映射：xtask-gates / watch。链接：https://doi.org/10.1145/3797091

48. **iCoRe: An Iterative Correlation-Aware Retriever for Bug Reproduction Test Generation**（FSE 2026）· P2
    一句话：相关性感知的迭代检索生成缺陷复现测试。
    可抄机制：检索（相关上下文）→生成→失败信号回填检索查询的迭代环。
    模块映射：adv-index / test-replay。链接：https://doi.org/10.1145/3808193

49. **AutoCodeRover: Agentic Program Repair for SonarQube Issues**（FSE 2026 · 工业轨，待验证）· P1
    一句话：把 AutoCodeRover 方法用于 SonarQube 告警的自动修复（工业落地版）。
    可抄机制：SA 告警→定位→修复的端到端管线，验证 ADV 告警输入格式对 agent 修复的适配性。
    模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1145/3803437.3805209

50. **RepoFuse: A Dual-Context Approach to Repository-Level Code Completion at Industrial Scale**（FSE 2026 · 工业轨，待验证）· P2
    一句话：双上下文（语法+语义）仓库级补全的工业部署。
    可抄机制：双通道上下文选择（相似代码+依赖签名）的检索结构。
    模块映射：adv-index。链接：https://doi.org/10.1145/3803437.3805230

## ASE 2023

51. **ReuNify: A Step Towards Whole Program Analysis for React Native Android Apps**（ASE 2023）· P1
    一句话：跨 JS/Java 边界的整程序分析。
    可抄机制：跨语言边界点的桩建模（bridge 表→数据流边），适用于 ADV 的 FFI/CLI 子进程边界。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE56229.2023.00113

52. **ConfTainter: Static Taint Analysis For Configuration Options**（ASE 2023）· P1
    一句话：把配置项当作污点源，检测危险配置传播。
    可抄机制：配置→危险 sink 的污点规则集，直接可做 adv-taint 的一组内置规则。
    模块映射：adv-taint / adv-rules。链接：https://doi.org/10.1109/ASE56229.2023.00067

53. **Revisiting and Improving Retrieval-Augmented Deep Assertion Generation**（ASE 2023）· P2
    一句话：检索增强的断言生成。
    可抄机制：相似代码→断言模板的检索库组织。
    模块映射：test-replay / adv-index。链接：https://doi.org/10.1109/ASE56229.2023.00090

54. **Characterizing Flaky Tests in Node.js Applications**（ASE 2023）· P2
    一句话：Node.js 生态 flaky 成因谱系。
    可抄机制：flaky 成因分类表（异步时序/资源/环境），用于 test-replay 的诊断提示。
    模块映射：test-replay / ci-ops。链接：https://doi.org/10.1109/ASE56229.2023.00025

55. **Compsuite: A Dataset of Java Library Upgrade Incompatibility Issues**（ASE 2023）· P2
    一句话：库升级不兼容问题的标注数据集。
    可抄机制：不兼容类型清单，可支撑 ADV 依赖更新影响的规则库。
    模块映射：ci-ops。链接：https://doi.org/10.1109/ASE56229.2023.00127

56. **ZC3: Zero-Shot Cross-Language Code Clone Detection**（ASE 2023）· P2
    一句话：零样本跨语言克隆检测。
    可抄机制：中间表示对齐做跨语言相似度，可增强 adv-index 的复用检测。
    模块映射：adv-index。链接：https://doi.org/10.1109/ASE56229.2023.00210

## ASE 2024

57. **Effective Unit Test Generation for Java Null Pointer Exceptions**（ASE 2024）· P2
    一句话：面向 NPE 触发的定向单测生成。
    可抄机制：以特定缺陷类为目标的用例生成（缺陷类型→生成目标谓词）。
    模块映射：test-replay。链接：https://doi.org/10.1145/3691620.3695484

58. **Hybrid Regression Test Selection by Integrating File and Method Dependences**（ASE 2024）· P0
    一句话：文件级（粗）与方法级（细）依赖混合的回归测试选择，兼顾安全与精确。
    可抄机制：两级依赖图 + 安全下界（文件级兜底）的选择策略，test-replay 的核心算法参考。
    模块映射：test-replay。链接：https://doi.org/10.1145/3691620.3695525

59. **The Importance of Accounting for Execution Failures when Predicting Test Flakiness**（ASE 2024）· P1
    一句话：把执行失败信号纳入 flaky 预测可显著改进。
    可抄机制：以历史执行失败模式做 flaky 风险评分，ci-ops 的重试/隔离策略输入。
    模块映射：ci-ops / test-replay。链接：https://doi.org/10.1145/3691620.3695261

60. **Reducing Test Runtime by Transforming Test Fixtures**（ASE 2024）· P2
    一句话：变换测试夹具（共享/降级）缩短运行时间。
    可抄机制：fixture 依赖分析驱动的等价变换清单。
    模块映射：test-replay。链接：https://doi.org/10.1145/3691620.3695541

61. **Balancing the Quality and Cost of Updating Dependencies**（ASE 2024）· P1
    一句话：依赖更新的收益/成本权衡模型。
    可抄机制：更新候选打分（安全修复收益 vs 适配成本），可作 ADV 依赖门（xtask）的决策规则。
    模块映射：ci-ops / xtask-gates。链接：https://doi.org/10.1145/3691620.3695595

62. **Cross-lingual Code Clone Detection: When LLMs Fail Short Against Embedding-based Classifiers**（ASE 2024）· P2
    一句话：实测跨语言克隆检测上嵌入分类器优于直接用 LLM。
    可抄机制：adv-index 的复用检测应以嵌入为主、LLM 为辅的实证依据。
    模块映射：adv-index。链接：https://doi.org/10.1145/3691620.3695335

63. **MR-Adopt: Automatic Deduction of Input Transformation Function for Metamorphic Testing**（ASE 2024）· P2
    一句话：自动推导蜕变测试的输入变换函数。
    可抄机制：从 API 语义自动生成蜕变关系，用于无 oracle 场景的质量门。
    模块映射：xtask-gates。链接：https://doi.org/10.1145/3691620.3696020

## ASE 2025

64. **Mockingbird: Efficient Excessive Data Exposures Detection via Dynamic Code Instrumentation**（ASE 2025）· P2
    一句话：动态插桩检测敏感数据过度暴露。
    可抄机制：插桩点选择策略（最小侵入捕获数据流）。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00247

65. **GlassWing: A Tailored Static Analysis Approach for Flutter Android Apps**（ASE 2025）· P1
    一句话：针对 Flutter（Dart/原生混合）的定制静态分析。
    可抄机制：跨 FFI 边界的混合分析组织，可借鉴到 ADV 对 Rust/C FFI 的建模。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00050

66. **Towards More Accurate Static Analysis for Taint-Style Bug Detection in Linux Kernel**（ASE 2025）· P1
    一句话：Linux 内核污点类缺陷检测的精度改进（宏/间接调用处理）。
    可抄机制：C 宏展开与间接调用的污点边处理技巧，adv-taint 的 C/C++ 支持参考。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00039

67. **VUSC: An Extensible Research Platform for Java-Based Static Analysis**（ASE 2025）· P1
    一句话：可扩展的 Java 静态分析研究平台（规则即插件）。
    可抄机制：规则插件化接口（输入 IR、输出统一告警）设计，与 adv-rules 架构同构。
    模块映射：adv-rules / adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00354

68. **STaint: Detecting Second-Order Vulnerabilities in PHP Applications with LLM-Assisted Bi-directional Analysis**（ASE 2025）· P2
    一句话：LLM 辅助双向分析检测二阶注入。
    可抄机制：二阶流（存储→再读取→sink）的显式建模。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00347

69. **ConfuseTaint: Exploiting Vulnerabilities to Bypass Dynamic Taint Analysis**（ASE 2025）· P2
    一句话：攻击者视角证明动态污点可被绕过。
    可抄机制：动态污点的对抗失效场景清单，用于 ADV 沙箱动态校验的局限声明。
    模块映射：adv-taint / reference。链接：https://doi.org/10.1109/ASE63991.2025.00340

70. **ACTaint: Agent-Based Taint Analysis for Access Control Vulnerabilities in Smart Contracts**（ASE 2025）· P2
    一句话：agent 式（LLM 参与决策）污点分析。
    可抄机制：把 LLM 放在"分析决策点"（是否是源/汇）而非全流程。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00210

71. **Incremental Program Analysis in the Wild: An Empirical Study on Real-World Program Changes**（ASE 2025）· P1
    一句话：实测真实变更下增量分析的失效模式与代价。
    可抄机制：增量失效分类（何时必须全量回退），与 EvoTaint 互补定 adv-taint 增量策略。
    模块映射：adv-taint / adv-index。链接：https://doi.org/10.1109/ASE63991.2025.00194

72. **From Technical Excellence to Practical Adoption: Lessons Learned Building an ML-Enhanced Static Analysis Tool**（ASE 2025 · 工业轨）· P1
    一句话：ML 增强静态分析工具落地的经验教训（采用障碍、反馈回路）。
    可抄机制：开发者反馈回路设计（误报一键反馈→规则迭代），ADV 的开发者体验参考。
    模块映射：adv-rules / reference。链接：https://doi.org/10.1109/ASE63991.2025.00286

73. **DiffFix: Incrementally Fixing AST Diffs via Context and Type Information**（ASE 2025）· P2
    一句话：以 AST diff 为单元的增量修复。
    可抄机制：修复产物最小化（diff 级而非文件级）。
    模块映射：xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00237

74. **TreeRanker: Fast and Model-Agnostic Ranking System for Code Suggestions in IDEs**（ASE 2025）· P1
    一句话：模型无关、低延迟的代码建议排序。
    可抄机制：轻量排序层重排任意生成器输出，可用于 ADV 索引建议排序。
    模块映射：adv-index。链接：https://doi.org/10.1109/ASE63991.2025.00290

75. **SATORI: Static Test Oracle Generation for REST APIs**（ASE 2025）· P1
    一句话：从规范静态推导 REST API 测试 oracle。
    可抄机制：规范（OpenAPI）→断言的编译式生成，服务 ADV API 兼容门（D11 相关）。
    模块映射：test-replay / xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00116

76. **RAML: Toward Retrieval-Augmented Localization of Malicious Payloads in Android Apps**（ASE 2025）· P2
    一句话：检索增强定位恶意载荷。
    可抄机制：恶意模式库检索+代码切片对齐的定位法。
    模块映射：adv-index / adv-rules。链接：https://doi.org/10.1109/ASE63991.2025.00351

77. **BitsAI-Fix: LLM-Driven Approach for Automated Lint Error Resolution in Practice**（ASE 2025 · 工业轨）· P1
    一句话：字节跳动生产环境 lint 错误自动修复的实践报告。
    可抄机制：lint 修复的安全护栏（可编译/测试通过/人工评审队列）与吞吐数据。
    模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00288

78. **FlakyGuard: Automatically Fixing Flaky Tests at Industry Scale**（ASE 2025 · 工业轨）· P1
    一句话：工业规模自动修复 flaky 测试（分类→针对性修复策略）。
    可抄机制：flaky 成因→修复策略映射表与自动修复管线。
    模块映射：test-replay / ci-ops。链接：https://doi.org/10.1109/ASE63991.2025.00179

79. **SPICE: An Automated SWE-Bench Labeling Pipeline for Issue Clarity, Test Coverage, and Effort**（ASE 2025）· P1
    一句话：自动给 SWE-bench 类基准条目标注（问题清晰度/测试覆盖/工作量）。
    可抄机制：修复任务的"可修性"预标注（issue 质量/测试覆盖），ADV 修复门的前置过滤器。
    模块映射：xtask-gates / test-replay。链接：https://doi.org/10.1109/ASE63991.2025.00192

80. **NexuSym: Marrying symbolic path finders with large language models**（ASE 2025）· P2
    一句话：符号执行与 LLM 协同的测试生成。
    可抄机制：LLM 猜解路径约束、符号引擎验证的分工边界。
    模块映射：xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00343

81. **Generating Failure-Based Oracles to Support Testing of Reported Bugs in Android Apps**（ASE 2025）· P2
    一句话：从缺陷报告生成"以失败为信号"的 oracle。
    可抄机制：缺陷报告→复现断言的转换模板。
    模块映射：test-replay。链接：https://doi.org/10.1109/ASE63991.2025.00182

82. **Automatic Fixing of Missing Dependency Errors**（ASE 2025）· P1
    一句话：自动修复缺失依赖错误（解析构建/导入错误并补依赖）。
    可抄机制：构建错误→依赖修复的动作映射，ADV 的 xtask 自动修依赖参考。
    模块映射：ci-ops / xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00029

83. **Unit Test Update through LLM-Driven Context Collection and Error-Type-Aware Refinement**（ASE 2025）· P2
    一句话：接口变更后按错误类型细化上下文来更新旧测试。
    可抄机制：错误类型→上下文需求映射，test-replay 的"测试漂移"维护策略。
    模块映射：test-replay。链接：https://doi.org/10.1109/ASE63991.2025.00206

84. **Risk Estimation in Differential Fuzzing via Extreme Value Theory**（ASE 2025）· P2
    一句话：用极值理论估计差分 fuzz 的残余风险。
    可抄机制：以统计模型给出"还要跑多久"的量化依据，fuzz 门预算分配。
    模块映射：xtask-gates。链接：https://doi.org/10.1109/ASE63991.2025.00036

85. **Function Clustering-Based Fuzzing Termination: Toward Smarter Early Stopping**（ASE 2025）· P2
    一句话：按函数聚类判定 fuzz 可提前停止。
    可抄机制：fuzz 早停判据（相似目标已充分探索），降 CI 时间。
    模块映射：xtask-gates / ci-ops。链接：https://doi.org/10.1109/ASE63991.2025.00145

86. **PromFuzz: LLM-Driven Bug-Oriented Composite Analysis for Detecting Functional Bugs**（ASE 2025）· P2
    一句话：LLM 生成 bug 导向的提示/输入做复合分析。
    可抄机制：把"目标缺陷类"翻译成 fuzz 目标与 oracles。
    模块映射：xtask-gates / adv-rules。链接：https://doi.org/10.1109/ASE63991.2025.00091

87. **Advancing Binary Code Similarity Detection via Context-Content Fusion and LLM Verification**（ASE 2025）· P2
    一句话：二进制相似度检测的上下文融合 + LLM 复核。
    可抄机制：SCA（组件识别）的相似度复核层。
    模块映射：adv-index。链接：https://doi.org/10.1109/ASE63991.2025.00033

88. **Context-Sensitive Pointer Analysis for ArkTS**（ASE 2025）· P2
    一句话：ArkTS 的上下文敏感指针分析。
    可抄机制：动态语言指针分析的上下文选择策略。
    模块映射：adv-taint。链接：https://doi.org/10.1109/ASE63991.2025.00269

89. **WEST: Specification-Based Test Generation for WebAssembly**（ASE 2025）· P2
    一句话：基于 Wasm 规范的测试生成。
    可抄机制：规范驱动生成器的结构（对 ADV 沙箱 Wasm 支持有参考）。
    模块映射：test-replay。链接：https://doi.org/10.1109/ASE63991.2025.00119

## ISSTA 2023

90. **Beware of the Unexpected: Bimodal Taint Analysis**（ISSTA 2023）· P1
    一句话：双模态污点分析（乐观/悲观模式切换）在精度与成本间权衡。
    可抄机制：按调用上下文切换分析模式的架构。
    模块映射：adv-taint。链接：https://doi.org/10.1145/3597926.3598050

91. **DeFiTainter: Detecting Price Manipulation Vulnerabilities in DeFi Protocols**（ISSTA 2023）· P2
    一句话：面向价格操纵的领域污点分析。
    可抄机制：业务不变量作为 sink 定义。
    模块映射：adv-taint。链接：https://doi.org/10.1145/3597926.3598124

92. **Automatic Testing and Benchmarking for Configurable Static Analysis Tools**（ISSTA 2023）· P1
    一句话：自动测试与基准化"可配置分析器"的配置空间。
    可抄机制：分析器配置的自动测试（配置矩阵→期望行为），ADV 规则配置自检可借鉴。
    模块映射：xtask-gates / adv-rules。链接：https://doi.org/10.1145/3597926.3605232

93. **Extracting Inline Tests from Unit Tests**（ISSTA 2023）· P1
    一句话：从单测中抽取"内联测试"（可执行的最小断言单元）。
    可抄机制：用例→可复用断言片段的抽取，adv-index 可积累"断言片段库"。
    模块映射：test-replay / adv-index。链接：https://doi.org/10.1145/3597926.3598149

94. **Systematically Producing Test Orders to Detect Order-Dependent Flaky Tests**（ISSTA 2023）· P1
    一句话：系统性构造测试顺序以暴露顺序依赖型 flaky。
    可抄机制：测试顺序的系统性枚举/选择策略，ci-ops 的 flaky 检测调度。
    模块映射：test-replay / ci-ops。链接：https://doi.org/10.1145/3597926.3598083

95. **More Precise Regression Test Selection via Reasoning about Semantics-Modifying Changes**（ISSTA 2023）· P1
    一句话：只对"改变语义"的变更做测试选择推理，降低误选。
    可抄机制：语义等价变更识别（格式化/注释）跳过无关测试。
    模块映射：test-replay。链接：https://doi.org/10.1145/3597926.3598086

96. **Third-Party Library Dependency for Large-Scale SCA in the C/C++ Ecosystem**（ISSTA 2023）· P2
    一句话：C/C++ 生态第三方依赖识别（SCA）的方法与差距。
    可抄机制：无包管理生态的依赖指纹策略。
    模块映射：adv-index / reference。链接：https://doi.org/10.1145/3597926.3598143

97. **Understanding Breaking Changes in the Wild**（ISSTA 2023）· P2
    一句话：实测库破坏性变更的类型谱系。
    可抄机制：破坏性变更分类表→API 兼容门（D11）规则。
    模块映射：ci-ops / reference。链接：https://doi.org/10.1145/3597926.3598147

98. **Improving Binary Code Similarity Transformer Models by Semantics-Driven Instruction Deemphasizing**（ISSTA 2023）· P2
    一句话：语义加权去噪改进二进制相似模型。
    可抄机制：指纹特征加权策略。
    模块映射：adv-index。链接：https://doi.org/10.1145/3597926.3598121

## ISSTA 2024

99. **AutoCodeRover: Autonomous Program Improvement**（ISSTA 2024）· P0
    一句话：结构感知（类/方法级搜索）的仓库级定位+修复，测试验证闭环。
    可抄机制：AST 导航式定位替代全文检索；分层上下文组装；失败测试反馈再修。
    模块映射：xtask-gates / adv-index。链接：https://doi.org/10.1145/3650212.3680384

100. **Silent Taint-Style Vulnerability Fixes Identification**（ISSTA 2024）· P2
     一句话：识别"静默"修复的污点类漏洞（未公开披露的修复提交）。
     可抄机制：从修复提交反推漏洞模式，喂给 adv-rules 规则库做历史挖掘。
     模块映射：adv-rules。链接：https://doi.org/10.1145/3650212.3652139

101. **CoEdPilot: Recommending Code Edits with Learned Prior Edit Relevance, Project-wise Awareness**（ISSTA 2024）· P2
     一句话：按编辑相关性推荐后续代码编辑。
     可抄机制：编辑推荐的相关性学习（改动级而非行级）。
     模块映射：adv-index。链接：https://doi.org/10.1145/3650212.3652142

102. **Detecting Build Dependency Errors in Incremental Builds**（ISSTA 2024）· P1
     一句话：检测增量构建中的依赖声明错误（漏声明却靠顺序侥幸通过）。
     可抄机制：构建图静态校验（声明依赖 vs 实际读取），直接可做 ADV 的 ci-ops 检查。
     模块映射：ci-ops / xtask-gates。链接：https://doi.org/10.1145/3650212.3652105

103. **Reproducing Timing-Dependent GUI Flaky Tests via a Single Event Delay**（ISSTA 2024）· P2
     一句话：单事件延迟复现时序型 flaky。
     可抄机制：最小干预复现法（单点延迟注入）。
     模块映射：test-replay。链接：https://doi.org/10.1145/3650212.3680377

104. **Ma11y: A Mutation Framework for Web Accessibility Testing**（ISSTA 2024）· watch
     一句话：可访问性测试的变异框架。
     可抄机制：变异评估测试强度的一般法（对 ADV 自身测试集适用）。
     模块映射：xtask-gates / watch。链接：https://doi.org/10.1145/3650212.3652113

## ISSTA 2025

105. **KRAKEN: Program-Adaptive Parallel Fuzzing**（ISSTA 2025）· P2
     一句话：按目标程序特征自适应调度的并行 fuzz。
     可抄机制：并行 fuzz 的算力分配策略（种子/配置多样性）。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3728882

106. **ZTaint-Havoc: From Havoc Mode to Zero-Execution Fuzzing-Driven Taint Inference**（ISSTA 2025）· P2
     一句话：零执行（纯静态变异空间）推断污点传播。
     可抄机制：fuzz 变异空间→污点推断的转换。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3728916

107. **Static Program Reduction via Type-Directed Slicing**（ISSTA 2025）· P2
     一句话：类型导向切片做程序化简（缩小复现用例）。
     可抄机制：最小复现用例的化简策略，fuzz/门禁失败报告必备。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3728968

108. **Dynamic Taint Tracking for Modern Java Virtual Machines**（ISSTA 2025）· P1
     一句话：适配现代 JVM（内联缓存/逃逸分析）的动态污点追踪。
     可抄机制：运行时优化与污点传播的兼容处理清单。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3729349

109. **STRUT: Structured Seed Case Guided Unit Test Generation for C Programs using LLMs**（ISSTA 2025）· P2
     一句话：结构化种子引导的 C 程序 LLM 测试生成。
     可抄机制：种子结构化（输入语法骨架）→生成约束。
     模块映射：test-replay。链接：https://doi.org/10.1145/3728970

110. **Intention-Based GUI Test Migration for Mobile Apps using LLMs**（ISSTA 2025）· P2
     一句话：以意图为中介迁移 GUI 测试。
     可抄机制：测试意图抽取→目标环境重实例化。
     模块映射：test-replay。链接：https://doi.org/10.1145/3728978

111. **Automated Test Transfer across Android Apps using LLMs**（ISSTA 2025）· P2
     一句话：跨应用测试迁移。
     可抄机制：同上（迁移法的另一实例）。
     模块映射：test-replay。链接：https://doi.org/10.1145/3728975

112. **REACCEPT: Automated Co-evolution of Production and Test Code Based on Dynamic Validation**（ISSTA 2025）· P1
     一句话：生产代码与测试代码协同演化的动态验证法。
     可抄机制：API 变更→测试同步更新的验证规则，test-replay 的测试漂移处理。
     模块映射：test-replay。链接：https://doi.org/10.1145/3728930

113. **CrossProbe: LLM-Empowered Cross-Project Bug Detection for Deep Learning Frameworks**（ISSTA 2025）· P2
     一句话：跨项目缺陷模式迁移检测。
     可抄机制：缺陷模式跨仓库匹配的检索结构。
     模块映射：adv-index / adv-rules。链接：https://doi.org/10.1145/3728886

114. **ALMOND: Learning an Assembly Language Model for 0-Shot Code Obfuscation Detection**（ISSTA 2025）· P2
     一句话：汇编语言模型零样本检测代码混淆/恶意。
     可抄机制：供应链包的混淆检测信号（supply chain 审计）。
     模块映射：adv-rules。链接：https://doi.org/10.1145/3728984

115. **AdverIntent-Agent: Adversarial Reasoning for Repair Based on Inferred Program Intent**（ISSTA 2025）· P2
     一句话：先推断程序意图再按意图对抗性推理修复。
     可抄机制：意图推断→修复目标校验（防"改测试就修复"）。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3728939

116. **Causality-Aided Evaluation and Explanation of LLM-Based Code Generation**（ISSTA 2025）· P2
     一句话：因果分析解释 LLM 生成失败原因。
     可抄机制：失败归因的因果归解框架（诊断门禁失败原因）。
     模块映射：reference / xtask-gates。链接：https://doi.org/10.1145/3728938

117. **Unlocking Low Frequency Syscalls in Kernel Fuzzing with Dependency-Based RAG**（ISSTA 2025）· P2
     一句话：RAG 补内核 fuzz 的低频系统调用覆盖。
     可抄机制：检索历史知识补盲区的 fuzz 输入策略。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3728913

## ISSTA 2026（场次由 Crossref issue 判定）

118. **Testing Static Taint Analyzers with Equivalence Modulo Taint**（ISSTA 2026）· P1
     一句话：等价模污点（EMT）变换自动生成污点分析器的测试输入。
     可抄机制：差分/等价变换测试 adv-taint 引擎自身质量，规则元测试门。
     模块映射：adv-taint / xtask-gates。链接：https://doi.org/10.1145/3832231

119. **STARS: Static Analysis-Guided Assertion Synthesis using Large Language Models**（ISSTA 2026）· P2
     一句话：静态分析引导 LLM 生成断言。
     可抄机制：SA 不变量→断言候选的通道。
     模块映射：test-replay / adv-taint。链接：https://doi.org/10.1145/3832283

120. **Augmenting Multi-technique Static Analysis with LLMs: A Neuro-symbolic Approach**（ISSTA 2026）· P1
     一句话：多分析器结果与 LLM 融合的神经-符号告警裁决。
     可抄机制：多工具告警聚合 + LLM 复核的架构（ADV 告警融合层）。
     模块映射：adv-rules / adv-taint。链接：https://doi.org/10.1145/3832222

121. **LLM-Based Repair of Static Nullability Errors**（ISSTA 2026）· P1
     一句话：LLM 修复可空性静态错误（大规模实证）。
     可抄机制：单一规则族的修复管线深度打磨（提示/验证/回归），adv-rules 修复建议模板参考。
     模块映射：adv-rules / xtask-gates。链接：https://doi.org/10.1145/3832114

122. **CausalRepair: Bridging the Causality Gap in LLM-Based APR**（ISSTA 2026）· P2
     一句话：补齐"根因→补丁"因果链的修复。
     可抄机制：根因证据链显式化（修复须引用根因）。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3832225

123. **HELO-APR: Enhancing Low-Resource Program Repair through Cross-Lingual Knowledge Transfer**（ISSTA 2026）· P2
     一句话：跨语言知识迁移修复低资源语言。
     可抄机制：Rust 等低资源生态借力 C/C++ 修复知识。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3832201

124. **To Run or Not to Run: Analyzing the Cost-Effectiveness of Code Execution in LLM-Based PR**（ISSTA 2026）· P1
     一句话：量化修复过程中"何时值得真正执行代码"。
     可抄机制：执行预算分配策略（先静态后执行），修复门的成本模型。
     模块映射：xtask-gates / ci-ops。链接：https://doi.org/10.1145/3832113

125. **Test vs Mutant: Adversarial LLM Agents for Robust Unit Test Generation**（ISSTA 2026）· P1
     一句话：测试 agent 与变异 agent 对抗提升测试强度。
     可抄机制：对抗自博弈结构（测试 vs 变异），ADV 变异门 + 测试建议的联动设计。
     模块映射：xtask-gates / test-replay。链接：https://doi.org/10.1145/3832122

126. **Uncovering Business Logic Bugs via Semantics-Driven Unit Test Generation**（ISSTA 2026 · Experience）· P2
     一句话：语义驱动单测生成挖业务逻辑缺陷。
     可抄机制：业务不变量→生成约束。
     模块映射：test-replay。链接：https://doi.org/10.1145/3832192

127. **Context Matters: Improving the Practical Reliability of LLM-Based Unit Test Generation**（ISSTA 2026 · Experience）· P2
     一句话：上下文工程决定 LLM 测试生成实际可靠性。
     可抄机制：上下文要素消融清单（哪些上下文最值钱）。
     模块映射：test-replay / adv-index。链接：https://doi.org/10.1145/3832242

128. **Repair-Driven Greybox Fuzzing**（ISSTA 2026）· P1
     一句话：以"可修复缺陷"为目标的 fuzz（只挖能自动修的缺陷类）。
     可抄机制：fuzz 目标与修复管线联动（发现即可修），ADV 门禁闭环设计。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3832229

129. **XSearch: Explainable Code Search via Concept-to-Code Alignment**（ISSTA 2026）· P1
     一句话：概念-代码对齐的可解释代码搜索。
     可抄机制：检索结果附证据（对齐片段），adv-index 的可解释检索层。
     模块映射：adv-index。链接：https://doi.org/10.1145/3832095

130. **A Dataset of Reproducible Flaky-Test Failures**（ISSTA 2026）· P1
     一句话：可复现 flaky 失败的标注数据集（含触发条件）。
     可抄机制：flaky 触发条件结构化记录格式，ci-ops 的 flaky 档案。
     模块映射：test-replay / ci-ops。链接：https://doi.org/10.1145/3832216

131. **Automated Type-IV Clone Generation via LLMs and Deterministic Validation**（ISSTA 2026）· P2
     一句话：LLM 生成语义等价变体（Type-IV 克隆）并以确定性验证把关。
     可抄机制：语义等价变换器+验证器，可做 adv-index 的复用检测基准与门禁测试语料。
     模块映射：adv-index / xtask-gates。链接：https://doi.org/10.1145/3832153

132. **Rethinking Mixture-of-Experts for Vulnerability Detection**（ISSTA 2026）· P2
     一句话：MoE 用于漏洞检测的实证与改进。
     可抄机制：专家路由（按缺陷类型分流模型）。
     模块映射：adv-rules。链接：https://doi.org/10.1145/3832223

## TOSEM 2025–2026（在线首发年）

133. **TaskFlow: LLMs for Android Taint Specification**（TOSEM 2025/2026）· P1
     一句话：用 LLM 半自动生成污点规范（source/sink/propagator）。
     可抄机制：规范生成的"LLM 起草 + 抽样验证"流水线，adv-taint 规则库冷启动方案。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3815184

134. **EvoTaint: Incremental Static Taint Analysis of Evolving Android Apps**（TOSEM 2025/2026）· P0
     一句话：系统研究演化中增量污点分析的失效模式并给出保持精度的增量策略。
     可抄机制：变更类型→重分析范围映射；保守失效边界；缓存失效规则。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3743132

135. **HapFlow: The Taint Analysis Framework for OpenHarmony Apps**（TOSEM 2025/2026）· P2
     一句话：OpenHarmony 应用污点分析框架。
     可抄机制：多端共享逻辑下的污点规范组织。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3793863

136. **Go With the Flow: Improving the Precision of Static Data Flow Analysis with Context-Sensitive…**（TOSEM 2025/2026）· P1
     一句话：上下文敏感策略改进静态数据流精度（成本可控的上下文选择）。
     可抄机制：按调用点热度选择上下文深度的预算化分析。
     模块映射：adv-taint。链接：https://doi.org/10.1145/3828165

137. **STALL+: Boosting LLM-based Repository-level Code Completion with Static Analysis**（TOSEM 2025/2026）· P1
     一句话：静态分析结果（调用图/类型）增强仓库级补全。
     可抄机制：SA 产物作为补全上下文的通道（adv-index→生成器的接口设计）。
     模块映射：adv-index。链接：https://doi.org/10.1145/3846180

138. **InferCG: Enhancing Python Call Graph Generation via Static Analysis and LLMs**（TOSEM 2025/2026）· P2
     一句话：SA+LLM 混合生成 Python 调用图。
     可抄机制：动态语言调用图缺失边由 LLM 猜测 + 静态验证。
     模块映射：adv-index / adv-taint。链接：https://doi.org/10.1145/3799990

139. **RustC4++: Improving Rust Code-Comment Inconsistency Detection via Hybrid LLM and Static Analysis**（TOSEM 2025/2026）· P2
     一句话：Rust 注释-代码不一致检测（LLM+静态混合）。
     可抄机制：Rust 特有规则族的混合实现样例。
     模块映射：adv-rules。链接：https://doi.org/10.1145/3800689

140. **A Systematic Literature Review on Large Language Models for Automated Program Repair**（TOSEM 2025/2026）· P2
     一句话：LLM×APR 系统综述（方法/基准/评估维度全景）。
     可抄机制：评估维度与基准清单，修复模块验收设计直接引用。
     模块映射：reference / xtask-gates。链接：https://doi.org/10.1145/3799693

141. **Practical LLM-Based Function-Level Automated Program Repair: How Far Are We?**（TOSEM 2025/2026）· P1
     一句话：函数级 LLM 修复的现实差距实证。
     可抄机制：失败模式分类（定位错/生成错/验证错），修复模块的验收基线。
     模块映射：xtask-gates。链接：https://doi.org/10.1145/3812804

142. **Integrating Various Software Artifacts for Better LLM-based Bug Localization and Program Repair**（TOSEM 2025/2026）· P2
     一句话：多产物（issue/提交/测试）融合的定位与修复。
     可抄机制：异构产物→定位证据的融合结构。
     模块映射：adv-index。链接：https://doi.org/10.1145/3770581

143. **Retrieval-Augmented Unit Test Suggestion Generation**（TOSEM 2025/2026）· P2
     一句话：检索增强的单测建议生成。
     可抄机制：相似测试检索→建议组装。
     模块映射：adv-index / test-replay。链接：https://doi.org/10.1145/3821423

144. **Type-aware LLM-based Test Generation for Python Programs**（TOSEM 2025/2026）· P2
     一句话：类型感知的 Python 测试生成。
     可抄机制：类型信息约束生成空间。
     模块映射：test-replay。链接：https://doi.org/10.1145/3830083

145. **Hallucination to Consensus: Multi-Agent LLMs for End-to-End JUnit Test Generation**（TOSEM 2025/2026）· P2
     一句话：多 agent 共识压制幻觉的测试生成。
     可抄机制：多候选→共识投票的聚合判据。
     模块映射：test-replay。链接：https://doi.org/10.1145/3803418

146. **Fusing LLMs and Genetic Algorithm for High-Quality Unit Test Generation**（TOSEM 2025/2026）· P2
     一句话：LLM 与遗传算法混合搜索测试。
     可抄机制：GA 适应度（覆盖+变异分数）驱动 LLM 变异，衔接 ADV 变异门。
     模块映射：test-replay / xtask-gates。链接：https://doi.org/10.1145/3819237

147. **Requirements-Based Test Generation: A Comprehensive Survey**（TOSEM 2025/2026）· P2
     一句话：基于需求的测试生成综述。
     可抄机制：从规则/需求生成测试的分类法（门禁→用例的生成路径参考）。
     模块映射：reference / test-replay。链接：https://doi.org/10.1145/3771727

---

## 附注（口径与边界）

- 本文共登记 147 条（P0 8 / P1 52 / P2 81 / watch 6；以 registry/P2.jsonl 为准）。另观察但未登记：SWT-bench（COLM 2024，非五会）；Agentless（五会归属待验证，未收）；SWE-agent（NeurIPS 2024，非五会）。
- ICSE 2023/2024 覆盖偏薄（DBLP 反爬不可达、Crossref 对该两届索引稀疏），如需补全可在 DBLP 恢复可访问后按 `dblp.org/db/conf/icse/icse2024.html` 复扫（待验证）。
- FSE 2026 / ISSTA 2026 条目的场次标注依赖 Crossref `issue` 字段；个别工业轨条目（#49、#50）标注为「待验证」。
- 「一句话结论」对未精读论文仅陈述其方法定位（依据标题/摘要级信息与公开条目），效果数字未收录；引用前建议按 P1 清单精读。
