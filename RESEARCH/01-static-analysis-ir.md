# 01 · Rust 静态分析引擎与 IR 调研（重写蓝图供弹）

> 调研日期：2026-10-03。方法：WebSearch/WebFetch 外部检索（20+ 次），优先 2025–2026 信息源。
> 口径：凡写不出机制细节的不写；查不到的标「待验证」；结论带适用范围；许可证以仓库为准，未亲验的标注。
> 本文档只做「吸收决策输入」，不含实现代码。

---

## 0. 四个核心问题 · 一页结论

### ① 解析层怎么选（tree-sitter vs syn vs rustc 私有接口）

业界先例（各扫描器实际用什么，已逐家核实）：

| 工具 | 解析层 | 语义层 |
|---|---|---|
| Semgrep OSS | tree-sitter CST → 自研 generic AST（OCaml） | OSS 仅文件内；跨文件/跨函数 taint 在闭源 Pro |
| ast-grep | tree-sitter + Rust 自写遍历 | 无绑定，kind/模式匹配 |
| rust-code-analysis | tree-sitter | 仅度量，无绑定 |
| Clippy / Dylint / Flowistry / MIRAI / Kani | rustc 私有接口（`rustc_private`，nightly） | 完整（HIR/MIR/类型/作用域） |
| rust-analyzer | 自写 Rowan AST（不用 syn） | salsa + 自建 HIR/名称解析 |
| CodeQL | 每语言独立 extractor（Rust 版 2025-06 preview、2025-10 GA，buildless） | 关系库 + QL 查询 |
| ruff / oxc | 自写解析器（Python/JS） | arena AST + AstKind 分发 + scope/symbol |

**结论（适用范围：ADV 的多语言主体）**：多语言主解析 = tree-sitter（0.25 线，MIT，增量、容错、无损），上面自建一层**最小 generic AST**（Semgrep 已验证的形状：CST → 归一 AST，带语言逃生舱）；**Rust 深语义独立边车**（`rustc_private` + 固定 nightly + salsa 隔离），nightly API 绝不下渗主引擎——MIRAI/Flowistry/stack-graphs 三个项目的生命周期问题共同证明了这条隔离纪律的必要性。syn 只在「单文件 Rust 快速规则」这种窄场景有价值， ADV 不依赖它（rust-analyzer 弃 syn 自建 AST 是权威反例）。

### ② 增量分析路径（salsa vs 手写 memoization）

- **salsa**（"3.0" 重写 = 当前 0.22 系新架构，rust-analyzer 2025 年初完成迁移；Red-knot/ty 也在用；官方路线图走向 1.0-alpha）：tracked query/function + 修订号失效，细粒度依赖图、并行、durable（跨进程持久化，实验性）。代价：所有中间结果必须建模成纯函数式 query，设计期就要拆好「输入/中间/终态」三层。
- **手写 memoization**：只值得用在两处——（a）进程外磁盘索引（文件哈希 → IR 缓存，跨运行复用）；（b）tree-sitter 的文件内增量 parse（引擎自带）。其余依赖关系自己维护容易漏边、错失效，MIRAI 时代的 summary 缓存 bug 是前车之鉴（机制层面成立，未逐条考证其 bug 单）。
- **结论**：ADV 用 salsa 管「文件→AST→绑定→summary→诊断」整条 query 链；进程外缓存用内容寻址哈希落盘。两者分工而非二选一。

### ③ 过程间污点/数据流的工业实现路径

- 理论底座：IFDS/IDE（Reps–Horwitz–Sagiv 1995，exploded supergraph + tabulation），**需求驱动**是工业界收敛到的主流形态（CodeQL 查询本质按需求值；Flowistry `compute_flow` 逐指令按需；2025 年出现 Rust 的 IFDS 框架论文 **Pincer**）。
- 工业三形态：(a) **关系库 + Datalog**（CodeQL：extractor 落库，QL 谓词递归求值）；(b) **summary-based 抽象解释**（MIRAI，已归档，思想可抄：函数级摘要 + 域标记传播）；(c) **归一 AST 上过程间 taint**（Semgrep Pro 闭源，机制无公开细节——对它只能抄 YAML schema 不能抄引擎）。
- **结论**：ADV 双轨。快轨（全语言）：tree-sitter + 调用图启发式 + YAML taint spec 编译成 matcher；深轨（Rust）：MIR + 自写 IFDS 风格需求驱动分析（Pincer/Flowistry 可参考，两者许可开放）。定值/格上的不动点推理可用 **Ascent**（纯 Rust 宏 Datalog，支持自定义 lattice），避免 Soufflé 的 C++ FFI 与 DDlog 的死项目。

### ④ 规则表达层选型

