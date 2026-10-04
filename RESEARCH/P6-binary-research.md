# P6：二进制/反编译研究论文扫视（USENIX/NDSS/CCS/S&P/ASE/NeurIPS + arXiv，2023–2026）

> 扫描日期：2026-10-04 · 扫描人：ZCode P6 子任务 · 配套登记表：`registry/P6.jsonl`（56 条）
> 定位：补 R1–R4 工具层调研之上的**论文前沿**——反编译质量、LLM×反编译、二进制相似性、函数/库识别、符号执行与 lifting、固件、补丁分析、反混淆。重点收"有开源实现与基准"的实证工作，为 ADV 评测集建设提供协议参考。

**方法与偏差声明**：本轮 ~14 组定向 WebSearch（arXiv/USENIX/NDSS/ACM DL 优先），6 条最关键链接经 WebFetch 逐页核实（SAILR/LLM4Decompile/BinaryCorp 相关/DeGPT/ReSym）；其中若干查询因搜索并发限制未完成，USENIX/NDSS/CCS/S&P 各届录取页未做人工全量扫描——覆盖存在缺口，结论受此样本限制。凡未核到原文/ID 的条目一律标「待验证」且 depth=题名级（约 40 条），宁少勿假；两处早期记忆被本轮证伪：①arXiv:2310.18608 不是 BinaryCorp（是推荐系统综述）；②LibDB 实为 MSR 2022 而非 S&P 2024；③DeGPT 实为 NDSS 2024 而非 ICSE 2025。后续补扫优先消化这批待验证条目。

---

## P0 清单（读了能直接改哪个设计决策）

