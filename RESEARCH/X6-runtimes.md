# X6 域调研：并行 / 异步运行时（ADV）

> 调研日期：2026-10-04（检索工具：WebSearch，锚点均标注检索日）。
> 项目前提（承接 W4 组定案）：文件级不规则并行交 rayon（wave-0 07）；有序相位保留自建分块+块序合并+sha256；异步文件 IO 不进扫描内核（compio 只留对照实验位）。
> 本文档只回答 X6 域：**并行栈怎么定、tokio 进不进、epoch GC 何时需要、dashmap 的坑、once_cell 还剩什么**。
> 许可证标注以 crates.io 当前页为准（2026-10-04 检索）；标“(待复核)”者表示本次未逐条核验 LICENSE 文件，落地前补一次。

---

## Top-3（本域定案建议）

**T1 — CPU 并行栈：rayon 是唯一 CPU 并行入口，自建池起步 0 处。**
rayon 覆盖所有“数据并行、无跨任务顺序依赖”的相位（文件级不规则并行正是其甜区）；indexed `par_iter().collect()` 本身保序，sha256 有序相位继续走 wave-0 07 的“分块+块序合并”，不必为保序绕开 rayon。自建池只有三条判据成立之一才立：①线程亲和/NUMA 绑核需求；②自定义调度策略（优先级/专用线程）；③阻塞 IO 与 CPU 阶段必须隔离。旧仓 par.rs 的 spawn+channel+join 范式不吸收（std scoped threads + rayon 已覆盖）。

**T2 — 异步栈：tokio 不进扫描内核；MCP stdio 走同步最小方案；compio 只留对照实验位。**
stdio MCP 是单连接顺序 JSON-RPC，同步读循环即可（方案见条目 X6-21）；引 tokio 的判据是“>1 种传输并存 / 超时取消编排 / async SDK 复用”，满足其一才进 adv-server 进程。compio（IOCP）2026-08 仍活跃发版，按 W4 定案留异步文件 IO 对照实验位；monoio（无 Windows 主线）、glommio（低维护）出局；async-std 官方停更，禁止进依赖树。

**T3 — 同步原语与惰性初始化：std 优先，dashmap 不进索引构建，once_cell 可弃。**
std 同步原语在 Linux（futex，1.62+）/Windows（SRWLock）已不弱，parking_lot 仅当基准示锁热点才引入；dashmap 的“逐分片锁、迭代非一致快照、跨键非原子”与索引构建的“局部 map+一次性有序合并”模式冲突，不用；`LazyLock/OnceLock`（1.70/1.80）覆盖 once_cell 约九成用例，剩余价值只剩 race/OnceBox/no_std——ADV 基本可弃。

---

## 五问五答（对应任务书）

**① ADV 并行栈定案建议？**
rayon 覆盖：文件哈希/规则匹配/内容解析等文件级数据并行相位 + indexed collect 保序的两两归约类相位。自建池只留“零处起步”，判据见 T1；若未来出现绑核或阻塞隔离需求，再评估“rayon 专用池（ThreadPoolBuilder 自建实例）”而非手写线程池——前者保留窃取语义，只隔离资源。

**② tokio 是否进 ADV？**
扫描内核：不进（定案不变）。MCP stdio 服务器：不需要——最小方案为 reader 线程阻塞读 stdin → crossbeam-channel 分发 → rayon/工作线程执行工具 → writer 线程串行写 stdout，超时用 `recv_timeout`。将来若 MCP 之外出现 HTTP/SSE 传输或并发会话，tokio 以 feature-gate 进 adv-server，与扫描内核进程/阶段边界隔离。

**③ crossbeam-epoch GC 机制与 ADV 何时需要？**
机制（条目 X6-06）：pin 临界区 + 线程本地垃圾袋 + 全局 epoch 推进，只回收“pin 时刻两个 epoch 之前”的垃圾，两代滞后保证无悬垂读。ADV 何时需要：仅当引入**无锁共享结构**（lock-free list/queue/map）且节点内存需要回收时。当前“每线程局部 map + 有序合并”设计没有任何无锁共享结构，epoch GC 用不上——watch 档，写进文档防止未来“为了去掉一把锁”而误引入。

