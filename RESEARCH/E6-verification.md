# E6 形式化验证与约束求解（调研日期 2026-10-03）

> 域目标：不追求全程序证明，找"验证思想进扫描器/规则引擎"的可用件。
> 证据边界：本会话 curl 与 WebFetch 均不可用（GitHub API 无网络出口、WebFetch 证书校验失败），一手状态取证受限；下述维护状态/许可证均标口径（搜索快照日期 2026-10-03 / crates.io 快照 / 论文日期），判"弃维护"一律不成立——缺一手归档证据。

---

## 1. 对象档案（18 个）

### A 组：Rust 验证器

#### A1. Verus
- **定位**：SMT 半自动 Rust 演绎验证器（CMU/VMware Research，Dafny 血统）；规格写 `requires/ensures/invariants/decreases`，代码进 `verus!` 宏与 `proof/ghost` 层，函数级生成验证条件（VC）交 Z3。支持 safe Rust 及部分 unsafe（裸指针、RefCell，搜索快照 2026-10-03，ResearchGate/EPFL 口径）。
- **可抄机制**：① 函数级 VC 生成——把"前置/后置/不变量"翻译成 SMT 查询的流水线形态（解析规格 → 编码函数体 → Z3 check），adv-rules 的规则校验可对标；② ghost/exec 双层代码——纯逻辑量与可执行量分离，规则引擎的"触发前提"可以用 ghost 层表达而不进运行时；③ stdlib 验证社区（Verustd，Cargo/跨 crate 支持在建，GitHub 快照）。
- **档位**：【有界】——ADV 用它自证核心模块可行，但代码需 `verus!` 标注、工具链绑定，不可能全仓吸收。
- **第一步动作**：挑 1 个纯算法模块（如路径敏感的 fixpoint 迭代器）用 Verus 写规格跑通 VC→Z3 全链，量"每人日产出证明行数"再定去留。
- **许可证/成熟度**：MIT（crates.io 口径，待一手复核）；**维护中**（crates.io 快照：verus_builtin_macros 有 2026-04-18 发布记录；Verustd 活跃；dev.to 指南 2025-12）。
- **链接**：https://crates.io/crates/verus · https://github.com/verus-lang/verus · https://github.com/KaminariOS/Verustd

#### A2. Creusot
- **定位**：Rust 演绎验证器，MIR → Why3/WhyML 编码，Pearlite 规格语言（契约、逻辑函数、refinement 类型），Why3 分派 Alt-Ergo/Z3/CVC 多证明器；默认检查无 panic/无溢出/断言不失败（crates.io 描述口径）。
- **可抄机制**：① **规格以 Rust attribute 形态存在**（`#[requires]/#[ensures]` + Pearlite 宏），代码保持普通 Rust、普通 cargo 可编译——比 Verus 的 `verus!` 侵入低，是"契约进仓库"的更好载体；② refinement 类型（逻辑层精化物理类型）；③ Why3 多证明器分派（一个 VC 多家求解器互相兜底）。
- **档位**：【有界】——ADV 自证核心模块的首选候选（侵入低）；代价是 rustc nightly 版本钉死（rustc 内部 API 耦合，历史普遍问题）。
- **第一步动作**：对照 Verus 跑同一个模块，产出"Verus vs Creusot 自动化率/标注量"对照表（先量后改）。
- **许可证/成熟度**：repo LICENSE 待一手复核（本会话无法访问 GitHub）；**维护中**（devlog.creusot.rs 2026-01-19 "Creusot 0.9.0"；creusot-std 0.13.0 发布于约 2026-01-10，libraries.io 快照；Coma 论文 2026-08-24）。
- **链接**：https://devlog.creusot.rs · https://guide.creusot.rs · https://github.com/creusot-rs/creusot