- **CodeQL**：Datalog 语义、数据库化 IR——思想层标杆（专有，只能抄思想）。
- **Semgrep**：YAML + pattern 代码模板 + 元变量 + taint spec（sources/sanitizers/propagators）——用户面 schema 的事实标准。
- **Oxlint/oxc**：Rust 内嵌 `Rule` trait + `AstKind` 枚举分发 + arena AST——「复杂逻辑放 Rust」的工程样板（~871 规则实测跑得快）。
- **ast-grep**：YAML 的 kind/pattern/transform——「轻规则全 YAML」的样板。
- **结论**：ADV 规则双层：YAML 快轨（pattern + taint spec，引擎**编译**为 Rust matcher——Semgrep「规则→匹配程序」的思路 + Oxlint 的 AstKind 分发执行）；复杂规则走 Rust trait（oxc/biome 先例）。Ascent/Soufflé 只进深轨内部推理，不暴露给用户写规则。

---

## A. 解析与语法树层

### A1. Tree-sitter
- **一行定位**：多语言增量解析的事实标准库（Rust 核心），CST 无损 + 错误恢复 + S 表达式 query，编辑器（Neovim/Helix/Zed）与代码扫描器（Semgrep、ast-grep）共同的地基。
- **值得抄的机制**：
  1. **增量解析协议**：编辑 → `Tree::edit` 标记变更区间 → `parse(old_tree, new_source)` 只重算受影响子树——文件级增量的现成实现，ADV 直接用而非重造。
  2. **0.25（2025-02）起 parser 元数据内嵌**（语言名/版本/超类型/保留字进生成代码）——多 grammar 版本管理可直接读元数据做兼容检查，不用自己建清单。
  3. **query 谓词体系**（`#eq?`/`#match?`）：规则引擎的「AST 模板匹配」可以直接踩在 query 上，不必先写 generic AST 才能匹配。
- **档位【吸收】**：多语言主解析层的唯一现实选择；错误恢复对扫描场景（脏代码也要出结果）是硬需求。
- **第一步动作**：Cargo 引入 `tree-sitter 0.25.x` + 先接 Python/JS/Go/Rust 四个官方 grammar，固化「grammar 版本锁定 + 元数据自检」的加载层。
- **许可证**：MIT。
- **链接**：https://github.com/tree-sitter/tree-sitter ；changelog v0.25.0（2025-02-01，后续 0.25.x 补丁到 2025 全年）。

### A2. rust-analyzer（Rowan + salsa + HIR 分层）
- **一行定位**：Rust 官方系 IDE 引擎，把「无损红绿树（Rowan，Roslyn 思路）+ 增量 query（salsa）+ 分层 HIR」做成完整开源示范。
- **值得抄的机制**：
  1. **Rowan 红绿树**：green 节点不可变 + 哈希复用，red 节点带父指针/位置——「编辑稳定 + 内存省」的语法树标准答案；generic AST 若要求无损可复用此结构。
  2. **HIR 分层**：文本 → AST → item tree（浅层）→ def map → 类型/作用域，每层一个 query——ADV 的 IR 分层蓝图直接照这个切片法。
  3. **名称解析不做构建**：它证明了「语言内语义可以在不跑 cargo 的情况下近似求解」——ADV 对 Rust 快轨可抄其简化版（crate 名 + 路径启发），深语义仍交给边车。
- **档位【吸收】**（架构样板，非直接依赖除 salsa/Rowan 外的部分）。
- **第一步动作**：精读 `rust-analyzer/crates/hir` 的 query 切片与 `rowan` README，把 ADV 的 IR 层次图对齐后落设计文档。
- **许可证**：MIT OR Apache-2.0。
- **链接**：https://github.com/rust-lang/rust-analyzer ；Rowan: https://github.com/rust-analyzer/rowan 。

### A3. syn / proc-macro2
- **一行定位**：Rust 源的 proc-macro 级解析库，快上手、强类型 AST，但无增量、无绑定、无 MIR。
- **值得抄的机制**：span 保真（proc-macro2 的 `Span` 与源码回写）——诊断定位层的参照。
- **档位【不吸收】**：rust-analyzer 弃用 syn 自建 Rowan AST 是决定性先例；ADV 主引擎是多语言的，为 Rust 单语言引入第二个解析器徒增分叉。
- **第一步动作**：无（仅在「临时脚本解析 Rust 片段」场景允许局部使用）。
- **许可证**：MIT OR Apache-2.0。
- **链接**：https://github.com/dtolnay/syn 。

### A4. rustc 私有接口与 `rustc_mir_dataflow`（原 `rustc_mir::dataflow`）
- **一行定位**：rustc 内部的数据流框架（实现 `Analysis` trait → `iterate_to_fixpoint`，支持 gen-kill 与通用问题，结果可用 `ResultsVisitor` 随机访问），经 `rustc_private` 可当库用，但仅 nightly、无稳定性承诺。
- **值得抄的机制**：
  1. **框架抽象**：`Analysis` trait + `iterate_to_fixpoint` + 结果集与遍历器分离——ADV 深轨自己的数据流框架按这个形状设计（即便不用它的代码）。
  2. **crate 化拆分**：dataflow 已从模块拆成独立 crate `rustc_mir_dataflow`（2024-12 NN 的 streamline 博文佐证 API 仍在动）——证明"当库用"通道真实存在，也证明 churn 风险真实存在。
  3. **驱动模式**：clippy/dylint 式「rustc_driver 内嵌回调」是接入标准姿势；Flowistry/MIRAI 都靠固定 nightly 才活。
