# P5：ML4Code / LLM×SE 论文扫视（NeurIPS/ICLR/ACL/EMNLP + arXiv 前沿，2023–2026）

> 扫描日期：2026-10-04 · 扫描人：ZCode P5 子任务 · 配套登记表：`registry/P5.jsonl`（92 条 = 首扫 72 + 同日专题 A/B 补扫 20）
> 定位：找"静态引擎×LLM 协作"的**实证机制**（不是做模型）。ADV 边界：LLM 只产规格/候选/初筛，确认权在静态引擎（IRIS）。

**方法与偏差声明**：本轮 ~12 组定向 WebSearch（arXiv 优先）；其中 5 组查询因并发/超时未完成，NeurIPS/ICLR/ACL 逐届录取列表未做人工全量扫描——覆盖存在缺口，结论受此样本限制。凡未核到原文/ID 的条目一律标「待验证」且 depth=题名级（30 条），宁少勿假；后续补扫优先消化这批。

---

## P0 清单（读了能直接改哪个设计决策）

| # | 论文 | 直接改动的设计决策 |
|---|------|--------------------|
| 1 | **PrimeVul: Vulnerability Detection with Code Language Models: How Far Are We?**（ICLR 2025）[arXiv:2403.18624](https://arxiv.org/abs/2403.18624) | ADV 漏洞评测集的抽样与计分口径：用"漏洞/补丁成对样本 + 真实稀缺分布 + 去泄漏"替代旧噪声基准；该研究据此报告 7B 模型 F1 从旧基准虚高跌到 ~68，说明旧口径的分数不可用作内部比较基线。 |
| 2 | **SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering**（NeurIPS 2024）[arXiv:2405.15793](https://arxiv.org/abs/2405.15793) | adv-server MCP 工具面：工具粒度与输出格式按"ACI（面向 LM 的接口）"原则裁剪——紧凑查看器 + 带护栏编辑原语；同模型不同接口的成功率差距是该文的核心证据。 |
| 3 | **Agentless: Demystifying LLM-based Software Engineering Agents**（2024）[arXiv:2407.01489](https://arxiv.org/abs/2407.01489) | 编排层取舍：先用"定位→修复→验证"确定性三段流水线，agent 循环只留给流水线失败的分支；该研究以更低成本取得与复杂 agent 相当或更优的结果。 |
| 4 | **TestGen-LLM: Automatically Improving Unit Tests using LLMs**（FSE 2024 Industry）[arXiv:2310.02393](https://arxiv.org/abs/2310.02393) | xtask-gates 验收链：LLM 候选一律过机械过滤器（可编译、可运行、提升覆盖、不破坏既有测试）——"Assured LLMSE"与 ADV"确认权在静态引擎"同构，且有 Meta 部署数字（75% 编译/25% 提覆盖）。 |
| 5 | **LiveCodeBench: Holistic and Contamination Free Evaluation of LLMs for Code**（2024）[arXiv:2403.07974](https://arxiv.org/abs/2403.07974) | 评测集治理协议：滚动收录截止日期后新题 + 发布时间分桶，隔离训练污染；ADV 评测集应带时间戳分桶而非一次性冻结集合。 |
| 6 | **AutoCodeRover: Autonomous Program Improvement**（2024）[arXiv:2404.05427](https://arxiv.org/abs/2404.05427) | adv-index 检索 API：为 agent/LLM 提供符号级（类/方法签名）结构检索端点而非纯文本检索；该文以 AST 签名级搜索替代文本搜索完成定位。 |

---

## 一、LLM×静态分析（误报过滤 / 告警分诊 / 规则与规格生成）

ADV 核心命题区。**现状判断**：直接可复现协议的 FP 过滤论文偏少且多未核到 ID（下列待验证条目是补扫第一优先级）；已核实的机制证据集中在"LLM 只做二次确认、静态器做主判"的增强式架构。

- **P5-019**（P1）**E&v: Prompting LLMs to Perform Static Analysis via Pseudo-code Execution and Verification**（arXiv 2023）[2312.08477](https://arxiv.org/abs/2312.08477)。结论：让 LLM 以"伪代码逐行执行+验证"的方式做静态推理，比直接问答可靠。可抄机制：LLM 分析结果必须经伪代码执行器自验证再输出。映射 adv-rules（LLM 初筛的自验证环节）。
- **P5-043**（watch，待验证）**A Comparative Study of LLM Agents in Vulnerability False-Positive Detection**（arXiv 2026-01）。多 agent 各持 SAST 告警+目标文件测试代码做 FP/TP 分诊对比。与 ADV 误报过滤直接对口，补扫时优先核 ID。
- **P5-044**（watch，待验证）**CodeCureAgent**（OpenReview）。同一代理先抑制误报、再修复真报的三步启发式流水线。
- **P5-045**（watch，待验证）**IRIS: LLM-Assisted Static Analysis**（OpenReview）。神经符号路线：静态分析中间表示供 LLM 做全仓推理（注意与 ADV 引擎同名，非同一工作）。
- **P5-046**（watch，待验证）**Evaluating Static Analysis Alerts with LLMs**（CMU SEI, Ampel et al., 2024）。LLM 裁决 SAST 告警的初步实证：可给分诊信号，需人工复核。
- **P5-070**（watch，待验证）**Towards Practical and Scalable Bug Detection: Augmenting Static Analysis with LLMs**（ACM）。静态器为主、LLM 二次确认的增强式架构。
- **P5-047/048**（watch，待验证）**AutoACSL** / **SpecSyn**（arXiv）。规格生成方向：静态分析抽取算术/循环/返回值等要素供 LLM 生成 C 形式化规约并闭环精化——对应 ADV"LLM 产规格"边界，值得核原文拿协议。
- **P5-072**（watch，待验证）**Datadog SAST LLM False-Positive Filtering**（工业落地，2025-10）。生产 SAST 产品内嵌 LLM FP 过滤的工业参照，佐证该方向已达产品化。

## 二、漏洞检测评测与数据集

- **P5-001**（P0）**PrimeVul**（ICLR 2025）。见 P0 清单。附加结论：该数据集约 7k 漏洞函数覆盖 140+ CWE，真实漏洞占比极低（6968 vs 228800）——ADV 评测集要按此分布设计采样，不能用均衡二分类口径。
- **P5-008**（P1）**LLM4Vuln**（2024）[2401.16125](https://arxiv.org/abs/2401.16125)。把漏洞检测拆成知识/推理/信息格式/提示四变量独立评测；可抄：ADV 评测 LLM 候选时逐变量消融。
- **P5-021**（P1）**DiverseVul**（RAID 2023）[2304.00436](https://arxiv.org/abs/2304.00436)。从真实安全提交挖 349K 函数；数据构建协议（保留完整函数上下文、去重、明示假阴性来源）可直接套进 ADV 数据管线。
- **P5-013**（P2）**CyberSecEval**（2023）[2312.04724](https://arxiv.org/abs/2312.04724)。不安全代码生成率与拒答率此消彼长（过度拒答）——ADV 若用 LLM 做安全审查初筛，需把"过度拒答"计为独立误报类。

## 三、修复评估与 APR 协议（测试充分性 / 过拟合）

- **P5-007**（P1）**SWE-bench**（ICLR 2024）[2310.06770](https://arxiv.org/abs/2310.06770)。fail-to-pass 测试定义可解性 + 区域上下文过滤防泄漏；后续研究发现部分任务不可解——用它时须复核任务子集。
- **P5-049**（watch，待验证）**Are "Solved Issues" in SWE-bench Really Solved Correctly?**（ACM）。对 agent 工具补丁做正确性复核：表面通过≠语义正确。
- **P5-051**（watch，待验证）**Practical Program Repair in the Era of LLMs**（ICSE 2024）。plausible≠correct 的 LLM 版实证。
- **P5-050/052/053**（watch，待验证）**Assessing and Advancing Benchmarks for APR** / **Yang et al. 补丁正确性评估综述**（TOSEM 2023）/ **Improving Automated Patch Correctness Assessment**。三类评估尺子（测试驱动/语义等价/学习式）与失效场景——ADV 修复确认器选型时的坐标系。
- **P5-036**（P2，ID 待核）**SWE-bench-Live**（2025）[2505.23419](https://arxiv.org/abs/2505.23419)。滚动更新基准抗老化。
- **P5-054**（watch，待验证）**Red Teaming LLM-Based Program Repair Agents**（OpenReview）。修复 agent 压力测试的失败模式清单。

## 四、测试生成与验证

- **P5-004**（P0）**TestGen-LLM**（FSE 2024 Industry）。见 P0 清单——本轮与本域最贴的一篇。
- **P5-011**（P1）**LIBRO**（ICSE 2024）[2309.08870](https://arxiv.org/abs/2309.08870)。从失败用例仿开发者风格生成复现测试；可抄：把复现测试用作静态告警的验证器（三角验证候选）。
- **P5-064**（watch，链接待验证）**CodaMosa**（ICSE 2024）。搜索式为主、LLM 只补覆盖平台期缺口；混合路线优于任一单独路线——与 ADV"静态为主、LLM 补盲区"同构。
- **P5-010**（P1）**EvalPlus**（NeurIPS 2023）[2305.01210](https://arxiv.org/abs/2305.01210)。LLM 生成+变异增强测试套件暴露假通过；测试充分性是评分上限——xtask-gates 的测试强度审计可参照。
- **P5-018/017**（P2）**No More Manual Tests?**（2023）[2305.04207](https://arxiv.org/abs/2305.04207) / **LLM-Guided Issue Generation from Uncovered Code**（2025）[2510.19898](https://arxiv.org/abs/2510.19898)。前者：LLM 单测覆盖接近人工但断言弱；后者：覆盖缺口→issue→复现测试两段链路。
- **P5-055/056/057/058**（watch，待验证）**改进 LLM 单测可靠性（ACM）** / **LLM 生成测试 flaky 实证（2026）** / **TiCoder** / **Issue2Test**。共同指向：可靠性来自验证/修复系统链，flaky 过滤应进验收链。

## 五、代理与工具面（工具面设计结论）

- **P5-002**（P0）**SWE-agent**（NeurIPS 2024）。ACI 设计证据。
- **P5-003**（P0）**Agentless**（2024）。确定性流水线优先。
- **P5-015**（P1）**CodeAct**（ICML 2024）[2402.01030](https://arxiv.org/abs/2402.01030)。可执行代码作为统一动作空间优于 JSON 动作；执行反馈支撑迭代。
- **P5-016**（P1）**OpenHands**（2024）[2407.16741](https://arxiv.org/abs/2407.16741)。平台工程抽象（事件流/沙箱/工具注册）——MCP 工具面组件清单来源。
- **P5-023**（P2）**CodePlan**（2023）[2309.12499](https://arxiv.org/abs/2309.12499)。仓库级变更=依赖图规划，LLM 只做单步变换——与 ADV 确认权边界一致。
- **P5-038**（P2）**Model-Based Agentic SE**（2026）[2608.25174](https://arxiv.org/abs/2608.25174)。agent 提升产能但不自动带来意图/结构/验收结构——外部结构化约束（即静态引擎）必要的最新佐证。
- **P5-040/059**（watch/P2，待验证）**OSS-Fuzz-Gen**（Google）/ **MapCoder**（ACL 2024，ID 待核）。fuzz driver 自动生成接入真实流水线；多代理检索-模仿链。

## 六、检索与仓库级上下文（评测协议优先）

- **P5-006**（P0）**AutoCodeRover**（2024）。AST 签名级检索端点。
- **P5-012**（P1）**RepoBench**（ICLR 2024）[2306.03091](https://arxiv.org/abs/2306.03091)。检索(R)/补全(C)/流水线(P)子任务拆分；检索质量是瓶颈。
- **P5-013**（P1）**CrossCodeEval**（NeurIPS 2023）[2310.11248](https://arxiv.org/abs/2310.11248)。构造"无跨文件上下文必失败"样本+去重防泄漏——ADV 检索评测集构造法。
- **P5-014**（P1）**RepoCoder**（EMNLP 2023）[2303.12570](https://arxiv.org/abs/2303.12570)。迭代式检索→生成→以生成结果再检索；稠密+稀疏混合结论。
- **P5-024**（P2，ID 待核）**RepoGraph**（2024）[2410.14684](https://arxiv.org/abs/2410.14684)。仓库图（定义/引用/依赖）跳转导航替代全文检索——adv-index 图结构候补。
- **P5-059/060/061**（watch，待验证）**RepoMirage** / **RepoExec** / **STALL+**（2024–2026）。仓库上下文推理探测与带执行校验的补全评测。
- **P5-062/063**（watch，待验证）**AST(NIT)** / **Samoaa et al. 代码表示系统映射研究**（ACM）。序列化 AST 增强输入 vs 源码；103 篇研究的表示×任务映射——ADV 是否需要 AST/图表示进检索路径时先读第二篇。

## 七、代码摘要与解释

本轮命中偏弱（查询一次超时一次失败），以下三条均为题名级待验证，补扫时建议扩展检索词（code documentation / explanation quality / LLM overconfidence）。

- **P5-064**（watch，待验证）**Xue et al. 自动提交消息生成实证**（TOSEM 2024）。
- **P5-065**（watch，待验证）**Commit Message Generation via ICL 实证**（arXiv 2025-02）。
- **P5-067**（watch，待验证）**Vulnerability Commit Message Generation**（ACM）。
- 判读：对 ADV 解释层的直接可用证据暂不足，仅确立"BLEU 类指标失真、需人工/落地指标"这一方向性结论；解释层设计暂以 P1/P2 域静态工具的解释能力为基底。

## 八、数据、基准与防泄漏

- **P5-005**（P0）**LiveCodeBench**（2024）。滚动收录+时间窗分桶。
- **P5-020**（P1）**BigCodeBench**（2024）[2406.15877](https://arxiv.org/abs/2406.15877)。1140 个多库调用+复杂指令任务、执行级校验——比 HumanEval 更贴近实用能力下界。
- **P5-068**（watch，待验证）**LiveCodeBench Pro**（2025）。竞赛难度分桶暴露推理深度差异。
- **P5-029/030/032/042**（P2）**OctoPack/CommitPack** [2308.07124](https://arxiv.org/abs/2308.07124) / **Magicoder-OSS-Instruct** [2312.02120](https://arxiv.org/abs/2312.02120) / **The Vault**（ID 待核）[2305.06156](https://arxiv.org/abs/2305.06156) / **WaveCoder**（ID 待核）。数据合成配方：提交对指令化、开源片段种子自指令、函数级多语语料、多样性蒸馏——ADV 自建评测/微调数据时的四个模板。
- **P5-035**（P2）**NLP×SE 统一综述**（2023）[2311.07990](https://arxiv.org/abs/2311.07990)。任务/数据/评测统一坐标系，作导航地图用。

## 九、本地小模型（不选型，只收结论）

- **P5-017**（P1）**DeepSeek-Coder**（2024）[2401.14196](https://arxiv.org/abs/2401.14196)。仓库级依赖打包预训练；1.3B/6.7B 级别具备可用代码能力，可离线。
- **P5-018**（P1）**Qwen2.5-Coder**（2024）[2409.12186](https://arxiv.org/abs/2409.12186)。小尺寸+长上下文与仓库级能力实证。
- **P5-019**（P1）**StarCoder2 / The Stack v2**（2024）[2402.19173](https://arxiv.org/abs/2402.19173)。全透明数据+许可证过滤（opt-out）管线——ADV 数据合规工程直接对标。
- **P5-025/026/027/028/031/039**（P2）**CodeLlama**（SPM infilling）/ **DeepSeek-Coder-V2**（MoE 折中）/ **Granite**（企业许可干净数据）/ **CodeGemma**（FIM，ID 待核）/ **WizardCoder**（Evol-Instruct）/ **OpenCodeInterpreter**（生成-执行-修复闭环，ID 待核）。
- 判读（限本轮样本）：分类/排序/单测初筛等轻任务在 2B–7B 级可离线完成是各报告的一致方向，但具体阈值须在 ADV 硬件上复测，本扫视不给出选型结论。

## 十、跨域接入（fuzz/安全生成，供 P3 域交叉引用）

- **P5-022**（P2）**Fuzz4All**（ICSE 2024）[2308.04748](https://arxiv.org/abs/2308.04748)。LLM 作输入生成与变异引擎+进化循环，目标无关 fuzz 前端。
- **P5-060**（watch，待验证）**Mitigating False Positive Crashes in OSS-Fuzz-Gen Using LLMs**（2025-10）。fuzz 误报崩溃的 AI 降噪。

---

## 十一、专题A补扫：LLM×静态分析误报过滤 / 告警三角验证（2024–2026，2026-10-04）

> 方法：4 组 arXiv API 定向查询（`ti:"false positives"+ti:"static analysis"`、`ti:"static analysis"+abs:"large language"+abs:"false positive"` 等）+ 3 组 WebSearch 复核来源；只收 arXiv ID 已核实、摘要能说出机制的条目（20 条新条目全部 sweep+arXiv ID 核实）。正文前十个章节里个别 P5 编号与本表有漂移，以 `registry/P5.jsonl` 为准。

- **P5-073**（P1）**SastBench: A Benchmark for Testing Agentic SAST Triage**（2026）[2601.02941](https://arxiv.org/abs/2601.02941)。真实 CVE 作真报 + 过滤后 SAST findings 作噪声拼成分诊基准，agent 无关评测。可抄机制：ADV 误报过滤评测集按"真报=真实 CVE、噪声=静态器告警"两源构造。映射 xtask-gates。
- **P5-074**（P1）**ZeroFalse: Improving Precision in Static Analysis with LLMs**（2025）[2510.02534](https://arxiv.org/abs/2510.02534)。把 LLM 二次复核做成 SAST 输出的后处理层提升精度。可抄：FP 过滤=静态器输出的独立后处理层，确认权留静态器。映射 adv-rules。
- **P5-075**（P1）**GPTScan: Detecting Logic Vulnerabilities in Smart Contracts by Combining GPT with Program Analysis**（ICSE 2024）[2308.03314](https://arxiv.org/abs/2308.03314)。GPT 判漏洞场景、程序分析匹配确认候选——静态模式反向过滤 LLM 误报。与 ADV"LLM 产候选、引擎确认"边界同构。映射 adv-rules。
- **P5-076**（P2）**KNighter: Transforming Static Analysis with LLM-Synthesized Checkers**（2025）[2503.09002](https://arxiv.org/abs/2503.09002)。LLM 从历史补丁合成 checker，静态分析全仓执行验证（Linux kernel）——LLM 产规则、引擎跑规则的模板。映射 adv-rules。
- **P5-077**（P2）**LLM-Driven Adaptive Source-Sink Identification and False Positive Mitigation**（2025-11）[2511.04023](https://arxiv.org/abs/2511.04023)。LLM 自适应补全 source-sink 规约并缓解不完备规约 FP。映射 adv-taint。
- **P5-078**（P2）**FuzzSlice: Pruning False Positives in Static Analysis Warnings Through Function-Level Fuzzing**（2024）[2402.01924](https://arxiv.org/abs/2402.01924)。函数级 fuzz 用执行证据剪除告警 FP——"执行验证三角"实例。映射 xtask-gates。
- **P5-079/080/081/082**（watch/P2）**Argus**（多 agent SAST 重编排）[2604.06633](https://arxiv.org/abs/2604.06633) / **Cracking IoT Security**（LLM 补平台语义过滤 context-blind FP）[2601.00559](https://arxiv.org/abs/2601.00559)（P2）/ **SAST-Genius**（LLM+SAST 混合技术报告）[2509.15433](https://arxiv.org/abs/2509.15433) / **Benchmarking LLM-Based Static Analysis for Secure Smart Contract Development**（可靠性基准）[2605.11163](https://arxiv.org/abs/2605.11163)。
- **回填成果**（原待验证 → 已核实）：P5-044 CodeCureAgent=[2509.11787](https://arxiv.org/abs/2509.11787)（FSE 2026，LLM 告警分类+修复，检索到 LLM 成本 ~2.9 美分/告警的量级数字）；P5-045=CMU SEI 博客页；P5-047=FalseCrashReducer=[2510.02185](https://arxiv.org/abs/2510.02185)；P5-048=[2503.15223](https://arxiv.org/abs/2503.15223)；P5-053=[2509.25894](https://arxiv.org/abs/2509.25894)；P5-056/064/067/070 同批核实（见登记表）。"Maracay" 一名多轮检索无果，判定为误记不收（宁少勿假）。

## 十二、专题B补扫：代码摘要 / 提交消息生成（2024–2026，2026-10-04）

> 方法：arXiv `ti:"commit message"+ti:generation`、`ti:"code summarization"` 按 submittedDate 各取近 8 条 + CommitBench/CommitSuite/GPTScan 定向核 ID，全部经 arXiv API 摘要核实。

- **P5-083**（P2）**CommitBench: A Benchmark for Commit Message Generation**（2024）[2403.05188](https://arxiv.org/abs/2403.05188)。CMG 统一评测基准；源码预训练 Transformer 优于其他路线。映射 reference。
- **P5-084**（P2）**CommitSuite: A Comprehensive Benchmark for Commit Classification and Message Generation**（2026）[2605.02256](https://arxiv.org/abs/2605.02256)。提交分类+生成双任务，按 Conventional Commits 规范校验一致性与信息量。映射 xtask-gates。
- **P5-085**（P2）**Evaluating Generated Commit Messages with Large Language Models**（2025）[2507.10906](https://arxiv.org/abs/2507.10906)。LLM 当评审替代失真的 BLEU 类指标。映射 xtask-gates（解释层验收协议候选）。
- **P5-086**（P2）**Brevity is the Soul of Wit: Condensing Code Changes to Improve Commit Message Generation**（2025）[2509.15567](https://arxiv.org/abs/2509.15567)。先压缩 diff 再生成——输入精简机制可移植到告警解释的上下文裁剪。映射 reference。
- **P5-087**（P2）**ReFEree: Reference-Free and Fine-Grained Method for Evaluating Factual Consistency in Code Summaries**（2026）[2604.10520](https://arxiv.org/abs/2604.10520)。无参考细粒度事实一致性评测（LLM 生成摘要的幻觉检测）——直修第七节"BLEU 失真"痛点。映射 xtask-gates。
- **P5-088/089**（P2）**Prompt-Driven Code Summarization: SLR**（2026）[2604.15385](https://arxiv.org/abs/2604.15385) / **Precision in Practice**（工业期望锚定的知识引导摘要）[2602.03400](https://arxiv.org/abs/2602.03400)。解释层设计导航与工业验收口径参照。
- **P5-090/091/092**（watch）**CoRaCMG**（上下文检索增强 CMG）[2509.18337](https://arxiv.org/abs/2509.18337) / **LLM-Enhanced CMG via Issue Information**（issue 外部上下文增益边界）[2608.22004](https://arxiv.org/abs/2608.22004) / **CommitLLM**（微调流水线）[2607.17532](https://arxiv.org/abs/2607.17532)。
- **判读更新**：第七节"对 ADV 解释层直接可用证据不足"已部分失效——评测侧有 LLM-as-judge（P5-085）与无参考事实一致性（P5-087）两条可落地协议；生成侧机制（diff 压缩、检索增强、规范校验）均有 arXiv 摘要级实证，具体数字待深读后回填。

---

## 统计与遗留（2026-10-04 专题 A/B 补扫后更新）

- 登记 92 条（72+20）：P0×6 · P1×18 · P2×30 · watch×38；补扫 20 条均 sweep + arXiv ID 已核。链接：原 47 条带 arXiv ID 基础上回填 9 条（P5-044/045/047/048/053/056/064/067/070），新增 20 条全带 ID。
- **待验证余额**：12 条（P5-043/046/050/051/052/054/057/059/062/066/068/072）——CodaMosa/TiCoder/RepoExec 在 arXiv API 按标题两轮未命中（三个记忆 ID 经 id_list 反查全部证伪，宁少勿假维持原状）；Datadog 博客（P5-072）两次 WebSearch 未定位原帖。
- **遗留补扫**：① NeurIPS/ICLR/ACL/EMNLP 2024–2025 录取列表人工扫；② SWE-bench 污染批判专题；③ CodeFuse-CommitEval（2025-11，消息-代码不一致检测基准）有题名未核 ID。
- 红线遵守：所有数字带出处锚；「待验证」未混入已核实集合；无绝对化表述；补扫样本限于关键词检索路径，录取列表未全量人工扫（结论受此限）。
