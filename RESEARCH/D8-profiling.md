# D8 — Profiling 与可观测性（Windows 优先）

> 调研日期：2026-10-03。口径：WebSearch 为主（samply/OTel/Parca/Pyroscope/fastrace/tracing-appender/tracelogging/coz/ETW 均有 2025–2026 检索锚）；平台重点 Windows（ADV 主战场），Linux 侧只做对照。每对象按「定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接」组织。凡自报数字未复测的都标注"自报/未复测"。

---

## 1. 采样 profiler

### 1.1 ETW 栈走查采样原理（内核 PerfInfo/Sample）
- **定位**：Windows 一切 CPU 采样剖析的地基。不是工具，是理解 WPR/xperf/WPA/samply-on-ETW 行为的前提。
- **可抄机制**：
  1. 开启采样剖析后，内核按默认 **1ms（1kHz）** 每 CPU 记录一条 PerfInfo/Sample 事件（"内核每 1ms 记录所有运行线程调用栈"，Alois Kraus 博客口径）；采样源由 `ProfileSource` 参数指定（timer 或 PMC），HAL 层经 `NtSetIntervalProfile` 生效（Geoff Chappell 内部机制文档口径）。
  2. 栈走查发生在采样中断路径上、由内核辅助完成，**符号解析延迟到后处理**（xperf merge/WPA 加载 PDB 时）——高负载下可能丢栈，样本数≠理论 tick 数，判据要看"事件计数"而非"预期计数"。
  3. WPR/UI 建的是专用会话而非经典 NT Kernel Logger（2013 口径）——脚本化自采要用 `wpr` 的 profile 名而不是手工拼 kernel flags。
- **档位**【吸收】（作为知识层进 perf-core 文档）
- **第一步动作**：perf 文档固化：`xperf -on PROC_THREAD+LOADER+PROFILE -stackwalk Profile` 与 `wpr -start CPU -filemode` 两条命令 + "Sampled vs Precise（上下文切换）" 区分表。
- **许可证/成熟度**：Windows 组件，专有免费（随 WPT/ADK 分发）；机制文档多来自 2013–2017 博客，**默认值口径按"1ms 默认、可调"表述，不写死常数**。
- **链接**：https://learn.microsoft.com/windows-hardware/test/wpt/ ；https://www.geoffchappell.com/notes/windows/ntoskrnl/api/ke/profile/index.htm

### 1.2 WPR / WPA（Windows Performance Recorder / Analyzer）
- **定位**：Windows 官方记录/分析器，ADV Windows 自采管线的"兜底官配"。
- **可抄机制**：
  1. WPR 内置 profile（CPU/GeneralProfile.light 等）= 预打包的 ETW 会话配方，CLI 契约稳定（`wpr -start <profile> -filemode` → `wpr -stop <file>.etl`），适合脚本化。
  2. WPA 的 **CPU Usage (Sampled)**（Profile tick）与 **CPU Usage (Precise)**（上下文切换）是两种数据源：前者回答"谁在烧 CPU"，后者回答"谁在等谁/调度延迟"——ADV 的 CPU 门与延迟门要用不同数据源。
  3. WPA 有火焰图视图（Flame view）+ 按 PDB 延迟符号解析；区域缩放后计数口径随选区变化。
- **可抄机制（收尾）**：ETL 是可后处理工件，可归档+diff（见 §6.3）。
- **档位**【吸收】
- **第一步动作**：把 `wpr -start CPU -filemode; 跑语料; wpr -stop` 写成 xtask 命令，人工在 WPA 里核一次基线火焰图。
- **许可证/成熟度**：专有免费（Windows ADK）；维护中（ADK 持续更新）。
- **链接**：https://learn.microsoft.com/windows-hardware/test/wpt/windows-performance-recorder

### 1.3 xperf
- **定位**：WPT 的老牌 CLI，比 WPR 参数面更宽（自选 kernel flags、`-heap` 堆追踪、`-ProfileSource`/采样间隔）。
- **可抄机制**：
  1. `xperf -on <FLAGS> -stackwalk Profile` 精确控制采什么；`-stackwalk HeapAlloc+HeapRealloc` 开堆栈走查的堆追踪（配 `-heap` 会话），WPA 有对应 Heap Allocations 视图。
  2. `xperf -i trace.etl -o out.csv -a <action>` 可命令行导出分析结果——这是"无需 WPA 人工操作"的自动化出口。
  3. 采样间隔/ProfileSource 可调（默认 1ms，可调高），但要按"开销 vs 样本量"自己标定，不抄别人数字。