- **档位【有界】**：只在「Rust 深轨边车」内使用，固定 nightly + 锁版本 + 适配层隔离；不进主引擎。
- **第一步动作**：建 `adv-rust-deep` 边车原型：`rustc_private` + `rust-toolchain.toml` 锁 nightly，跑通「加载 crate → 取 MIR body → 打印」三步，验证 CI 可复现。
- **许可证**：Apache-2.0 WITH LLVM-exception（rustc 源码；`rustc_private` 是不稳定契约，升级伴随 rustc 版本）。
- **链接**：https://doc.rust-lang.org/beta/nightly-rustc/rustc_mir_dataflow/framework/index.html ；rustc-dev-guide dataflow 章节 https://github.com/rust-lang/rustc-dev-guide/blob/main/src/mir/dataflow.md ；NN 博文 https://nnethercote.github.io/2024/12/19/streamlined-dataflow-analysis-code-in-rustc.html 。

## B. 增量计算

### B1. Salsa
- **一行定位**：rust-analyzer 系的增量计算框架（tracked query + 修订号失效），2025 年初 rust-analyzer 完成新架构迁移，Red-knot（astral-sh 的 Python 类型检查器）跟进，走向 1.0-alpha。
- **值得抄的机制**：
  1. **修订号 + 依赖图失效**：query 结果带版本，输入变更沿依赖边失效——比「文件时间戳 + 整体重算」细一个数量级。
  2. **纯函数式 query 建模**：所有中间态都是 tracked function 的输出——框架强制 ADV 把 IR 各层拆成可独立失效的查询（这本身就是架构纪律）。
  3. **Durable incrementality**（rust-analyzer 2023 博文）：query 结果可跨进程持久化——与 ADV 的磁盘索引诉求同构，先看它做到哪一步再决定自建多少。
- **档位【吸收】**：增量层直接用；代价是设计期必须把「输入/派生」边界画干净，这个代价买的是不自己维护依赖图。
- **第一步动作**：用 salsa 0.22 系新 API 写 100 行玩具（两个 tracked fn 链 + 输入变更观察失效），量出冷/热路径延迟差，再定 IR 查询分层。
- **许可证**：MIT OR Apache-2.0。
- **链接**：https://github.com/salsa-rs/salsa ；docs https://salsa-rs.github.io/salsa/overview.html ；3.0 计划 https://hackmd.io/@salsa/ry5ZGyBiA ；issue #906（"3.0" 命名澄清）。

### B2. 手写 memoization（对照项）
- **一行定位**：不引框架、自己缓存——适合依赖边少而稳定的场景。
- **值得抄的机制**：内容寻址缓存（源文件哈希 → 解析树/摘要），进程外复用；ruff/oxc 系也用类似思路做跨运行缓存（细节未逐一核实，标待验证）。
- **档位【有界】**：仅限「磁盘索引」与「单文件 parse」两处；依赖关系一复杂就换 salsa，不手搓。
- **第一步动作**：磁盘索引格式定稿（哈希、schema 版本、失效策略三字段必备）。
- **许可证**：—（自建）。

## C. IR 与查询层（数据库化 / Datalog）

### C1. CodeQL
- **一行定位**：GitHub 的语义化代码查询平台：每语言 extractor 把代码落成关系库，QL（类 Datalog）在上面做谓词递归查询；闭源引擎，OSS/研究免费档。
- **值得抄的机制**：
  1. **数据库化 IR**：AST/调用/类型全进关系表，分析=查询——ADV 的中间产物（尤其深轨）按「可落库的关系事实」设计，天然获得可缓存、可离线、可增量。
  2. **buildless 提取**：Rust 支持不走编译直接从源提取（2025-06 preview → 2025-10-14 GA）——证明「不构建也能拿到够深的 Rust 事实」，ADV 快轨的可行性有了业界背书。
  3. **查询按需求值**：QL 谓词只在被问时算——与 IFDS 需求驱动同源，ADV 深轨对全库不动点要克制，优先按需。
- **档位【不吸收（代码专有）｜吸收（思想）】**：只抄「关系库 IR + 按需 + buildless」三层思想。
- **第一步动作**：装 CodeQL CLI 跑一个 Rust 样例仓，看它导出的 Rust 数据库 schema（表名/列）作为 ADV 事实 schema 的参照。
- **许可证**：专有（闭源；OSS/研究可免费使用其 CLI 与库）。
- **链接**：https://github.blog/changelog/2025-06-30-codeql-support-for-rust-now-in-public-preview ；https://github.blog/changelog/2025-10-14-codeql-scanning-rust-and-c-c-without-builds-is-now-generally-available 。