#### A3. Prusti
- **定位**：ETH Zurich 的 Rust 静态验证器，Viper 后端；规格写注释/attribute，默认验证无 panic 与 UB；flow-sensitive 细化类型推断。
- **可抄机制**：① folding → encoding → Viper 三段式权限编码（functional permission）；② **panic-free-by-default**——把"无 panic"当默认契约而非附加证明，ADV 的"panic-free 论证"直接对标此设计；③ Viper 的 SnapShot/fold-unfold 机制（见 A5）。
- **档位**：【watch】。
- **第一步动作**：不加依赖；订阅 repo release，若出现 2026 下半年 release 再复评。
- **许可证/成熟度**：Apache-2.0 / MIT 双许可（二手口径，待一手复核）；**维护信号弱，不判弃维护**——证据：crates.io `prusti-contracts` 停在 0.2.0（2023 年中发布，lib.rs 快照口径），2024–2025 未见新 release 迹象（搜索快照 2026-10-03）；但"repo 改名 viperproject/prusti、开发延续到 2024–2025"的说法同时存在（同批搜索结果），两个信号矛盾，须一手复核后才能定档。
- **链接**：https://lib.rs/crates/prusti-contracts · https://viperproject.github.io/prusti-user-guide/ · https://github.com/viperproject/prusti-dev

#### A4. Kani（补充：契约模式）
- **定位**：Rust 位精确模型检查器（CBMC 血统 + SMT 后端），harness 驱动；契约模式提供 `#[kani::requires]/#[kani::ensures]/#[kani::proof_for_contract]`，契约可当 stub 用于模块化验证（Ferrous Systems 培训教材口径）。Linux kernel Rust（zerocopy）已在真实代码里用 `kani::requires/ensures`（kernel docs 快照）。
- **可抄机制**：① **契约即 stub**——上层 harness 验证时用被调函数的契约替代其实现，这是"逐函数契约进扫描器"最直接的参考实现；② `proof_for_contract` 把"实现满足契约"与"调用方依赖契约"拆成两个独立检查点；③ KaPilot（arXiv 2025+）用多智能体 LLM 生成 Kani 规格——规格书写成本的自动化信号。
- **档位**：【吸收】——wave-0 03 已定 Kani 周期档；本域补充：把契约模式纳入该档（核心模块 requires/ensures + panic-free harness）。
- **第一步动作**：在 wave-0 03 的周期档里加一个契约 harness 样例（挑 1 个 ADV 核心函数写 requires/ensures，跑 `proof_for_contract`），验证契约检查在 CI 的耗时档位。
- **许可证/成熟度**：Apache-2.0 OR MIT（文档口径）；**维护中**（Linux kernel 采用、Ferrous 培训教材、KaPilot 2025+ 论文均为活跃信号）。
- **链接**：https://model-checking.github.io/kani/ · https://rust-training.ferrous-systems.com · https://arxiv.org（KaPilot）

#### A5. Viper（Prusti 后端基础设施）
- **定位**：ETH 的验证基础设施（Silicon/Karma 编码器），权限式函数式编码，不限于 Rust 前端。
- **可抄机制**：① fold/unfold 权限断言的表达式级编码——规则引擎里"这条规则消耗什么上下文、产出什么事实"可用同构思路建模；② 独立后端意味着前端死了后端还活着（Prusti 若真弃维护，Viper 是重建前端的底座，但成本高，仅备案）。
- **档位**：【reference】。
- **第一步动作**：无；仅在 ADV 规则引擎需要权限/资源语义建模时回来翻编码论文。
- **许可证/成熟度**：待一手复核（Viper 官网 LICENSE）；**维护中**（Prusti 后端仍在引用）。
- **链接**：https://www.viperproject.org

#### A6. Why3 / Coma
- **定位**：Creusot 的后端；Coma 是 Why3 新一代中间验证语言（显式控制流 + 程序变换式 VC 生成，论文 2026-08-24）。
- **可抄机制**：Coma 的"VC 生成 = 程序变换到纯逻辑"——与 ADV 把 AST 规则变换为 SMT 断言同构；多证明器分派策略（fast prover 先试、慢 prover 兜底）可直接抄进验证档调度。
- **档位**：【reference】。
- **第一步动作**：无。
- **许可证/成熟度**：LGPL（二手口径，待一手复核）；**维护中**（Coma 论文 2026-08）。
- **链接**：https://www.why3.org · Coma 论文（ResearchGate 2026-08-24 快照）

