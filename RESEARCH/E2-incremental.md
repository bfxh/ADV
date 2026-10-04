# E2 增量计算与响应式系统（adv-parse / adv-index / xtask-gates 的增量机制调研）

- 调研日期：2026-10-03；执行方式：WebSearch（2025–2026 优先）+ 既知文献锚定。
- 检索限制声明（否证留档）：crates.io / GitHub API 直连在本环境被拒（HTTP 403 / TLS 吊销检查失败），
  **精确版本号与 Materialize 现行许可证未能当日核实**，相关处均已标注「以 crates.io/官方为准」；其余锚点来自当日检索结果。
- 范围口径：按任务书，可微编程（differentiable programming）、Turbo 构建链**不纳入**本域。
- 档位语义：**吸收**（直接进实现）｜**有界**（限点使用、带边界条件）｜**不吸收**（明确拒绝并留因）｜**watch**（跟踪再看）｜**reference**（思想/对照，不集成）。
- ADV 既定背景：adv-parse/adv-index 已定 **salsa 统管内存侧增量**（wave-0 01），磁盘侧走内容 hash + 原子换名；本域补「还能从增量谱系抄什么」。

---

## 0. 谱系总览（一张图看增量计算 20 年）

```
self-adjusting computation (Acar 2005)
  ├─ Memoize/Propagate/Reduce 原语；Modifiable 的 changeability（变化=一阶值）
  ├─→ 增量 λ 演算 ILC（Hammer 等，POPL 2015 起）→ Adapton（Rust crate，2018 后停更）
  │       └─→ salsa（借鉴 adapton/glimmer/rustc，rust-analyzer 生产化，MIT/Apache-2.0）★ADV 已选
  ├─→ rustc query 系统（2006 RFC on-demand 起步，try-mark-green + fingerprint + dep-graph 落盘）
  │
timely dataflow（Naiad, SOSP 2013）→ differential dataflow（CIDR 2013）
  └─→ DBSP（VLDB 2023）→ Feldera 引擎（2023 创立，2025 融资 $15M）
       （另一支：DRed / counting 算法（SIGMOD 1995/1993）→ 物化视图维护学术谱系）

响应式 UI 支线：Fr signals → SolidJS → leptos reactive_graph（Rust）/ TC39 Signals 提案（2024 Stage 1）
电子表格支线：Excel 依赖树重算（脏链 + 拓扑 + 循环组，2010 起多线程）
构建系统支线：Build Systems à la Carte（ICFP 2018）——early cutoff 的分类学
内容寻址支线：LLVM CAS（RFC 2022-02）→ swift-driver —— 磁盘侧增量缓存的工程形态
```

ADV 的两个增量面与本域机制的映射：
- **内存侧**（salsa 统管）：从 rustc/salsa/signals 抄「依赖记录 + 重验」；
- **磁盘侧**（内容 hash + 原子换名）：从 DBSP/LLVM CAS/SQLite 触发器抄「变更代数 + 内容寻址缓存 + 触发器式维护」。

---

## 1. 增量计算框架

### 1.1 salsa（2025–2026 进展补全）【机制级 · 深挖】
- **定位**：按需增量查询框架；rust-analyzer 的地基。ADV 已选型（wave-0 01），此处只补新进展与机制细节。
- **可抄机制**：
  1. 查询式 API：`#[salsa::tracked]` 函数 + 输入结构体（`#[salsa::input]`），依赖在**执行期**经 QueryContext 记录——不要求手写依赖声明（与 1.5 rustc 同源）。
  2. red-green：memo 存 fingerprint（输入指纹聚合），复验未变→标绿→直接复用结果，避免重算。
  3. 周期恢复（CycleRecovery）：定点迭代 + fallback 值回滚，循环依赖不 panic。