**④ dashmap 类 sharded map 的坑与索引构建取舍？**
坑（条目 X6-18）：跨键复合操作非原子、`iter()` 逐分片加锁非一致性快照、持 guard 再碰同分片可自阻塞、写多场景可能劣于整表粗锁。索引构建的语义要求是“确定性、可复现、块序合并”，dashmap 的并发可变共享恰好破坏确定性——取舍：构建期用每线程局部 `HashMap`，合并期单线程归并；dashmap 只在“扫描中途对外并发查询部分结果”这种只读为主的场景才有意义（有界复评位）。

**⑤ LazyLock 标准库化后 once_cell 基本可弃？**
是。证据：`OnceLock/OnceCell`(1.70)、`LazyLock/LazyCell`(1.80) 是 once_cell sync/unsync 两族的直接移植（锚：官方迁移综述 codeandbitters、reddit/extendr 迁移案例）；社区共识“std 类型消除约九成用例”。once_cell 仅剩 race 模块、OnceBox、no_std 三个差异点，ADV 全都不需要。判据式结论：新代码 0 处 once_cell；出现 no_std 或 first-wins 初始化需求再回看（条目 X6-19/X6-20）。

---

## 条目清单（21 条；格式：定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接）

### A. CPU 并行

#### A1. rayon 【X6-01】
- **定位**：数据并行的事实标准库；fork-join + 并行迭代器，work-stealing 负载均衡。
- **可抄机制**：
  1. 窃取拓扑：每 worker 持双端队列，属主从一端 pop（保局部性），窃贼从另一端偷—— victim 尾部是最老任务，偷它能减少活锁并摊平不均衡。
  2. `join(a,b)` 惰性续体：只为另一侧建一个 job；被偷则当前线程阻塞等待，未被偷则直接内联执行——无偷取时 join 退化为顺序调用，这就是“join 开销仅几十–几百 ns 量级”（锚：Matsakis《Rayon: data parallelism in Rust》2015；docs.rs 粒度建议“每任务至少数千 ns 才摊得过来”，精确值需 cap 断面自测）。
  3. indexed `collect()` 分块写回：`IndexedParallelIterator` 的 collect 按索引段并行写预分配缓冲，结果顺序确定——sha256 有序相位的分块+块序合并可以建立在同一骨架上。
- **档位**：【吸收】（文件级不规则并行的唯一入口）。
- **第一步动作**：扫描内核统一走显式 `ThreadPoolBuilder::num_threads(N).build()`（防宿主进程 RAYON_NUM_THREADS 干扰），禁用进程级全局池假设。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频但稳定）。
- **链接**：https://github.com/rayon-rs/rayon · https://smallcultfollowing.com/babysteps/blog/2015/12/18/rayon-data-parallelism-in-rust/

#### A2. rayon 并行迭代器陷阱 【X6-02】
- **定位**：rayon 使用纪律，不是库。
- **可抄机制/陷阱清单**：
  1. 顺序性：只有 indexed 迭代器的 `collect()`/`indexed` 类操作保序；`for_each`/`reduce` 无顺序保证，`reduce` 还要求合并函数满足结合律。
  2. panic 传播：任务内 panic 会传播回 `collect`/`join` 调用方，但“其他任务可能先跑完”——扫描器要在 collect 边界统一收口错误，不能假设 fail-fast。
  3. 阻塞占坑：rayon worker 里做阻塞 IO/`recv()` 会占住线程且无 `block_in_place` 类逃生口——扫描内核只放 CPU 密集段，IO 交异步/专用线程（与 W4 定案一致）。
- **档位**：【有界】（纪律清单，进代码评审检查项）。
- **第一步动作**：把这三条写进 perf-core 的 rayon 使用规范注释。
- **许可证/成熟度**：—（模式条目）。
- **链接**：https://docs.rs/rayon

#### A3. 自定义线程池时机（旧仓 par.rs 范式存废） 【X6-03】
- **定位**：判据条目，不是站队。
- **可抄机制（判据）**：自建池只在以下之一成立时合理：①线程亲和/NUMA 绑核；②自定义调度策略（优先级、专用线程、公平性）；③阻塞 IO 与 CPU 阶段必须池级隔离。spawn+channel+join 手写范式已被 scoped threads + rayon 覆盖，无独有价值。
- **档位**：【不吸收】（旧仓 par.rs 手写范式；判据保留，触发即重评，且优先考虑 rayon 自建实例而非手写池）。
- **第一步动作**：在 perf-core 设计文档记录三条判据；ADV 起步自建池计数=0。
- **许可证/成熟度**：—。
- **链接**：https://doc.rust-lang.org/std/thread/index.html