### C2. Semgrep
- **一行定位**：YAML 规则 + pattern 代码模板的扫描器；OSS 核心 OCaml（2025 年完成 OCaml 4→5 多核升级并开源 GC 调优工具，秋季版宣称大仓 3× 提速），跨文件/跨函数 taint 在闭源 Pro。**未发现 Rust 重写核心的官方动作**（2025-10 检索确认）。
- **值得抄的机制**：
  1. **tree-sitter CST → 自研 generic AST**（OCaml 按语言映射，grammar 锁定版本）：多语言归一的成熟形状，ADV 的 generic AST 按此设计但注意控制体量（Semgrep 的 generic AST 以臃肿著称，属于已知反面教材）。
  2. **taint spec schema**：sources/sanitizers/propagators 三段式 YAML——用户面污点配置的事实标准，ADV 的 YAML taint 轨直接兼容其形态（兼容它的规则资产 = 降低迁移成本）。
  3. **规则→匹配程序编译**：pattern 被编译成 AST 匹配器而非解释执行——ADV 快轨引擎照此把 YAML 编译为 Rust matcher + AstKind 分发。
- **档位【有界】**：OSS 引擎 LGPL-2.1（OCaml），借代码不如重写；schema 形态与思想吸收。
- **第一步动作**：抄 Semgrep 的 taint spec 字段集做 ADV YAML schema v0（字段一一对应），拿它公开规则库跑通度做回归集。
- **许可证**：OSS LGPL-2.1；Pro 专有。
- **链接**：https://github.com/semgrep/semgrep/issues/2558 （CST→generic AST 映射指南）；https://semgrep.dev/blog/2025/upgrading-semgrep-from-ocaml-4-to-ocaml-5 ；https://docs.semgrep.dev/semgrep-code/semgrep-pro-engine-examples 。

### C3. Soufflé
- **一行定位**：Datalog → 并行 C++ 编译器（另有解释模式），学术与安全分析（如某些安全属性推理）常用后端；无官方 Rust 绑定（issue #1466 讨论中）。
- **值得抄的机制**：
  1. **编译型 Datalog**：把声明式规则编译成命令行级性能的程序——「规则层编译化」的极致参照（ADV 快轨把 YAML 编译成 matcher 同源思想）。
  2. **嵌入模式**（`__EMBEDDED_SOUFFLE__`）证明 Datalog 可以当库内嵌——但走 C++ FFI，对零依赖目标是负资产。
- **档位【不吸收】**：C++/FFI 与零依赖 Rust 主体冲突；思想（编译化、并行算子）已被 Ascent 路线覆盖。
- **第一步动作**：无；若未来深轨算力不足再评估（仅届时）。
- **许可证**：UPL-1.0（待验证，以仓库 LICENSE 为准）。
- **链接**：https://souffle-lang.github.io/interface 。

### C4. Flix
- **一行定位**：JVM 上的函数式 Datalog 混合语言（带格与单调性检查），Apache-2.0；对 Rust 项目意味着引入 JVM 运行时。
- **值得抄的机制**：**格 + 单调框架的静态检查**（规则必须单调才合法）——ADV 若用 Ascent 自定义 lattice，照此原则约束规则写法，防「越推越大不收敛」。
- **档位【不吸收】**：JVM 依赖排除；只留单调性纪律。
- **第一步动作**：无。
- **许可证**：Apache-2.0（待验证）。
- **链接**：https://flix.dev/ 。

### C5. Ascent（含 datafrog 对照）
- **一行定位**：Rust 宏内嵌的 Datalog（`ascent` proc-macro，另有并行 `ascent_par`），支持**自定义 lattice 不动点**；2025 年末仍活跃发版（MSRV 1.85）。
- **值得抄的机制**：
  1. **`lattice` 关键字**：可对用户定义格求不动点（BYODS 工作：Bring Your Own Data Structures to Datalog）——ADV 深轨的数据流方程（污点传播、指针集）可以落成 Ascent 规则 + 格，不必自写半朴素求值器。
  2. **纯 Rust 宏、零 FFI**：Datalog 规则与 Rust 函数互调无缝——满足「本地优先、零依赖」主线。
  3. **对照 datafrog**：rust-analyzer 早期的极简 join 型 Datalog 库——若嫌 Ascent 编译慢，datafrog 是极小依赖备选（机制更原始，需自己写迭代策略）。
- **档位【吸收】**：深轨推理基座候选。
- **第一步动作**：用 Ascent 写「函数内定值 + 简单污点传播」原型，与手写 worklist 对比耗时与表达力，再决定深轨是否以它为基座。
- **许可证**：MIT OR Apache-2.0（检索结果表述；以 crates.io 页面为准）。
- **链接**：https://crates.io/crates/ascent ；https://github.com/s-arash/ascent 。