- **2025–2026 信号**：仓库活跃（salsa-rs/salsa，CHANGELOG 跟踪）；另见 `rust-analyzer-salsa` 分离发布包（crates.io，4 个版本，experimental 标注）——存在 fork/分流形态，ADV 跟主线即可。
- **档位**：**吸收**（已定）｜**第一步**：adv-parse 已接线；核对当前 CHANGELOG 与依赖版本（本日未核实精确版本号，网络受限）。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（rust-analyzer 主力）。
- **链接**：https://github.com/salsa-rs/salsa · https://salsa-rs.github.io/salsa/

### 1.2 salsa Durability 与周期恢复（机制细化）【机制级 · 深挖】
- **定位**：salsa 2022 重写后的两个关键机制，ADV 用法设计的直接输入。
- **可抄机制**：
  1. **Durability 分层**：输入按 LOW/MEDIUM/HIGH 分耐久级；低耐久输入（如"上次扫描时间戳"）失效不触碰高耐久输入（源文件内容）关联的 memo——失效局部化。
  2. **changed-at 溯源**：memo 记录变化来源，可回答"谁导致它变"（调试增量失效的抓手，对应 1.6 可观测性）。
  3. **回滚式周期恢复**：进入循环的查询回退到 fallback 值并标记，避免假依赖。
- **ADV 映射**：把「源文件内容」设 HIGH、「规则配置」MEDIUM、「门缓存元数据」LOW——规则配置热更不失效全量索引。
- **档位**：**吸收**｜**第一步**：在 adv-parse 输入定义处落 durability 分层约定（1 天内可做）。
- **许可证/成熟度**：同 salsa；机制自 2022 重写起稳定。
- **链接**：https://salsa-rs.github.io/salsa/

### 1.3 Adapton 与增量 λ 演算（ILC）
- **定位**：salsa 的学术前身之一（"inspired by adapton, glimmer, rustc"——salsa 官方自述）。需求驱动增量图：Ref/Cell/Demand 三类节点。
- **可抄机制**：
  1. **ILC（Hammer 等，POPL 2015 起）**：把"变化"做成语言级一阶值——函数结果 = 输出对输入变化的响应函数（change transformation），即 changeability 的语言级形态。
  2. demand 模式：消费者拉取时才构造依赖边（惰性图），图只含"真实被读的路径"。
- **档案价值**：机制已被 salsa 吸收大半；Rust crate `adapton` 约 2018 后停更（crates.io 档案口径），不宜作为依赖。
- **档位**：**reference**｜**第一步**：无；留在谱系图即可。
- **许可证/成熟度**：MPL-2.0；**弃维护**（crate 层面）。
- **链接**：https://github.com/cuplv/adapton · ILC: POPL 2015 "Incremental Lambda Calculus"

### 1.4 self-adjusting computation（Acar 脉络）＋ Change Actions【机制级 · 深挖】
- **定位**：整个领域的理论根基。Acar（CMU 博士论文 2005，后续期刊版）：自适应计算 = Memoize（记录计算）/ Propagate（记录控制依赖并传播）/ Reduce（增量归并）三原语；成本模型：无变化时理想开销，变化传播 ∝ **受影响闭包**而非全量。
- **可抄机制**：
  1. **changeability**：Modifiable 的"变化"本身是一阶值——`旧值 → 新值` 的映射可以显式构造、传递、复合。失效判定从「脏了→全部重算」升级为「有 delta → 沿闭包传播 delta」。
  2. **Reduce 原语**：聚合结果可随元素 delta 增量更新（count/sum/union 天然可 reduce；min/exists 需 tombstone）。
  3. **Change Actions（Alvarez-Picallo 等，POPL 2019，DeltaML 原型）**：把 change 推广为"作用"（可逆/可组合），证明 delta 传播可以系统化到函数式语言全层。
