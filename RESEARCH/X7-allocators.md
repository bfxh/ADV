# X7 · 内存分配器调研（ADV W4）

- 日期：2026-10-04（检索日；平台重点 Windows，目标 x86_64-pc-windows-msvc）
- 对象仓：ADV（D:\KF\ADV，分支 adv-rewrite）；本文件只进 RESEARCH/
- 方法与置信：官方文档/GitHub 优先；转述级锚已在 §5 标注；未锚定的机制描述在条目内注明
- 配套登记：`registry/X7.jsonl`（14 条：机制级 9 / 扫视 5）

## 0. Top-3

1. **默认档：std 分配器 + Segment Heap 政策；可选档：mimalloc（cargo feature 门控）**。判据：扫描器负载 = 大量小对象 + 短命（每文件解析后转句柄存储），两个候选都为此类负载优化；默认档零新依赖零 ABI 风险，mimalloc 仅当默认档 RSS 超内存预算门、且 A/B（同语料、n≥3 窗口均值）显示分配器侧差距显著时启用（§3①）。
2. **解析期 bumpalo arena（每文件一 arena）+ 出口转句柄化存储**；arena 引用不逃逸、不跨文件复用（§3②⑤）。
3. **内存账：后端无关的计数 GlobalAlloc 包装（alloc/dealloc/peak，AtomicU64）作为内存预算门主数据源**；进程级 Private Bytes（PSAPI）交叉校验；mimalloc 档启用后叠加其原生 stats（§3④）。

## 1. 主表（14 条：一句机制 + 档位 + 链接）