### C6. DDlog（differential Datalog）
- **一行定位**：VMware 的增量 Datalog，编译产物就是 **Rust 库**（基于 differential dataflow）——但项目已归档（vmware-archive），团队去了 Materialize，2025 HN 讨论确认不再维护。
- **值得抄的机制**：**增量 Datalog 语义**（输入变更只重算受影响关系，semi-naive + 差分 join）——这是「CodeQL 式库 + salsa 式增量」合体的思想原型；ADV 若要 IR 事实库增量更新，按此语义自建或借 differential-dataflow 库组装。
- **档位【不吸收（代码冻结）｜吸收（思想）】**。
- **第一步动作**：无（记录其架构图即可）。
- **许可证**：MIT（归档仓库）。
- **链接**：https://github.com/vmware-archive/differential-datalog 。

## D. 过程间数据流 / 污点 / 名称解析

### D1. IFDS/IDE 与 Pincer（2025 论文）
- **一行定位**：IFDS/IDE（Reps–Horwitz–Sagiv 1995）是过程间数据流的标准形态（exploded supergraph + tabulation + 需求驱动）；**Pincer**（ACM 2025，dl.acm 10.1145/3798266）号称首个面向 Rust 的需求驱动、流/域/上下文敏感 IFDS 框架（typestate/内存安全方向）。
- **值得抄的机制**：
  1. **exploded supergraph 表驱动求值**：把过程间问题压成图可达性——ADV 深轨的污点分析按 IFDS 建模可复用全部成熟算法性质（上下文敏感、需求驱动）。
  2. **需求驱动（按查询反推）**：只算被问的 (node, fact) 对——大仓扫描先答「这个 sink 有没有 source 可达」，不做全库全事实物化。
  3. **与格结合的 IDE 扩展**（edge value 为传递函数）：污点清洗器/传播器天然是 IDE 的边函数。
- **档位【吸收（思想）】**：Pincer 代码是否开源未确认（ACM 页 403，待验证）——机制照学，代码视 availability。
- **第一步动作**：读 Pincer 论文（或其 preprint 版）提取「Rust 上 IFDS 的 MIR 建模」一节，落成 ADV 深轨设计笔记；同时留 IFDS 教科书章（Naeem & Lhoták 的 Practical Extensions）做实现参考。
- **许可证**：论文思想无许可问题；代码许可待验证。
- **链接**：https://dl.acm.org/doi/full/10.1145/3798266 ；https://plg.uwaterloo.ca/~nanaeem/papers/cc10.pdf 。

### D2. Flowistry
- **一行定位**：Will Crichton 的 Rust 模块化信息流分析（ownership-aware），基于 `rustc_mir_dataflow` 框架做**按需、逐指令**的信息流计算；MIT 开源，nightly 固定。
- **值得抄的机制**：
  1. **`compute_flow` 需求驱动结构**（`FlowDomain`/`FlowResults` + 逐指令结果访问器）：「复用 rustc 数据流框架 + 需求驱动包装」的完整开源范本——ADV 深轨污点分析的骨架直接参照。
  2. **ownership-aware 流修正**（论文 arXiv:2111.13662）：用 move/借用语义修剪假依赖——比语言无关的保守污点少报很多假阳性，这是 Rust 特有的精度红利。
  3. **nightly 锁定工程**（rust-toolchain 固定 + 适配层）：用 rustc_private 的项目怎么活下来的实操模板。
- **档位【吸收】**（参照实现 + 部分代码可移植；注意 nightly 版本耦合，移植按算法抄、不按依赖引入）。
- **第一步动作**：clone 仓库精读 `flowistry::infoflow` 与 `flowistry::mir` 两个模块，产出「ADV 深轨污点引擎」的数据结构草案（哪些类抄哪些改）。
- **许可证**：MIT（OR Apache-2.0，以仓库为准）。
- **链接**：https://github.com/willcrichton/flowistry ；https://arxiv.org/abs/2111.13662 。

### D3. MIRAI
- **一行定位**：Meta（facebookexperimental）的 MIR 抽象解释器：函数级摘要 + 域标记（tagging）做过程间污点/不变量检查；**2024-08-22 已归档**（最后 release v1.1.8，2023-05）。
- **值得抄的机制**：
  1. **summary-based 过程间**：每个函数输出摘要（入参→出参的标记映射），调用点查摘要不重算——ADV 深轨跨文件污点的摘要协议可按此设计（与 salsa tracked fn 天然契合：摘要=query）。
  2. **抽象标记传播（tagging）**：把污点做成抽象域上的标签集合随值流动——比 bool 污点表达力强，能带清洗/传播语义。
  3. **Z3 求解器条件化报告**：路径敏感的告警过滤思想（是否引入 SMT 由 ADV 成本权衡，机制先记下）。