- **对门缓存失效的启示（重点问题②，详见 §6-②）**：门（xtask-gates）的输入（规则集/语料集/配置）都可用「内容 hash 的 diff」表达为一阶变化值；门缓存的失效 = delta 沿门依赖图传播，而非任一输入变就全量重跑。
- **档位**：**吸收**（概念层进 xtask-gates 设计）｜**第一步**：在门缓存设计文档中定义「gate input delta」的三个构造子（增/删/改 文件集与规则集）。
- **许可证/成熟度**：学术文献；思想成熟（20 年被反复引用）。
- **链接**：Acar "Self-Adjusting Computation"（CMU 2005 / TOPLAS 2009）；DeltaML: https://github.com/allyourcodebase? (以 POPL 2019 论文页为准)

### 1.5 rustc query 系统：dep-graph 增量编译【机制级 · 深挖】
- **定位**：salsa 的另一血统来源；编译器级增量查询系统，工程细节比 salsa 更极端（跨会话、编码器、分片）。
- **可抄机制（拆解，对应重点问题③）**：
  1. **DepNode/DepKind + 执行期 read 记录**：每个查询实例一个 dep 节点；查询执行中经 `DepGraph::read` 记录读取边（TaskContext 栈）——依赖是**测出来的**，不是声明的。
  2. **128-bit fingerprint + try-mark-green**：惰性着色——输入指纹全绿则本节点绿（结果从跨会话缓存加载），否则红（重执行）。绿判定逐层上推，最坏 O(依赖深度)。
  3. **dep-graph 序列化落盘**：dep 图与查询结果缓存（on-disk cache）跨进程复用——增量不止在单会话内存内。
  4. **按 owner 分片**：依赖节点锚在 `LocalDefId`（HIR 项）粒度——文件内一项改动只污染该项的闭包。
- **与 salsa 的差异与选层结论（③）**：核心 red-green 同源；rustc 多出「dep 图落盘 + owner 分片 + 编码器流水线」，salsa 多出「Durability + 库形态易嵌入」。**ADV 学哪层**：学 rustc 的（a）owner 级分片映射为 per-file/per-symbol 依赖锚点，（b）dep 图持久化思路（弥补 salsa 跨进程复用短板），（c）`-Z incremental-info` 式命中率观测；**不学**其编译器专用编码器/灰节点特例。
- **档位**：**吸收**（选层如上）｜**第一步**：adv-index 的 salsa 查询全部以 `FileId/SymbolId` 为粒度锚点并落「增量命中率」统计。
- **许可证/成熟度**：rust-lang 生态（MIT/Apache-2.0）；维护中。
- **链接**：https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html · RFC "rustc-on-demand-and-incremental"

### 1.6 rustc 增量可观测性（detailed dep-graph / -Z 调试设施）
- **定位**：`-Zdump-dep-graph`、`-Zincremental-info`（rustc-dev-guide 调试章）：边计数、命中/失效统计、着色结果转储——增量系统必须"能被看见"。
- **可抄机制**：门/索引跑完输出「命中率 / 失效原因 top-N / 传播深度分布」三类计数——失效异常（如全量重算）能被一眼定位。
- **档位**：**reference**｜**第一步**：并入 xtask-gates 输出格式（一行 JSON 汇总即可）。
- **许可证/成熟度**：同 rustc；长期存在的调试设施。

### 1.7 Build Systems à la Carte（early cutoff 分类学）【机制级】
- **定位**：Mokhov / Mitchell / Peyton Jones（ICFP 2018）：把所有构建系统拆成「Scheduler × Capper」两轴；Excel/rustc/Shake/Bazel 各是坐标系中的点。
- **可抄机制**：**early cutoff**（Shake 的招牌）：依赖**值**未变（非仅仅"动过"）则下游不传播——dirty 标记（保守上界）与值重验（精确下界）分离。这是「影响集不漏」问题的分类学答案（对应 §6-⑤）。
- **档位**：**吸收**｜**第一步**：xtask-gates 的门缓存 key 必须是「输入内容 hash」而非「输入 mtime/版本号」。
- **许可证/成熟度**：论文开放获取；Shake 实现成熟（MIT）。
- **链接**：https://dl.acm.org/doi/10.1145/3236778 （ICFP 2018）

---

## 2. 流与增量视图

