# 论文阅读清单 2023–2026（为 ADV 重写工程选型服务）

> 口径声明：本文数字均为**论文口径**（未复算，标注"论文口径"）；检索日期 2026-10-03。
> 收录标准：每篇必须说得出"抄什么机制"，说不出即不收；仅拿到摘要级信息的条目标「待验证」。
> 窗口 2023–2026 为主，个别**窗口外锚点**（机制源头）明确标注。

## P0 清单（最值得先读，各配一句"读了能直接改哪个设计决策"）

| # | 论文 | 读了能直接改哪个设计决策 |
|---|------|--------------------------|
| 1 | SVF 3.0 | 污点引擎是否以"单一值流图 + 可插拔 solver"为核心抽象（而不是每个 checker 自带一套数据流） |
| 2 | IRIS | LLM 层的边界：LLM 只产规格（sources/sinks）与候选，确认权永远在静态引擎 |
| 3 | PrimeVul | 自家扫描器的评测协议：配对评测 + 防数据泄漏，否则门禁数字不可信 |
| 4 | cAST | 代码检索的分块器：按 AST 递归分块替代按行切，直接决定 RRF 检索质量上限 |
| 5 | Agentless | 修复管线形态：固定"分层定位→修复→验证"三阶段流水线，不引入自由 agent 循环 |

---

## 正文条目

### 1. SVF3.0: Static Value-Flow Analysis Framework（ISSTA 2024）— P0
- **一句话结论**：把过程间分析统一到"值流图"一种抽象上，指针分析、内存泄漏检测（Saber）、source/sink 扩展都跑在同一张图上换 solver。
- **可抄机制**：memory-SSA 构建值流图；demand-driven 指针分析；taint 作为图上的 source/sink 扩展（接口形状直接可借鉴）；IFDS/IDE 风格求解器的工程化封装。附带 SVF-NPM：让外部脚本写分析器。
- **对哪个模块**：taint 引擎（Rust 版对应 MIR 层值流图）；all-checker 共用的分析基础设施。
- **范围限制**：语料是 C/C++/LLVM IR，机制语言无关，但 Rust 无 LLVM 裸指针语义，需映射到借用/MIR 概念。
- **链接**：https://arxiv.org/abs/2407.11234 ，代码 https://github.com/svf-tools/SVF

### 2. IRIS: LLM-Assisted Static Analysis for Detecting Security Vulnerabilities（ICLR 2025）— P0
- **一句话结论**：LLM 推断 taint 规格（sources/sinks）+ 静态调用图切片过滤做仓库级检测，优于纯静态与纯 LLM；论文口径 GPT-4 单干误报发现率 84.82%，混合法大幅压回。
- **可抄机制**：LLM 产规格/裁剪候选、静态引擎做最终确认的分工协议；contextual filtering（结合 CVE 上下文裁剪告警）流程；规格缓存与复用。
- **对哪个模块**：LLM 三角验证层；规则/规格自动生成。
- **链接**：https://arxiv.org/abs/2405.17238 ，代码 https://github.com/iris-sast/iris

### 3. Vulnerability Detection with Code Language Models: How Far Are We?（PrimeVul，ICSE 2025）— P0
- **一句话结论**：旧基准被数据泄漏与标签噪声污染：BigVul 上 68.26% F1 的模型在配对任务上掉到 3.09%（论文口径）。
- **可抄机制**：配对评测协议（同一函数漏洞/修复版成对出题）；数据集去重与标签清洗管线；pairwise/strict 指标定义。
- **对哪个模块**：评测基线与门禁——ADV 自家检出率、LLM 过滤器准确率的口径必须照此设计。
- **链接**：https://arxiv.org/abs/2403.18624 ，数据 https://github.com/DLVulDet/PrimeVul

### 4. cAST: Enhancing Code RAG with Structural Chunking via AST（Findings of ACL 2025）— P0
- **一句话结论**：按 AST 结构递归分块比按行/定长分块检索质量更高（论文口径 RepoEval +5.31 点、SWE-bench +2.67 点）。
- **可抄机制**：递归 AST 分块算法（块超 token 上限就沿语法树继续切，天然保留定义边界）；开源参考实现 ASTChunk（Python，可对标重写成 Rust）。
- **对哪个模块**：代码检索（BM25/语义混合 + RRF 的分块前端）。
- **链接**：https://arxiv.org/abs/2506.15655

### 5. Agentless: Demystifying LLM-based Software Engineering Agents（arXiv 2024）— P1
- **一句话结论**：不用 agent 自主循环，固定"分层定位（文件→元素→行）→修复→验证"三阶段流水线，SWE-bench Lite/Verified 上追平重型 agent，成本 <$1/issue（论文口径）。
- **可抄机制**：分层故障定位；候选补丁重排（按测试+重排序模型）；后续工作 OrcaLoca/IssueExec 只换其中一段，证明该流水线是可组合的插槽结构。
- **对哪个模块**：扫描报告→修复建议管线；本地化 APR 的可行性依据（小模型+固定流程比自由 agent 省钱且可控）。
- **链接**：https://arxiv.org/abs/2407.01489