- **档位【不吸收（代码冻结）｜吸收（机制）】**。
- **第一步动作**：读其 `abstract_domain` 与 summary 相关模块的注释与论文级 README，写一页「ADV summary 协议」初稿。
- **许可证**：MIT（归档仓库）。
- **链接**：https://github.com/facebookexperimental/MIRAI 。

### D4. Stack Graphs
- **一行定位**：GitHub 的 Rust 名称解析库：把「名字定义/引用/作用域」编码为图 + 双向路径查找（类下推自动机），不要求完整构建即可解析；**2025-09-09 已归档**（tree-sitter-stack-graphs 同归档），曾支撑 GitHub 代码导航（Python/Java/JS/TS）。
- **值得抄的机制**：
  1. **图编码 + 双向路径**：把名称解析做成「def 节点到 ref 节点的路径存在性」——ADV 若做多语言跨文件符号绑定，这是「不构建也能近似绑定」的成熟算法（代码 MIT/Apache 可直接用，但上游已死需自养 fork）。
  2. **按语言规则文件（stack graph rules）从 tree-sitter 生成图**——「解析层只产 CST，语义图由声明式规则拼装」的分工方式。
- **档位【有界】**：算法与结构可抄/可 fork；依赖其活跃度的决策不可取（已归档）。多语言绑定是 ADV 二期需求，一期可用「crate 名 + 路径启发」先顶。
- **第一步动作**：把 stack-graphs 的核心论文/README 存档进设计资料库；原型期先用启发式绑定，二期评估 fork。
- **许可证**：MIT OR Apache-2.0（归档仓库）。
- **链接**：https://github.com/github/stack-graphs ；https://github.blog/open-source/introducing-stack-graphs 。

## E. 规则表达层先例

### E1. Oxlint / oxc
- **一行定位**：Rust 写的 JS/TS linter：arena 分配 AST + `AstKind` 枚举分发 + 语义（scope/symbol）随取，规则数 ~871（默认开 111、带 autofix 325，2025 数字）。
- **值得抄的机制**：
  1. **AstKind 分发**：规则声明自己关心的节点 kind，引擎按 kind 分发而非全树遍历回调——ADV 快轨 matcher 的执行模型照此（YAML 编译产物落到同一分发器）。
  2. **arena AST + 线性扫描遍历**：节点在连续内存里——generic AST 的内存布局参照（碎片化指针追逐是 ESLint 系的性能死因，oxc 的 ARCHITECTURE.md 有明说）。
  3. **`Rule` trait 收敛**：诊断/修复/选项/抑制在一个 trait 里——Rust 内嵌规则层的 API 形状样板（Biome 的 `Rule` trait 同思路，未在本轮逐一核实，标待验证）。
- **档位【吸收】**（执行模型与 API 形状；oxc 本身是 JS 工具不引入依赖）。
- **第一步动作**：定义 ADV 的 `Rule` trait 与 `AstKind`-式 kind 枚举（对 generic AST），先写 5 条内部规则验证分发开销。
- **许可证**：MIT。
- **链接**：https://oxc.rs/docs/learn/architecture/linter.html ；https://github.com/oxc-project/oxc/blob/main/ARCHITECTURE.md 。

### E2. ast-grep
- **一行定位**：Rust + tree-sitter 的结构化搜索/重写工具：YAML 规则（pattern/kind/transform/relational），pattern 代码即 AST 模板带元变量。
- **值得抄的机制**：
  1. **pattern 代码 = AST 模板 + 元变量**（`$VAR`/`$$$`）：用户写规则的学习成本最低形态——ADV YAML 快轨的 pattern 语法直接对齐。
  2. **relational 规则**（inside/has/matches 组合）：YAML 里表达结构关系而不写代码——快轨规则表达力的天花板参照。
  3. 自写遍历优化（第三方对比称 ~30% 提升，二手信息，待验证）：证明 tree-sitter 上层还可以再优化一层。
- **档位【有界】**：作为工具可引入测试/验证流程；引擎思想吸收。
- **第一步动作**：拿 ast-grep 的 YAML schema 做对照，把 ADV pattern/relational 字段定稿。
- **许可证**：MIT。
- **链接**：https://github.com/ast-grep/ast-grep ；https://ast-grep.github.io/advanced/tool-comparison 。

### E3. Biome（简记）
- **一行定位**：Rust 的 JS/TS 工具链（Rome 后继），analyzer 框架同样走 `Rule` trait + 语法声明 + 修复动作。机制与 Oxlint 同族（本轮未逐一核实其 2025 细节，待验证），作为「Rust 内嵌规则层」的第二先例存档。
- **档位【有界】**。**许可证**：MIT OR Apache-2.0。**链接**：https://github.com/biomejs/biome 。

