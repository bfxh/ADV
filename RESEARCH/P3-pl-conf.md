# P3 · PL 四大顶会论文扫视（PLDI / POPL / OOPSLA / CAV，2023–2026）

> 扫视日期 2026-10-04。工程导向：只收「能说出可抄机制」的分析机制/程序变换/类型系统/解析/SMT/concolic/Datalog/切片类论文；纯理论证明、纯语义形式化（无工具落地）、量子/硬件类不收。
> 检索口径（锚）：条目元数据（题名/DOI）取自 Crossref REST API 当日拉取，container=Proceedings of the ACM on Programming Languages 的 PLDI/OOPSLA/POPL 各 issue（vol.7–10，对应 2023–2026）；先经关键词过滤（449 条候选）再人工筛选。
> 限制：CAV（Springer LNCS）在 Crossref 的 container-title 变体未可靠命中、DBLP 三镜像当日均被 Anubis 反爬拦截，CAV 仅收 3 条定向核实条目，**CAV 覆盖不视为完备**。奖项/存证信息来自当日 web 检索。
> depth 口径：deep=读过原文/多源确认；abstract=题名+领域知识（未逐节读）；title=仅由题名+领域惯例推断（机制句是推断，采纳前先读原文）。2026 年各卷为初扫，depth 偏 title。
>
> 共 90 条：P0=6，P1=28，P2=51，watch=5。链接以 DOI 为主；CAV 两条用卷目录链接并已在条目内注明。

## P0 清单（读了能直接改哪个设计决策）

1. **Better Together: Unifying Datalog and Equality Saturation**（PLDI 2023）—— egglog：把 Datalog 关系推导与 e-graph 等价饱和统一进一个执行模型，规则既做推导也做改写合并，天然支持等价项折叠与去重。**决策：**决定 adv-taint/adv-rules 的规则内核是否采用 egglog 式「推导+等价合并」统一模型——它直接改变告警折叠、摘要去重的实现方式，而不是外挂一个后处理步骤。链接：<https://doi.org/10.1145/3591239>
2. **Falcon: A Fused Approach to Path-Sensitive Sparse Data Dependence Analysis**（PLDI 2024）—— 把路径敏感性与稀疏数据依赖融合为单遍分析：稀疏执行承载路径条件，避免路径枚举爆炸同时保留路径敏感精度。**决策：**决定 adv-taint 主引擎的第一架构决策：Falcon 式「路径敏感×稀疏融合」还是经典 IFDS/SDA 分层——这决定 IR、摘要与上下文编码的整套设计。链接：<https://doi.org/10.1145/3656400>
3. **Restart and Refine: Scalable IFDS Taint Analysis across Memory Budgets**（PLDI 2026）—— 内存预算内做 IFDS 污点：超限即重启并降精度精化，跨内存预算档位化扩展。**决策：**决定 IFDS 污点在「本地内存受限」运行模型下的档位划分与重启/回退策略——直接对应 ADV 本地优先的资源约束。链接：<https://doi.org/10.1145/3808326>
4. **Flan: An Expressive and Efficient Datalog Compiler for Program Analysis**（POPL 2024）—— 面向程序分析的 Datalog 编译器，支持函数式特性并把高阶结构编译为一阶求值。**决策：**决定规则内核选型：Flan 式带函数的 Datalog 编译 vs Soufflé vs 自研——adv-taint 规则层的地基决策。链接：<https://doi.org/10.1145/3632928>
5. **A Flow-Sensitive Refinement Type System for Verifying eBPF Programs**（OOPSLA 2025）—— 流敏感 refinement 类型验证 eBPF 程序——「检查器=类型系统」的完整工程案例。**决策：**决定 adv-rules 是否引入流敏感 refinement 型规则 DSL，以及检查时机（部署前门禁 vs 运行时）——eBPF 案例给出了完整参照。链接：<https://doi.org/10.1145/3763799>
6. **IncIDFA: An Efficient and Generic Algorithm for Incremental Iterative Dataflow Analysis**（OOPSLA 2025）—— 迭代数据流分析的通用增量算法：变更后只重算受影响部分。**决策：**决定增量分析骨架：编辑保存即重算的通用算法，adv-taint/adv-rules 共用的增量底座。链接：<https://doi.org/10.1145/3720436>