### B 组：SMT 求解器与增量模式

#### B1. Z3
- **定位**：微软 SMT 求解器，Rust 侧有 `z3` crate；ADV 验证档的默认引擎候选。
- **可抄机制**：① **push/pop 作用域 + 命名假设字面量**——`check-sat` 可带 assumption 字面量，`get-unsat-core` 返回导致不可满足的最小假设集（需 `:named` 标注）；② `get-model` 从 sat 结果提取具体模型（反例具体化）；③ 增量 API（`z3` crate 暴露 Solver::push/pop/assert/check）。
- **档位**：【吸收】——作为验证档引擎进 xtask-gates（可选档，非每次构建）。
- **第一步动作**：`z3` crate 写一个最小 demo：10 条候选规则前提作为假设一次性 check，取 unsat core 解释规则冲突，量单实例批量 vs 每规则单进程的耗时差。
- **许可证/成熟度**：MIT；**维护中**（长期活跃，MIT 口径为 repo 公开事实）。
- **链接**：https://github.com/Z3Prover/z3 · https://crates.io/crates/z3

#### B2. Bitwuzla（Boolector 合流的增量模式补充）
- **定位**：Boolector 的后继（2018 fork 起，Boolector 终版 3.2.4 后不再维护）；位向量/浮点/数组强项，SMT-COMP 2023/2024 QF_FPA 全类别冠军（官网口径）。
- **可抄机制**：① **solving under assumptions + push/pop**（docs 有 pushpop.smt2 增量示例）；② 2025-era release 给 Kissat 加了增量支持（此前每个 SAT 查询单独开实例）——"增量 = 复用实例还是重开实例"是个可量化的调度决策，这个先例说明两档并存是常态；③ BTOR2 输入（与硬件/位级模型检查互通）。
- **档位**：【watch】——R2 域已拆求解器，本域只补增量模式口径；若 ADV 的 SMT 查询被证明位向量主导（unsat core 显示 QF_BV），再换装。
- **第一步动作**：在 Z3 验证档跑通后，用同一查询集对 Bitwuzla 做 30 秒级 benchmark，记录位向量占比。
- **许可证/成熟度**：MIT（二手口径，待一手复核）；**维护中**（2025-era releases；2026 Springer 章节讲其并行验证）。
- **链接**：https://bitwuzla.github.io · https://github.com/bitwuzla/bitwuzla/releases

#### B3. cvc5
- **定位**：Stanford/EPFL 系 SMT 求解器，全理论覆盖 + 插值/unsat core 产出。
- **可抄机制**：① `--produce-unsat-cores`（SMT-COMP unsat core 赛道常客，快照 2026-10-03）；② **增量模式正确性坑位证据**——官方 issue #11001"增量求解怎么才算对"（2024+）说明增量语义边界仍在打磨，生产用法应：每文件新实例 / 优先 assumptions 而非跨查询复用 push 栈；③ 非线性算术的 unsat core 线性化（ResearchGate 论文口径）。
- **档位**：【reference】——机制与 Z3/Bitwuzla 重叠，先留作多证明器分派的第三家。
- **第一步动作**：无；仅在 Creusot 档启用多证明器时作为 Why3 的分派目标自然进入。
- **许可证/成熟度**：BSD-3（二手口径，待一手复核）；**维护中**（Neural cvc5 arXiv 2025-01-16；SMT-COMP 持续参赛）。
- **链接**：https://cvc5.github.io · https://github.com/cvc5/cvc5/issues/11001