#### A4. std::thread::scope（scoped threads，1.63+） 【X6-04】
- **定位**：标准库一次性 fork-join；替代手写 join-with-Arc。
- **可抄机制**：scope 守卫保证“scope 内 spawn 的线程在 scope 返回前全部 join”，因此借用栈上数据免 `'static`/`Arc`；scope 内 panic 在边界传播。与 rayon 关系：少量线程的结构化阶段（如“ reader+2 worker”的流水线段）用 scope 更直白；大规模数据并行仍归 rayon。
- **档位**：【吸收】。
- **第一步动作**：有序相位的“分块→合并”两步桥接处优先用 scope 写结构化并发。
- **许可证/成熟度**：Rust 标准库（MIT/Apache-2.0）；1.63（2022-08）起稳定。
- **链接**：https://doc.rust-lang.org/std/thread/fn.scope.html

#### A5. crossbeam-deque 【X6-05】
- **定位**：work-stealing 双端队列原语，rayon-core 的窃取队列底座。
- **可抄机制**：Chase–Lev 变体——worker 端单属主 LIFO push/pop，stealer 端多属主 CAS FIFO steal；`steal()` 返回 Empty/Success/Retry 三态供调用方重试。
- **档位**：【reference】（ADV 不自建池则无直接使用点；理解它=理解 rayon/tokio 的共同机制）。
- **第一步动作**：无；仅作为 A1/T2 机制的底层出处存档。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频）。
- **链接**：https://github.com/crossbeam-rs/crossbeam

### B. 异步运行时

#### B1. tokio（multi_thread 调度器拆解） 【X6-09】
- **定位**：生态主流异步 runtime（锚：v1.50.0，2026-03 发版）；本域关注其调度器机制与“何时需要它”的判据。
- **可抄机制**：
  1. worker 结构：线程 + 本地队列（256 槽）+ 1 个 LIFO slot；同 worker 上被唤醒的任务优先进 LIFO slot（消息传递局部性），调度序 = LIFO slot → 本地队列 → 全局注入队列。
  2. 防饥饿：LIFO slot 连续约 61 次优先执行后强制清空让位（tokio 源码内部常量）；coop 预算：单次 poll 内 128 次调度操作后强制 yield。
  3. 窃取：从受害 worker 本地队列偷一半（与 rayon 的“偷一个”不同，摊平批量不均衡）。
  4. `block_in_place`：把当前 multi_thread worker 转为阻塞线程并让 runtime 补一个新 worker——仅在 multi_thread runtime 可用；这是“异步上下文里必须跑同步重活”的官方逃生口。
- **档位**：【有界】（不进扫描内核；adv-server 出现多传输/超时编排/async SDK 复用时才 feature-gate 引入）。
- **第一步动作**：adv-server 设计文档记录“引 tokio 三判据”；当前 MCP stdio 方案不含 tokio。
- **许可证/成熟度**：MIT；维护中（极活跃）。
- **链接**：https://github.com/tokio-rs/tokio

#### B2. tokio 通道族（mpsc/oneshot/watch/broadcast） 【X6-10】
- **定位**：与 runtime 绑定的异步通道语义参照系。
- **可抄机制（语义差异）**：mpsc=多生产者**单**消费者、有界版用信号量背压；watch=单值最新+多读者（可丢中间值，适合配置版本号/取消信号）；broadcast=全量扇出、慢读者 lag 即断开；oneshot=单次应答。与 crossbeam/flume 的关系（两端点名）：tokio mpsc 对 crossbeam-channel 的差异是“异步唤醒+单消费者 对 同步 MPMC”。
- **档位**：【reference】。
- **第一步动作**：无；adv-server 若引入 tokio 再按语义选型。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://docs.rs/tokio

#### B3. smol 【X6-11】
- **定位**：组件化轻量 runtime（async-io/async-executor/async-net 可单取）；async-std 官方继任者。
- **可抄机制**：生态分层方式——把“事件源（async-io）/执行器（async-executor）/工具”拆成独立 crate，按需取用；适合“不想背整套 tokio”的轻量场景。
- **档位**：【watch】（ADV 当前无 async 需求；若未来嫌 tokio 重且只需单线程事件循环再评）。
- **第一步动作**：无。
- **许可证/成熟度**：MIT/Apache-2.0；维护中。
- **链接**：https://github.com/smol-rs/smol

