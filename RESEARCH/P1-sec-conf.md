# P1 安全四大会议论文扫视（2023–2026）

> 域：S&P / CCS / USENIX Security / NDSS 中可落地的检测/分析机制（工程导向）。扫视日 2026-10-03。
> 方法：官网录取/程序页抓取（USENIX technical-sessions、NDSS accepted-papers、S&P accepted-papers）+ 关键词初筛 + 精选；
> 结论默认 **sweep 级**（标题+已知背景，未逐篇读全文）；P0 与奖项标注经检索核对。收录 **129** 篇（P0 8 / P1 29 / P2 86 / watch 6）。

初筛命中（关键词过滤后的标题数，2026-10-03 抓取）：USENIX Sec 23/24/25/26 = 92/82/115/87；NDSS 23/24/25/26 = 14/32/56/89；S&P 24/25/26 = 53/51/74。CCS 2023–2025 官网录取页 404/JS 渲染未抓到（见文末缺口）。

链接政策：USENIX/NDSS 为单篇官方页；S&P 无单篇页，链到录取总页（2025/26 带 #collapse 锚）。「待验证」= 链接/条目信息未二次核实。

## P0 清单（≤8，读了能直接改哪个设计决策）

- **TRust: A Compilation Framework for In-process Isolation to Protect Safe Rust against Untrusted Code**（USENIX Security 2023）— 类型级隔离检查器 + 不可信代码访问守卫；据此评 ADV 插件沙箱"进程内 Rust 隔离 vs 进程外 OS 隔离"取舍。 [链接](https://www.usenix.org/conference/usenixsecurity23/presentation/bang)
- **Ahoy SAILR! There is No Need to DREAM of C: A Compiler-Aware Structuring Algorithm for Binary Decompilation**（USENIX Security 2024）— 以编译器结构化模式反推 goto 消除顺序；验证口径（可读性+语义等价）。 [链接](https://www.usenix.org/conference/usenixsecurity24/presentation/basque)
- **MalGuard: Towards Real-Time, Accurate, and Actionable Detection of Malicious Packages in PyPI Ecosystem**（USENIX Security 2025）— 静态特征族 + 轻量模型的实时管线 + 人读解释输出。 [链接](https://www.usenix.org/conference/usenixsecurity25/presentation/gao-xingan)
- **ZIPPER: Static Taint Analysis for PHP Applications with Precision and Efficiency**（USENIX Security 2025）— 上下文敏感摘要层 + 循环/别名按需精化层的两级引擎；验收=告警率/耗时双指标。 [链接](https://www.usenix.org/conference/usenixsecurity25/presentation/wang-xinyi)
- **Bond: Constraint-Directed Fuzzing for Automated Validation of Taint Analysis Results in Linux-based IoT Firmware**（USENIX Security 2026）— 告警条件→约束求解→DGF 目标的自动闭环；接进告警出口。 [链接](https://www.usenix.org/conference/usenixsecurity26/presentation/peng-jiaqian)
- **From Large to Mammoth: A Comparative Evaluation of Large Language Models in Vulnerability Detection**（NDSS 2025）— 统一评测协议 + 上下文供给消融；据此定 ADV 的 LLM 检测默认"辅助+复核"与评测口径（防泄漏/分 CWE/报成本）。 [链接](https://www.ndss-symposium.org/ndss-paper/from-large-to-mammoth-a-comparative-evaluation-of-large-language-models-in-vulnerability-detection)
- **SoK: Prudent Evaluation Practices for Fuzzing**（IEEE S&P 2024）— 评测实践清单（基线/重复次数/统计检验/脚本留档）→ ADV 一切对比报告的验收口径。 [链接](https://sp2024.ieee-security.org/accepted-papers.html)
- **Hey, Your Secrets Leaked! Detecting and Characterizing Secret Leakage in the Wild**（IEEE S&P 2025）— 泄露模式分类学 → 检测规则分级 + 撤销处置指引（adv-secrets 规则库组织方式）。 [链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-169)


## USENIX Security 2023

- **[P0]** **TRust: A Compilation Framework for In-process Isolation to Protect Safe Rust against Untrusted Code** — 编译期类型驱动检查+运行时防护可让安全 Rust 与不可信代码进程内共存。 可抄机制：类型级隔离检查器 + 不可信代码访问守卫；据此评 ADV 插件沙箱"进程内 Rust 隔离 vs 进程外 OS 隔离"取舍。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/bang)
- **[P1]** **ARGUS: A Framework for Staged Static Taint Analysis of GitHub Workflows and Actions** — 把 GitHub Actions/workflow 当程序做分阶段流敏感污点，可规模化查出注入类配置漏洞。 可抄机制：YAML→图模型 + 域内 source/sink 定义 + 两级筛选降成本。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/muralee)
- [P2] **BoKASAN: Binary-only Kernel Address Sanitizer for Effective Kernel Fuzzing** — 无源码内核也能上 KASAN：影子内存重映射+二进制插桩。 可抄机制：二进制级 shadow memory 重定向方案。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/cho)
- [P2] **DAFL: Directed Grey-box Fuzzing guided by Data Dependency** — 从"沉默崩溃"反推数据依赖做导向 fuzzing，可复活不可达崩溃。 可抄机制：崩溃点→source 的数据流切片作为导向目标。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/kim-tae-eun)
- **[P1]** **FISHFUZZ: Catch Deeper Bugs by Throwing Larger Nets** — OS 级泛化去重代替精确去重并把能量偏向深路径，能抓更多深层 bug。 可抄机制：崩溃聚类泛化 + 按簇能量分配策略。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/zheng)
- [P2] **FirmSolo: Enabling dynamic analysis of binary Linux-based IoT kernel modules** — 二进制内核模块可重绑定进可编译内核做动态分析。 可抄机制：符号重绑定 + 模块粒度 rehost。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/angelakopoulos)
- [P2] **MTSan: A Feasible and Practical Memory Sanitizer for Fuzzing COTS Binaries** — COTS 二进制可做实用内存消毒（压缩影子+两遍检查）。 可抄机制：1/8 影子编码与双检机制。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/chen-xingman)
- [P2] **Pushed by Accident: A Mixed-Methods Study on Strategies of Handling Secret Information in Source Code Repositories** — secret 入库多源于流程/工具缺口而非无知，防护要贴工作流。 可抄机制：行为学结论 → 告警文案与拦截点设计。 模块：secrets 检测（adv-secrets）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/krause)
- [P2] **Remote Code Execution from SSTI in the Sandbox: Automatically Detecting and Exploiting Template Escape Bugs** — 模板引擎沙箱逃逸可自动检测并生成 PoC。 可抄机制：沙箱语法树 vs 原生语法树 diff 驱动 payload 合成。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/zhao-yudi)
- [P2] **SandDriller: A Fully-Automated Approach for Testing Language-Based JavaScript Sandboxes** — 语言语义差分 oracle 可自动测 JS 沙箱逃逸。 可抄机制：同语义双实现差分 + 变异调度。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/alhamdan)
- **[P1]** **Trojan Source: Invisible Vulnerabilities** — Bidi 重排/同形字符可在源码里隐藏与视觉顺序不符的逻辑并骗过人审。 可抄机制：Bidi+confusable 检测规则族（近乎零成本，直接进规则库）。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/boucher)
- [P2] **UVSCAN: Detecting Third-Party Component Usage Violations in IoT Firmware** — 固件里第三方组件的"使用违规"可静态检出。 可抄机制：组件指纹匹配 + 使用点策略差分。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/zhao-binbin)
- **[P1]** **V1SCAN: Discovering 1-day Vulnerabilities in Reused C/C++ Open-source Software Components** — 无版本信息时，"漏洞函数分类器+组件内代码匹配"能命中复用组件里的 1-day。 可抄机制：函数级漏洞/补丁特征码索引 + 复用代码对齐。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/woo)
- [P2] **VulChecker: Graph-based Vulnerability Localization in Source Code** — 补丁派生程序切片图 + 边特征 GNN 能定位漏洞语句。 可抄机制：PMG 图特征 + 边缘加权训练。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/mirsky)
- [P2] **WHIP: Improving Static Vulnerability Detection in Web Application by Forcing tools to Collaborate** — 让多个弱 SAST 工具互验能显著压误报。 可抄机制：跨工具告警对齐 + 自动复核链。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity23/presentation/al-kassar)