#### B4. SMT 增量求解模式（push/pop 假设-检查-回溯）——模式对象
- **定位**：把 N 条候选违规（或 N 条污点路径约束）编码进**单个**求解器实例的批检查模式，服务 adv-taint 路径可行性 + test-replay 最小反例。
- **可抄机制**：① **假设-检查-回溯循环**：`push → assert 假设 H1..Hn（各配 named literal）→ check → unsat ⇒ get-unsat-core 得最小冲突假设子集（用于解释"哪几条规则/路径条件互相矛盾"）→ pop`；sat ⇒ `get-model` 具体化输入喂给 test-replay 重放；② unsat core 当**规则冲突解释器**——两条规则不能同时触发，core 给出证据链而非"报错"；③ 模型提取当**最小反例生成器**——solver 给的输入赋值就是复现用例，直接落进测试。
- **档位**：【吸收】。
- **第一步动作**：B1 的 demo 即本模式的第一步；验收判据 = 单实例批量检查在 N≥10 时耗时低于 N 次独立进程，且 unsat core 能正确指出注入的矛盾规则对。
- **许可证/成熟度**：模式不适用许可证；成熟度依附 Z3（维护中）。
- **链接**：同 B1/B2/B3（Bitwuzla pushpop.smt2 示例是模式的最小文档锚）。

### C 组：轻量验证思想

#### C1. 契约式设计（DbC）
- **定位**：Eiffel 血统的逐函数前置/后置/不变量三件套；现代载体 = Kani/Prusti/Creusot 的 requires/ensures（A2/A3/A4 已证三条独立实现路线）。
- **可抄机制**：① **前置条件 = 规则触发前提**：ADV 每条规则声明 `pre`（文件/AST 形态必须先成立才允许报这条规则）——防误报的机械闸，不需要求解器；② **后置条件 = 建议可检查事实**：规则给出 fix 建议时声明 `post`（补丁应用后某断言成立，如"无新警告""仍可解析"），在测试语料上机械校验，防止规则产出编不过的补丁；③ **不变量 = visitor 上下文断言**：遍历期间对上下文状态的断言（如"处于同一条函数体内"），debug_assert 落地。
- **档位**：【吸收】——纯思想、零依赖，直接进规则引擎数据模型。
- **第一步动作**：给 adv-rules 的规则 schema 加 `pre`/`post` 两个可选字段，先只对 3 条存量规则补齐并在语料上校验 post。
- **许可证/成熟度**：思想不适用；成熟度 = 三条现代实现路线均维护中（见 A2/A3/A4）。
- **链接**：A2/A3/A4 各链接。

#### C2. Typestate 模式
- **定位**：把状态机编码进类型（状态 = 类型参数、转移 = 消费 self），非法 API 调用变编译错误；GitHub `typestate` topic 2025-10-04 仍在更新，含"宏辅助构造 typestate API"与"sealed 状态机"等 Rust 项（搜索快照口径）。
- **可抄机制**：① 扫描器管线的阶段编码为类型（`Raw→Parsed→Analyzed`，越阶调用编译不过）；② 规则引擎的"规则生命周期"（draft→validated→active）用 typestate 防跳步；③ 对外 SDK（若 ADV 出嵌入 API）防误用的第一道墙。
- **档位**：【吸收】——设计纪律，无代码依赖。
- **第一步动作**：在 ADV API 设计纪律文档里加一条"跨阶段状态用类型参数编码"，并在现有 builder 上挑一处试点。
- **许可证/成熟度**：模式不适用；生态**维护中**（topic 更新 2025-10）。
- **链接**：https://github.com/topics/typestate

#### C3. Panic-free 论证（ownership 体系外的轻量证明）
- **定位**：不证正确性、只证"不炸"——Prusti 默认验证无 panic（A3）、Creusot 默认无 panic/溢出/断言失败（A2）、Kani harness 可证 harness 输入域上无 panic（A4）；UBC 本科项目已在批量统计 crates.io 函数可被 Prusti 证明 panic-free 的比例（cs.ubc.ca 快照）——说明该档位有工具支撑且可规模化。
- **可抄机制**：① **档位化验证目标**：正确性（贵）→ 无 panic/无溢出（中）→ 测试（便宜），ADV 门禁按模块重要性分配档位；② debug_assert + overflow-checks profile 是零工具链的最低档；③ Kani/Creusot 是该档的机械验证档。
- **档位**：【吸收】。
- **第一步动作**：xtask-gates 加"核心模块 panic-free 档"：先 overflow-checks+debug_assert 全量开，再对 1 个核心模块跑 Kani harness。
- **许可证/成熟度**：不适用；依附 A2/A3/A4。
- **链接**：同 A2/A3/A4。