### 6. GPTScan: Detecting Logic Vulnerabilities in Smart Contracts by Combining GPT with Program Analysis（ICSE 2024）— P1
- **一句话结论**：LLM 按 CWE 场景匹配产候选 + 程序分析确认变量依赖/控制流，在 39k 合约、约 300 个真实漏洞上明显优于纯 LLM 与纯规则（论文口径 precision 60%+）。
- **可抄机制**：两段式"CWE 场景描述→LLM 候选→静态确认"管线；确认阶段只查变量依赖关系（便宜、可解释、可复算）；场景模板即"半自动规则生成"。
- **对哪个模块**：逻辑漏洞规则引擎（现有棘轮门禁的 LLM 辅助扩展路线）。
- **链接**：https://arxiv.org/abs/2308.04348

### 7. CodeXEmbed: A Generalist Embedding Model Family for Multilingual and Multi-task Code Retrieval（arXiv 2024，Salesforce）— P1
- **一句话结论**：0.6B–14B 的代码 embedding 家族，400M+ 代码-文本对训练，repo 级检索 SOTA（论文口径），并保持通用文本检索能力。
- **可抄机制**：任务混合训练配方（snippet 函数级 + repo 级 + issue 定位）；repo 级检索评测集构成，可直接当自家检索模块的验收基准。
- **对哪个模块**：语义检索向量层选型（本地部署小尺寸档）与评测集。
- **链接**：https://arxiv.org/abs/2411.12644

### 8. SWE-bench: Can Language Models Resolve Real-World GitHub Issues?（ICLR 2024）— P1
- **一句话结论**：把"issue→补丁"变成机器可验收的基准（2,294 实例 / 12 仓库，论文口径），验收靠 fail-to-pass 测试而非模型自评。
- **可抄机制**：fail-to-pass / pass-to-pass 双测试门设计；从真实修复提交自动构造任务实例的流水线（复用到自家语料建设）。
- **对哪个模块**：APR 验收门；扫描器回归语料的自动构造。
- **链接**：https://arxiv.org/abs/2310.06770

### 9. ARVO: Atlas of Reproducible Vulnerabilities for Open Source Software（USENIX Security 2023；Atlas 版 arXiv 2024）— P1
- **一句话结论**：自动重建"漏洞版/修复版"构建对并验证 PoC 触发，得到上千个可复现 OSS 漏洞（论文口径），已成为下游安全研究的标准基准。
- **可抄机制**：Docker 化构建对 + PoC 自动触发验证的流水线（输入 CVE 修复提交 → 输出可复现环境）。
- **对哪个模块**：扫描器已知漏洞回归语料；本地复现环境模板。
- **链接**：https://arxiv.org/abs/2408.02153

### 10. Understanding and Detecting Real-World Safety Issues in Rust（TSE 2024，system-pclub 系）— P1
- **一句话结论**：对 Rust 生态真实 bug 的实证分类：70 个内存 bug、100 个并发 bug、110 个 API 误用（论文口径），给出根因与检测器缺口清单。
- **可抄机制**：真实 bug 根因分类法——直接决定 lint/门禁规则的选题与优先级排序；配套 artifact 可当测试语料。
- **对哪个模块**：Rust bug 模式规则库（对标现有 bug_scan 规则集做补缺）。
- **待验证**：TSE 正式题名/DOI 待补；artifact 入口 https://github.com/system-pclub

### 11. Mutation Testing in Practice: Insights from Open-Source Repositories（IEEE Software 2024，Sánchez et al.）— P1
- **一句话结论**：开源仓变异测试落地的实证：全量变异成本不可行，增量/降本策略是被实际采用的关键变量。
- **可抄机制**：只对 diff 行变异、每行至多一个存活变异体上报（策略源头是 Google "Practical Mutation Testing at Scale"，ICSE 2021，窗口外锚点 https://research.google/pubs/practical-mutation-testing-at-scale/ 待复核）。
- **对哪个模块**：变异测试模块的成本控制（与 CI 门禁的接法）。
- **链接**：IEEE Software 2024（DOI 待补）

### 12. DiverseVul: A New Vulnerable Source Code Dataset（RAID 2023）— P2
- **一句话结论**：从 CVE 修复提交提取 33 万级函数的数据集（论文口径），规模与多样性优于 BigVul，并实证标签去噪对训练/评测的影响。
- **可抄机制**：CVE 修复提交→函数级语料的提取/去重/去噪流程（与 ARVO 互补：一个函数级、一个可复现构建级）。
- **对哪个模块**：污点规则测试语料；（若做 ML 告警过滤）训练数据。
- **链接**：arXiv 2304.00409（ID 待复核）