## USENIX Security 2024

- **[P0]** **Ahoy SAILR! There is No Need to DREAM of C: A Compiler-Aware Structuring Algorithm for Binary Decompilation**（论文（开源实现）） — 编译器感知的结构恢复在可读性上显著优于既有算法，把反编译质量拉向源码级。 可抄机制：以编译器结构化模式反推 goto 消除顺序；验证口径（可读性+语义等价）。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/basque)
- **[P1]** **A Taxonomy of C Decompiler Fidelity Issues** — 反编译失真可系统分类（类型/控制流/数据等），决定"何时能信反编译"。 可抄机制：失真分类学 → 归一化与置信度标注清单。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/dramko)
- [P2] **Data Coverage for Guided Fuzzing** — 数据流覆盖并入反馈可补齐纯代码覆盖的盲区。 可抄机制：数据流覆盖度量定义与调度融合。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/wang-mingzhe)
- [P2] **Endokernel: A Thread Safe Monitor for Lightweight Subprocess Isolation** — 线程安全的进程内监控器可实现轻量隔离。 可抄机制：并发安全 shadow-stack 监控架构。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/yang-fangfei)
- **[P1]** **FIRE: Combining Multi-Stage Filtering with Taint Analysis for Scalable Recurring Vulnerability Detection** — "多级廉价过滤器+污点精筛"能把复现型漏洞检测推到全仓库规模。 可抄机制：函数级粗筛 → 精污点验证的漏斗式管线。 模块：污点分析（adv-taint）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/feng-siyue)
- [P2] **Fuzzing BusyBox: Leveraging LLM and Crash Reuse for Embedded Bug Unearthing** — LLM 生成 harness/输入 + 崩溃复用能低成本测嵌入式。 可抄机制：LLM harness 合成 + 历史崩溃迁移。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/asmita)
- [watch] **HYPERPILL: Fuzzing for Hypervisor-bugs by Leveraging the Hardware Virtualization Interface** — 以硬件虚拟化接口为 oracle 可系统 fuzz hypervisor。 可抄机制：KVM 接口语义差分 + 输入合成。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/bulekov)
- **[P1]** **Large Language Models for Code Analysis: Do LLMs Really Do Their Job?** — LLM 在二进制/IR 等低层任务上不可靠、高层摘要上可用，任务边界清晰。 可抄机制：五任务能力画像 → "LLM 直答 vs 必须配工具"的路由表。 模块：评测/方法论（reference）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/fang)
- **[P1]** **Operation Mango: Scalable Discovery of Taint-Style Vulnerabilities in Binary Firmware Services** — 二进制固件服务的污点式漏洞可规模化自动挖掘（产出数百 CVE）。 可抄机制：跨服务调用图重建 + 攻击面聚焦的污点/符号分析。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/gibbs)
- [P2] **SymBisect: Accurate Bisection for Fuzzer-Exposed Vulnerabilities** — 符号执行可把 fuzzer 崩溃精确二分到引入点。 可抄机制：路径约束差分二分。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/zhang-zheng)
- [P2] **SymFit: Making the Common (Concrete) Case Fast for Binary-Code Concolic Execution** — 二进制 concolic 给"常见具体路径"开快车道可提速数倍。 可抄机制：具体执行优先 + 按需符号化。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/qi)
- **[P1]** **Uncovering the Limits of Machine Learning for Automatic Vulnerability Detection** — ML 漏洞检测的上限被标签噪声与近重复泄漏压制，评测必须去泄漏。 可抄机制：数据去重/防泄漏评测协议 + 成本收益核算口径。 模块：评测/方法论（reference）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/risse)
- [P2] **iHunter: Hunting Privacy Violations at Scale in the Software Supply Chain on iOS** — 包粒度污点摘要让供应链级隐私违规可规模化。 可抄机制：包级污点摘要 + 跨包组合分析。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/liu-dexin)

## USENIX Security 2025