### D 组：模型检查与运行时验证

#### D1. TLA+ / PlusCal
- **定位**：Lamport 系状态机规格语言（Spec = Init ∧ □[Next]_vars）+ TLC 显式状态模型检查器；PlusCal 是转译到 TLA+ 的算法记法。
- **可抄机制**：① 不变量写成状态谓词（合并队列的"队列内无重复 PR""取消后不得合并"）；② TLC 穷举可达状态 + deadlock 检测；③ 规格先于实现——adv ci-ops 合并队列（enqueue→CI 起→CI 过→rebase 检查→merge，含取消/跳过事件）约 10^3–10^6 可达状态量级，TLC 吃得动。
- **对合并队列协议的适用性判据**：事件集合**封闭**且状态数有限 → 适用；事件开放（外部 CI 时序不可枚举）或需要与实现持续同步 → 不适用，规格漂移是主要成本。2025 前沿信号：有"从代码自动生成 TLA+ 规格 / 接入 CI/CD"的进行中工作（dou.ua 文章口径），自动化程度决定漂移成本能不能压下来。
- **档位**：【watch】——一次性 spec 产物、不进常规 CI；协议结构变更时重跑。
- **第一步动作**：给合并队列写 50 行 PlusCal（3 个不变量 + 1 个 deadlock 检查），只做一次性校验，产出"发现的设计缺陷数"作为是否续用的判据。
- **许可证/成熟度**：MIT（tla2tools）；**维护中**（社区示例仓、自动生成工作 2025 口径）。
- **链接**：https://lamport.azurewebsites.net/tla/tla.html · https://github.com/tlaplus

#### D2. Alloy 6
- **定位**：一阶关系逻辑 + 有限范围可满足性检查（模型寻找器）；Alloy 6 原生时序逻辑（"it's about time"，Hillel Wayne），时序模型检查可归约为 Alloy 一致性问题（Vakili/Waterloo；Cunha 有界时序 BMC）；有免费书《Formal Software Design with Alloy 6》（haslab）。
- **可抄机制**：① **一致性检查 = 反例不存在性检查**：把 adv-rules 规则集建成关系（rule→scope、rule→severity、rule→precedence），断言"不存在两条规则在同一 scope 上矛盾地都触发"，检查器要么证明范围内无反例、要么给出反例（=冲突规则对）——**反例输出直接就是规则冲突报告**，与 B4 的 unsat core 互为印证；② scope 边界即检查预算（规则条数 × 关系深度）；③ 静态关系检查不需要时序逻辑，Alloy 5 能力已够，6 的时序留给"规则应用顺序"类断言。
- **对规则一致性的适用性判据**：规则集是**静态关系结构**且条数有限（adv-rules 数百条以内）→ 适用；若规则语义含不可编码的浮点/外部 IO → 不适用。范围有界性必须显式声明，否则检查是无界的。
- **档位**：【有界】——离线工具，进 xtask-gates 作低频一致性门（规则集变更时跑，非每次构建）。
- **第一步动作**：把 10 条规则的手写冲突对建成 Alloy 模型验证能找出注入的冲突，再谈接 CI。
- **许可证/成熟度**：MIT（待一手复核）；**维护中**（alloytools.org、免费书 2023+）。
- **链接**：https://alloytools.org · https://www.hillelwayne.com/post/alloy6/ · https://haslab.github.io/formal-software-design/