## 分会议条目

格式：**题目**〔优先级｜模块映射〕（depth 标注，仅非 deep 时标注）机制：……。映射已内联于标签，链接在行尾。

### PLDI 2023

- **Better Together: Unifying Datalog and Equality Saturation**〔P0｜adv-taint〕机制：egglog：把 Datalog 关系推导与 e-graph 等价饱和统一进一个执行模型，规则既做推导也做改写合并，天然支持等价项折叠与去重。<https://doi.org/10.1145/3591239>
- **Context Sensitivity without Contexts: A Cut-Shortcut Approach to Fast and Precise Pointer Analysis**〔P1｜adv-taint〕（abstract 级）机制：不显式枚举/编码调用上下文，用图上 cut-shortcut 捷径近似上下文敏感，在 k-CGLS 一类方案的速度-精度曲线上给出新取点。<https://doi.org/10.1145/3591242>
- **Flux: Liquid Types for Rust**〔P1｜adv-rules〕机制：液态 refinement 类型接入 rustc 管线：签名携带 refinement 谓词、模块化检查、谓词交给 SMT；refinement 类型在主流系统语言落地的完整参照。<https://doi.org/10.1145/3591283>
- **flap: A Deterministic Parser with Fused Lexing**〔P2｜adv-parse〕（title 级）机制：词法与确定性解析融合，去掉独立 tokenizer 中间层；adv-parse 单遍归一解析的性能参照。<https://doi.org/10.1145/3591269>
- **Interval Parsing Grammars for File Format Parsing**〔P2｜adv-parse〕（title 级）机制：用区间文法声明式描述二进制文件格式并带容错解析；归一层处理非文本格式的启发。<https://doi.org/10.1145/3591264>
- **Repairing Regular Expressions for Extraction**〔P2｜adv-rules〕（title 级）机制：面向抽提用途自动修复正则表达式；可用于规则库中 regex 类规则的自修复。<https://doi.org/10.1145/3591287>
- **Scallop: A Language for Neurosymbolic Programming**〔P2｜adv-taint〕（abstract 级）机制：带溯源半环与概率语义的 Datalog 方言，规则可附不确定度；告警置信度传播的备选底座。<https://doi.org/10.1145/3591280>

### PLDI 2024

- **Falcon: A Fused Approach to Path-Sensitive Sparse Data Dependence Analysis**〔P0｜adv-taint〕（abstract 级）机制：把路径敏感性与稀疏数据依赖融合为单遍分析：稀疏执行承载路径条件，避免路径枚举爆炸同时保留路径敏感精度。<https://doi.org/10.1145/3656400>
- **Scaling Type-Based Points-to Analysis with Saturation**〔P2｜adv-taint〕（title 级）机制：类型基指针分析用「饱和」扩缩上下文/域，在类型级精度档位上做到可扩展。<https://doi.org/10.1145/3656417>
- **Daedalus: Safer Document Parsing**〔P1｜adv-parse〕（abstract 级）机制：声明式描述文档格式→编译生成解析器，并可对递归长度等性质做验证；adv-parse 声明式格式处理的参照。<https://doi.org/10.1145/3656410>
- **Reducing Static Analysis Unsoundness with Approximate Interpretation**〔P1｜adv-taint〕（title 级）机制：用「近似解释」补静态分析的不健全处，给出音准权衡的工程化路径。<https://doi.org/10.1145/3656424>
- **Equivalence by Canonicalization for Synthesis-Backed Refactoring**〔P1｜adv-rules〕（title 级）机制：重构/修复改写前后的语义等价用规范化(canonicalization)判定——自动修复「不改语义」的安全判据可直接借鉴。<https://doi.org/10.1145/3656453>
- **A Lightweight Polyglot Code Transformation Language**〔P1｜adv-rules〕（title 级）机制：轻量多语言代码变换 DSL；修复规则表达层（规则→改写动作）的选型参考。<https://doi.org/10.1145/3656429>
- **Efficient Static Vulnerability Analysis for JavaScript with Multiversion Dependency Graphs**〔P1｜adv-taint〕（title 级）机制：多版本依赖图把依赖演化并入污点分析，定位漏洞影响的版本范围。<https://doi.org/10.1145/3656394>
- **Static Analysis for Checking the Disambiguation Robustness of Regular Expressions**〔P2｜adv-rules〕（title 级）机制：静态判定 regex 歧义消解的鲁棒性；可做规则库 regex 质量门。<https://doi.org/10.1145/3656461>