### 2.1 DBSP（Feldera 的增量数据流代数）【机制级 · 深挖】
- **定位**：VLDB 2023（Budiu/Chajed/McSherry/Ryzhyk/Toman）：把"任意查询电路"**机械地**变换成增量电路的代数。不是又一个流引擎——是"算子微分"的形式化。
- **机制拆解（对应重点问题①）**：
  1. **Z-set**：`键 → 整数权重` 的映射，构成阿贝尔群——插入=+1，删除=-1，更新=(-1 旧, +1 新) 对。**一切变更都是可加减的代数对象**。
  2. **lift（算子微分）**：对电路中每个算子 T 定义增量版本：线性算子（filter/投影/聚合和）直接 `T(Δ)`；非线性算子（join）展开为「Δ左 × 积分(右) + 积分(左) × Δ右 + Δ左 × Δ右」——积分=当前累积状态。电路可组合 ⇒ 复合查询的增量版本自动导出。
  3. **单调性优化**：输入单调（如 append-only 键）时积分类项可免于回撤处理。
  4. （Developer Voices 访谈口径：核心算子约 4 个即可组合出任意 SQL 的增量版——检索锚点 2026-10。）
- **对 adv-index 的映射（①的正面回答，详见 §6-①）**：扫描 diff = `(path, entity_kind)` 上的 Z-set；索引摘要（符号计数表、目录聚合、反向依赖计数）全部是线性算子 → 可用 ~200 行微内核维护；join 类（符号→定义点）需一侧积分（保留现有索引即积分）。
- **档位**：**吸收**（思想与最小实现，非引依赖）｜**第一步**：adv-index 定义 `ZSet<K>` 差分类型 + 两个线性算子（project/aggregate），跑通目录级符号计数增量。
- **许可证/成熟度**：Apache-2.0（feldera/feldera）；维护中且商业化加速。
- **链接**：https://github.com/feldera/feldera · DBSP paper (VLDB 2023)

### 2.2 Feldera（DBSP 引擎产品现状）
- **定位**：DBSP 的产品化：流式 SQL → 增量计算引擎，定位"毫秒级延迟、增量维护视图"。
- **2025–2026 信号**：2025 年获 **$15M** 私募融资（StreetZero / Form D 口径，Feldera 2023 年创立）；用例覆盖 CDC/流处理/低延迟服务/Iceberg 管理；工程博客有"拆分自动生成的大 Rust 代码以全核并行编译"等机制文；2026 年已有第三方项目（Datalog 上下文引擎）显式引用 "DBSP-style retractions"——**机制外溢到非数据库领域**，与 ADV 的用法同型。
- **档位**：**watch**｜**第一步**：订阅其 changelog；不引依赖（ADV 不需要 SQL 引擎，只需要 Z-set 代数）。
- **许可证/成熟度**：Apache-2.0；维护中（商业公司支撑）。
- **链接**：https://www.feldera.com · https://github.com/feldera/feldera

### 2.3 differential dataflow【机制级 · 深挖】
- **定位**：McSherry 等（CIDR 2013）：在 timely 之上做"差分"——计算 = 对**带部分序时间戳的变化流**的函数。
- **可抄机制**：
  1. **(数据, 时间, diff) 三元组**：变更带时间戳与权重；同一键的 diff 可合并（历史压缩的前提）。
  2. **frontier（边界）**：部分有序时间下的"哪些时刻已定"判定——ADV 多源扫描（并行扫描器完成顺序不一）对齐时可借用此概念定义"到哪一刻可以安全落盘"。
  3. **arrangement**：共享的有序索引 + 历史差分压缩——下游算子按需重放。
- **有界理由**：部分序时间对 ADV 是过度设计（单机批处理时序简单）；其「diff 可合并→历史可压缩」直接可抄。
- **档位**：**有界**｜**第一步**：仅采纳「变更记录携带 (内容 hash, 序号) 并按键合并压缩」进 adv-index 磁盘格式。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频但稳定）。
- **链接**：https://github.com/TimelyDataflow/differential-dataflow