#### D3. 运行时验证（monitor 生成）
- **定位**：从时序逻辑公式生成逐事件监控器：过去时 LTL 常量内存、逐事件出裁决（detection）；MLTL（有界窗口）给未来时需求一个有界版本。
- **对 adv-sandbox 策略执行的判据**（四条，须同时满足才判可承载）：① 策略可表达为**事件流上的过去时公式**（或 MLTL 有界窗口内）——sandbox 观测的是已发生的系统调用/API 事件序列，过去时天然匹配；② 裁决必须是**拒绝钩子（veto）**语义——monitor 只能 detect+veto，不能修复，策略引擎要接受"事后拦截"而非"事前阻止"的档位；③ 每事件内存预算可测且封闭（过去时 monitor O(1)~O(log n)，公式数 × 窗口宽度决定常数）；④ 策略数量与事件率的乘积有实测上限。四条里①②是形态约束，③④是性能判据，先量后改。
- **可抄机制**：① LTL→监控器翻译的固定管线；② R2U2 的 MLTL 时间窗（对齐 sandbox 的滑动窗口语义）；③ 裁决与动作分离（monitor 出三值裁决：violation/satisfaction/pending）。
- **档位**：【有界】。
- **第一步动作**：把 adv-sandbox 现有 3 条策略改写成过去时 LTL，手写 monitor 对拍原实现的判定结果，测每事件内存。
- **许可证/成熟度**：思想不适用；**维护中**（RV 社区持续活跃）。
- **链接**：https://en.wikipedia.org/wiki/Runtime_verification · R2U2 论文（RV 2015 及后续）

#### D4. R2U2
- **定位**：NASA 出身的模块化 RV 框架，面向资源受限环境实时监控 MLTL/LTL 规格集；曾用于 UAS 安全威胁监控（RV 2015 论文起）；核心已用 Rust 重写（R2U2 Rust port，搜索快照口径）。
- **可抄机制**：① MLTL 有界窗口监控（未来时算子必须有界——正是 D3 判据①的实现范本）；② 规格编译为常量内存监控器的 C/Rust 代码生成；③ 多规格集并行监控的调度。
- **档位**：【watch】——若 D3 第一步的对拍通过，R2U2 是现成实现候选而非自研。
- **第一步动作**：clone Rust port，跑其自带样例，量"单事件裁决耗时"（这个数字决定它进不进 sandbox 热路径）。
- **许可证/成熟度**：仓内 LICENSE 待一手复核（NASA 开源出身）；**维护信号待复核**（Rust port 存在，2025 活跃度未见一手证据）。
- **链接**：https://github.com/nasa-sw-vnt/R2U2

### E 组：自动化信号

#### E5. KaPilot（LLM 生成 Kani 规格）
- **定位**：多智能体 LLM 框架自动生成 Kani 规格（验证 unsafe Rust 内存安全），arXiv 2025+（ACM/ResearchGate 镜像）。
- **可抄机制**：规格草稿生成 → 机械验证兜底（LLM 提议 requires/ensures，验证器裁决真伪）——与 E6 域"验证思想进扫描器"的规模化路径直接相关：契约书写成本若由 LLM 摊薄，规则级契约（C1）的覆盖面可以扩一个量级。
- **档位**：【watch】。
- **第一步动作**：无依赖动作；当 C1 的 pre/post 字段落地后，用 LLM 批量起草存量规则的 post 断言、语料机械校验（这正是"提议-裁决"闭环）。
- **许可证/成熟度**：论文不适用；**实验**。
- **链接**：https://arxiv.org（KaPilot 检索 2026-10-03）

---

## 2. 重点问题判定（①–⑤）

**① Verus/Creusot/Prusti 2026 生存状态与适用档**：Verus 与 Creusot 均活跃且可锚（Verus：crates.io 2026-04 发布记录快照；Creusot：devlog 2026-01-19 + creusot-std 0.13.0 2026-01-10）。适用判据：要"规格进仓库、代码保持普通 Rust、普通 cargo 可编译"→ Creusot（侵入最低）；要"自动化率最高、SMT 集成最深、社区最大"→ Verus（代价：`verus!` 标注 + 工具链绑定）。Prusti 两个二手信号矛盾（crates.io 0.2.0 停在 2023-中 vs "开发延续到 2024–2025"），缺一手归档证据，不判弃维护，定 watch。**结论：ADV 自证核心模块 = Creusot 首选、Verus 对照，两个都先在单个模块上量自动化率与标注量再定。**