### PLDI 2025

- **Taking Out the Toxic Trash: Recovering Precision in Mixed Flow-Sensitive Static Analyses**〔P1｜adv-taint〕（title 级）机制：混合流敏感分析中定位并清除精度污染源（「毒物」），恢复被拖累部分的精度。<https://doi.org/10.1145/3729297>
- **Relaxing Alias Analysis: Exploring the Unexplored Space**〔P2｜adv-taint〕（title 级）机制：系统探索别名分析「松弛」设计空间；别名精度档位选择的参考。<https://doi.org/10.1145/3729254>
- **An Interactive Debugger for Rust Trait Errors**〔P2｜reference〕（title 级）机制：交互式展开 trait 求解过程；规则/类型诊断的解释层 UX 参考。<https://doi.org/10.1145/3729302>
- **Robustifying Debug Information Updates in LLVM via Control-Flow Conformance Analysis**〔P2｜reference〕（title 级）机制：用控制流一致性分析校验调试信息更新——告警位置映射正确性的判据来源。<https://doi.org/10.1145/3729267>
- **Solving Floating-Point Constraints with Continuous Optimization**〔P2｜reference〕（title 级）机制：用连续优化解浮点约束；SMT 后端补强选项。<https://doi.org/10.1145/3729279>
- **Usability Barriers for Liquid Types**〔P2｜adv-rules〕（title 级）机制：液态类型可用性实证——refinement 规则 DSL 设计的采纳性判据。<https://doi.org/10.1145/3729327>

### PLDI 2026

- **Restart and Refine: Scalable IFDS Taint Analysis across Memory Budgets**〔P0｜adv-taint〕（title 级）机制：内存预算内做 IFDS 污点：超限即重启并降精度精化，跨内存预算档位化扩展。<https://doi.org/10.1145/3808326>
- **Bridging Coverage and Confidence: Reliable Static False Alarm Elimination via Input-Agnosticity**〔P2｜adv-rules〕（title 级）机制：证明告警条件与输入无关⇒确定性误报，可自动静默。<https://doi.org/10.1145/3808301>
- **Soteria: Efficient Symbolic Execution as a Functional Library: Perhaps You Should Write Your Own Symbolic Execution Engine!**〔watch｜reference〕（title 级）机制：把符号执行做成可组合函数库；若 ADV 未来内嵌 concolic 的架构参考。<https://doi.org/10.1145/3808306>
- **Typestate via Revocable Capabilities**〔P2｜adv-rules〕（title 级）机制：用可撤销能力编码 typestate；资源状态类规则的类型化表达。<https://doi.org/10.1145/3808323>
- **Hayroll: A Modular Wrapper for Translating C Macros and Conditional Compilation to Rust**〔P2｜reference〕（title 级）机制：C 宏/条件编译→Rust 的模块化翻译壳；预处理层源到源变换参考。<https://doi.org/10.1145/3808276>
- **Cpp2Rust: Automatic Translation of C++ to Safe Rust**〔P2｜reference〕（title 级）机制：C++→安全 Rust 自动翻译；大规模源到源变换的工程参考。<https://doi.org/10.1145/3808266>
- **Abstract Interpretation with Confidence: Quantifying the Precision of Dataflow Analysis with Probabilities**〔P2｜adv-taint〕（title 级）机制：给数据流分析精度做概率量化——分析质量指标设计的参考。<https://doi.org/10.1145/3808351>
- **Evolving Abstract Transformers for Gradient-Guided, Adaptable Abstract Interpretation**〔watch｜adv-taint〕（title 级）机制：梯度引导演化抽象转移函数，抽象域随目标自适应。<https://doi.org/10.1145/3808346>

### POPL 2023