### 2.4 timely dataflow
- **定位**：differential 的底座：逻辑坐标格 + capabilities 令牌控制 frontier 推进 + epoch 化消息。
- **可抄机制**：capability 令牌 = "我还未完成、下游别指望我"的显式化——ADV 的扫描进度条/可落盘判定可借用（对应 2.3-2）。
- **档位**：**reference**（ADV 无多轮迭代流拓扑需求）｜**第一步**：无。
- **许可证/成熟度**：MIT/Apache-2.0；维护中。
- **链接**：https://github.com/TimelyDataflow/timely-dataflow

### 2.5 Materialize
- **定位**：differential dataflow 之上的流式 SQL 数据库（物化视图即产品）。
- **2025–2026 信号**：builtin.com（2026-08-05）评其"流式 SQL 小众头部、发布节奏快、平台覆盖有限"。许可证有变动史（曾为 BSL 1.1 系源可用许可；**现行条款当日未能核实**——检索环境受限，引用前以官方 LICENSE 为准）。
- **档位**：**watch**｜**第一步**：无（其价值已由上游 differential/DBSP 覆盖）。
- **许可证/成熟度**：源可用许可（待核）；维护中（商业公司）。
- **链接**：https://materialize.com

### 2.6 Kafka Streams（对照，浅）
- **定位**：日志/变更日志范式：KTable 状态表 = changelog topic 回放重建。
- **机制对照**：每行变更一条重放记录、无代数微分——重放成本随历史增长（compaction 缓解但不消除）；与 DBSP 的 Z-set 对照说明"有代数"与"有日志"的差距。
- **档位**：**reference**（不深入，任务书口径）｜**第一步**：无。
- **许可证/成熟度**：Apache-2.0；ASF 维护中。

---

## 3. 响应式系统

### 3.1 leptos / reactive_graph（Rust 细粒度响应式）【机制级】
- **定位**：leptos 0.8 起响应式核心独立为 `reactive_graph` crate（docs.rs 在档 0.8.20，2025–2026 持续维护）；signal/memo/effect 构成依赖图，无虚拟 DOM diff，只更新真正依赖变化的节点。
- **可抄机制**：
  1. **push-pull 混合**：脏标记自源下推（push），值仅在读取时上拉重算（pull）——避免了纯 push 的风暴与纯 pull 的全扫。
  2. **依赖运行期记录**：memo/effect 执行时闭包内对 signal 的读取被 owner 上下文捕获成边——与 salsa/rustc 的"执行期记录"同构（跨域验证了该机制是共识解）。
  3. **owner 树**：节点生命周期随 owner 销毁级联清理——ADV 的 per-file 分析缓存可借用（文件删除→其 owner 子树整个释放）。
- **档位**：**有界**｜**第一步**：不引依赖；将 owner 树语义写进 adv-index 缓存生命周期设计。
- **许可证/成熟度**：MIT；维护中（活跃）。
- **链接**：https://docs.rs/reactive_graph · https://book.leptos.dev

### 3.2 TC39 Signals 提案
- **定位**：JS 标准化的信号图提案（2024 进入 Stage 1，2025–2026 仍在推进）；把各框架（Solid/Vue/Preact 等）的细粒度机制收敛成语义规范。
- **可抄机制**：**dirty/check/clean 三态 + 等值短路**——上游变→下游先标 dirty；读取时 check（重算比较值）→ 值相等则恢复 clean、**不再向下传播**。等值短路是影响集精度的免费来源（对应 §6-⑤：值相等即传播截止）。
- **档位**：**reference**｜**第一步**：把"比较后相等即截断"写进 ADV memo 重验协议。
- **许可证/成熟度**：提案（未定稿）；设计文档完善。
- **链接**：https://github.com/tc39/proposal-signals