| # | 论文 | 直接改动的设计决策 |
|---|------|--------------------|
| 1 | **Ahoy SAILR! There is No Need to DREAM of C: A Compiler-Aware Structuring Algorithm for Binary Decompilation**（USENIX Security 2024）[usenix.org](https://www.usenix.org/conference/usenixsecurity24/presentation/basque) | adv-bin 反编译质量评测口径：弃 DREAM 式"编译器模式匹配"评分，改用与源码对齐的结构差异度量（spurious goto 计数等）；structuring 选 compiler-aware 路线，评测协议可直接搬进 adv-bin 验收。 |
| 2 | **LLM4Decompile: Decompiling Binary Code with Large Language Models**（EMNLP 2024）[arXiv:2403.05286](https://arxiv.org/abs/2403.05286) | adv-bin 反编译评测主指标：用 re-executability（可重执行率）替代 BLEU/编辑距离这类代理指标；产品线分两条——直接反编译（End）与 Ghidra 输出后处理（Ref），Ref 比 End 再 +16.2%，支持"静态反编译器为主、LLM 做后处理"的架构选择。 |
| 3 | **BinaryCorp: A New Benchmark and Dataset for Binary Code Similarity Detection**（ASE 2023，arXiv ID 待验证）[dataset 页](https://papersgraph.com/datasets/binarycorp) | ADV 评测集建设：从发行版包源（Arch 官方仓+AUR）自动构建"多编译器×多优化档×多架构（20 架构、~3900 项目、8.8M 函数对）"的函数对流水线——评测集应靠流水线可再生，而非人工挑选。 |
| 4 | **ReSym: Recovering Variable Symbols and Data Structures in Stripped Binaries**（CCS 2024）[PDF](https://www.cs.purdue.edu/homes/lintan/publications/resym-ccs24.pdf) | adv-bin 类型/变量恢复模块：LLM 恢复变量与结构体时的输入构造（反编译草稿+符号线索）与评测协议；全剥离二进制场景下的变量符号恢复是 adv-bin 输出质量的关键缺口。 |
| 5 | **CEBin: A Cost-Effective Framework for Large-Scale Binary Code Similarity Detection**（2024，venue 记 ASE'24 待验证）[GitHub](https://github.com/Hustcw/CEBin) | 二进制相似模块的评测口径：首次给出 1-day 漏洞检测任务上 BCSD 方法的精确评测方案——先粗筛/剪枝再精排的级联评测，避免大规模全比对的高成本与指标虚高；adv-bin 相似性验收直接套用。 |
| 6 | **DeGPT: Optimizing Decompiler Output with LLM**（NDSS 2024）[官方 PDF](https://www.ndss-symposium.org/wp-content/uploads/2024-401-paper.pdf) | LLM 后处理链路的验证设计：三角色（重构/命名/审查）互相校验的 MVC 式机制防 LLM 幻觉改语义；其"认知负担降低 24.4%"的阅读时间代理评测法可移植为 adv-bin 输出可读性指标。 |

---

## 一、反编译质量（控制流结构 / 类型 / 变量名）

- **P6-001**（P0）**Ahoy SAILR! There is No Need to DREAM of C: A Compiler-Aware Structuring Algorithm for Binary Decompilation**（USENIX Security 2024，ASU）[链接已核](https://www.usenix.org/conference/usenixsecurity24/presentation/basque)。结论：DREAM 类评测依赖编译器特定模式有缺陷；SAILR 用编译器感知的 structuring 减少 spurious goto。可抄机制：以"源码为真值"的 structuring 差异评测 + 开源冻结版 angr 复现。映射 adv-bin（反编译内核与验收）。
- **P6-002**（P0）**ReSym: Recovering Variable Symbols and Data Structures in Stripped Binaries**（CCS 2024）[PDF 已核](https://www.cs.purdue.edu/homes/lintan/publications/resym-ccs24.pdf)。结论：LLM 可从全剥离二进制恢复变量与数据结构。可抄机制：反编译草稿+线索注入的输入构造、逐变量评测协议。映射 adv-bin（类型/变量恢复）。
- **P6-003**（P1，待验证）**OSPREY**（USENIX Security，年份/ID 待核）。概率式变量/结构体恢复，与 ReSym 同题竞争；补扫时核对评测集与错误分布口径。
- **P6-004**（P2，待验证）**GENNM**（NDSS 2025）。生成式模型从剥离二进制恢复函数名；与 LLM 命名路线互为基线。
- **P6-005**（P2）**Practical Type Inference: High-Throughput Recovery of Types from Stripped Binaries**（arXiv 2026）[arXiv:2603.08225](https://arxiv.org/html/2603.08225v2)。结论：类型恢复的瓶颈在工程吞吐而非模型精度。可抄机制：高吞吐类型恢复流水线。映射 adv-bin（批量类型推断）。
- **P6-006**（P2，待验证）**Towards Sound Reassembly of Modern x86-64 Binaries**（KAIST，2025）。现代 x86-64 重汇编的声音性（soundness）论证；对 adv-bin 的 lifting/reassembly 验收是直接的安全边界参考。
- **P6-007**（P2，ID 待验证）**HexT5: Unified Pre-Training for Stripped Binary Code for Program Summarization**（ASE 2023，Xiong et al.）。剥离二进制代码的统一预训练+摘要生成；数据构建（DIRE 上的实验组织方式）可参考。
- **P6-008**（watch，待验证）**DIRTY**。为反编译代码生成有意义的变量名/类型；LLM 命名路线的早期基线。
- **P6-009**（P2）**Unstripping Binaries: Restoring Debugging Information in GDB with Pwndbg**（Trail of Bits 博客 2024-09）[链接已核](https://blog.trailofbits.com/2024/09/06/unstripping-binaries-restoring-debugging-information-in-gdb-with-pwndbg/)。工程参照：把恢复的调试信息注回 DWARF 供 GDB 使用——adv-bin "恢复结果落盘为标准调试信息"的出口格式参考。

## 二、LLM×反编译（只收有评测的）

- **P6-010**（P0）**LLM4Decompile: Decompiling Binary Code with Large Language Models**（EMNLP 2024，Tan/Luo/Li/Zhang）[arXiv:2403.05286 已核](https://arxiv.org/abs/2403.05286)。结论：1.3B–33B 开源模型系列，End（直接反编译）/Ref（Ghidra 后处理）两条线；re-executability 为指标，超 GPT-4o 与 Ghidra 100%+，Ref 比 End +16.2%。可抄机制：可重执行率评测 + HumanEval/ExeBench 编译配对数据构造。映射 adv-bin。
- **P6-011**（P0）**DeGPT: Optimizing Decompiler Output with LLM**（NDSS 2024，Peihu Hu et al.）[官方 PDF 已核](https://www.ndss-symposium.org/wp-content/uploads/2024-401-paper.pdf)。结论：三角色 MVC 式互相校验重构反编译输出，认知负担 -24.4%。可抄机制：多角色校验防语义漂移 + 阅读时间代理评测。映射 adv-bin（后处理验证链）。
- **P6-012**（P1）**D-LiFT: Improving LLM-based Decompiler Backend via Code Quality-driven Fine-tuning**（arXiv 2025）[arXiv:2506.10125 已核](https://arxiv.org/abs/2506.10125)。结论：GRPO 强化学习 + D-SCORE 质量度量对齐反编译后端。可抄机制：把"反编译输出质量评分"做成可微奖励/验收分数（D-SCORE 维度划分）。映射 adv-bin。
- **P6-013**（watch，待验证）**BinRAG: An RAG-Based Decompilation Framework**（ACM DL 2026）。检索增强反编译：用相似函数库辅助反编译；对 adv-index 的跨函数检索复用有启发。
- **P6-014**（watch）**CoDe-R: Refining Decompiler Output with LLMs via Rationale**（arXiv:2604.12913）。带推理链的反编译输出精化；与 DeGPT/D-LiFT 同族，补扫时对比三者评测集。
- **P6-015**（watch，待验证）**FidelityGPT**。检测/修正反编译输出与原二进制的语义偏差；"保真度检查器"可并入 adv-bin 验收。
- **P6-016**（watch，待验证）**ShieldedCode**（OpenReview）。VMP 混淆代码的鲁棒表示；在 BinaryCorp-VirtualAssembly 上评测——混淆鲁棒性评测集的组织方式可借鉴。
- **P6-017**（watch，待验证）**Control Flow-Augmented Decompiler based on LLM**（2026）。控制流信息注入 LLM 反编译；与 adv-bin CFG 中间表示对接的自然接口。
- **P6-018**（watch，待验证）**Nova: Generating Dual-Level Explanations between a Binary and its Decompiled Code**（arXiv 2024，ID 待核）。二进制↔反编译代码的双层解释；解释对齐评测可作 adv-bin 报告模块参考。
- **P6-019**（watch，待验证）**NOVA: Generative Language Models for Assembly**（OpenReview，N. Jiang et al.）。汇编代码生成式模型（注意与上一条重名，是两篇不同工作）。

## 三、二进制相似性、数据集与评测协议（评测集建设重点区）

- **P6-020**（P0）**BinaryCorp: A New Benchmark and Dataset for Binary Code Similarity Detection**（ASE 2023；arXiv ID 待验证）[dataset 页已核](https://papersgraph.com/datasets/binarycorp)。结论：源自 Arch 官方仓+AUR，~3900 项目/20 架构/8.8M 函数对（另有 BinaryCorp-3M 训练子集），one-to-many 检索口径。可抄机制：发行版包源→编译矩阵→函数对的自动化评测集流水线。映射 adv-bin 评测集。
- **P6-021**（P1，ID 待验证）**DIRE 数据集**（与 jEcoForge 配套，GitHub 开源）。同源函数不同编译优化档的"编译器效应"配对数据集；可抄机制：靠编译器差异自动生成带真值的相似/不相似对（真值不需要人工标注）。映射 adv-rules/评测集。
- **P6-022**（P1，ID 待验证）**Is Function Similarity Over-Engineered? Building a Benchmark**（NeurIPS 2024，Saul/Liu/Fleischmann/Zak/Micinski/Raff/Holt，NeurIPS 37 卷 pp.21636–21655）。结论：以 Assemblage 语料构建基准，质疑函数相似性模型在"任务实效"上的过度工程。可抄机制：从任务（漏洞检测等）反推评测指标而非模型间竞赛指标。映射 adv-bin 验收。
- **P6-023**（P0）**CEBin: A Cost-Effective Framework for Large-Scale Binary Code Similarity Detection**（2024，venue 待验证）[GitHub 已核](https://github.com/Hustcw/CEBin)。结论：首个面向 1-day 漏洞检测任务的 BCSD 精确评测方案。可抄机制：级联（粗筛→精排）评测与成本计量。映射 adv-bin。
- **P6-024**（P2）**BinSimDB: Benchmark Dataset Construction for Fine-Grained Binary Code Similarity Analysis**（arXiv 2024）[arXiv:2410.10163](https://arxiv.org/abs/2410.10163)。细粒度（函数内区域级）相似数据集构建；粒度切分协议可借鉴。
- **P6-025**（P2，待验证）**BinCodex: A Comprehensive and Multi-Level Dataset for BCSD Evaluation**（2024，ScienceDirect）。多层级相似数据集；补扫核对层级定义与 BinaryCorp 的差异。
- **P6-026**（P2）**jTrans: Jump-Aware Transformer for Binary Code Similarity Detection**（NeurIPS 2022，基线地位）[GitHub 已核](https://github.com/vul337/jTrans)。跳转感知表征；当前多数相似性工作（含 BinaryCorp 评测）以它为基线，adv-bin 自研评分器需与之对比。
- **P6-027**（watch，待验证）**Knowledge-Aware Transformer for Binary Code Embedding**（arXiv）。在 BinaryCorp 上评测的知识注入式嵌入；知识注入方式（调用图/符号）可借鉴。
- **P6-028**（P2，venue 待验证）**CodeArt: Better Code Models by Attention Regularization**（ACM DL；用 BinaryCorp-3M 评测）。注意力正则提升代码模型；训练技巧参考。
- **P6-029**（watch，待验证）**Multimodal Instruction Disassembly with Covariate Shift**（arXiv 2024-10）。反汇编的多模态与分布偏移问题；对 adv-bin 反汇编器跨编译器泛化是直接警示。
- **P6-030**（watch，待验证）**Robustness of binary function similarity models**（Capozzi et al., Sapienza）。对相似性模型做对抗/扰动稳健性检验；评测集需要"扰动维度"字段。

## 四、函数识别与库识别（FLIRT 后继 / SBOM）

- **P6-031**（P2）**LibDB: An Effective and Efficient Framework for Detecting Third-Party Libraries in Binaries**（MSR 2022——本轮已证伪"S&P 2024"记忆）。基于函数内容的第三方库检测，含剥离二进制；adv-bin 库识别模块的基线方法。
- **P6-032**（P1，ID 待验证）**V1SCAN: Discovering 1-day Vulnerabilities in Reused C/C++ Open-Source Software Components**（USENIX Security 2023）。代码分类技术在复用组件 1-day 漏洞发现上的应用；库识别→漏洞关联的完整链路参照。映射 adv-bin+adv-rules。
- **P6-033**（watch，待验证）**Tool or Toy: Are SCA Tools Ready for Challenging Scenarios?**（ACM DL 2025-11）。对 SCA 工具（含二进制侧）在挑战场景下的系统评测；ADV 的 SCA 能力定位与差距清单。
- **P6-034**（watch，待验证）**CASTLE: Comprehensive Detection of 1-day Vulnerability through Code Patch Analysis in Third-Party Library Reuse**（2024-11）。补丁分析→复用组件 1-day 检测；与 V1SCAN 互补。
- **P6-035**（watch，待验证）**P1OVD: Patch-Based 1-Day Out-of-Bounds Vulnerabilities Detection for Downstream Binaries**。下游自编译二进制的补丁式检测；"下游分发链"场景与 ADV 本地优先定位契合。

## 五、符号执行与 lifting 的新算法

- **P6-036**（watch，ID 待验证）**Gordian: Defusing Logic Bombs in Symbolic Execution with LLMs**（arXiv）。LLM 拆解符号执行中的逻辑炸弹约束，覆盖率较传统基线 +52–84%（原文口径）；LLM 引导路径选择的速度/覆盖率实证，可作 adv-bin 深度分析模式的参照。
- **P6-037**（watch，待验证）**JIGSAW: Efficient and Scalable Path Constraints Fuzzing**。路径约束的规模化 concolic；约束批处理与求解预算管理可借鉴。
- **P6-038**（watch，待验证）**SEDiff: Scope-Aware Differential Fuzzing for Internal Function Models**（ACM DL）。符号执行引擎内部函数模型的差分测试——测试 lifting/模型层正确性的方法论。
- **P6-039**（watch，待验证）**Direct State Manipulation in Hybrid Virtual CPU Fuzzing**（NDSS）。虚拟 CPU 模糊测试中的直接状态操纵；与 R2 域的仿真器路线衔接。

## 六、固件分析（大规模数据集 / 工具链）

- **P6-040**（P1）**FirmSolo: Enabling Dynamic Analysis of Binary Linux-based IoT Firmware**（USENIX Security 2023，Angelakopoulos et al.）[GitHub 已核](https://github.com/BUseclab/FirmSolo)。内核空间并入固件仿真，可启动与动态分析；"内核+根文件系统配对构建"的数据管线是固件域评测集的骨架。映射 reference（R2/固件线）。
- **P6-041**（watch，待验证）**FirmDiff: Improving the Configuration of Linux Kernels for Firmware Analysis**（NDSS，同组续作）。修内核配置使 re-hosting 成功率提升；10 固件/148 内核模块的失败样本集可复用。
- **P6-042**（watch，待验证）**SoK: A Large-Scale Empirical Study of Emulation-Based MCU Firmware Analysis**（arXiv 2025）。MCU 仿真分析的系统化综述；选型坐标系。
- **P6-043**（watch，待验证）**RT-Fuzzer: Task Driven Fuzzing of RTOS Firmware**（NDSS）。RTOS 任务驱动模糊测试。
- **P6-044**（watch，待验证）**IoTBolt**（arXiv 2025-05）。挖掘 IoT 隐藏服务的自动固件分析。
- **P6-045**（watch，待验证）**ADFEmu**（2025-07）。DMA 直接内存访问仿真增强固件模糊测试——外设仿真缺口的针对性方案。
- **P6-046**（watch，待验证）**MetaEmu**（2026）。架构无关仿真器合成器（车载固件 rehosting）；"按需合成仿真器"思路对多架构 adv-bin 有长期参考价值。
- **P6-047**（watch，待验证）**FirmAEHF**（2025）。FirmAE 谱系的动态分析增强；与 FirmSolo 路线的互补性待核对。

## 七、二进制差分与补丁分析（1-day 自动化）

- **P6-048**（P1，具体页待验证）**PPTSP: Patch Presence Test via Semantic Normalization**（ACM DL 2026，Xu et al.）。语义归一化做补丁存在性判定，摆脱"仅靠脆弱信号"的旧路线；语义归一化可并入 adv-bin 补丁比对预处理。映射 adv-bin。
- **P6-049**（P2，待验证）**Fine-Grained 1-Day Vulnerability Detection in Binaries via Patch Code Localization**（2025，Dong et al., IIE）。补丁代码定位→细粒度 1-day 检测；"先定位后判定"的两段式与 CEBin 级联同构。
- **P6-050**（watch，待验证）**Match & Mend: Minimally Invasive Local Reassembly for Patching N-day Vulnerabilities in ARM Binaries**（arXiv 2025-10）。从检测走到"二进制打补丁"（局部重汇编）；adv-bin 若做修复输出，这是安全边界参考。

## 八、反混淆与去虚拟化（有开源实现的优先）

- **P6-051**（P1，ID 待验证）**Can LLMs Deobfuscate Binary Code? A Systematic Study (BinDeObfBench)**（arXiv 2026-04）。首个评测 LLM 去混淆能力的基准；混淆算子×LLM 能力的矩阵是 adv-rules 混淆检测规则的评测坐标系。
- **P6-052**（P2，待验证）**LLM-DAS: An LLM-Powered Deobfuscation System for ARM Binary**（ACM 2025）。首个专攻 ARM 的 LLM 去混淆系统+混淆数据集；跨架构混淆语料参考。
- **P6-053**（P2）**LLVM-Powered Deobfuscation of Virtualized Binaries**（Thalium 工程博客 2024-11，[blog.thalium.re](https://blog.thalium.re)，post 路径待验证）。动态污点+LLVM lift+优化管线的去虚拟化全流程；工程上最完整的开源思路模板。映射 adv-bin（lifting+优化复用）。
- **P6-054**（P2，页待验证）**Deobfuscation of Virtualization-Obfuscated Code Through Symbolic Semantics & Compile-Time Optimizations**（Temple Univ.，开源工具）。符号语义提取+编译期优化做部分虚拟化简化；"可验证的简化"是 ADV 要求的否证式验收形态。
- **P6-055**（P2，归属待验证）**dewolf**（开源反编译器，Uni Bonn/FKIE 谱系，基于 angr）。开源反编译器实现与论文谱系——adv-bin 自研反编译器的对照组与 structuring 参考。
- **P6-056**（watch，待验证）**D-810**（IDA 去混淆插件）。规则式去混淆的工程实现；规则组织方式可对照 adv-rules。

---

## 横向结论：对 ADV 评测集建设的四条可执行借鉴

1. **流水线生成真值**：BinaryCorp/DIRE 的共同点是"编译矩阵自动产生带真值的函数对"——ADV 评测集应以构建流水线为中心资产，样本集是流水线的输出（可复算、可再生）。
2. **任务实效指标**：CEBin（1-day 检测）与"Is Function Similarity Over-Engineered?"（NeurIPS 2024）共同指向：模型间竞赛指标会过度工程化，验收应挂任务实效（补丁存在性判定准确率、可重执行率）。
3. **代理指标要换血**：反编译质量从 DREAM 式编译器匹配（SAILR 批判）与 BLEU（LLM4Decompile 的 re-executability 替代）两次换血——adv-bin 评测集应避免一次性锁死代理指标，留"真值对齐"口径。
4. **级联与成本入账**：CEBin/PPTSP/补丁定位类工作的共同形态是"粗筛→精排/先定位后判定"——adv-bin 大规模索引的评测要同时记精度与算力成本两个维度。

## 补扫清单（下轮优先）

- BinaryCorp/HexT5/DIRE/V1SCAN/Gordian/BinDeObfBench 的 arXiv ID 与官方页（当前仅凭搜索摘要定位）。
- USENIX Security 2025 / NDSS 2025 录取页中 binary/decompilation 主题的全量扫描（本轮未做，缺口最大）。
- OSPREY、GENNM、Nova 双层解释三篇的 venue 与链接核实。
- SBOM/二进制成分分析 2024–2026 专门论文（本轮只捞到 SCA 评测与库识别旧基线，缺口明显）。

## USENIX Sec 25 / NDSS 25 / CCS 25 补扫（2026-10-04）

全量抓取 usenixsecurity25 technical-sessions（455 题）与 ndss2025 accepted-papers（211 题），取二进制/反编译相关新增；CCS 25 反编译两篇（Walking The Last Mile、Disa）并入本域。条目 P6-057–P6-063 共 7 条，link 为官方 presentation/PDF 页。上节「补扫清单」中 USENIX/NDSS 全量扫描缺口就此闭合。

- `P6-057` Tady: A Neural Disassembler without Structural Constraint Violations（USENIX Sec 2025） — P1（OA-USENIX）
- `P6-058` TRex: Practical Type Reconstruction for Binary Code（USENIX Sec 2025） — P1（OA-USENIX）
- `P6-059` REVDECODE: Enhancing Binary Function Matching with Context-Aware Graph Representations and Relevance Decoding（USENIX Sec 2025） — P2（OA-USENIX）
- `P6-060` BLens: Contrastive Captioning of Binary Functions using Ensemble Embedding（USENIX Sec 2025） — P2（OA-USENIX）
- `P6-061` Retrofitting XoM for Stripped Binaries without Embedded Data Relocation（NDSS 2025） — P2（open-access）
- `P6-062` Unleashing the Power of Generative Model in Recovering Variable Names from Stripped Binary（NDSS 2025） — P2（open-access）
- `P6-063` VeriBin: Adaptive Verification of Patches at the Binary Level（NDSS 2025） — P1（open-access）