- **From SMT to ASP: Solver-Based Approaches to Solving Datalog Synthesis-as-Rule-Selection Problems**〔P1｜adv-rules〕（title 级）机制：把 Datalog 合成归约为「规则选择」，用 ASP/SMT 求解器求解——规则自动生成的一条路径。<https://doi.org/10.1145/3571200>
- **CN: Verifying Systems C Code with Separation-Logic Refinement Types**〔P2｜adv-rules〕（abstract 级）机制：分离逻辑+refinement 类型工程化为 C 验证工具 CN；「规格即类型化断言」的形态参考。<https://doi.org/10.1145/3571194>
- **Grisette: Symbolic Compilation as a Functional Programming Library**〔P2｜reference〕（abstract 级）机制：「符号编译」库：把构造符号执行/求解工具变成 DSL 组合件。<https://doi.org/10.1145/3571209>

### POPL 2024

- **Flan: An Expressive and Efficient Datalog Compiler for Program Analysis**〔P0｜adv-taint〕（abstract 级）机制：面向程序分析的 Datalog 编译器，支持函数式特性并把高阶结构编译为一阶求值。<https://doi.org/10.1145/3632928>
- **On-the-Fly Static Analysis via Dynamic Bidirected Dyck Reachability**〔P1｜adv-taint〕（title 级）机制：动态双向 Dyck 可达：CFL 可达的按需算法，边增删下仍高效。<https://doi.org/10.1145/3632884>
- **Monotonicity and the Precision of Program Analysis**〔P2｜adv-taint〕（title 级）机制：刻画「细化抽象何时反而损精度」——抽象域设计的理论判据。<https://doi.org/10.1145/3632897>
- **Inference of Robust Reachability Constraints**〔P2｜adv-rules〕（title 级）机制：自动推断鲁棒可达性约束，作规则触发的前置条件。<https://doi.org/10.1145/3632933>

### POPL 2025

- **An Incremental Algorithm for Algebraic Program Analysis**〔P1｜adv-taint〕（title 级）机制：代数（路径表达式/半环）分析的增量算法；与 IncIDFA 同为增量骨架候选。<https://doi.org/10.1145/3704901>
- **Derivative-Guided Symbolic Execution**〔P2｜reference〕（title 级）机制：用导数（差分）信息引导符号执行路径选择。<https://doi.org/10.1145/3704886>
- **CF-GKAT: Efficient Validation of Control-Flow Transformations**〔P2｜reference〕（title 级）机制：KAT 变体高效验证控制流变换等价。<https://doi.org/10.1145/3704857>
- **Biparsers: Exact Printing for Data Synchronisation**〔P2｜adv-parse〕（title 级）机制：解析/打印互逆的 biparser；AST↔源文本往返一致性参考。<https://doi.org/10.1145/3704910>

### POPL 2026

- **U-Turn: Enhancing Incorrectness Analysis by Reversing Direction**〔P2｜adv-taint〕（title 级）机制：反向错误分析（证「存在坏执行」）的方向反转增强——找真 bug 侧的分析。<https://doi.org/10.1145/3776688>
- **Security Reasoning via Substructural Dependency Tracking**〔P2｜adv-taint〕（title 级）机制：亚结构逻辑依赖追踪的安全推理——污点追踪的语义基础参考。<https://doi.org/10.1145/3776669>
- **Fuzzing Guided by Bayesian Program Analysis**〔watch｜reference〕（title 级）机制：贝叶斯程序分析引导 fuzzing；贝叶斯分析系列之一。<https://doi.org/10.1145/3776659>

### OOPSLA 2023

- **A Cocktail Approach to Practical Call Graph Construction**〔P1｜adv-taint〕（title 级）机制：多种调用图构建技术鸡尾酒式混搭，兼顾精度与开销。<https://doi.org/10.1145/3622833>
- **Exploiting the Sparseness of Control-Flow and Call Graphs for Efficient and On-Demand Algebraic Program Analysis**〔P1｜adv-taint〕（title 级）机制：利用 CFG/调用图稀疏性做按需代数分析。<https://doi.org/10.1145/3622868>
- **Interactive Debugging of Datalog Programs**〔P1｜adv-taint〕（abstract 级）机制：Datalog 交互式调试（解释推导/反问 why-not）——规则引擎解释层的直接参考。<https://doi.org/10.1145/3622824>
- **Rapid: Region-Based Pointer Disambiguation**〔P2｜adv-taint〕（title 级）机制：区域基指针对消。<https://doi.org/10.1145/3622859>
- **Verus: Verifying Rust Programs using Linear Ghost Types**〔P2｜adv-rules〕（abstract 级）机制：线性幽灵类型把 Rust 所有权编码进验证状态机。<https://doi.org/10.1145/3586037>
- **Adventure of a Lifetime: Extract Method Refactoring for Rust**〔P2｜adv-rules〕（title 级）机制：借用检查友好的 Extract Method 重构——Rust 侧自动重构落地样本。<https://doi.org/10.1145/3622821>