**② TLA+/Alloy 对两个协议的适用性判据**：合并队列状态机 → TLA+ 适用的前提是事件封闭 + 状态有限（判据见 D1）；其规格漂移成本高，建议一次性 spec（watch）。规则一致性 → Alloy 6 适用的前提是规则集为静态关系结构 + scope 有界（判据见 D2），且反例输出直接构成冲突报告，与 SMT unsat core 互证，判有界吸收进 xtask-gates 低频档。

**③ SMT push/pop 在扫描器内的模式**：单实例批检查：`push → 假设集（named literals）→ check → unsat ⇒ unsat-core 定位最小冲突假设子集 → pop`；sat ⇒ `get-model` 具体化反例喂 test-replay。两条边界：cvc5 issue #11001 表明增量语义仍有坑 → 每文件新实例、优先 assumptions；Bitwuzla 给 Kissat 加增量的先例说明"复用实例 vs 重开实例"要按 workload 量化选档。

**④ RV monitor 能否承载 adv-sandbox 策略执行**：能，但有四条判据（D3）：过去时可表达、veto 语义可接受、每事件内存封闭可测、策略数×事件率有实测上限。任一不满足则退回"检测+告警"档而非"执行"档。

**⑤ DbC 契约进规则引擎的形态**：是——前置条件 = 规则触发前提（防误报机械闸），后置条件 = 建议应用后的可检查事实（防产出编不过的补丁，语料上机械校验），不变量 = visitor 上下文断言。Kani/Prusti/Creusot 三条独立实现证明该形态在 Rust 生态可行；LLM 起草 + 机械校验（KaPilot 模式）是扩覆盖的低成本路径。

## 3. 2025–2026 前沿信号
1. **Rust stdlib 验证社区多工具协作**（arXiv 2025 "Lessons Learned…"；Verustd）——验证器生态向"逐函数契约 + 模块化 + 多工具分工"收敛，与 ADV 规则引擎的逐规则契约形态同构。
2. **LLM×验证加速规格书写**：KaPilot（Kani 规格，arXiv 2025+）、Verus LoRA 微调数据集（Zenodo 2026-07）——契约书写成本在快速下降，"契约进仓库"的经济性拐点信号。
3. **Creusot devlog 创刊**（2026-01-19）+ Coma 论文（2026-08-24）——演绎验证栈活跃且中间层在换代。
4. **Bitwuzla 增量 Kissat 支持**（2025-era releases）+ 2026 Springer 并行验证章节——位级求解器的增量/并行能力补齐。
5. **Neural cvc5**（arXiv 2025-01-16）——ML 引导搜索进主线求解器。
6. **Typestate 宏生态更新**（GitHub topic 2025-10-04 快照）——类型层防误用持续有 Rust 落地件。
7. **Prusti crates.io 停滞**（0.2.0，2023-中口径）——生态注意力移向 Verus/Creusot 的间接信号（待一手复核）。

## 4. Top-3
1. **Kani 契约模式 + panic-free 门**【吸收】——最低成本把验证思想放进 xtask-gates：核心模块 requires/ensures + `proof_for_contract` + 无 panic harness；Linux kernel 已有真实用例可抄。
2. **Creusot**【有界】——核心模块自证首选：规格以 Rust attribute 存在、代码保持普通 Rust、2026 活跃度有一手快照锚；先在单模块与 Verus 对照量自动化率。
3. **SMT 增量求解模式**【吸收】——push/pop + assumptions + unsat core + model 四件套进 adv-taint 路径可行性与 test-replay 最小反例；unsat core 兼作规则冲突解释器。
次级：DbC 三件套映射进规则 schema（吸收，零依赖）；Alloy 6 规则一致性低频门（有界）。