### 13. It's Like Flossing Your Teeth: Reproducible Builds for Software Supply Chain Security（IEEE S&P 2023，Fourné/Wermke 等）— P2
- **一句话结论**：开发者访谈研究：可复现构建的价值被认可，但成本与责任分配是落地瓶颈；失败模式集中在环境差异而非只有时间戳。
- **可抄机制**：失败模式清单（时间戳、路径、工具链版本、环境差异）→ 决定自建构建指纹要固定哪些自由度。
- **对哪个模块**：本地确定性构建（cargo 构建指纹/复现验证）。
- **链接**：https://ieeexplore.ieee.org/document/10179320 （arXiv 2308.06850 待复核）

### 14. Practical Mutation Testing at Scale（ICSE 2021，Google）— P2【窗口外锚点】
- **一句话结论**：数千仓库级别的增量变异测试工业实践：只变异变更行、每行至多报一个变异体（论文口径）。
- **可抄机制**：diff 驱动变异调度；与代码评审流集成的上报策略。收录理由：条目 11 的机制源头，且是"增量变异怎么做"最直接的参照。
- **对哪个模块**：变异测试模块。
- **链接**：https://research.google/pubs/practical-mutation-testing-at-scale/（待复核）

### 15. LiteRSan: Lightweight Memory Safety via Rust-specific Sanitizer（arXiv 2025）— P2
- **一句话结论**：面向 Rust 的轻量 sanitizer 路线，用 Rust 特有信息降低插桩开销（仅摘要级信息，待验证）。
- **可抄机制**：Rust 特有插桩点选择（unsafe 边界、FFI 边界）。
- **对哪个模块**：动态验证补充——对扫描器/污点结果做 PoC 级复核。
- **链接**：https://arxiv.org/abs/2509.16389

### 16. 用 RL 降低 Rust 静态内存安全分析误报（arXiv 2026）— P2【待验证】
- **一句话结论**：以人工裁决为奖励信号训练"告警值得报吗"的排序器，压 Rust 静态分析误报（仅搜索摘要级信息，全文未核）。
- **可抄机制**：告警排序器 + 人工反馈奖励回路。
- **对哪个模块**：告警去噪/ranking（与棘轮门禁的告警预算衔接）。
- **链接**：https://arxiv.org/abs/2605.04000 （待验证）

---

## 2025–2026 前沿信号

1. **LLM×静态分析收敛到固定分工**：LLM 产规格/产候选（IRIS、GPTScan，及 2026 年的 FiCoVuL、OpenAnt 一类多级管线），静态引擎做确认——不是"LLM 替代分析器"。**战略含义**：ADV 的 Rust 分析引擎资产不贬值；LLM 层应设计成可插拔的规格生成器 + 告警过滤器，断网时引擎仍可用。
2. **评测口径集体变严**：PrimeVul 配对评测、SWE-bench Verified、AgentLens 揭示 "lucky pass"（测试通过但没修对）——指标虚高正在被系统性拆穿。**战略含义**：ADV 门禁从第一天就要配对评测 + 防泄漏 + 测试驱动验收，数字才有公信力。
3. **"无 agent 的结构化流水线"在成本上赢**：Agentless 及后续（OrcaLoca、IssueExec）证明插槽式固定流水线 > 自由 agent 循环。**战略含义**：本地化部署（有限算力）天然适配该形态；修复管线按三阶段设计，每段可独立替换。
4. **检索侧标准前置**：AST-aware chunking（cAST/ASTChunk）正成为代码 RAG 的标配；embedding 进入 repo 级 + 任务混合时代（CodeXEmbed、SWE-Rank 等 issue 定位方向）。**战略含义**：ADV 的混合检索（BM25+语义+RRF）的分块器值得重写为 AST 递归分块，收益直接可测。
5. **Rust 分析从"内存安全"转向"语义/逻辑层"**：SoK 类工作指出生态工具对已知 bug 类型覆盖不均；RuPTA（指针分析）、RL 去噪等在补精度。**战略含义**：高质量 Rust 漏洞语料仍稀缺——用 ARVO/DiverseVul 式流程自建语料库是护城河，比堆规则更重要。
6. **可复现性从"时间戳问题"升级为"环境级"**：Docker 可复现性研究（2026）、Lila（arXiv 2601.20662，构建复现性监测）。**战略含义**：ADV 的构建指纹/快照要锁环境与工具链，不只锁源码哈希。

## 未收录说明（诚实边界）
- **secrets 检测**：2023–2026 未找到机制清晰、非营销口径的代表作（GitGuardian 年报属营销材料）；现有启发式+熵方法继续用，文献缺口如实在此标注。
- **模糊测试**：窗口内未找到直接回答"成本-收益"的代表作（OSS-Fuzz 实证研究为 ICSE 2022，窗口外）；该方向以变异测试条目 11/14 覆盖工程价值。
- **RustPanda**（传闻 ISSTA 2024 的 Rust 漏洞检测论文）：多轮检索未能核实存在性，按纪律不收。
- 条目 10/11/12/13/14/15/16 的链接或题名存在待复核项，已在各条目内标注「待验证/待复核」。