- **档位**【吸收】（作为 WPR 的补充档，非默认）
- **第一步动作**：仅当 WPR 内置 profile 不够用时启用；先在文档里留两条已验证命令。
- **许可证/成熟度**：专有免费（WPT）；维护中但定位偏 legacy。
- **链接**：https://learn.microsoft.com/windows-hardware/test/wpt/xperf-command-line-reference

### 1.4 samply（Rust 跨平台采样器）
- **定位**：**ADV Windows 侧首选的"一条命令出火焰图"工具**：`samply record <cmd>` 采完直接打开 Firefox Profiler UI。
- **可抄机制**：
  1. 全平台（macOS/Linux/**Windows**）采样器，v0.13.1（corrode.dev 索引 2026-09 口径），MIT OR Apache-2.0，Rust 1.77+。
  2. 解栈靠 **framehop** crate：支持无 frame pointer 的展开、以及"离线展开"（先存寄存器/栈字节、后解符号）——这使 ADV 可以低侵入采样自有扫描进程。
  3. 与 Firefox Profiler 同生态：`--save-only` 产出 profile 文件，可归档到 CI 工件并在 profiler.firefox.com 打开——连续剖析闭环的载体（§6.3）。
- **档位**【吸收】
- **第一步动作**：`cargo install samply`；对 ADV 扫描语料各跑一次 `samply record`，核对 Windows 下 Rust PDB 符号是否完整解析。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（mstange，Firefox Profiler 生态）。
- **链接**：https://github.com/mstange/samply

### 1.5 inferno（Rust 火焰图工具链）
- **定位**：Brendan Gregg FlameGraph 的 Rust 重实现（fold/dump/flamegraph 全套），剖面 diff 的关键件。
- **可抄机制**：
  1. fold 格式（collapsed stacks）是剖面通用中间表示；`inferno-diff-folded` 对两份 folded 文件做差，配 `--negate-differential` 出差分火焰图（红=退化/绿=改善方向以工具实际配色为准，不凭记忆断言配色）。
  2. 从 `perf script`、dtrace 等多种输入折叠——Linux 对照线与 Windows 导出 CSV 可统一到同一 IR。
  3. crate 形态可作为库被 xtask 调用（不依赖 shell 管道）。
- **档位**【吸收】
- **第一步动作**：xtask 新增 `profile diff baseline.folded current.folded` 子命令。
- **许可证/成熟度**：MIT OR Apache-2.0（crate 双许可口径）/ 维护中。
- **链接**：https://github.com/jonhoo/inferno

### 1.6 perf（Linux 对照线）
- **定位**：Linux 标准采样器；ADV 非 Windows 机器上的对照工具，不是本域主战场。
- **可抄机制**：
  1. callgraph 两模式：`fp`（frame pointer，低开销但要求编译期留 FP）vs `dwarf`（DWARF CFI 展开，高开销全栈）——ADV release 构建可选择性加 `-C force-frame-pointers=yes` 换取低开销采样。
  2. PEBS/精确采样（`:pp` 修饰）与 LBR——PMU 级精度只有 Linux 用户态可直接拿（Windows 无对应公开接口，见 §5.3）。
- **档位**【有界】（仅对照线）
- **第一步动作**：无（等 CI 有 Linux runner 再启用）。
- **许可证/成熟度**：GPL-2.0（工具）；内核组件随发行版。
- **链接**：https://perfwiki.github.io/main/

### 1.7 PerfView
- **定位**：微软免费 ETL 分析器/采集器，比 WPA 更"程序员向"，命令行可采集/合并。
- **可抄机制**：
  1. 命令行采集（`PerfView /ThreadTime collect` 等）+ GUI 火焰图/热点表——对不想装 ADK 的协作者低门槛。
  2. 对 ETW 的 GC/堆/JIT 视图是 WPA 之外的第二把尺（交叉验证用）。
- **档位**【有界】
- **第一步动作**：仅当 WPA 结论存疑时用 PerfView 复核，不进默认管线。
- **许可证/成熟度**：MIT（github Microsoft/perfview）/ 维护中（vancem 持续更新）。
- **链接**：https://github.com/microsoft/perfview

### 1.8 VTune 与"专有思想层"（TMA / Roofline）
- **定位**：不吸收工具本体（专有、驱动重），吸收方法论：Top-down Microarchitecture Analysis（前端受限/后端受限/错误推测/退役 四分）与 Roofline——把"慢"归因到微架构瓶颈类别，而不是只看函数热点。
- **可抄机制**：
  1. 自顶向下分层归因：先用采样确认 CPU 忙，再判断瓶颈类（分支/缓存/内存带宽），最后才谈函数级优化——ADV 的 perf runbook 按此排序。
  2. 硬件计数器维度（cycles/cache-miss/branch-miss）作为"第二证据"，防止函数级火焰图的误导。
- **档位**【有界】（思想吸收，工具不进管线）
- **第一步动作**：runbook 里加"CPU 忙但火焰图分散 → 怀疑微架构瓶颈 → 需 PMU 数据（Linux/VTune）"分支。
- **许可证/成熟度**：Intel EULA 专有；2024 起随 oneAPI 免费公开（公开口径，条款需复核官网）；维护中。
- **链接**：https://www.intel.com/content/www/us/en/developer/tools/oneapi/vtune-profiler.html

### 1.9 pprof crate（进程内信号采样）
- **定位**：Rust 进程内 CPU 剖析（pprof 格式），机制是 SIGPROF 定时信号采样。
- **可抄机制**：信号驱动采样 + CPU 时间过滤 + pprof protobuf 输出，可被 Pyroscope/otlp 消费。
- **档位**【不吸收】（Windows 主战场）：**Windows 无 POSIX 信号机制，该 crate 的 CPU 剖析不支持 Windows**——ADV Windows 自采只能走外部采样（samply/ETW）或 TraceLogging 埋点。
- **第一步动作**：无（记录否证结论即可；Linux 对照线可作为可选依赖）。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中。
- **链接**：https://docs.rs/pprof

---

## 2. 插桩生态

### 2.1 tracing / tracing-subscriber（分层设计）
- **定位**：Rust 插桩事实标准；ADV 默认插桩层。
- **可抄机制**：
  1. **Layer 组合器**：Subscriber=格式化/过滤/导出彼此正交的 Layer 堆叠，开关一个 Layer 就是开关一档观测（ADV 三档：quiet/console/otlp）。
  2. **Span/Event 语义分离**：span=有时间区间的工作单元（树形，id 复用 NonZeroU64），event=瞬时点——这正是"per-span 预算"的语义基础（§6.2）。
  3. 未被订阅的事件在源头即禁用（no-subscriber 成本≈一次原子判断），因此"常驻埋点+按档启用"是可行架构。
- **档位**【吸收】
- **第一步动作**：在 ADV 定插桩规范（§6.2）并 review 现有 span 粒度。
- **许可证/成熟度**：MIT / 维护中（tokio-rs）。
- **链接**：https://github.com/tokio-rs/tracing

### 2.2 tracing-appender non_blocking
- **定位**：tracing 文件输出的标准异步化组件；ADV 日志不阻塞扫描热路径的机制件。
- **可抄机制**：
  1. 专用写线程 + 有界通道：应用侧只入队，写盘在工作线程；**默认 buffered_lines_limit=128,000 行，队列满默认 lossy 丢行**（docs.rs 口径）。
  2. `WorkerGuard` 必须 drop 时才 flush——ADV 必须把 guard 持到 main 结束，否则尾部日志静默丢失（rustcc 上大量"不写文件"即此因）。
  3. 可显式调 `lossy(false)`（阻塞换取不丢）或调小 buffer（省内存）；ADV 门禁类日志应"不丢"，扫描热路径日志可"丢"——两套配置。
- **档位**【吸收】
- **第一步动作**：初始化处显式写明 lossy 与 buffer 选择 + guard 生命周期注释。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.NonBlocking.html

### 2.3 tracing-chrome
- **定位**：把 tracing span/event 转成 Chrome Trace Event 格式 JSON，丢进 chrome://tracing / Perfetto UI 看时间线。
- **可抄机制**：Trace Event JSON 作为零依赖的本地时间线交换格式；单文件产物天然适配 ADV"本地优先"（一次扫描落一个 trace 文件）。
- **档位**【有界】（小 crate、单维护者；ADV 更该配 tracelogging（§2.6）做 Windows 原生时间线）
- **第一步动作**：作为可选 feature 保留；验证 Perfetto UI 能吃它输出的文件。
- **许可证/成熟度**：MIT（crate 口径）/ 维护中（低频）。
- **链接**：https://docs.rs/tracing-chrome

### 2.4 fastrace（原 minitrace-rust）
- **定位**：低开销分布式追踪库，对标 tracing 的"热路径插桩"档。
- **可抄机制**：
  1. 自报基准：span 创建纳秒级（README 宣称"10~100x faster"，Span::new 约 6–20ns 对比 tracing 数百 ns——**自报未复测**，Reddit 讨论亦指出基准局限）。
  2. 机制核心：per-thread 记录器本地聚合、span 树延迟组装、无全局锁热路径——ADV 若做"每次扫描一条 trace"可抄此结构（本地 buffer + 末尾一次性序列化）。
  3. 官方博客 2025-05（fast.github.io）仍在活跃布道；生态带 fastrace-tokio/fastrace-opentelemetry。
- **档位**【watch】（机制抄、依赖暂不上：ADV 热路径先用"event 代替细 span"控预算，预算压不住再评估换库）
- **第一步动作**：在本机复测其 README 基准（同机同口径）后再定档。
- **许可证/成熟度**：Apache-2.0 / 维护中（FastLabs/fast 仓）。
- **链接**：https://github.com/fast/fastrace ；https://fast.github.io/blog/fastrace-a-modern-approach-to-distributed-tracing-in-rust

### 2.5 OpenTelemetry Rust（1.0 时代）
- **定位**：外送档（otel-exporter）的标准格式层；ADV 本地优先，OTel 只作为"可选外送"而非必选。
- **可抄机制**：
  1. **1.0 已落地**（traces/metrics/logs 三信号，官方 migration_0.28.md 宣告口径）——0.x 破坏性漂移时代结束，上库风险显著下降。
  2. SDK 采样器（ParentBased + TraceIdRatioBased）与 BatchSpanProcessor（异步批量导出）——外送开销可控的机制基础。
  3. 与 tracing 的桥（tracing-opentelemetry）——ADV 不必二选一，tracing 常驻 + OTel 桥按档启用。
- **档位**【有界】
- **第一步动作**：不急接；等 ADV 真有远端观测需求时按"tracing→tracing-opentelemetry→otlp"路径接，先锁版本。
- **许可证/成熟度**：Apache-2.0 / 维护中（1.0 后，部分子 crate 仍 0.x/-alpha）。
- **链接**：https://github.com/open-telemetry/opentelemetry-rust

### 2.6 tracelogging（微软 Rust TraceLogging / ETW）
- **定位**：**ADV Windows 一等插桩**：进程内直接发 TraceLogging ETW 事件，WPA/PerfView 原生消费，与系统级 trace 同一时间线。
- **可抄机制**：
  1. `Provider::new(name, &GUID)` + register + `event.write(...)` 带类型字段——manifest-free 自描述事件，无需 MC.exe 生成 manifest。
  2. 事件可被 WPA 直接分组/过滤（provider GUID/level/keyword），与 §1.1 的 CPU 采样、§1.3 的堆追踪在同一 ETL 里关联——"自采管线"的关键缝合点：自证扫描阶段边界 + 系统级采样互相印证。
  3. `tracelogging_dynamic` 提供运行时可配置 provider（按档开关）。
- **档位**【吸收】
- **第一步动作**：给扫描主循环加"阶段边界 ETW 事件"（begin/end scan-file），手工用 WPA 验证可见。
- **许可证/成熟度**：MIT（microsoft/tracelogging 仓口径）/ 维护中（微软官方）。
- **链接**：https://docs.rs/tracelogging ；https://github.com/microsoft/tracelogging

### 2.7 puffin（Embark，游戏帧剖析对照）
- **定位**：游戏行业"每帧聚合 + egui 内嵌 UI"的剖析器；对 ADV 的价值是**产品形态**对照（剖析数据如何呈现给用户）。
- **可抄机制**：
  1. `puffin::profile_scope!` 极轻宏 + 每帧聚合 thread-local 数据，UI 只取聚合视图——与 fastrace 同族的"本地聚合、按需上屏"。
  2. puffin_http 把剖析流推出给外部查看器——"本地采集/外部查看"分离的参考实现。
- **档位**【watch】
- **第一步动作**：无代码动作；ADV TUI 剖析面板设计时参考其聚合模型。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（Embark Studios）。
- **链接**：https://github.com/EmbarkStudios/puffin

### 2.8 tokio-console
- **定位**：异步任务级诊断（任务轮询延迟/挂起时长），与 CPU 剖析互补——找"await 荒"。
- **可抄机制**：console-subscriber 以 tracing Layer 形态收集 task spawn/poll/wake 事件，console UI 聚合出"long poll/never woken"——ADV 若有 tokio 运行时，门禁可加"最长 poll 时长"判据。
- **档位**【有界】（仅当扫描管线用 tokio 且出现调度类症状时启用）
- **第一步动作**：在 runbook 加一行症状入口（§7）。
- **许可证/成熟度**：MIT / 维护中（tokio-rs）。
- **链接**：https://github.com/tokio-rs/console

---

## 3. 连续剖析

### 3.1 Grafana Pyroscope（+ pyroscope-rs 衰退信号）
- **定位**：连续剖析后端+agent 家族；对 ADV 的参考价值在**协议与工作流**（pprof 格式入库、pull/push 两模式）。
- **可抄机制**：pprof 作为 profile 的通用交换格式；后端按 (service, 周期) 存 profile 序列并支持前后 diff。
- **档位**【watch】：**pyroscope-rs（Rust agent）衰退信号明确**：crates.io 归属解析失效（"github:grafana:pyroscope-rs User not found"）+ 2024 年起 issue 迁移到其他仓；Grafana 2025 方向是把 eBPF 采集收敛到 OpenTelemetry eBPF profiler + Alloy（Pyroscope 文档口径）。Rust 语言 agent 不再是投入方向。
- **第一步动作**：不接 agent；若未来要后端，直接推 pprof 格式（可与 §1.9/OTel eBPF 对接）。
- **许可证/成熟度**：服务端 AGPL-3.0（Grafana 系口径）/ pyroscope-rs Apache-2.0；agent 线**弃维护级衰退（2024–2025 信号）**，后端维护中。
- **链接**：https://grafana.com/docs/pyroscope/ ；https://crates.io/crates/pyroscope

### 3.2 Parca / Polar Signals
- **定位**：连续剖析另一家（eBPF 为主）；2026 现状：**未归档、仍活跃**（parca-agent v0.50.0，2026-09，newreleases.io 口径；新增 BPF uprobe 时长事件走 OTLP logs）。
- **可抄机制**：eBPF 无侵入采样 + profile 存储查询；uprobe 进/出时长 → OTLP——"无侵入打点+标准化导出"的组合值得记。
- **档位**【watch】：Linux/eBPF 主场，与 ADV Windows 优先错位；另见 Grafana 侧 Parca 数据源 2027-01-02 终止支持的信号（生态迁移中）。
- **第一步动作**：无；每年复核一次活跃度。
- **许可证/成熟度**：Apache-2.0（parca/parca-agent 口径）/ 维护中。
- **链接**：https://github.com/parca-dev/parca

### 3.3 Google Cloud Profiler（思想层）
- **定位**：生产级 always-on 剖析的形态参考：常驻 agent、低频采样、后台聚合上传、以"天/周"为窗口看回归。
- **可抄机制**：always-on + 低开销（采样而非全量）+ 与部署版本绑定的 profile 对比——这正是 ADV 连续剖析闭环（§6.3）要做的三要素，区别只在 ADV 本地化、不进云。
- **档位**【reference】
- **第一步动作**：无（作为 §6.3 设计依据引用）。
- **许可证/成熟度**：服务专有（agent Apache-2.0）。
- **链接**：https://cloud.google.com/profiler/docs

### 3.4 Bencher（wave-0 已定；此处只补剖面维度）
- **定位**：perf 门基座（wave-0 07 已定，不动）。本域补的是**剖面对比**维度，不重做基准门。
- **可抄机制**：Bencher 管"数值阈值门"，剖面 diff 管"形状漂移"——两把尺互补：数值不变但热点换人（内部结构性退化）只有剖面 diff 能抓。
- **档位**【有界】
- **第一步动作**：见 §6.3 最小闭环设计。
- **许可证/成熟度**：开源 CLI + SaaS（license 以 wave-0 07 决议为准，本域不另锚）。
- **链接**：https://bencher.dev/

---

## 4. 内存剖析

### 4.1 dhat-rs（DHAT in Rust）
- **定位**：Valgrind DHAT 思想的 Rust 进程内实现；**ADV 内存剖析首选**（纯 Rust、跨平台、无外部依赖）。
- **可抄机制**：
  1. 全局分配器包裹（`dhat::Profiler::new_heap()`）：记录每分配点（backtrace 聚合）的 **total blocks/bytes、最大存活、结束时仍存活**——"结束时仍存活"正是找泄漏与常驻增长的关键列，普通分配器统计给不了。
  2. 产物 `dhat-heap.json` 用随仓 `dh_view.html` 打开，树形按分配点聚合。
  3. **ad-hoc 模式**（`dhat::ad_hoc_event`）：对任意事件（不只内存）做同样的"总量/峰值/存活"统计——可用来剖析"规则命中"这类对象生命周期。
- **档位**【吸收】
- **第一步动作**：加 `dhat` feature；对固定语料跑一次 heap profile，留基线 json 进 RESEARCH 附件。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（nnethercote）。
- **链接**：https://github.com/nnethercote/dhat-rs

### 4.2 jemalloc Profiling（jeprof）
- **定位**：Linux 侧生产级 heap profile 经典（`--enable-prof` + `MALLOC_CONF=prof:true` + jeprof 火焰图）。
- **可抄机制**：采样式堆剖析（按分配大小概率采样，开销近常量）+ 回溯聚合输出 pprof——"低开销常驻 heap profile"的机制参照。
- **档位**【不吸收】：**jemalloc 无官方 Windows 支持**（Windows 移植非一等），ADV Windows 优先不引入；Linux 对照线也不值得为它换分配器。
- **第一步动作**：无。
- **许可证/成熟度**：BSD-2-Clause / 维护中。
- **链接**：https://github.com/jemalloc/jemalloc

### 4.3 UMDH（Windows 堆快照 diff）
- **定位**：Windows 官方"两快照 diff 找泄漏"工具（gflags +ust 开启用户态栈迹数据库后 `umdh -p:<pid> -f:snap1.log` … diff）。
- **可抄机制**：快照对 + diff 按调用栈聚合出"新增未释放字节/块"——判据形式是"差分"而非绝对值，与 §6.3 剖面 diff 同构。
- **档位**【有界】（手工诊断用；自动化脚本脆弱——依赖 gflags 与符号路径）
- **第一步动作**：runbook 收录三行命令（开 +ust、两次快照、diff）。
- **许可证/成熟度**：Windows 专有（Debugging Tools for Windows）/ 维护中。
- **链接**：https://learn.microsoft.com/windows-hardware/drivers/debugger/umdh

### 4.4 ETW 堆追踪（xperf -heap / WPA Heap Allocations）
- **定位**：Windows 内核侧堆分配追踪（HeapAlloc/HeapRealloc 栈走查），WPA 有 Heap Allocations 视图。
- **可抄机制**：与 CPU 采样同一 ETW 框架、同一时间线——"内存涨的同时 CPU 在干嘛"可以一张图看（对比 UMDH 只见堆不见时序）。
- **档位**【watch】（开销与配置门槛高于 dhat-rs；dhat 覆盖不了的"跨模块/第三方堆行为"才启用）
- **第一步动作**：验证一次 `xperf -start <Session> -heap ...` 可用性，记入 runbook 备用。
- **许可证/成熟度**：WPT 专有免费 / 维护中。
- **链接**：https://learn.microsoft.com/windows-hardware/test/wpt/xperf-command-line-reference

---

## 5. 专项

### 5.1 计时方法学（QPC / TSC / RDTSC / Instant）
- **定位**：一切性能判据的地基；旧仓教训：**跨机器判据别靠线程/时钟竞速**。
- **可抄机制**：
  1. Windows `QueryPerformanceCounter`：优先用不变 TSC 实现，启动时定频、亚微秒分辨率；无 TSC 保障时回退 HPET/PM 时钟（开销可达数百 ns）——**同一判据在两种机器上语义不同，跨机器不比绝对值**（MS "Acquiring high-resolution time stamps" 口径）。
  2. Rust `std::time::Instant` 在 Windows 即 QPC——默认公共尺；进程内微基准可用 `core::arch::x86_64::{_rdtsc, _rdtscp}`（rdtscp+lfence 序列化防乱序），但要小心核间 TSC 漂移与迁移。
  3. 方法论：先测"时钟自身开销"（空循环 N 次取均值，标定复现），再测被测物；判据用**同机同基线相对比值 + 多窗口均值**（单点≠稳态），判据要时序独立（不依赖"线程 A 先于 B"这类竞争）。
- **档位**【吸收】
- **第一步动作**：xtask 里落 `clock_overhead` 探针命令，每次跑 perf 门先打印时钟口径。
- **许可证/成熟度**：标准库/文档级 / 稳定。
- **链接**：https://learn.microsoft.com/windows/win32/sysinfo/acquiring-high-resolution-time-stamps

### 5.2 coz（因果剖析）+ coz crate
- **定位**：经典学术工具（ICSE 2020，plasma-umass/coz）：不是问"谁最忙"，而是问"**加速哪一行，端到端才真的变快**"。
- **可抄机制**：
  1. 在程序里插 progress point（"又推进了一个工作单元"）；运行时对线程施加虚拟加速，观测 progress point 触发率变化 → 输出"每行代码的潜在端到端收益%"。
  2. 解决的正是扫描器型程序的典型痛点：函数 A 是火焰图第一热点，但优化它总时长不动（A 与关键路径无关）；coz 直接量测因果贡献。
  3. Rust 支持经 `coz` crate（coz.h 的 Rust 移植；once_cell 等知名 crate 已含插桩点）。
- **适用性评估（⑤）**：**适用但非首选**。ADV 扫描器有天然 progress point（"完成一个文件/一条规则"）且负载可重复，满足 coz 前提；限制是：需要插桩 + 较长稳定负载（虚拟加速要统计收敛）、运行时有开销、**Linux 主场（Windows 支持弱，不能进 ADV 主门）**。定位：当"优化了热点但总时长不降"时启用的诊断手段（runbook 进阶行）。
- **档位**【watch】
- **第一步动作**：Linux 机上对 ADV 扫描核试跑一次 coz，验证 progress point 设计（"每文件一条"粒度）。
- **许可证/成熟度**：BSD-2-Clause（repo 口径，本次检索未直接复核 LICENSE 文件）/ 研究工具，低频维护。
- **链接**：https://github.com/plasma-umass/coz ；https://docs.rs/coz

### 5.3 PMU / 硬件计数器在 Rust 的可达性
- **定位**：cycles/cache-miss 级证据的获取路径盘点。
- **可抄机制**：Linux `perf_event_open`（crate：perf-event 等）用户态直读计数器；**Windows 无公开用户态 PMU 接口**，计数器访问要靠厂商驱动（VTune SEP / AMD uProf）——决定了"微架构证据"在 Windows 主门里不可得，只能靠采样剖析 + dhat 补位。
- **档位**【有界】
- **第一步动作**：无（Windows 不做 PMU 门；Linux 对照线可选）。
- **许可证/成熟度**：perf-event crate MIT OR Apache-2.0（crate 口径）/ 维护中（低频）。
- **链接**：https://docs.rs/perf-event

---

## 6. 重点回答

### 6.1 ① Windows 性能剖析完整工具链选型 + ADV 自采管线
三档分工，互不替代：
- **交互诊断档：samply**（§1.4）。一条命令、Firefox Profiler UI、framehop 解栈（不强求 FP）。默认工具。
- **脚本/CI 采集档：WPR/xperf（ETW）**（§1.2/1.3）。CLI 稳定契约：`wpr -start CPU -filemode` → 跑语料 → `wpr -stop out.etl`；分析用 WPA/PerfView，导出折栈走 xperf action 或 PerfView 导出。判据分开：CPU 忙→Sampled；等待/调度→Precise。
- **进程内埋点档：tracelogging（ETW）**（§2.6）。阶段边界事件与系统采样同一时间线，是自采管线的"坐标轴"。

自采管线（perf-core 设计稿）：`xtask profile record` = perf_lock acquire → 可选 tracelogging 开启 → `samply record --save-only`（首选）或 wpr 采集 → 产物（profile json / etl）+ 元数据（commit、语料版本、机器口径、时钟探针输出）落 `perf-profiles/` → 折叠为 folded 文本（inferno）→ 入 §6.3 diff。**Windows 全程不引入需要驱动/管理员特权的 PMU 组件**（除 ETW 采样本身）。

### 6.2 ② tracing 低开销设计要点 + ADV 插桩规范
- **per-span 预算**：span 只给"批次/阶段/文件级"工作单元；**逐条目数据用 event，不开 span**（span 创建是插桩里最贵的动作；fastrace 的纳秒级 span 是另一档方案，先按预算纪律控制，不上新依赖——§2.4）。规则：单个 span 内 event 数上限作为规范值（先给 50 的初值，按实测标定）。
- **异步 appender**：文件输出走 non_blocking；guard 生命周期进规范（main 持有到结束）；**门禁/审计日志 lossy(false)，扫描热路径日志允许丢**（§2.2 两套配置）；buffer 大小显式声明，不吃 128k 默认。
- **分档开关**：Layer 堆叠按 quiet/console/otlp 三档组装；热路径埋点常驻但 no-subscriber 成本近零（§2.1 机制 3）。
- **Windows 坐标轴**：阶段边界同步发 tracelogging ETW 事件（§2.6），与 tracing 输出并行不互斥。

### 6.3 ③ 连续剖析最小闭环（CI 定期采 profile + diff）
1. **触发**：nightly/每周（xtask-gates 之外的 ci-ops 定时任务），非每次提交（采样剖析噪声大，不进提交流水线）。
2. **采集**：固定语料 + §6.1 自采管线（samply `--save-only` 优先；失败回退 wpr）。采集前 `perf_lock acquire`，同时打印时钟探针输出（§5.1）。
3. **归档**：profile 原件 + folded 文本 + 元数据按 (commit, 机器口径) 入库，保留 N 份滚动。
4. **diff**：`inferno diff-folded` 对上一基线；门：新增热点（基线中不在前 M 的函数进入前 M）与热点漂移（前 M 名份额变化超过阈值 T）→ 标红。**T/M 必须先跑 4–8 个窗口标定（单点≠稳态），不先写死数值**。
5. **出口**：profile 上传为 CI 工件（Firefox Profiler 可打开），diff 报告文本留档。与 Bencher 数值门互补（数值不变、形状换人也能抓，§3.4）。

### 6.5 ⑤ coz 适用性
见 §5.2：诊断手段（watch 档），满足前提（可重复负载 + 天然 progress point），但 Windows 支持弱、有运行时开销、收敛要长负载——只作为"热点优化不奏效"时的进阶诊断，不进门禁。

---

## 7. 性能剖析 runbook（④：症状 → 工具 → 判据）

| 症状 | 工具 | 判据（走真路径，时序独立） |
|---|---|---|
| CPU 烧满、总时长超标 | samply record（默认）→ 必要时 WPR CPU + WPA Sampled | 同机同基线热点前 M 名份额比值；多窗口均值 |
| 火焰图分散、热点优化不奏效 | WPA Precise（等待/调度）→ coz（Linux 诊断档） | Precise：ready 线程等待时长占比；coz：每行端到端收益% |
| 等待多、CPU 不忙（尾延迟） | WPA CPU Usage (Precise)、tokio-console（若 tokio） | 调度延迟分布 P95；最长 poll 时长 |
| 内存增长/疑似泄漏 | dhat-rs heap profile（首选）；UMDH 快照 diff（跨模块）；xperf -heap（时序对照） | dhat"结束时仍存活"列跨窗口增量；UMDH diff 新增未释放字节 |
| 单条规则/函数微优化 | dhat ad-hoc + Instant/QPC 微基准（先测时钟开销） | 同机相对比值 ≥ 标定阈值才合入 |
| 跨机器/跨时段回归比较 | 自采管线（§6.1）+ 折栈 diff（§6.3） | 只比同机相对比值；机器口径与时钟探针输出随工件归档；禁止跨机器绝对值判据 |
| 需要微架构证据（缓存/分支） | Linux perf PEBS / VTune（watch） | 对照线专用；Windows 无用户态 PMU，不作门 |
| 启动/阶段时间线异常 | tracelogging 阶段事件 + WPA/PerfView 时间线 | 阶段边界 ETW 事件时长，与采样剖析互证 |
| 门禁数值绿但形状不对 | §6.3 剖面 diff | 新增热点/漂移阈值（窗口标定后生效） |

---

## 8. 2025–2026 前沿信号
- **samply v0.13.1 全平台**（Windows 在列，corrode.dev 2026-09 索引口径），framehop 离线解栈使其可嵌自采管线。
- **OpenTelemetry Rust 1.0 落地**（traces/metrics/logs 三信号；migration_0.28.md 官方宣告）——0.x 漂移期结束。
- **fastrace 官方博客 2025-05**（fast.github.io），活跃布道 + 生态桥（tokio/otlp）；自报"10~100x"基准未复测。
- **Grafana 采集栈收敛**：Rust agent pyroscope-rs 归属解析失效 + issue 迁移（2024 信号）；eBPF 采集并入 OpenTelemetry eBPF profiler + Alloy（Pyroscope 文档 2025 口径）。
- **Parca 2026 仍活跃**（agent v0.50.0，2026-09；uprobe→OTLP logs），但 Grafana 侧 Parca 数据源 2027-01-02 终止——连续剖析生态在向 OTel 格式收敛。
- **微软 Rust TraceLogging（tracelogging/tracelogging_dynamic）**成熟可用，Windows 原生观测与 Rust 的缝合点。
- **Intel VTune 免费化**（oneAPI 口径，2024 起；条款需复核）——PMU 证据的 Windows 出口仍走厂商驱动。

## 9. Top-3（给 ADV 的最高优先动作）
1. **samply 作为 Windows 自采主工具**（吸收 → perf-core）：一条命令出 Firefox Profiler 剖析 + `--save-only` 产物进 §6.3 闭环；先验证 Rust PDB 符号完整解析。
2. **tracelogging（ETW）作为 Windows 坐标轴插桩**（吸收 → perf-core）：阶段边界事件进 WPA 时间线，与系统级 CPU 采样互证；这是"本地优先 + Windows"语境下比 OTel/Chrome-JSON 更贴合的选择。
3. **dhat-rs + inferno diff-folded 组成"内存账本 + 剖面形状门"**（吸收 → xtask-gates/ci-ops）：dhat 的"结束时仍存活"列管泄漏与常驻增长，diff-folded 管热点漂移；两者产物都是纯文本/JSON，天然本地归档。

## 10. 否证与边界（防把局部当总体）
- fastrace/PPROF 的数字均为自报或他引，本会话未复测；落档前不得进判据。
- "Windows 无用户态 PMU 公开接口"是公开生态口径（靠厂商驱动），若未来微软开放原生计数器需重估 §5.3。
- ETW 默认 1ms 采样率是文档/博客口径，不同 Windows 版本可调；判据设计不得写死该常数。
- pyroscope-rs"衰退"基于 crates.io 归属失效 + issue 迁移两信号，未见官方归档公告——按时间戳标签对待，非永久事实。