### E4. rust-code-analysis
- **一行定位**：Mozilla 的 tree-sitter 多语言度量库（圈复杂度/Halstead/LOC 等），未归档但低维护（本轮未发现归档横幅；同级 mozilla/masche 已标 Deprecated）；2025 学术评测中解析成功率 100%（单样本口径）。
- **值得抄的机制**：**多语言度量归一层**（一套度量接口 × 多 grammar）——ADV 的「通用指标类规则」可参考其归一方式；另证明 tree-sitter 容错解析在真实大仓的可用性。
- **档位【有界】**。
- **第一步动作**：无需引入；列其度量清单进 ADV 规则 backlog。
- **许可证**：MPL-2.0（待验证，以仓库 LICENSE 为准）。
- **链接**：https://github.com/mozilla/rust-code-analysis 。

## F. 验证层（判断「有界吸收」价值）

### F1. Miri
- **一行定位**：rustc 自带的 MIR 解释器，逐执行检测 UB/别名违规（Stacked/Tree Borrows）；活跃维护，nightly only。
- **机制**：解释执行 = 精度天花板；成本天花板也在那——不能当扫描器。
- **档位【有界】**：作为 ADV 对高敏 Rust crate 的「CI 深检可选档」集成目标（跑得动才跑），不进扫描主路径。
- **第一步动作**：设计 ADV 报告格式时预留「Miri 结果源」字段（证据来源可区分）。
- **许可证**：Apache-2.0 WITH LLVM-exception。
- **链接**：https://github.com/rust-lang/miri 。

### F2. Kani
- **一行定位**：AWS 的 Rust 有界模型检查器（CBMC 后端，无需写规格语言）；非常活跃，正驱动 Rust std 验证计划（单次变更 16k+ harness，arXiv 2607.01504）。
- **机制**：harness + 边界展开找反例——「按函数/按属性的有界验证」恰是「有界吸收」的原始定义。
- **档位【有界】**：可选集成档（同 Miri 定位）；其 tool-comparison 页是 ADV 讲清「扫描 vs 验证」分工的现成话术库。
- **第一步动作**：无代码动作；在蓝图里给「验证档位」留位置（Miri/Kani 并列可选）。
- **许可证**：MIT OR Apache-2.0（以仓库为准）。
- **链接**：https://github.com/model-checking/kani 。

### F3. Prusti
- **一行定位**：ETH Viper 系演绎验证器；**维护趋缓**（2025 年中 PR 无人响应的社区反馈），仍被广泛引用。
- **档位【不吸收】**：规格负担重 + 维护风险；仅学术引用价值。
- **第一步动作**：无。
- **许可证**：Apache-2.0（待验证）。
- **链接**：https://github.com/viperproject/prusti-dev 。

### F4. Creusot（+ Verus 观察）
- **一行定位**：CEA/Inria 的 Rust 演绎验证器（Pearlite 规格语言，Why3/SMT 后端）；活跃（creusot.rs，POPL 2026 教程，参与 verify-rust-std）；Verus 为另一上升中的演绎验证器（社区 2025 多次并提）。
- **机制**：规格 + 证明义务——对扫描器无直接机制可抄；验证「proof-carrying 结果」趋势值得跟踪。
- **档位【不吸收】**（能力分层高于 ADV 目标；仅作为「未来高保障档」观察对象）。
- **第一步动作**：无。
- **许可证**：Creusot MIT（待验证）。
- **链接**：https://creusot.rs/ ；https://github.com/model-checking/verify-rust-std/issues/493 。

---

## G. 2025–2026 前沿信号

1. **CodeQL Rust：2025-06-30 公开预览 → 2025-10-14 GA（buildless）**。大厂用「不构建提取 + 关系库 + QL」正面解决 Rust 扫描——ADV 快轨「tree-sitter 也能拿到够深事实」的路线被验证；同时把「深轨」的差异化压在可自控与离线上。
2. **归档潮确认**：MIRAI（2024-08-22）、stack-graphs（2025-09-09）、DDlog（更早，vmware-archive）——三个「深度但依赖单一赞助方」的项目接连冻结。机制层教训：**依赖 rustc_private 或单一公司兴趣的部件必须架构隔离 + 可替换**。
3. **Salsa 新架构落地**：rust-analyzer 2025 年初完成迁移（RustConf 演讲 + 2025-03 公测），Red-knot/ty 采用，官方走向 1.0-alpha——增量框架选 salsa 的时机成熟度提高，风险从「设计变动」降为「跟随 0.22 系 API」。
4. **Pincer（ACM 2025）**：首个 Rust 需求驱动、流/域/上下文敏感 IFDS 框架论文——Rust 深轨的 IFDS 建模有了直接可引的学术蓝图（代码可得性待验证）。
5. **Semgrep 2025**：OCaml 4→5 多核升级 + 开源 dynamic_gc + 秋季 CE 版宣称大仓 3× 提速与原生 Windows——竞对性能基线抬升；无 Rust 重写迹象；跨文件能力仍锁在 Pro。
6. **Rust std 验证计划**（verify-rust-std，Kani 主力 + Creusot 参与提案 + Verus 上升）：「有界验证进标准库 CI」——验证层作为可选档位的价值被官方化。
7. **Oxlint 规则规模**（~871 条，2025）：AstKind 分发 + arena AST 的 Rust 规则层模式在千条规则量级被验证可撑住性能。
8. **tree-sitter 0.25 线**（2025-02 起）持续维护、parser 元数据内嵌——多语言解析层的生态风险低；但 grammar 版本兼容摩擦仍是社区常见抱怨（绑定/grammar/CLI 版本对齐要自动化）。
9. **验证层分层共识**（Kani tool-comparison 页 + 2025 HN/社区讨论「跨工具统一规格语言」呼声）：扫描器/验证器分工明确——ADV 报告模型按「证据来源分层」设计即可跟上。