- **[P0]** **MalGuard: Towards Real-Time, Accurate, and Actionable Detection of Malicious Packages in PyPI Ecosystem** — PyPI 恶意包可在安装路径上实时检测且告警低误报、可解释、可行动。 可抄机制：静态特征族 + 轻量模型的实时管线 + 人读解释输出。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/gao-xingan)
- **[P0]** **ZIPPER: Static Taint Analysis for PHP Applications with Precision and Efficiency** — PHP 静态污点同时做到精度与效率的工程解：按需流敏感+函数摘要。 可抄机制：上下文敏感摘要层 + 循环/别名按需精化层的两级引擎；验收=告警率/耗时双指标。 模块：污点分析（adv-taint）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/wang-xinyi)
- **[P1]** **Configuration-Sensitive Linux Kernel Fuzzing (CSGO)** — KConfig 配置敏感的内核 fuzzing 显著提升暴露面覆盖（USENIX Sec 25 Best Paper，单篇链接待验证）。 可抄机制：构建配置纳入 fuzz 状态空间 + 配置感知覆盖映射。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity25)
- **[P1]** **DISPATCH: Unraveling Security Patches from Entangled Code Changes** — 安全补丁可从纠缠 commit 中自动剥离，喂 N-day 检测。 可抄机制：LLM+结构特征拆分 patch → 特征入库。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/sun-shiyu)
- **[P1]** **Exploring and Exploiting the Resource Isolation Attack Surface of WebAssembly Containers** — WASM 容器的资源隔离面存在系统性可利用缺口。 可抄机制：资源隔离威胁清单 → 插件沙箱选型判据。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/yu-zhaofeng)
- [P2] **From Alarms to Real Bugs: Multi-target Multi-step Directed Greybox Fuzzing for Static Analysis Result Verification** — 静态告警可用多目标多步 DGF 自动验证成真 bug。 可抄机制：告警→可 fuzz 目标转换 + 多告警合并调度。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/bao-andrew)
- [P2] **From Constraints to Cracks: Constraint Semantic Inconsistencies as Vulnerability Beacons for Embedded Systems** — 约束语义不一致是嵌入式漏洞信标。 可抄机制：约束对齐 diff 检测。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/zhao)
- **[P1]** **LLMxCPG: Context-Aware Vulnerability Detection Through Code Property Graph-Guided Large Language Models** — 把 CPG 事实注入上下文能显著提升 LLM 漏洞检测。 可抄机制：CPG 查询结果→结构化 prompt 上下文的接口设计。 模块：污点分析（adv-taint）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/lekssays)
- [P2] **Make Agent Defeat Agent: Automatic Detection of Taint-Style Vulnerabilities in LLM-based Agents** — LLM agent 的污点式漏洞（提示→工具→副作用）可自动检测。 可抄机制：agent 数据流建图 + 污点策略。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/liu-fengyu)
- [P2] **Robust, Efficient, and Widely Available Greybox Fuzzing for COTS Binaries with System Call Pattern Feedback** — syscall 模式反馈让无源码二进制灰盒可行。 可抄机制：syscall 序列泛化反馈。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/xiao-jifan)
- [P2] **SoK: Automated Vulnerability Repair: Methods, Tools, and Assessments** — AVR 方法/工具/评测全景与坑位。 可抄机制：评测口径清单。 模块：评测/方法论（reference）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/hu-yiwei)
- [P2] **The Doom of Device Drivers: Your Android Device (Most Likely) has N-Day Kernel Vulnerabilities** — Android N-day 内核漏洞存量巨大且补丁滞后。 可抄机制：存量测量口径（adv-sca 输入）。 模块：评测/方法论（reference）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/maar-doom)
- [P2] **Waltzz: WebAssembly Runtime Fuzzing with Stack-Invariant Transformation** — 栈不变量变换能提升 WASM runtime fuzzing 覆盖。 可抄机制：语义保持变换扩大输入空间。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/zhang-lingming)
- **[P1]** **We Have a Package for You! A Comprehensive Analysis of Package Hallucinations by Code Generating LLMs** — 代码 LLM 幻觉包名比例高且已被抢注利用（slopwapping）。 可抄机制：幻觉包名清单 + 注册监测喂给 SCA 黑名单。 模块：SCA/供应链（adv-sca）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/spracklen)
- [P2] **XSSky: Detecting XSS Vulnerabilities through Local Path-Persistent Fuzzing** — 本地路径持久化 fuzzing 可挖 XSS。 可抄机制：本地持久路径反馈机制。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/shi-youkun)

## USENIX Security 2026