### OOPSLA 2024

- **Boosting the Performance of Alias-Aware IFDS Analysis with CFL-Based Environment Transformers**〔P1｜adv-taint〕（title 级）机制：用 CFL 环境转换器加速别名感知 IFDS——IFDS 内别名处理的实现级方案。<https://doi.org/10.1145/3689804>
- **A Learning-Based Approach to Static Program Slicing**〔P2｜adv-taint〕（title 级）机制：学习式静态切片；切片用于告警解释的候选实现。<https://doi.org/10.1145/3649814>
- **A Pure Demand Operational Semantics with Applications to Program Analysis**〔P2｜adv-taint〕（title 级）机制：纯需求操作语义：「按需只算被问的」的形式化——需求驱动分析的语义基础。<https://doi.org/10.1145/3649852>
- **A Typed Multi-level Datalog IR and Its Compiler Framework**〔P2｜adv-taint〕（title 级）机制：多层类型化 Datalog IR+编译框架——规则引擎中间表示设计参考。<https://doi.org/10.1145/3689767>
- **Monotone Procedure Summarization via Vector Addition Systems and Inductive Potentials**〔P2｜adv-taint〕（title 级）机制：用向量加法系统+归纳势函数表示单调过程摘要。<https://doi.org/10.1145/3689777>
- **The ART of Sharing Points-to Analysis: Reusing Points-to Analysis Results Safely and Efficiently**〔P1｜adv-taint〕（title 级）机制：指针分析结果的跨版本/跨配置安全复用——ADV 分析缓存与增量复用的判据来源。<https://doi.org/10.1145/3689803>
- **Making Formulog Fast: An Argument for Unconventional Datalog Evaluation**〔P2｜adv-taint〕（abstract 级）机制：Formulog（Datalog+函数+SMT）的非常规求值策略——规则内嵌 SMT 调用的性能路径。<https://doi.org/10.1145/3689754>
- **Finding Cross-Rule Optimization Bugs in Datalog Engines**〔P2｜adv-taint〕（title 级）机制：交叉规则差分找 Datalog 引擎优化 bug——自研规则引擎的测试判据。<https://doi.org/10.1145/3649815>
- **Learning Abstraction Selection for Bayesian Program Analysis**〔P2｜adv-taint〕（title 级）机制：学习选抽象：贝叶斯程序分析的抽象自动选择。<https://doi.org/10.1145/3649845>
- **Enhancing Static Analysis for Practical Bug Detection: An LLM-Integrated Approach**〔P1｜adv-rules〕（abstract 级）机制：LLM 为静态分析补前置条件与摘要，压误报/补漏报——ADV 告警治理的 LLM 集成点。<https://doi.org/10.1145/3649828>
- **Cocoon: Static Information Flow Control in Rust**〔P2｜adv-taint〕（abstract 级）机制：Rust 静态 IFC：类型级信息流控制——污点规则在 Rust 语义上的参照。<https://doi.org/10.1145/3649817>
- **Cedar: A New Language for Expressive, Fast, Safe, and Analyzable Authorization**〔P2｜adv-rules〕（abstract 级）机制：授权策略语言：形式化核+快速判定+可分析性——策略类规则语言的设计标杆。<https://doi.org/10.1145/3649835>
- **ParDiff: Practical Static Differential Analysis of Network Protocol Parsers**〔P2｜adv-parse〕（title 级）机制：协议解析器静态差分——多实现一致性检查，AST 归一层可类比。<https://doi.org/10.1145/3649854>
- **Refinement Type Refutations**〔P2｜adv-rules〕（title 级）机制：refinement 类型检查的反例（refutation）生成——失败告警可解释。<https://doi.org/10.1145/3689745>
- **WhiteFox: White-Box Compiler Fuzzing Empowered by Large Language Models**〔P2｜reference〕（abstract 级）机制：LLM 以白盒触发优化路径的方式生成编译器测试——对规则引擎/分析器做定向测试的同型思路。<https://doi.org/10.1145/3689736>