### 3.3 Excel 式依赖图重算
- **定位**：电子表格是最老牌的细粒度响应式生产系统：公式依赖树 + 脏链传播 + 拓扑序重算。
- **可抄机制**：
  1. **循环引用 → 迭代计算组**：环不报错而是隔离成迭代组按收敛阈值算——ADV 规则依赖若出现环（规则 A 的建议影响规则 B 的判定）可用同法。
  2. **多线程重算**（Excel 2010 起）：依赖链独立分区并行——门调度并行化的先例。
  3. 重建依赖树（结构变化后全量重建树再增量重算）——简单粗暴但被验证可维护。
- **档位**：**吸收**（机制参考）｜**第一步**：门调度器按"独立依赖链分区"并行。
- **许可证/成熟度**：不适用（产品机制）；成熟度以 30 年生产验证为锚。
- **链接**：Microsoft 官方文档 "Excel performance: improving calculation performance"

---

## 4. 编译器增量（rustc 之外）

### 4.1 Swift：swift-driver CAS 调度与细粒度依赖【机制级】
- **定位**：Swift 编译驱动（swift-driver）用 **CAS（内容寻址存储）+ 细粒度依赖跟踪**调度重编译；含 dependency verifier（校验依赖图正确性的工具）与细粒度依赖扫描器。
- **可抄机制 / 教训**：
  1. 编译动作缓存键 = 输入内容 hash 树（CAS）——与 ADV 磁盘侧"内容 hash + 原子换名"同型，验证了路线。
  2. **Soundness 事故教训**：SPM 曾有"增量构建漏重建依赖目标→内存损坏"的公开 bug（GitHub issue，检索锚 2026-10）——失效判定**漏检的代价是正确性而非性能**。
  3. 2026-06 SwiftPM 提案新增"增量构建性能调试"CLI 选项（forums.swift.org pitch 锚）——工具链普遍把增量可观测性做成一等公民。
- **档位**：**有界**｜**第一步**：把「依赖图 verifier」（离线校验 dep 记录完整性）列入 xtask-gates 低频门。
- **许可证/成熟度**：Apache-2.0（带运行时例外）；维护中。
- **链接**：https://github.com/swiftlang/swift-driver · LLVM CAS RFC（2022-02, discourse.llvm.org）

### 4.2 LLVM CAS
- **定位**：内容寻址存储库 + 细粒度动作依赖跟踪（RFC 2022-02）：缓存键由输入内容 hash 层级构成，跨构建声音复用。
- **可抄机制**：**hash 树键**（输入文件 hash → 动作 hash → 产物 hash 的 Merkle 式结构）——adv-index 磁盘分片命名与失效判定可直接采用该结构：父 shard 的键含子 shard hash，改动自然上浮。
- **档位**：**有界**｜**第一步**：adv-index 磁盘布局文档中落 Merkle 键规范（与既有内容 hash 方案兼容）。
- **许可证/成熟度**：Apache-2.0（LLVM）；维护中。
- **链接**：https://discourse.llvm.org/rfc/ (CAS RFC, 2022-02)

---

## 5. 持久化增量

### 5.1 SQLite 触发器式物化视图模式
- **定位**：SQLite **无原生物化视图**（长期路线图议题，未落地口径）；社区标准做法 = 聚合表 + INSERT/DELETE/UPDATE 触发器维护。
- **可抄机制**：触发器更新本质是**手工 Z-set**：行插入对计数列 +1、删除 -1、更新 = 删旧加新——与 DBSP 代数在"线性聚合"子集上重合。启示：adv-index 的持久聚合（符号计数、目录统计）用"写入路径顺带维护"即可，不必读时全量重算；但**触发器完备性即一致性**（漏一个触发点=索引悄悄错）。
- **档位**：**有界**｜**第一步**：adv-index 落盘层只在两类写入点（扫描写、规则写）挂"维护钩子"，并用低频全量校验门兜底。
- **许可证/成熟度**：SQLite Public Domain；模式成熟。
- **链接**：https://sqlite.org/lang_createtrigger.html