- **[P0]** **Bond: Constraint-Directed Fuzzing for Automated Validation of Taint Analysis Results in Linux-based IoT Firmware** — 把污点告警的路径条件转成可解约束、再约束导向 fuzzing，可自动验证污点告警真伪。 可抄机制：告警条件→约束求解→DGF 目标的自动闭环；接进告警出口。 模块：污点分析（adv-taint）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/peng-jiaqian)
- **[P1]** **Breaking the Lethal Trifecta: Secure Agentic Computing via Causal Isolation Architecture** — agent 的"私有数据+不可信内容+对外通信"三要素需因果隔离架构来拆。 可抄机制：能力边界的数据流隔离设计（MCP/agent 工具面）。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/tsao)
- [P2] **Bulbasaur: Branch-Guided Online Mutator Generation for Greybox Fuzzing** — 分支引导在线生成 mutator 优于固定变异集。 可抄机制：分支反馈→mutator 合成。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/wang-yiyi)
- [P2] **CombiSan: Unifying Software Sanitizers for Comprehensive Fuzzing** — 统一调度多消毒器比单消毒器暴露面更全。 可抄机制：消毒器组合与调度策略。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/marini)
- [P2] **Exposing Resource-Exhaustion DoS Vulnerabilities with Leak-Oriented Minimum Path Covers** — 泄漏导向最小路径覆盖能定位资源耗尽 DoS。 可抄机制：路径覆盖上的泄漏组合分析。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/zhan)
- [P2] **Firmenstein: Scaling Dynamic Analysis for Linux-Based Firmware Services via API-Centric Intervention Code Synthesis** — API 中心干预代码合成可替代全量 rehost 规模化固件动态分析。 可抄机制：API 干预合成方案。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/wang-yanzhong)
- [P2] **From Texts to Rules: Generating Sigma Rules with Large Language Models from Cyber Threat Reports** — LLM 可从威胁报告生成经验证的检测规则。 可抄机制：报告→规则草稿→验证回路。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/cai)
- [watch] **GoodVibe: Security-by-Vibe for LLM-Based Code Generation** — LLM 生成代码的安全性可由生成期模式约束保障。 可抄机制：生成期安全模式注入与校验。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/thang)
- [P2] **JScamd: An Automated Static Taint Analysis Framework for Detecting Cryptographic API Misuses in JavaScript** — JS 加密 API 误用可静态污点检测。 可抄机制：加密 API 语义规则 + 污点传播。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/jia-shijie)
- **[P1]** **Patch-Guided Vulnerability Detection: Extracting Java API Security Rules via Attack-Defense Cross-Analysis** — 攻防补丁交叉分析可半自动产出 Java API 安全规则。 可抄机制：补丁对→规则模板→检测器的规则产线。 模块：规则/检测器（adv-rules）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/chen-bofei)
- [P2] **Retrofit: Continual Learning with Controlled Forgetting for Binary Security Detection and Analysis** — 带"控制遗忘"的持续学习缓解二进制检测模型漂移。 可抄机制：样本回放+遗忘约束。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/he-yiling)
- [P2] **Towards Generality: Task-Adaptive Binary Analysis via Semantic Retrieval and Verifiable Reasoning** — 语义检索+可验证推理让单一模型适配多二进制分析任务。 可抄机制：检索增强 + 自验证推理链。 模块：二进制分析（adv-bin）。[链接](https://www.usenix.org/conference/usenixsecurity26/presentation/liu-yuzhe)

## NDSS 2023

- [watch] **Assessing the Impact of Interface Vulnerabilities in Compartmentalized Software** — 隔离子系统的接口漏洞影响可量化度量。 可抄机制：接口攻击面度量框架。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.ndss-symposium.org/ndss-paper/assessing-the-impact-of-interface-vulnerabilities-in-compartmentalized-software)
- [P2] **BinaryInferno: A Semantic-Driven Approach to Field Inference for Binary Message Formats** — 语义特征打分可从二进制消息格式推断字段布局。 可抄机制：40+ 语义特征评分 + 约束组合。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/binaryinferno-a-semantic-driven-approach-to-field-inference-for-binary-message-formats)
- [P2] **BlockScope: Detecting and Investigating Propagated Vulnerabilities in Forked Blockchain Projects** — fork 链上传播的漏洞可跨仓库函数级追踪（机制通用于任意 fork 生态）。 可抄机制：跨仓库函数相似度对齐 + 漏洞传播图。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/blockscope-detecting-and-investigating-propagated-vulnerabilities-in-forked-blockchain-projects)
- [P2] **DARWIN: Survival of the Fittest Fuzzing Mutators** — 多臂赌博机在线选 mutator 组合优于静态集合。 可抄机制：UCB 调度 mutator。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/darwin-survival-of-the-fittest-fuzzing-mutators)
- [P2] **FUZZILLI: Fuzzing for JavaScript JIT Compiler Vulnerabilities**（论文（开源实现）） — JS 引擎 IR 级 fuzzing（成熟开源）持续产出高价值 bug。 可抄机制：自定义 IR + 值规约 + 覆盖引导。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/fuzzilli-fuzzing-for-javascript-jit-compiler-vulnerabilities)
- [P2] **No Grammar, No Problem: Towards Fuzzing the Linux Kernel without System-Call Descriptions** — 无 syscall 描述也能做内核 fuzzing。 可抄机制：在线推断 syscall 语法。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/no-grammar-no-problem-towards-fuzzing-the-linux-kernel-without-system-call-descriptions)
- [P2] **SynthDB: Synthesizing Database via Program Analysis for Security Testing of Web Applications** — 程序分析合成数据库状态可解 Web 测试的 DB 依赖。 可抄机制：从代码推 schema/数据约束再合成。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/synthdb-synthesizing-database-via-program-analysis-for-security-testing-of-web-applications)
- [P2] **VulHawk: Cross-architecture Vulnerability Detection with Entropy-based Binary Code Search** — 熵引导跨架构二进制搜索能查复用组件 1-day。 可抄机制：多特征嵌入 + 熵筛选。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/vulhawk-cross-architecture-vulnerability-detection-with-entropy-based-binary-code-search)

## NDSS 2024

- [P2] **DeGPT: Optimizing Decompiler Output with LLM** — LLM 重构反编译输出可读性显著提升且保语义。 可抄机制：参照增强 + 微调抑制幻觉 + 语义校验。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/degpt-optimizing-decompiler-output-with-llm)
- [P2] **DeepGo: Predictive Directed Greybox Fuzzing** — 可达性预测模型可剪枝导向 fuzzing 搜索。 可抄机制：路径可达性预测器预筛。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/deepgo-predictive-directed-greybox-fuzzing)
- [watch] **EnclaveFuzz: Finding Vulnerabilities in SGX Applications** — enclave 应用可用双语义 oracle fuzz。 可抄机制：enclave 边界差分 oracle。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.ndss-symposium.org/ndss-paper/enclavefuzz-finding-vulnerabilities-in-sgx-applications)
- [P2] **Facilitating Non-Intrusive In-Vivo Firmware Testing with Stateless Instrumentation** — 无状态插桩可在真实设备上做在体固件测试。 可抄机制：状态无关插桩点 + 快照机制。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/facilitating-non-intrusive-in-vivo-firmware-testing-with-stateless-instrumentation)
- [P2] **Faster and Better: Detecting Vulnerabilities in Linux-based IoT Firmware with Optimized Reaching Definition Analysis** — 优化到达定值分析可显著加速固件漏洞检测。 可抄机制：RDA 工程化优化（摘要/稀疏化）。 模块：污点分析（adv-taint）。[链接](https://www.ndss-symposium.org/ndss-paper/faster-and-better-detecting-vulnerabilities-in-linux-based-iot-firmware-with-optimized-reaching-definition-analysis)
- [P2] **File Hijacking Vulnerability: The Elephant in the Room** — 文件劫持（DLL planting 类）漏洞面可系统化枚举。 可抄机制：路径解析缺陷分类学 → 规则族。 模块：规则/检测器（adv-rules）。[链接](https://www.ndss-symposium.org/ndss-paper/file-hijacking-vulnerability-the-elephant-in-the-room)
- [P2] **MOCK: Optimizing Kernel Fuzzing Mutation with Context-aware Dependency** — 上下文依赖感知的 mutation 提升内核 fuzz 效率。 可抄机制：syscall 依赖图引导变异。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/mock-optimizing-kernel-fuzzing-mutation-with-context-aware-dependency)
- [P2] **ShapFuzz: Efficient Fuzzing via Shapley-Guided Byte Selection** — Shapley 值选字节可提效变异。 可抄机制：字节贡献度归因调度。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/shapfuzz-efficient-fuzzing-via-shapley-guided-byte-selection)