### OOPSLA 2025

- **A Flow-Sensitive Refinement Type System for Verifying eBPF Programs**〔P0｜adv-rules〕（title 级）机制：流敏感 refinement 类型验证 eBPF 程序——「检查器=类型系统」的完整工程案例。<https://doi.org/10.1145/3763799>
- **IncIDFA: An Efficient and Generic Algorithm for Incremental Iterative Dataflow Analysis**〔P0｜adv-taint〕（title 级）机制：迭代数据流分析的通用增量算法：变更后只重算受影响部分。<https://doi.org/10.1145/3720436>
- **Software Model Checking via Summary-Guided Search**〔P1｜adv-taint〕（title 级）机制：摘要引导的模型检查搜索——摘要作为搜索启发而非仅作缓存。<https://doi.org/10.1145/3763142>
- **Efficient Abstract Interpretation via Selective Widening**〔P1｜adv-taint〕（title 级）机制：选择性 widening：只在该加宽处加宽，降迭代次数且少损精度。<https://doi.org/10.1145/3763083>
- **Universal Scalability in Declarative Program Analysis (with Choice-Based Combination Pruning)**〔P1｜adv-taint〕（title 级）机制：声明式分析普适可扩展性：上下文组合的选择式剪枝。<https://doi.org/10.1145/3763129>
- **Towards a Theoretically-Backed and Practical Framework for Selective Object-Sensitive Pointer Analysis**〔P1｜adv-taint〕（title 级）机制：选择性对象敏感的理论化框架——上下文档位自动选择的依据。<https://doi.org/10.1145/3763111>
- **Flix: A Design for Language-Integrated Datalog**〔P1｜adv-taint〕（abstract 级）机制：语言集成 Datalog：函数式语言与 Datalog 约束求解互通的设计。<https://doi.org/10.1145/3763126>
- **Artemis: Toward Accurate Detection of Server-Side Request Forgeries through LLM-Assisted Inter-procedural Path-Sensitive Taint Analysis**〔P1｜adv-taint〕（title 级）机制：LLM 辅助的过程间路径敏感污点分析查 SSRF——污点精度档位与 LLM 的分工。<https://doi.org/10.1145/3720488>
- **Static Inference of Regular Grammars for Ad Hoc Parsers**〔P1｜adv-parse〕（title 级）机制：从手写（ad hoc）解析器推断正则文法——归一层识别解析逻辑的机制。<https://doi.org/10.1145/3763054>
- **An Empirical Study of Bugs in the rustc Compiler**〔P2｜reference〕（abstract 级）机制：rustc bug 实证——分析工具自身质量的经验分布。<https://doi.org/10.1145/3763800>
- **Revealing Sources of (Memory) Errors via Backward Analysis**〔P2｜adv-rules〕（title 级）机制：反向分析溯源（内存）错误来源——告警根因解释。<https://doi.org/10.1145/3720486>
- **Combining Formal and Informal Information in Bayesian Program Analysis via Soft Evidences**〔P2｜adv-taint〕（title 级）机制：软证据把非形式信息（文档/命名）并入贝叶斯分析。<https://doi.org/10.1145/3720508>

### OOPSLA 2026