### 5.2 物化视图维护算法谱系（DRed / counting / 键保持）【机制级 · 深挖】
- **定位**：IVM（增量视图维护）学术谱系，是"影响集怎么算才不漏"的理论库。
- **机制拆解**：
  1. **DRed**（Griffin & Libkin，SIGMOD 1995）：删除基表元组→先尝试从其余基表重推导→重推导成功则保留（防误删）；处理的是"删除可能只是备选推导之一"。
  2. **Counting**（Gupta/Mumick/Subrahmanian，SIGMOD 1993）：维护每条视图元组的**推导路径计数**，支持递归视图；精确但计数存储是常数级负担。
  3. **键保持/自维护性**（Chirkova 等）：分析视图是否只需基表增量本身即可维护（无需查基表）——ADV 的"摘要表"若满足自维护性，扫描 diff 就够，无需回读文件。
  4. 与 DBSP 的关系：DBSP 把该谱系统一成代数（微分+积分）；DRed/counting 是"集合论 + 计数"路线。
- **档位**：**吸收**（理论判据）｜**第一步**：adv-index 每张持久摘要表标注「自维护性：是/否」——是的走触发器式维护，否的标"读时重算"。
- **许可证/成熟度**：学术文献；成熟。
- **链接**：Griffin & Libkin SIGMOD 1995；Gupta/Mumick/Subrahmanian SIGMOD 1993

---

## 6. 重点问题回答

**① DBSP/differential 的"算子微分"能否用于 adv-index 增量摘要？——能，且映射表现成：**
| 代码变更 | Z-set 表达 | adv-index 增量算子 |
|---|---|---|
| 新文件/删文件 | (path,+1)/(path,-1) | 目录级文件计数、路径前缀聚合（线性） |
| 文件改写 | (旧hash,-1),(新hash,+1) | 符号表 diff：旧符号集 Δ 出、新符号集 Δ 入（投影算子，线性） |
| 符号增删 | (symbol@file,±1) | 反向引用计数、目录符号密度（线性 reduce） |
| 符号重命名/移动 | join 型变更 | **非线性**：需对侧积分（保留的索引即积分），展开为「Δ左×积分右 + 积分左×Δ右」 |
实现形态：`ZSet<K>` 差分类型 + project/aggregate 两三个线性算子的微内核（估 ~200 行，无外部依赖），只覆盖线性摘要；join 型留给 salsa 重算 + 值重验。**边界**：ADV 不需要部分序时间/分布式，Z-set 之上的时序机制不搬。

**② SAC 的 changeability 对门缓存失效的启示**：把门输入的变化从"布尔脏标记"升级为"一阶 delta 值"（文件集增/删/改三元组、规则集 diff）；失效 = delta 沿门依赖闭包传播（Propagate 原语），聚合类门（统计、覆盖率）用 Reduce 原语增量归并。理论锚：Acar 成本模型——传播成本 ∝ 受影响闭包而非全量（CMU 2005）。落到实现：门缓存 key=输入内容 hash；delta 缺失时（如未知来源变更）回退为"标脏 + 值重验"（早截止兜底）。

**③ rustc dep-graph 拆解与选层**：rustc = 查询实例级 DepNode + 执行期 read 边记录 + 128-bit fingerprint + try-mark-green 惰性着色 + dep 图/结果缓存落盘 + LocalDefId owner 分片；"detailed dep-graph" 只是调试设施（-Z 开关），**rustc 运行时粒度并没有细到语句级**。salsa 与之同源（red-green），差别在 Durability/库形态。**ADV 该学哪层**：学 rustc 的（a）owner 分片→per-file/symbol 锚点，（b）dep 图持久化→跨进程复用（补 salsa 短板），（c）命中率观测；不学编码器流水线与灰节点特例。粒度维持查询级——rustc 30 年经验表明更细粒度收益边际递减且 soundness 风险上升（见 4.1-2）。