## NDSS 2025

- **[P0]** **From Large to Mammoth: A Comparative Evaluation of Large Language Models in Vulnerability Detection** — 多 LLM 大规模对比：漏洞检测能力受任务形式化与上下文供给主导，模型规模非决定项。 可抄机制：统一评测协议 + 上下文供给消融；据此定 ADV 的 LLM 检测默认"辅助+复核"与评测口径（防泄漏/分 CWE/报成本）。 模块：评测/方法论（reference）。[链接](https://www.ndss-symposium.org/ndss-paper/from-large-to-mammoth-a-comparative-evaluation-of-large-language-models-in-vulnerability-detection)
- [P2] **A Comprehensive Memory Safety Analysis of Bootloaders** — bootloader 内存安全可系统分析（多数项目带历史漏洞）。 可抄机制：跨架构 bootloader 分析管线。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/a-comprehensive-memory-safety-analysis-of-bootloaders)
- [P2] **Beyond Classification: Inferring Function Names in Stripped Binaries via Domain Adapted LLMs** — 领域适配 LLM 可恢复剥离二进制的函数名。 可抄机制：预训练域适配 + 类型提示。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/beyond-classification-inferring-function-names-in-stripped-binaries-via-domain-adapted-llms)
- [P2] **BinEnhance: An Enhancement Framework Based on External Environment Semantics for Binary Code Search** — 外部环境语义（库/OS）可增强二进制代码搜索。 可抄机制：环境语义嵌入融合。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/binenhance-an-enhancement-framework-based-on-external-environment-semantics-for-binary-code-search)
- [P2] **DUMPLING: Fine-grained Differential JavaScript Engine Fuzzing** — 细粒度差分（含语义 oracle）提升 JS 引擎 fuzz 产出。 可抄机制：多 oracle 差分管线。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/dumpling-fine-grained-differential-javascript-engine-fuzzing)
- **[P1]** **Enhancing Security in Third-Party Library Reuse - Comprehensive Detection of 1-day Vulnerability through Code Patch Analysis** — 补丁 diff 派生的代码模式可在复用代码中检 1-day。 可抄机制：补丁→脆弱模式匹配 + 可达性确认。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/enhancing-security-in-third-party-library-reuse-comprehensive-detection-of-1-day-vulnerability-through-code-patch-analysis)
- **[P1]** **Generating API Parameter Security Rules with LLM for API Misuse Detection** — LLM 生成 API 参数安全规则 + 自动验证可产出可用规则库。 可抄机制：生成→沙箱验证→入库回路（规则产线范式）。 模块：规则/检测器（adv-rules）。[链接](https://www.ndss-symposium.org/ndss-paper/generating-api-parameter-security-rules-with-llm-for-api-misuse-detection)
- [watch] **IsolateGPT: An Execution Isolation Architecture for LLM-Based Agentic Systems** — LLM agent 的执行隔离（会话/能力域）架构可行。 可抄机制：隔离域 + 信息流控制。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.ndss-symposium.org/ndss-paper/isolategpt-an-execution-isolation-architecture-for-llm-based-agentic-systems)
- [P2] **NodeMedic-FINE: Automatic Detection and Exploit Synthesis for Node.js Vulnerabilities** — Node.js 漏洞可自动检测并合成利用（告警可验证）。 可抄机制：污点 + 利用合成管线。 模块：fuzzing/测试复放（test-replay）。[链接](https://www.ndss-symposium.org/ndss-paper/nodemedic-fine-automatic-detection-and-exploit-synthesis-for-node-js-vulnerabilities)
- [P2] **QMSan: Efficiently Detecting Uninitialized Memory Errors During Fuzzing** — 1/4 影子可在 fuzzing 中高效检未初始化内存。 可抄机制：压缩影子 + 快速路径。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/qmsan-efficiently-detecting-uninitialized-memory-errors-during-fuzzing)
- [P2] **Statically Discover Cross-Entry Use-After-Free Vulnerabilities in the Linux Kernel** — 内核跨入口 UAF 可静态状态机化检出。 可抄机制：入口间状态交互图分析。 模块：污点分析（adv-taint）。[链接](https://www.ndss-symposium.org/ndss-paper/statically-discover-cross-entry-use-after-free-vulnerabilities-in-the-linux-kernel)
- [P2] **The Midas Touch: Triggering the Capability of LLMs for RM-API Misuse Detection** — RM-API 误用检测可用提示策略激发 LLM。 可抄机制：资源管理 API 规则提示模板。 模块：规则/检测器（adv-rules）。[链接](https://www.ndss-symposium.org/ndss-paper/the-midas-touch-triggering-the-capability-of-llms-for-rm-api-misuse-detection)
- [P2] **The Skeleton Keys: A Large Scale Analysis of Credential Leakage in Mini-apps** — 小程序生态凭据泄露可大规模测量并归类。 可抄机制：凭据模式 + 泄露路径分类。 模块：secrets 检测（adv-secrets）。[链接](https://www.ndss-symposium.org/ndss-paper/the-skeleton-keys-a-large-scale-analysis-of-credential-leakage-in-mini-apps)

## NDSS 2026

- [P2] **A Deep Dive into Function Inlining and its Security Implications for ML-based Binary Analysis** — 内联使 ML 二进制分析特征漂移，需内联感知策略。 可抄机制：内联检测 + 特征归并。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/a-deep-dive-into-function-inlining-and-its-security-implications-for-ml-based-binary-analysis)
- [P2] **Accurate Identification of the Vulnerability-Introducing Commit based on Differential Analysis of Patching Patterns** — 补丁模式差分可定位引洞 commit。 可抄机制：修复模式反推引入模式。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/accurate-identification-of-the-vulnerability-introducing-commit-based-on-differential-analysis-of-patching-patterns)
- [P2] **Anota: Identifying Business Logic Vulnerabilities via Annotation-Based Sanitization** — 注解驱动净化可定位业务逻辑漏洞。 可抄机制：注解推断 + 净化点校验。 模块：规则/检测器（adv-rules）。[链接](https://www.ndss-symposium.org/ndss-paper/anota-identifying-business-logic-vulnerabilities-via-annotation-based-sanitization)
- [P2] **BINALIGNER: Aligning Binary Code for Cross-Compilation Environment Diffing** — 跨编译环境二进制对齐可 diff 构建差异（供应链可复现）。 可抄机制：对齐 + 环境差分归因。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/binaligner-aligning-binary-code-for-cross-compilation-environment-diffing)
- [P2] **Chasing Shadows: Pitfalls in LLM Security Research** — LLM 安全研究常见方法学陷阱（数据泄漏/错误前提）。 可抄机制：陷阱清单 → 评测自查表。 模块：评测/方法论（reference）。[链接](https://www.ndss-symposium.org/ndss-paper/chasing-shadows-pitfalls-in-llm-security-research)
- [P2] **FidelityGPT: Correcting Decompilation Distortions with Retrieval Augmented Generation** — RAG 可修正反编译失真。 可抄机制：失真模式检索 + 修正模板。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/fidelitygpt-correcting-decompilation-distortions-with-retrieval-augmented-generation)
- **[P1]** **FirmCross: Detecting Taint-style Vulnerabilities in Modern C-Lua Hybrid Web Services of Linux-based Firmware** — C-Lua 混合固件服务的跨语言（跨 FFI）污点可检。 可抄机制：跨语言调用图 + 边界污点传播规则。 模块：污点分析（adv-taint）。[链接](https://www.ndss-symposium.org/ndss-paper/firmcross-detecting-taint-style-vulnerabilities-in-modern-c-lua-hybrid-web-services-of-linux-based-firmware)
- **[P1]** **From Noise to Signal: Precisely Identify Affected Packages of Known Vulnerabilities in npm Ecosystem** — npm 已知漏洞→受影响包的精确映射可显著降 SCA 误报。 可抄机制：补丁/代码级归属验证替代版本区间推断。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/from-noise-to-signal-precisely-identify-affected-packages-of-known-vulnerabilities-in-npm-ecosystem)
- **[P1]** **Idioms: A Simple and Effective Framework for Turbo-Charging Local Neural Decompilation with Well-Defined Types** — 类型习语注入可大幅提升本地小模型反编译质量（本地优先友好）。 可抄机制：习语库 + 类型约束解码。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/idioms-a-simple-and-effective-framework-for-turbo-charging-local-neural-decompilation-with-well-defined-types)
- [P2] **Les Dissonances: Cross-Tool Harvesting and Polluting in Pool-of-Tools Empowered LLM Agents** — 工具池 agent 存在跨工具收割/污染攻击。 可抄机制：工具间信息流滥用模型。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.ndss-symposium.org/ndss-paper/les-dissonances-cross-tool-harvesting-and-polluting-in-pool-of-tools-empowered-llm-agents)
- [P2] **Prompt Injection Attack to Tool Selection in LLM Agents** — LLM agent 工具选择可被注入操纵（MCP 工具面威胁模型）。 可抄机制：工具描述投毒面 + 防御清单。 模块：沙箱/隔离（adv-sandbox）。[链接](https://www.ndss-symposium.org/ndss-paper/prompt-injection-attack-to-tool-selection-in-llm-agents)
- [P2] **Through the Authentication Maze: Detecting Authentication Bypass Vulnerabilities in Firmware Binaries** — 固件二进制认证绕过可自动检测。 可抄机制：认证逻辑状态恢复 + 绕过路径搜索。 模块：二进制分析（adv-bin）。[链接](https://www.ndss-symposium.org/ndss-paper/through-the-authentication-maze-detecting-authentication-bypass-vulnerabilities-in-firmware-binaries)
- [P2] **Token Time Bomb: Evaluating JWT Implementations for Vulnerability Discovery** — JWT 实现漏洞（算法混淆/过期等）可系统化测试。 可抄机制：JWT 语义 oracle + 变异。 模块：规则/检测器（adv-rules）。[链接](https://www.ndss-symposium.org/ndss-paper/token-time-bomb-evaluating-jwt-implementations-for-vulnerability-discovery)
- [P2] **Trust Me, I Know This Function: Hijacking LLM Static Analysis using Bias** — 可通过函数名/模式偏见劫持 LLM 静态分析（LLM 特征的对抗面）。 可抄机制：偏见注入实验范式 → LLM 结论不可单信。 模块：评测/方法论（reference）。[链接](https://www.ndss-symposium.org/ndss-paper/trust-me-i-know-this-function-hijacking-llm-static-analysis-using-bias)
- **[P1]** **VulSCA: A Community-Level SCA Approach for Accurate C/C++ Supply Chain Vulnerability Analysis** — 社区级（依赖图+代码证据）SCA 能对无锁文件的 C/C++ 做漏洞归属。 可抄机制：社区共现 + 代码证据双通道。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/vulsca-a-community-level-sca-approach-for-accurate-c-c-supply-chain-vulnerability-analysis)
- [P2] **What Do They Fix? LLM-Aided Categorization of Security Patches for Critical Memory Bugs** — LLM 可把安全补丁归类到内存 bug 类别（喂 N-day 规则库）。 可抄机制：补丁分类管线。 模块：SCA/供应链（adv-sca）。[链接](https://www.ndss-symposium.org/ndss-paper/what-do-they-fix-llm-aided-categorization-of-security-patches-for-critical-memory-bugs)

## IEEE S&P 2024

- **[P0]** **SoK: Prudent Evaluation Practices for Fuzzing** — fuzzing 论文评测普遍存在统计与可复现缺陷，SoK 给出规范清单。 可抄机制：评测实践清单（基线/重复次数/统计检验/脚本留档）→ ADV 一切对比报告的验收口径。 模块：评测/方法论（reference）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- **[P1]** **"False negative - that one is going to kill you." - Understanding Industry Perspectives of Static Analysis based Security Testing** — 工业界更怕 SAST 假阴性；工具协同、上下文供给与告警分级决定采纳。 可抄机制：需求画像 → 默认配置偏召回 + 分级展示策略。 模块：评测/方法论（reference）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- [P2] **"Len or index or count, anything but v1": Predicting Variable Names in Decompilation Output with Transfer Learning** — 迁移学习可预测反编译输出变量名。 可抄机制：decompiler 输出 + 迁移训练。 模块：二进制分析（adv-bin）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- [P2] **APP-Miner: Detecting API Misuses via Automatically Mining API Path Patterns** — API 路径模式挖掘可自动产误用规则。 可抄机制：正确用例→路径模式→规则。 模块：规则/检测器（adv-rules）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- [P2] **AirTaint: Making Dynamic Taint Analysis Faster and Easier** — 动态污点可更快更易用（架构解耦+按需）。 可抄机制：传播策略解耦 + 按需启停。 模块：污点分析（adv-taint）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- [P2] **LLMs Cannot Reliably Identify and Reason About Security Vulnerabilities (Yet?): A Comprehensive Evaluation, Framework, and Benchmarks** — LLM 漏洞推理存在系统性失败模式，需基准化后再上岗。 可抄机制：失败模式分类 + 基准集。 模块：评测/方法论（reference）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)
- [P2] **Where URLs Become Weapons: Automated Discovery of SSRF Vulnerabilities in Web Applications** — SSRF 可自动发现（骨架请求+污点）。 可抄机制：URL 组件污点 + 响应差分。 模块：规则/检测器（adv-rules）。[链接](https://sp2024.ieee-security.org/accepted-papers.html)

## IEEE S&P 2025

- **[P0]** **Hey, Your Secrets Leaked! Detecting and Characterizing Secret Leakage in the Wild** — 线上 secret 泄露的规模、模式与根因可系统刻画，检测需按模式分级。 可抄机制：泄露模式分类学 → 检测规则分级 + 撤销处置指引（adv-secrets 规则库组织方式）。 模块：secrets 检测（adv-secrets）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-169)
- [P2] **Evaluating the Effectiveness of Memory Safety Sanitizers** — 内存消毒器检出力差异大，需按目标选型。 可抄机制：消毒器对比口径。 模块：评测/方法论（reference）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-57)
- [P2] **GoSonar: Detecting Logical Vulnerabilities in Memory Safe Language Using Inductive Constraint Reasoning** — 内存安全语言（Go）的逻辑漏洞可归纳约束推理检出。 可抄机制：不变式归纳 + 违反检测。 模块：规则/检测器（adv-rules）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-22)
- [P2] **MOCGuard: Automatically Detecting Missing-Owner-Check Vulnerabilities in Java Web Applications** — Java Web 缺失属主检查可自动检测。 可抄机制：对象权限图 + 检查点校验。 模块：规则/检测器（adv-rules）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-88)
- **[P1]** **Oxidizer: Toward Concise and High-fidelity Rust Decompilation** — Rust 反编译可做到简洁且高保真（利用 Rust 特有结构）。 可抄机制：Rust 结构感知重建（MIR/LLVM IR 层）。 模块：二进制分析（adv-bin）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-181)
- [P2] **PyLingual: Toward Perfect Decompilation of Evolving High-Level Languages** — Python 字节码可近乎完美反编译（应对版本演进）。 可抄机制：版本感知语法定义 + 神经辅助。 模块：二进制分析（adv-bin）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-11)
- [P2] **RGFuzz: Rule-Guided Fuzzer for WebAssembly Runtimes** — 规范规则引导的 WASM runtime fuzzing 更有效。 可抄机制：规范规则→oracle。 模块：沙箱/隔离（adv-sandbox）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-90)
- **[P1]** **SpecAuditor: Generating Audit Specifications for LLM-Driven Bug Detection** — LLM 驱动的 bug 检测需要先自动生成审计规约。 可抄机制：规约生成 + 对齐校验回路。 模块：规则/检测器（adv-rules）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-212)
- [P2] **The File That Contained the Keys Has Been Removed: An Empirical Analysis of Secret Leaks in Cloud Buckets and Responsible Disclosure Outcomes** — 云存储 secret 泄露的披露后处置普遍缺位。 可抄机制：泄露生命周期测量口径。 模块：secrets 检测（adv-secrets）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-17)
- **[P1]** **The Secrets Must Not Flow: Scaling Security Verification to Large Codebases** — 大代码库的安全验证（污点/信息流）可扩展到生产规模。 可抄机制：摘要/增量/并行的可扩展性工程套路。 模块：污点分析（adv-taint）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-27)
- [P2] **You Can't Judge a Binary by Its Header: Data-Code Separation for Non-Standard ARM Binaries using Pseudo Labels** — 非标准 ARM 二进制的数据/代码分离可用伪标签自训练解决。 可抄机制：伪标签迭代训练。 模块：二进制分析（adv-bin）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-32)
- **[P1]** **deepSURF: Detecting Memory Safety Vulnerabilities in Rust Through Fuzzing LLM-Augmented Harnesses** — LLM 增强 harness 能 fuzz 出 Rust 内存安全漏洞。 可抄机制：LLM 补全 harness + unsafe 焦点策略。 模块：fuzzing/测试复放（test-replay）。[链接](https://sp2025.ieee-security.org/accepted-papers.html#collapse-61)

## IEEE S&P 2026

- **[P1]** **Bridge: High-Order Taint Vulnerabilities Detection in Linux-based IoT Firmware** — 高阶污点（经存储/多函数多跳）在 IoT 固件中普遍且可检。 可抄机制：存储介导的污点传递建模。 模块：污点分析（adv-taint）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-0)
- [P2] **Contextualizing Sink Knowledge for Java Vulnerability Discovery** — sink 知识上下文化可降 Java 漏洞误报。 可抄机制：sink 语义上下文检索。 模块：规则/检测器（adv-rules）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-231)
- [P2] **Cosseter: GitHub Actions Permission Reduction Using Demand-Driven Static Analysis** — 按需静态分析可自动收敛 GitHub Actions 权限。 可抄机制：权限需求分析 → 最小权限重写。 模块：SCA/供应链（adv-sca）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-68)
- **[P1]** **Cottontail: Large Language Model-Driven Concolic Execution for Highly Structured Test Input Generation** — LLM 驱动 concolic 可生成强结构化输入。 可抄机制：LLM 解结构约束 + 具体值回填。 模块：fuzzing/测试复放（test-replay）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-112)
- [P2] **Detecting Privilege Escalation in Polyglot Microservices via Agentic Program Analysis** — 多语言微服务的提权可由 agentic 程序分析检测。 可抄机制：agent 规划 + 工具化分析调用。 模块：污点分析（adv-taint）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-122)
- [P2] **Hidden Secrets in the arXiv: Discovering, Analyzing, and Preventing Unintentional Information Disclosure in Source Files of Scientific Preprints** — 预印本源文件无意泄密普遍（含凭据）。 可抄机制：源文件披露面扫描规则。 模块：secrets 检测（adv-secrets）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-224)
- [P2] **PORTGPT: Towards Automated Backporting Using Large Language Models** — LLM 可自动把补丁回移植到旧分支（N-day 处置）。 可抄机制：跨版本补丁适配 + 验证。 模块：SCA/供应链（adv-sca）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-35)
- [P2] **PUFFERDOS: Efficient and Effective Attack String Generation for Regular Expression Denial of Service Vulnerabilities** — ReDoS 攻击串可自动生成（规则库配套弹药）。 可抄机制：正则 NFA 分析 + 串合成。 模块：规则/检测器（adv-rules）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-172)
- [watch] **Site Isolation is Dead: How Site Isolation is Broken in Agentic Browsers and Extensions** — agentic 浏览器/扩展使站点隔离假设失效。 可抄机制：隔离失效模型。 模块：沙箱/隔离（adv-sandbox）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-249)
- [P2] **The First Large-Scale Systematic Study of Python Class Pollution Vulnerability** — Python 类污染漏洞面首次系统化（新规则族）。 可抄机制：污染路径模型 + 检测规则。 模块：规则/检测器（adv-rules）。[链接](https://sp2026.ieee-security.org/accepted-papers.html#collapse-110)

## 缺口与后续
- **CCS 2023/2024/2025 全缺**：官网录取页 404 或 JS 渲染；补扫入口（真实 TOC 页，待下会话补）：dblp.org/db/conf/ccs/ccs2023.html、ccs2024.html、ccs2025.html。
- **S&P 2023 缺**：sp2023 录取页 404；补扫入口 dblp.org/db/conf/sp/sp2023.html。
- 「Configuration-Sensitive Linux Kernel Fuzzing (CSGO)」为 USENIX Sec 25 Best Paper（检索锚：nsl.cs.waseda.ac.jp 记录），单篇官方页待验证。
- S&P 条目的 #collapse 锚号来自抓取日页面结构，改版后可能漂移（页面本身稳定）。
- 原始抓取证据：`.tmp-p1/papers.tsv`（745 条初筛标题+链接）、抓取脚本 `.tmp-p1/scrape*.py`。


## CCS 2023–2025 补扫（2026-10-04）

方法：Crossref 按 DOI 前缀 3576915/3658644/3719027 拉主会条目（合计 1460 条，按前缀剔除 AsiaCCS 2023–25 混入）+ 关键词初筛 + WebSearch 摘要佐证；机制写不出的一律不收。条目 P1-130–P1-154 共 25 条，link 为 doi.org 官方页，license=ACM。AsiaCCS 副产品清单落盘 workspace tmp_ccs_all.json，未入册。

- `P1-130` SyzDirect: Directed Greybox Fuzzing for Linux Kernel（CCS 2023） — P2（ACM）
- `P1-131` NestFuzz: Enhancing Fuzzing with Comprehensive Understanding of Input Processing Logic（CCS 2023） — P2（ACM）
- `P1-132` DSFuzz: Detecting Deep State Bugs with Dependent State Exploration（CCS 2023） — P2（ACM）
- `P1-133` CryptoBap: A Binary Analysis Platform for Cryptographic Protocols（CCS 2023） — P1（ACM）
- `P1-134` SymGX: Detecting Cross-boundary Pointer Vulnerabilities of SGX Applications via Static Symbolic Execution（CCS 2023） — P2（ACM）
- `P1-135` PyRTFuzz: Detecting Bugs in Python Runtimes via Two-Level Collaborative Fuzzing（CCS 2023） — P2（ACM）
- `P1-136` Greybox Fuzzing of Distributed Systems（CCS 2023） — P2（ACM）
- `P1-137` PackGenome: Automatically Generating Robust YARA Rules for Accurate Malware Packer Detection（CCS 2023） — P1（ACM）
- `P1-138` Large Language Models for Code: Security Hardening and Adversarial Testing（CCS 2023） — P1（ACM）
- `P1-139` Prompt Fuzzing for Fuzz Driver Generation（CCS 2024） — P1（ACM）
- `P1-140` CountDown: Refcount-guided Fuzzing for Exposing Temporal Memory Errors in Linux Kernel（CCS 2024） — P2（ACM）
- `P1-141` LiftFuzz: Validating Binary Lifters through Context-aware Fuzzing with GPT（CCS 2024） — P1（ACM）
- `P1-142` When Compiler Optimizations Meet Symbolic Execution: An Empirical Study（CCS 2024） — P2（ACM）
- `P1-143` PowerPeeler: A Precise and General Dynamic Deobfuscation Method for PowerShell Scripts（CCS 2024） — P2（ACM）
- `P1-144` Demystifying RCE Vulnerabilities in LLM-Integrated Apps（CCS 2024） — P2（ACM）
- `P1-145` SoK: Where to Fuzz? Assessing Target Selection Methods in Directed Fuzzing（CCS 2024） — P2（ACM）
- `P1-146` Walking The Last Mile: Studying Decompiler Output Correction in Practice（CCS 2025） — P1（ACM）
- `P1-147` Disa: Accurate Learning-based Static Disassembly with Attentions（CCS 2025） — P1（ACM）
- `P1-148` SyzSpec: Specification Generation for Linux Kernel Fuzzing via Under-Constrained Symbolic Execution（CCS 2025） — P1（ACM）
- `P1-149` PromeFuzz: A Knowledge-Driven Approach to Fuzzing Harness Generation with Large Language Models（CCS 2025） — P2（ACM）
- `P1-150` DiveFuzz: Enhancing CPU Fuzzing via Diverse Instruction Construction（CCS 2025） — P2（ACM）
- `P1-151` RVISmith: Fuzzing Compilers for RVV Intrinsics（CCS 2025） — P2（ACM）
- `P1-152` What Gets Measured Gets Managed: Mitigating Supply Chain Attacks with a Link Integrity Management System（CCS 2025） — P1（ACM）
- `P1-153` Augmenting Search-based Program Synthesis with Local Inference Rules to Improve Black-box Deobfuscation（CCS 2025） — P1（ACM）
- `P1-154` Protocol-Aware Firmware Rehosting for Effective Fuzzing of Embedded Network Stacks（CCS 2025） — P2（ACM）