| # | 对象 | 机制一句话 | 档位 | verdict | 链接 |
|---|------|-----------|------|---------|------|
| 1 | mimalloc（微软） | free-list 分片 + 每线程 heap + 可控页 purge；Windows 动态 override 走重定向 DLL | 可选档后端 | 吸收 | [GitHub](https://github.com/microsoft/mimalloc) |
| 2 | mimalloc Rust 绑定 | `mimalloc` crate 提供 `Mimalloc` 类型作 `#[global_allocator]` | 可选档载体 | 有界 | [crates.io](https://crates.io/crates/mimalloc) |
| 3 | snmalloc（微软） | 消息传递：free 对象批量、无锁回送 owner 线程的 allocator（ISMM 2019） | 机制参照 | watch | [GitHub](https://github.com/microsoft/snmalloc) |
| 4 | Windows Segment Heap | NT 堆新实现：小对象内存占用降、CPU 略增；manifest 或 IFEO 注册表启用 | 政策档（Win10 2004+） | 吸收 | [Thurrott 2020-06（转述）](https://www.thurrott.com) |
| 5 | std System（HeapAlloc） | Rust `System` → 进程默认堆 HeapAlloc/HeapFree，随 Segment Heap 政策切换实现 | 默认档 | 吸收 | [std docs](https://doc.rust-lang.org/std/alloc/struct.System.html) |
| 6 | bumpalo | 分块 bump：O(1) 指针推进分配；arena drop 一次释放；arena 内 **Drop 不执行** | 解析期 arena | 吸收 | [crates.io](https://crates.io/crates/bumpalo) |
| 7 | typed-arena | 单类型 arena，RefCell 支持成环 | 备选 | 有界 | [crates.io](https://crates.io/crates/typed-arena) |
| 8 | jemalloc | mallctl 可编程统计最完整，但 MSVC 目标不支持 | 不用（接口设计参照） | 不吸收 | [tikv-jemallocator](https://github.com/tikv/jemallocator) |
| 9 | rpmalloc | 线程本地缓存 + 全局 span 缓存，OS 页自管（机制未逐条锚定） | 候补 | 有界 | [GitHub](https://github.com/rigtorp/rpmalloc) |
| 10 | talc | LSB 位图 + claim 区段 + 模块化 OOM handler；多线程靠单自旋锁 | no_std/WASM 定位 | watch | [crates.io](https://crates.io/crates/talc) |
| 11 | `#[global_allocator]` 工程 | 编译期单选、运行时不可换 → feature 门控；回退 = 默认档 CI 常绿 | 工程框架 | 吸收 | [GlobalAlloc](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html) |
| 12 | 分配器统计/内存账 | 计数包装 + mimalloc/jemalloc/rpmalloc 原生 stats + PSAPI 进程级 | 预算门数据源 | 吸收 | [GetProcessMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo) |
| 13 | OOM 策略 | stable 不可自定义 alloc_error_handler → 预算门前置 + try_reserve 降级 + abort 兜底 | 工程策略 | 吸收 | [handle_alloc_error](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html) |
| 14 | arena 长命/短命分层 | 短命 = 每文件 Bump 出口转句柄；长命 = 全局分配器 + u32 句柄互指 | 规范 | 吸收 | 同 #6 |

## 2. 机制级条目

### X7-01 mimalloc（微软）
- 定位：微软维护的通用分配器，Windows 生态原生契合；ADV 可选分配档的首选后端。
- 可抄机制：① free-list 分片（同页内多条 free list、随机化顺序）服务多核与抗碰撞；② 每线程 heap 自动创建，跨线程 free 走延迟路径；③ 页 purge 可调：`MIMALLOC_PURGE_DELAY`（v3 默认 1000ms；0=立刻，-1=禁用——官方 environment.html，2026-10-04 抓取）；④ Windows 动态 override = 重定向 DLL `mimalloc-override.dll`，拦截含其他 DLL/CRT 的 malloc/free（官方 overrides.html）——Rust 侧不走此路径，直接 `#[global_allocator]`。
- 档位：默认不开；feature `alloc-mimalloc`。secure 档（MI_SECURE 构建，或 `mi_option_set(mi_option_secure,1)`）默认不开：ADV 主体是 Rust，use-after-free/堆破坏类风险主要落在 FFI 依赖侧，secure 档有性能代价，仅当威胁模型要求 FFI 依赖同堆加固时评估。
- 第一步动作：接 `mimalloc` crate 建 feature 档；用计数包装跑同语料 A/B（RSS + 时延，n≥3 窗口均值），先量后改。
- 许可/成熟度：MIT；微软仓库，v3.x，生产级（Windows 生态广泛使用）。
- 链接：https://github.com/microsoft/mimalloc · https://microsoft.github.io/mimalloc/overrides.html · https://microsoft.github.io/mimalloc/environment.html

### X7-03 snmalloc（微软）
- 定位：微软 Research 的消息传递分配器（ISMM 2019 论文），多核/时间安全场景（Verona、CHERI Cornucopia 在用）。
- 可抄机制：① 跨线程 free 不写共享结构：对象批量打包成消息、无锁回送 owner 线程的 allocator；② per-thread heap + slab 元数据。该"远程 free 批量化"思想可被 ADV 跨线程任务/结果通道的设计借鉴——对已选后端（mimalloc/std）不可直接调参，只作设计参照。
- 档位：不作 ADV 后端（Rust 绑定 `snmalloc-rs` 成熟度不足）；watch 其 Rust ABI 进展。
- 许可/成熟度：MIT；微软，研究向，生产证据偏 CHERI/Verona 场景。
- 链接：https://github.com/microsoft/snmalloc

### X7-04 Windows Segment Heap（OS 堆政策）
- 定位：NT 堆的 modern 实现（Windows 10 2004 引入），微软口径"总体降低内存占用"（Outlook/Edge 案例，转述级）；代价 CPU 略增，故 Win10 为 per-app 选择加入。
- 可抄机制/启用法：① manifest：`<heapType>SegmentHeap</heapType>`（开发者首选，Chrome 2020-06 先例）；② 注册表：`HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\<adv.exe>\SegmentHeap`(DWORD)=1（第三方转述锚，接入前以微软文档复核）；③ Win11 系统级默认口径见 §5，待真机复核。
- 档位：默认政策档。判据：目标机若为 Win11（系统默认已开），manifest 增益有限；Windows 10/Server 场景才以 manifest 显式选择加入；CPU 敏感的扫描主线程要带 CPU 计数一起量。
- 许可/成熟度：OS 组件（微软专有）；机制级确认需在 ADV 真机复测（RSS/HeapWalk 对照）。

### X7-05 std System 分配器（HeapAlloc）
- 定位：Rust 默认 `System` 在 Windows 走进程默认堆 HeapAlloc/HeapFree——"默认档 = std + OS 堆政策"即由此组合而来。
- 可抄机制：自身无可调机制，价值在与 Segment Heap 政策的组合：默认堆随 manifest/IFEO 政策切换实现，Rust 代码零改动。
- 档位：ADV 默认档。
- 许可/成熟度：Rust std（MIT OR Apache-2.0），最稳的一档。

### X7-06 bumpalo（解析期 arena）
- 定位：Rust 通用 bump arena；对归一化 AST 构建期直用价值最高。
- 可抄机制：① 线性 bump 分配（指针推进，O(1)，chunk 扩展）；② 整 arena drop 一次释放——匹配"每文件 AST 构建后整体丢弃"；③ arena 内对象 **Drop 不执行**（官方 docs）——AST 这类纯数据无损，含资源句柄/锁的类型禁入。
- 档位：adv-parse 解析/归一化期 per-file arena；不进索引长命层。
- 第一步动作：先量（计数包装测当前单文件 alloc 次数/字节量），构建期分配密度达标再落（判据见 §3②）。
- 许可/成熟度：MIT OR Apache-2.0；生态成熟（rustc 内部走同类 arena 路线，作同类实践参照）。
- 链接：https://crates.io/crates/bumpalo

### X7-11 `#[global_allocator]` 替换工程
- 机制：全局分配器编译期单选（全 crate 唯一 static），运行时不可替换 → 用 cargo feature 构建期切档：`default = []`（std）+ `alloc-mimalloc`；两档共用同一计数包装组合（CountingAlloc\<Backend>），保证内存账口径跨档一致。
- 代价与回退：每次分配一次间接调用（幅度未测——先量后改，不预设可忽略）；回退路径 = 分配档异常时回默认档构建，默认档 CI 必须常绿。
- 档位/动作：feature 框架先行，后端逐个进。

### X7-12 分配器统计（内存账）
- 接口面（可编程读数）：
  - jemalloc：mallctl("stats.allocated") 等——可编程读数最完整，但 MSVC 不可用 → 只作接口设计参照。
  - mimalloc：`mi_stats_print_out` / `mi_thread_stats_print_out` / `mi_process_info`（API stats 组）；env `MIMALLOC_SHOW_STATS=1`（已抓取锚）；Rust 侧经 libmimalloc-sys——第一步动作 = 验证绑定符号导出。
  - rpmalloc：全局/线程 statistics 接口（未锚定，接入前复核）。
  - Windows 原生堆：无每堆编程读数；进程级用 GetProcessMemoryInfo（WorkingSetSize/PrivateUsage，PSAPI）。
  - **推荐主数据源：自写 CountingAlloc（AtomicU64：alloc_bytes / dealloc_bytes / peak）包装任意后端**——口径跨档一致；配合 PSAPI 私有字节交叉校验（差值 = 分配器内部缓存/保留未提交部分，这个差值本身也是观测项）。
- 档位：perf-core 内存预算门直接消费。

### X7-13 OOM / alloc error 策略
- 机制：Rust stable 不能自定义 alloc_error_handler（nightly 特性），分配失败默认走 handle_alloc_error → abort。
- 可抄策略：① 预算门前置：CountingAlloc 峰值过阈值（阈值定档待测，不写死）→ 告警/熔断新扫描任务；② 关键大分配用 `try_reserve` 系（Vec/HashMap，stable）优雅降级；③ abort 兜底 + 会话日志留档（否证可复算）。
- 参照：talc 的模块化 OOM handler（可失败/可恢复）是接口设计参照。
- 链接：https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html

### X7-14 arena 长命/短命分层规范
- 短命层：每文件/每任务一个 `Bump`；解析 + 归一化在 arena 内完成；出口即转句柄化存储（数据复制进 `Vec`/紧凑结构，互指用 u32 id）；arena 整体 drop。
- 长命层：索引/符号表走全局分配器；句柄互指；**禁止把 arena 引用/指针存入长命结构**。
- 防泄漏红线（防"arena 泄漏成全局内存池"）：① arena 不跨文件复用；确需复用只许 `Bump::reset`，且 arena 峰值单列进预算门；② 含资源句柄类型禁入 arena；③ review 门检查 arena 引用逃逸函数签名。

## 3. 重点回答

**① Windows 分配器定档建议（判据，非结论）**
- 负载画像：ADV 扫描器 = tree-sitter 解析每文件产生大量小对象、生命周期到"归一化为句柄存储"为止（短命）；索引层长命大对象。该画像同时利好 Segment Heap（微软口径：小对象内存占用）与 mimalloc（free-list 分片 + purge）。
- 默认档：std + Segment Heap 政策（Windows 10/Server 经 manifest 显式选择加入；Win11 系统默认口径待真机复核）。理由：零新依赖、零 ABI 风险、OS 级支持。
- 可选档：mimalloc（feature 门控）。启用判据：默认档在真实语料上 RSS 超内存预算门，且 A/B 对照（同语料、同窗口数 n≥3）显示分配器侧差距显著——两个数字都带窗口口径，不单点定档。
- 排除：jemalloc（MSVC 不可用）；talc（多线程单自旋锁，与扫描器多线程负载不匹配）；snmalloc（Rust 绑定成熟度不足）。

**② bumpalo 对归一 AST 构建期的收益判据（与句柄化组合）**
- 先量后改：计数包装测单文件构建期 alloc 次数与字节量；判据 = 构建期分配次数达万级/文件量级，且节点生存期不超单文件处理。
- 组合方式：arena 出口把 AST 数据复制进 Vec/句柄存储，arena 立即 drop——bumpalo 只吃构建期抖动，不承载长命数据（否则 arena 成为第二套全局堆，见 X7-14 红线）。
- 若句柄化存储已是主载体，需对比"arena 内构建 + 拷贝出口"与"直接构造句柄存储"的总成本再定——收益未证实前保持默认路径。

**③ override 机制与线程缓存参数（何时调）**
- mimalloc：Rust 下不需要 Windows override DLL（那是给 C/C++ 拦 CRT 的路）——直接 `#[global_allocator]`。线程 heap 自动；可调参数集中在 purge：RSS 驻留超标 → 调小 `MIMALLOC_PURGE_DELAY`（v3 默认 1000ms）或设 0；吞吐抖动/分配热点 → 增大或 -1。secure 档默认不开（理由见 X7-01）。
- snmalloc：核心参数在消息批量与 per-thread 缓存（细节以 repo 为准）——watch，不展开。

**④ 内存账：从分配器拿统计的接口面**
- 见 §2 X7-12。要点：主数据源 = 自写计数包装（后端无关、跨档口径一致）；mimalloc 档叠加 mi_* stats（经 libmimalloc-sys，符号面待验证）；进程级 PSAPI Private Bytes 交叉校验；jemalloc 的 mallctl 读数最完整但 Windows 不可用，仅作接口设计参照。

**⑤ arena 长命/短命分层规范（防 arena 泄漏成全局内存池）**
- 见 §2 X7-14。要点：短命层 per-file Bump + 出口转句柄；长命层全局分配器 + u32 句柄；红线 = 不跨文件复用（reset 才许，峰值入预算门）、资源句柄禁入、arena 引用不逃逸签名。

## 4. 反模式

- 把 arena 当全局内存池（跨文件复用、不 reset、峰值无人观测）。
- 未量先换分配器：全局分配器替换影响全进程，回退成本高——先上计数包装建立基线。
- 多线程扫描主路径用单锁分配器（talc 多线程口径）。
- 用 mimalloc-override.dll 路径接 Rust——绕远路，`#[global_allocator]` 才是正道。

## 5. 锚与置信声明

- 已抓取官方/一手锚（2026-10-04）：mimalloc environment.html（`MIMALLOC_PURGE_DELAY` v3 默认 1000ms、`MIMALLOC_SHOW_STATS`）、overrides.html（重定向 DLL）、bumpalo docs（Drop 不执行）、tikv-jemallocator 0.6.0（MSVC 目标不支持）、snmalloc ISMM 2019 论文、talc v5.1.1（crates.io，2024-04 更新时间注）、talc 多线程单自旋锁（repo issue #18 口径）。
- 转述级（未直接抓微软原文，第一步动作含真机/文档复核）：Segment Heap 的 manifest/IFEO 启用法与 Win11 系统级默认口径（Thurrott 2020-06 等转述）。
- 未锚定（接入前复核）：rpmalloc 机制细节、mimalloc stats API 精确符号面、talc claim 区段内部实现。