---

## H. Top-3（对新项目最值钱的三条）

1. **解析与 IR：tree-sitter 0.25 + 自建最小 generic AST + Rust 深轨边车三件套**。generic AST 按 Semgrep 的「CST→归一 AST」形状设计但控制体量（oxc 的 arena + kind 枚举做内存与分发底座）；Rust 深语义独立边车（`rustc_private` + 固定 nightly + salsa），nightly API 绝不下渗主引擎——MIRAI/Flowistry/stack-graphs 的归档潮是这条纪律的实证。位置：架构图最底两层（Parse → Normalized IR → Query 层全部只看 generic AST，深轨结果作为旁路事实注入）。
2. **增量与规模：salsa 统管依赖失效，内容寻哈希管跨运行缓存**。把「文件→AST→IR→绑定→summary→诊断」全部建成 salsa tracked query（rust-analyzer 2025 迁移后的成熟用法），手写 memoization 只留磁盘索引一处；过程间摘要协议（MIRAI 思想：函数级 summary + 标记传播）直接映射为 salsa query，天然获得细粒度失效。位置：IR 层之上的「增量骨架」横切所有派生数据。
3. **过程间污点与规则层：YAML taint spec（sources/sanitizers/propagators，兼容 Semgrep 形态）编译成 AstKind 分发的 Rust matcher（快轨，全语言）；深轨 Rust 用 MIR + IFDS 需求驱动建模（Pincer 2025 + Flowistry 机制，均为开放许可），不动点推理用 Ascent（纯 Rust、自定义 lattice）**。Soufflé/Flix/DDlog/MIRAI 代码一律不进依赖，思想进设计笔记。位置：Query 层的「分析内核」与用户面「规则 schema」两个模块，中间用「规则编译」隔离。

## 附：对象 × 档位速查

| 对象 | 档位 | 许可证 | 状态锚点 |
|---|---|---|---|
| tree-sitter | 吸收 | MIT | 0.25 线活跃（2025） |
| rust-analyzer/Rowan | 吸收（架构） | MIT OR Apache-2.0 | 活跃 |
| syn | 不吸收 | MIT OR Apache-2.0 | 活跃 |
| rustc_mir_dataflow / rustc_private | 有界（深轨边车） | Apache-2.0 + LLVM-exception | unstable，churn 证实 |
| salsa | 吸收 | MIT OR Apache-2.0 | 0.22 系新架构，rust-analyzer/Red-knot 在用 |
| CodeQL | 思想吸收（专有） | 专有 | Rust GA 2025-10-14 |
| Semgrep | 有界（schema/思想） | LGPL-2.1 OSS + 专有 Pro | OCaml 5（2025），核心未迁 Rust |
| Soufflé | 不吸收 | UPL-1.0（待验证） | 无官方 Rust 绑定 |
| Flix | 不吸收 | Apache-2.0（待验证） | JVM 依赖 |
| Ascent | 吸收 | MIT OR Apache-2.0 | 活跃（2025 末发版） |
| DDlog | 不吸收（冻结） | MIT | 已归档 |
| Flowistry | 吸收 | MIT（OR Apache-2.0） | nightly 固定范本 |
| MIRAI | 不吸收（代码）/吸收（机制） | MIT | 归档 2024-08-22 |
| Stack Graphs | 有界 | MIT OR Apache-2.0 | 归档 2025-09-09 |
| Oxlint/oxc | 吸收（执行模型） | MIT | ~871 规则（2025） |
| ast-grep | 有界 | MIT | 活跃 |
| Biome | 有界（待验证细节） | MIT OR Apache-2.0 | 活跃 |
| rust-code-analysis | 有界 | MPL-2.0（待验证） | 低维护未归档（本轮未见归档横幅） |
| Miri | 有界（可选档） | Apache-2.0 + LLVM-exception | 活跃 |
| Kani | 有界（可选档） | MIT OR Apache-2.0 | 活跃（std 验证） |
| Prusti | 不吸收 | Apache-2.0（待验证） | 维护趋缓 |
| Creusot / Verus | 不吸收（观察） | 待验证 | 活跃 / 上升 |
| Pincer | 吸收（思想） | 待验证 | ACM 2025 论文 |