#### B4. async-std 【X6-12】
- **定位**：曾经的“std 风格异步” runtime。
- **可抄机制**：无（停更项目只剩教训：API 贴 std 不等于生态可持续）。
- **档位**：【不吸收】。官方停更：README/lib.rs 声明 “async-std has been discontinued; use smol instead”，末版 1.13.1（锚：2026-10-04 检索；GitHub issue #1538 于 2025-11 引用官方停更；Fedora 已提案打包弃用）。
- **第一步动作**：依赖审计规则里禁止 async-std 进树（cargo-deny 一把尺可加）。
- **许可证/成熟度**：MIT/Apache-2.0；弃维护。
- **链接**：https://github.com/async-rs/async-std

#### B5. monoio 【X6-13】
- **定位**：ByteDance/CloudWeGo 的 thread-per-core + Linux io_uring runtime；completion-based IO 语义的代表。
- **可抄机制**：completion-based 与 readiness-based 的 API 形态差异——buffer 所有权需提前交给内核（提交即转移），回调拿到的是“已填充的 buffer”；这决定了它无法直接套 std Read/Write 抽象，也决定了移植成本。
- **档位**：【watch】（检索 2026-10-04：未见 Windows/IOCP 主线支持，CloudWeGo 文档页 2025-10 更新；ADV Windows 优先不引入。若出现 Linux 服务端部署形态再评）。
- **第一步动作**：无；watch 触发条件=部署形态变更为 Linux 服务端。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频）。
- **链接**：https://github.com/bytedance/monoio

#### B6. glommio 【X6-14】
- **定位**：Datadog 的 thread-per-core + io_uring runtime。
- **可抄机制**：调度器按时延敏感/不敏感队列拆分并给时延任务配额的思路（可为“扫描进度上报类轻任务不让道重任务”提供参照——仅思路，不引依赖）。
- **档位**：【不吸收】（不支持 Windows + 低维护：0.9.x 线（2024）后未见新版，维护者变动后低活动、未官方归档；锚：2026-10-04 检索）。
- **第一步动作**：无。
- **许可证/成熟度**：MIT/Apache-2.0（待复核）；低维护（介于维护中与弃维护之间，按弃维护对待）。
- **链接**：https://github.com/DataDog/glommio

#### B7. compio（Windows IOCP 对照实验位） 【X6-15】
- **定位**：thread-per-core + completion-based IO runtime；驱动层 Proactor 抽象覆盖三平台。W4 定案的异步文件 IO 对照实验载体。
- **可抄机制**：Proactor 驱动分层——Windows IOCP（GetQueuedCompletionStatus + overlapped 提交）/Linux io_uring/polling 兜底，上层 API 统一；这正是“Windows 上也能做 completion-based”的活样本。
- **档位**：【有界】（仅对照实验位，不进扫描内核）。活跃锚：compio-driver 2026-08-18 仍有发版（crates.io 检索）；README 自述 release 节奏不稳（可能用新 Rust 特性）。
- **第一步动作**：对照实验设计（同步 std fs vs compio 大批量小文件元数据+哈希读）写进 W4 实验清单，跑完归档数据。
- **许可证/成熟度**：MIT/Apache-2.0（待复核）；维护中（实验性节奏）。
- **链接**：https://github.com/compio-rs/compio

### C. 同步原语

#### C1. parking_lot vs std 【X6-16】
- **定位**：锁实现选型。
- **可抄机制**：std 现状——Linux futex 实现（1.62+，锚：rust-lang/rust#93740 跟踪 issue）、Windows SRWLock；parking_lot 相对 std 的剩余差异：无毒化（`lock()` 直接返回 Guard，少一层 Result）、const 构造、`lock_fair()` 公平接口、非竞争路径更快（锚：users.rust-lang.org 2022 共识；HN “Inside Rust's std and parking_lot mutexes – who wins?”）；**竞争下平台相关，且 std 在持续改进、差距收窄**——任何“parking_lot 更快”的说法都要绑平台与负载才有意义。
- **档位**：【有界】（默认 std；cap 断面基准示锁热点才换，换时记录前后基准）。
- **第一步动作**：无预引入；在基准清单里加“锁热点”观测点。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频）。
- **链接**：https://github.com/Amanieu/parking_lot