- **From Raw Pointers to Memory Safety: A Modular Demand-Driven Typestate Analysis for Rust**〔P1｜adv-taint〕（title 级）机制：Rust 指针安全的模块化需求驱动 typestate 分析——需求驱动×typestate 组合样本。<https://doi.org/10.1145/3798266>
- **Hermes: Making Path-Sensitive Pointer Analysis Scalable for Sparse Value-Flow Analysis**〔P1｜adv-taint〕（title 级）机制：路径敏感指针分析在稀疏值流框架下的可扩展化。<https://doi.org/10.1145/3798211>
- **SPONGE: Adaptive Boundary-Anchored Indexing for Online Value-Flow Queries**〔P1｜adv-taint〕（title 级）机制：在线值流查询的自适应边界锚索引——LSP 式即时问答的索引方案。<https://doi.org/10.1145/3839472>
- **Mechanically Translating Iterative Dataflow Analysis to Algebraic Program Analysis**〔P1｜adv-taint〕（title 级）机制：把迭代数据流分析机械翻译为代数分析——两套框架统一的路由决策。<https://doi.org/10.1145/3798216>
- **Automated Debugging of Datalog Programs**〔P2｜adv-taint〕（title 级）机制：统计视角的 Datalog 程序调试——规则引擎排障。<https://doi.org/10.1145/3839516>
- **Grammar Repair with Examples and Tree Automata**〔P2｜adv-parse〕（title 级）机制：用样例+树自动机修文法——归一层文法自愈。<https://doi.org/10.1145/3798242>
- **Taming the Hydra: Targeted Control-Flow Transformations for Dynamic Symbolic Execution**〔P2｜reference〕（title 级）机制：定向控制流改写抑制路径爆炸——DSE 工程化手段。<https://doi.org/10.1145/3798202>
- **Determining the Unreachable: Constraint-Guided Reachability Analysis for Dependency Vulnerabilities**〔P2｜reference〕（title 级）机制：约束引导判定依赖漏洞不可达——SCA 误报治理。<https://doi.org/10.1145/3798255>
- **LLM-Based Alarm Resolution Guided by Bayesian Program Analysis**〔P2｜adv-rules〕（title 级）机制：贝叶斯分析引导 LLM 处置告警。<https://doi.org/10.1145/3839536>
- **First-Class Refinement Types for Scala**〔P2｜adv-rules〕（title 级）机制：一等 refinement 类型的 Scala 落地。<https://doi.org/10.1145/3839541>
- **Fully-Automatic Type Inference for Borrows with Lifetimes**〔P2｜adv-rules〕（title 级）机制：借用生命周期的全自动推断——Rust 借用语义规则化参考。<https://doi.org/10.1145/3798221>

### CAV 2023

- **Bitwuzla**〔P2｜reference〕（abstract 级）机制：Boolector 后继的工业级位向量/浮点/数组 SMT 求解器——约束求解后端选型。<https://boolector.github.io>
- **Symbolic Execution of Floating-point Programs: How far are we?**〔watch｜reference〕（title 级）机制：浮点程序符号执行的能力边界实证（章节级 DOI 未单独核实，链接为 CAV 2023 卷目录）。<https://dblp.org/db/conf/cav/cav2023.html>

### CAV 2024

- **Optimal Concolic Dynamic Partial Order Reduction**〔watch｜reference〕（title 级）机制：concolic 与 DPOR 结合的最优序（章节级 DOI 未单独核实，链接为 CAV 2024 卷目录）。<https://dblp.org/db/conf/cav/cav2024.html>

## 待验证 / 未收录（检索到存在，但条目级链接当日未能核实——宁缺毋假）

- Program Repair Guided by Datalog-Defined Static Analysis（OOPSLA 2023；检索摘要命中，DOI 未核实）。若核实并入 P1：Datalog 规则定义即修复定位。
- Boosting Compiler Testing by Injecting Real-World Code（PLDI 2024；检索确认获 Distinguished Paper Award，DOI 未入表）——规则引擎测试语料构造的参考。
- Points-To Analysis with Efficient Strong Updates（OOPSLA 2025；检索确认获 Distinguished Artifact Award；Crossref 过滤集中未见，疑卷次/题名变体）——强更新是 adv-taint 精度档位的关键机制，待补录。
- Re-thinking Datalog for Fast and Extensible Static Analysis（arXiv 2025；归属会讯待验证）——与 Flan/Soufflé 选型直接相关。
- Fray: An Efficient General-Purpose Concurrency Testing Platform for the JVM（OOPSLA 2025；检索提及与 Distinguished Paper 相关，未核实）。

## 覆盖限制与下一步

- CAV 2023–2026 只收 3 条（检索通道受限），不构成完备扫视；后续用 Springer 卷目录人工补扫，优先方向：SMT 求解器、符号执行、DPOR。
- PACMPL 侧 Crossref 共回 1495 条、关键词过滤后 449 条候选，本文收录 90 条；被过滤掉的等价饱和之外的张量编译/量子/数值类条目与本域无关。
- 贝叶斯程序分析系列（OOPSLA 2024–2026 多篇）与 LLM×静态分析系列建议作单独主题深读；本文只收入口篇。
- 2026 年条目（PLDI/POPL/OOPSLA 2026）均为已见刊卷的初扫，机制句多为题名级，深读优先级排在 2023–2025 的 P0/P1 之后。