**④ 细粒度响应式（signals）对 ADV 规则间依赖的启示**：三条共识机制（leptos reactive_graph、TC39 提案、salsa 三方同构）：依赖**执行期记录**（规则经 context 对象读输入时自动记边，杜绝手写声明漂移）；push-pull（脏下推 + 读取时上拉）；**等值短路**（重算后值相等则截断传播——TC39 的 check/clean 语义）。ADV 规则引擎落法：规则函数只准经 ctx 读，读即记边；memo 结果带 hash，重验相等即绿。

**⑤ 影响集怎么算才不漏（保守上界 vs 精确下界）**：结论——**声音性用保守上界（静态闭包），精度用便宜复验（动态值比对），不追求昂贵精确追踪**。分四点：
1. 上界：dep 图闭包污染（A 变→其全部下游标脏）——rustc/Shake/Bazel 的共同底线；依赖必须是**测出来**的（执行期记录），声明式依赖会漂移（4.1-2 的 Swift 事故即漏检代价锚）。
2. 下界：值重验（fingerprint/内容 hash 相等→截断传播，early cutoff——BSaLC §1.7；等值短路——§3.2）。
3. 中间态：DRed/counting 提供精确影响集但存储/复杂度上税（§5.2），ADV 的表规模（单机索引）不值得；自维护性标注（§5.2-3）是最便宜的精确化。
4. 防漏兜底：低频全量校验门（canary 语料期望值锚到规则之外的观测——与既有质量红线同族）+ 依赖图 verifier（§4.1-1）。**取舍口径：漏检代价是正确性（Swift 内存损坏事故），过度保守代价只是性能**——所以上界宁可宽。

---

## 7. 2025–2026 前沿信号

1. **DBSP 机制外溢**：2026 年第三方项目（Datalog 上下文引擎，GitHub 2026-08 口径）显式采用 "DBSP-style retractions 与插入对称流动"——增量代数正从数据库域进入通用工具域（与 ADV 用法同型）。
2. **Feldera 商业化加速**：2025 年 $15M 融资（Form D 锚），CDC/Iceberg 用例扩张——DBSP 生态的维护风险下降。
3. **增量可观测性成为一等公民**：SwiftPM 2026-06 增量性能调试 pitch（forums.swift.org）；rustc `-Zincremental` 系列长期存在——工具链共识：增量系统必须自带命中率/失效原因输出。
4. **reactive_graph 独立化**：leptos 0.8 把响应式核心拆成独立 crate（docs.rs 0.8.20 在档）——细粒度依赖图可作为库独立评估。
5. **salsa 生态分流形态出现**：`rust-analyzer-salsa` 分离发布包（crates.io，experimental）——上游活跃但存在分流，ADV 锁主线并固定版本。
6. **swift-driver 路线验证 ADV 磁盘侧**：CAS 内容寻址缓存 + Merkle 式键（LLVM RFC 2022-02 起）与"内容 hash + 原子换名"路线重合，且被 2025–2026 持续生产使用。
7. **Materialize 维持小众头部**（builtin.com 2026-08）——differential 路线在商业产品侧未再扩张，机制价值集中在上游库与 DBSP。

## 8. Top-3（按对 ADV 的边际收益排序）

1. **DBSP 算子微分 → adv-index 磁盘侧摘要微内核**（§2.1/§6-①）：Z-set 差分类型 + 线性算子，~200 行把扫描 diff 变成摘要增量维护；join 型不硬微分，交回 salsa。
2. **rustc 选层：owner 分片 + dep 图持久化 + 命中率观测**（§1.5/§1.6/§6-③）：粒度锁查询级，把增量做成"可看见、可跨会话"的系统。
3. **SAC changeability + early cutoff 组成门缓存失效协议**（§1.4/§1.7/§6-②⑤）：delta 一阶值传播（有 delta 走增量）+ 值重验截断（无把握走保守上界）+ 低频全量校验兜底。

---
*登记：`registry/E2.jsonl`（20 条，与本文件对象一一对应）。检索口径限制见文首声明。*