#### C2. Mutex 公平性与 RwLock 写饥饿 【X6-17】
- **定位**：判据/机制条目。
- **可抄机制**：std Mutex 无公平性保证（允许 barging，策略随平台实现——SRWLock/futex 都偏吞吐）；std RwLock 读写优先级**未指定**，Windows SRWLock 偏读者，持续读者流可能饿死写者（“可能”，非绝对，平台相关）；parking_lot 默认非公平+可选 `lock_fair()`，其 RwLock 在写者排队时让新读者等待（防写饥饿，以其文档为准）。
- **档位**：【reference】。
- **第一步动作**：索引读取路径若出现“长读持锁”，优先用分片/快照拆解而非换锁。
- **许可证/成熟度**：—。
- **链接**：https://doc.rust-lang.org/std/sync/struct.RwLock.html

#### C3. dashmap（sharded map 机制与坑） 【X6-18】
- **定位**：并发 sharded HashMap；理解“分片锁范式”的收益与代价。
- **可抄机制**：固定分片数组（默认分片数 ≈ 4×逻辑核数向上取 2 的幂，源码默认、以版本为准），每分片一把 std RwLock + 一个 HashMap；`get()` 返回持分片读锁的 Ref。
- **坑（四个都实测可复现的机制性坑）**：①跨键复合操作非原子——`get`+`insert` 两步之间同键可被其他线程改（entry API 也只锁单分片）；②`iter()` 逐分片加锁，**非一致性快照**——迭代期间他人的写入会呈现“部分新部分旧”；③持 Ref guard 期间再操作同一分片可自阻塞/死锁；④写多场景分片锁竞争+guard 开销可能劣于整表粗锁（读多写少才是甜区）。
- **档位**：【有界】（索引构建不用；仅“扫描中并发查询部分结果”的只读为主场景复评，且需接受迭代不一致）。
- **第一步动作**：索引构建按“每线程局部 HashMap → 单线程有序归并”实现，在文档写明该决定依据本条。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://github.com/xacrimon/dashmap

#### C4. std OnceLock/LazyLock（1.70/1.80 标准库化） 【X6-19】
- **定位**：零依赖的线程安全惰性初始化。
- **可抄机制**：`OnceLock` = Once + Option<T>（首次 init 阻塞其他 get，1.70 稳定）；`LazyLock` = OnceLock + 延迟闭包（1.80 稳定）；单线程版 `std::cell::{OnceCell, LazyCell}` 同期可用。init panic 后的再访问行为以 std 文档为准（本次未深挖，不写死）。
- **档位**：【吸收】（替代 once_cell 的 sync/unsync 两族）。
- **第一步动作**：adv 全仓静态/惰性配置统一用 `LazyLock`；`cargo deny`/lint 提示 once_cell 新增即拦。
- **许可证/成熟度**：Rust 标准库；1.70（2023-06）/1.80（2024-07）稳定。
- **链接**：https://doc.rust-lang.org/std/sync/struct.LazyLock.html

#### C5. once_cell 剩余价值评估 【X6-20】
- **定位**：被 std 收编后的存量库。
- **可抄机制（剩余差异点，仅三件）**：①`race` 模块——first-wins 不阻塞初始化语义（std OnceLock 是阻塞等待语义，无对应物）；②`OnceBox`——无 `T: Send/Sync` 约束的 `OnceCell<Box<T>>`；③no_std(+alloc) 目标支持。证据锚：codeandbitters《Once Upon a Lazy Init》、多个下游迁移案例（extendr issue #579 等），“std 类型消除约九成用例”为检索综述口径。
- **档位**：【不吸收】（基本可弃；触发回看条件=未来 no_std 目标或需要 race 语义）。
- **第一步动作**：新代码 0 处 once_cell；既有依赖在下次依赖清理时移除。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（稳定低频）。
- **链接**：https://github.com/matklad/once_cell

### D. 通道与进程边界

#### D1. crossbeam-channel 【X6-07】
- **定位**：同步 MPMC 通道基准常青树；ADV 内部线程分发的默认选型。
- **可抄机制**：unbounded 走链表、bounded 走数组环+状态字（send/recv 用状态字原子翻转完成等待/唤醒）；`select!` 宏支持多路+超时，随机顺序保证公平。
- **成熟度警示（重要锚）**：RUSTSEC-2025-0024——0.5.12 为修内存泄漏引入的 unbounded 通道 Drop 双重释放，0.5.15 修复，2025-04 披露；更早有 RUSTSEC-2020-0052（bounded Drop，0.4.4 修）。结论：可用，但**锁 ≥0.5.15**，且 cargo-deny 覆盖它。
- **档位**：【吸收】（MCP stdio 方案与扫描相间队列用）。
- **第一步动作**：依赖清单锁 `crossbeam-channel >=0.5.15`。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频但在修——两次 RUSTSEC 均有修复动作）。
- **链接**：https://rustsec.org/advisories/RUSTSEC-2025-0024.html

#### D2. flume 【X6-08】
- **定位**：MPMC + 同步/异步双 API 通道。
- **可抄机制**：同一类型同时提供阻塞与 async API（`send_async`/`recv_async`）+ Selector 多路选择，内部 spin fast-path；这是“跨同步/异步边界只需要一个队列类型”时的解法。基准口径（锚：rustycloud.org 通道综述 + fereidani/rust-channel-benchmarks）：bounded 场景与 crossbeam-channel 互有胜负，unbounded MPMC 略慢，无单方面碾压。
- **档位**：【有界】（仅在出现“同一队列两端分别是同步代码与异步代码”时选它；纯同步用 crossbeam-channel）。
- **第一步动作**：无预引入；对照实验若跑 compio，可顺带测 flume 跨界队列形态。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（低频）。
- **链接**：https://github.com/zesterer/flume

#### D3. MCP stdio 服务器最小方案（sync vs async 判据） 【X6-21】
- **定位**：adv-server 的 MCP 传输形态定案建议。
- **可抄机制（拓扑）**：stdio MCP = 单连接、顺序 JSON-RPC——专用 reader 线程阻塞读 stdin → crossbeam-channel 分发 → rayon/工作线程执行工具 → writer 线程串行写 stdout（**stdout 必须单写者**，防多线程交叠破坏 JSON 帧）；请求超时用 `recv_timeout`，结果以请求 ID 关联。
- **需要 async 的判据（满足其一才引 tokio）**：①>1 种传输并存（HTTP/SSE/WebSocket）；②单请求需要超时/取消编排树；③要复用的生态 SDK 只有 async 版。三条都不满足时引 async 是纯负担（runtime、Send/Sync 约束传染、栈上状态机化）。
- **档位**：【吸收】。
- **第一步动作**：MCP 实现按本拓扑起最小骨架，标注三条判据注释在代码入口。
- **许可证/成熟度**：—（模式条目；MCP 规范 stdio 传输为官方形态）。
- **链接**：https://spec.modelcontextprotocol.io/

---

## 检索锚点（2026-10-04）

| 锚 | 内容 | 来源 |
|---|---|---|
| async-std 停更 | “discontinued; use smol instead”，末版 1.13.1 | 官方 README 转述 + GitHub issue #1538（2025-11 引用） |
| compio 活跃 | compio-driver 发版 2026-08-18；IOCP/io_uring/polling | crates.io 检索快照 |
| glommio 低活动 | 0.9.x（2024）后未见新版；未官方归档 | 多轮检索无新发版证据（否证性证据，留时间戳） |
| monoio 无 Windows | CloudWeGo 文档页 2025-10 更新，无 IOCP 支持 | 检索快照 |
| tokio 成熟度 | v1.50.0（2026-03） | 检索快照 |
| std 锁实现 | Linux futex（1.62+）/Windows SRWLock | rust-lang/rust#93740 + HackMD std 锁设计文档 |
| 通道基准 | crossbeam 同步领先、flume bounded 互有胜负、tokio mpsc 语义绑定 runtime | fereidani/rust-channel-benchmarks + rustycloud.org 综述 |
| crossbeam-channel 安全 | RUSTSEC-2025-0024（0.5.12 引入，0.5.15 修）+ RUSTSEC-2020-0052（0.4.4 修） | rustsec.org |
| rayon 开销量级 | join 数十–数百 ns；任务粒度建议 ≥数千 ns | Matsakis 2015 + docs.rs（精确值需本地复测） |
| once_cell 余值 | race/OnceBox/no_std 三点；std 覆盖约九成 | codeandbitters + 迁移案例综述 |

> 口径声明：以上数字均为检索到的量级/口径，ADV 落地时以本地 cap 断面复测为准；否证结论（如“monoio 无 Windows 支持”）按检索日打时间戳，非永久事实。
