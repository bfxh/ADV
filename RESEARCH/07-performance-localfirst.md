# 07 · 性能与本地优先架构调研（Rust 重写蓝图供弹）

> 调研日期：2026-10-03（外部检索窗口 2025–2026 优先）。
> 方法：WebSearch 实查 + 训练期知识；**查不到的标「待验证」**；数字全部带出处锚。
> 纪律：区分营销话术与机制；结论带适用范围与量级，不绝对化。
> 内部锚说明：凡标「unified-rx 实测」的是旧项目自测数字（口径在 CLI-SPEED/项目 S 编号），复算口径为单机 Windows + 大批量统计相位，不构成普遍结论。

---

## 0. 档位总览（先看这张表）

| 对象 | 档位 | 一句话理由 |
|---|---|---|
| memchr | **吸收** | ripgrep 同款字节扫描原语，MIT/Apache，直接当依赖 |
| aho-corasick（Teddy SIMD） | **吸收** | 多字面量签名预过滤的业界标准件 |
| regex crate（lazy DFA + prefilter） | **吸收** | 匹配层终判器，别再自写正则 |
| std::simd (portable-simd) | **不吸收（2026-10 仍 nightly）** | 语义未定、稳定化无期；稳定依赖不许用 nightly 门 |
| core::arch 手写 SIMD + 运行时检测 | **吸收** | memchr/ripgrep 的实际路线，1.87 起 safe intrinsics 已稳定 |
| wide crate | **有界** | 稳定的便携向量类型，适合「不追求极致」的内积/统计；许可证待核 |
| fearless_simd | **有界（观察）** | BurntSushi 2025 新作，等成熟度信号 |
| rayon | **吸收（文件级不规则并行）** | work-stealing 对不均匀文件负载是硬优势 |
| 自建分块线程池（par.rs 路线） | **有界（保留于有序相位）** | 块序合并 + sha256 验证的确定性输出 rayon 不直接给 |
| memmap2 | **有界（大文件档）** | 小文件 mmap 反而慢（ripgrep 实测结论），要分档 |
| zerocopy | **吸收** | 安全 zero-cast，头部/记录解析直接用 |
| bumpalo / arena | **有界** | 逐文件 arena 值得；全工程 arena 过度 |
| SoA 列式布局 | **吸收** | 统计批处理 + GPU 缓冲区天然同构 |
| compio / tokio-uring / 异步文件 IO | **不吸收（扫描内核）/ 有界（MCP 外壳）** | 本地扫描 IO 不是瓶颈；tokio-uring 已沉寂 |
| wgpu 计算 | **有界（按相位上卡）** | 大批量统计相位实测 31–38×（内部锚），小批量纯亏 |
| tracing + tracing-chrome | **吸收** | 时间线档案 = 可复现否证记录 |
| pprof / samply | **有界** | 采样火焰图；Windows 支持度待验证 |
| Windows ETW/WPA | **有界** | 系统级真相之源，但工具链重 |
| rusqlite | **吸收** | 本地单文件存储的地基 |
| Tantivy | **有界** | 强但重；自建 BM25 已有测量，先当对照 |
| sqlite-vec | **有界（倾向吸收）** | 纯 C 零依赖、int8/binary 量化、pre-v1 注意 breaking |
| fastembed-rs / ort | **吸收** | 本地 ONNX 嵌入事实标准；candle 推理性能不够 |
| Qwen3-Embedding-0.6B | **吸收** | Apache-2.0、32K ctx、2025-06 发布，本地代码嵌入默认选项 |
| jina-code-embeddings | **不吸收（许可证）** | CC-BY-NC 非商用；只可抄思想（last-token pooling、任务前缀） |
| cargo vendor | **吸收** | 本地化的硬要求，零争议 |
| sccache | **有界** | 本地磁盘档收益明确；分布式档 experimental |
| PGO | **吸收** | rustc 官方实测 +10~15%（2021–2023 锚），分支密集扫描器同属受益形态 |
| BOLT | **有界** | 在 PGO 之上 +5~6%（锚：rustc/Android）；Windows PE 支持待验证 |

---

## 1. 重点①：扫描引擎的 IO / 解析 / 匹配三层性能架构

### 1.1 业界先例怎么搭（拆解，不是罗列）

**ripgrep（BurntSushi/Andrew Gallant）**——最值得抄的总体形态：
- 并行在**文件级**（walker 多线程遍历 + 每文件独立匹配），不在行级；行内靠 SIMD。
- 单文件内：`memchr` 找行界/罕见字节做**预过滤**（SIMD 快速剔除不可能区域）→ lazy DFA/regex 只跑幸存区。
- 机制结论：**「便宜 SIMD 预过滤 → 贵正则终判」的两段式**是 rg 性能的骨架，不是正则本身快。
- 来源：blog.burntsushi.net/ripgrep/（2016 原文，机制至今未过时）；memchr 仓库 README "algo" 节。

**Intel Hyperscan / Vectorscan（开源移植）**——多模式匹配的另一个天花板：
- 字面量预过滤（FDR）+ NFA/DFA 混合，全 SIMD；单核多 Gbps 量级（厂商口径，未复测）。
- 机制可抄：**先按「最罕见字面量组合」筛，再进状态机**；Vectorscan（ARM 移植，2024–2025 活跃，BSD 系许可待核）证明该思路可移植。
- 适用范围：签名数量大（数百~数万条字面量规则）时才划算；几十条规则 aho-corasick 就够。

**KeyHog（2025 出现的 Rust GPU 秘密扫描器，lib.rs 收录）**——GPU 档的直接先例：
- 显式选择加速器，初始化失败**可见报错而非静默回退 CPU**；这一点与 unified-rx 的 rust/gpu/cpu 三档如实上报一致，可视为业界收敛。
- 证明「扫描器 + GPU 加速档」在 2025 已有公开同类，不是异想天开。

**simdjson**（对照物）：两遍解析、SIMD 内层 + 标量外层，GB/s 级。可抄思想：**SIMD 负责「分类/标记」，标量负责「结算」**。

### 1.2 三层组合架构（建议蓝图）

```
[IO 层]  并行 walker（文件级 work-stealing）
         ├─ 小文件（<1–4MB）：普通 buffered read
         └─ 大文件：memmap2 + 顺序预取；Windows 可试 PrefetchVirtualMemory（待验证收益）
[解析层] zerocopy 安全 zero-cast 头部；每文件 bumpalo arena 承接解析垃圾
         记录落 SoA 列式（offsets/lengths/entropy/... 平铺数组）
[匹配层] aho-corasick(Teddy) 字面量预过滤 → memchr 罕见字节定位 → regex lazy DFA 终判
         统计相位（熵/直方图/XOR/MinHash）走 SoA 批处理，大批量时进 GPU 档
[并行层] 不规则文件级并行 = rayon；确定性有序输出 = 保留自建「div_ceil 分块 + 块序合并 + sha256 验证」
```

关键机制判断（带适用范围）：
- **IO 层并行 > 行内并行**：文件间负载不均匀（大小/类型差异大），work-stealing 比静态分块少尾部等待；这也是 rayon 对自建池的核心理由（见 §3）。
- **预过滤省的是终判器的调用次数**，不是字节数——签名匹配的成本大头在「候选进入贵匹配器」的次数（Hyperscan FDR 与 rg prefilter 同构）。
- **SoA 不是审美偏好**：GPU（wgpu）storage buffer 要求连续平铺，SoA 布局使「CPU 统计结构 = GPU 上传缓冲」零转换，这是它在本项目的第一性理由。

---

## 2. SIMD 与搜索

### memchr（ripgrep 的字节扫描原语）
- 一行定位：Rust 生态事实标准的 SIMD 字节/子串搜索，x86_64 SSE2 基线 + 运行时检测 AVX2、aarch64 NEON。
- 值得抄的机制：
  1. **双 32 字节向量成对比较**：块首与块尾各比一次向量，双命中才回标量确认——把假阳性压到几乎不付成本（算法见 README "algo" 节，源自 Wojciech Muła 的 SIMD 子串搜索）。
  2. **罕见字节启发式**：从模式里挑出现概率最低的字节先行扫描（memchr2/memchr3 同时比 2–3 个候选字节）。
  3. **专用计数路径**：ripgrep 用 memchr 的 count 例程做行数统计——统计类 API 与搜索 API 分开写，各有优化。
- 档位：**吸收**。理由：这就是「扫描器字节层」的完成态，自写只会更慢；MIT/Apache 双许可。
- 第一步动作：新引擎的行扫描、字节签名定位、计数统计全部改走 memchr 家族；删掉自写逐字节循环。
- 许可证：MIT OR Apache-2.0。
- 链接：https://github.com/BurntSushi/memchr ；https://blog.burntsushi.net/ripgrep/

### aho-corasick + regex（匹配层配套）
- 一行定位：多字面量并行匹配（SIMD Teddy 算法）+ lazy DFA 正则引擎，rg 的另外两块积木。
- 值得抄的机制：
  1. Teddy：一次 SIMD 周期内并行比对多个模式的首/次字节，候选再精查——字面量签名库（凭据模式、熵标签）直接受益。
  2. regex crate 的 prefilter 集成：字面量前缀自动下沉到 memmem/memchr，正则只在候选区间跑。
- 档位：**吸收**。
- 第一步动作：把现有「签名表」分成字面量层（aho-corasick）与结构层（regex），统计两层的命中成本差（先量后改）。
- 许可证：两者皆 MIT OR Apache-2.0。
- 链接：https://github.com/BurntSushi/aho-corasick ；https://github.com/rust-lang/regex

### std::simd（portable-simd）——2026 现状
- 一行定位：`#![feature(portable_simd)]`，**2026-10 仍 nightly-only**，跟踪 issue rust-lang/rust#86656，稳定化阻塞项列表在 rust-lang/portable-simd#364（target feature 语义等未定）。
- 值得抄的机制：无（它自己还没定型）；可观察其 API 形状为未来迁移做准备。
- 档位：**不吸收**。理由：本地优先工具的核心依赖进 nightly = 供应链与 CI 双重风险；语义未定意味着升级即重写。
- 第一步动作：主路径用 **stable `core::arch` intrinsics + 运行时 `is_x86_feature_detected!` 多版本分发**（memchr 正是此路线；safe intrinsics 自 Rust 1.87 起稳定）。关注 fearless_simd（BurntSushi 2025 起的新便携 SIMD crate，成熟度待验证）作为 std::simd 的稳定替身候选。
- 许可证：不适用（语言特性）。
- 链接：https://github.com/rust-lang/rust/issues/86656 ；https://github.com/rust-lang/portable-simd/issues/364

### wide crate
- 一行定位：稳定通道的便携固定宽度向量类型（f32x4/i32x8 等），LLVM IR 向量化直通。
- 值得抄的机制：便携 API 下沉到 LLVM 向量 IR，编译期即可测试；适合内积/距离/统计这类「写一遍就好」的算子。
- 档位：**有界**。理由：快不到 memchr 手写特化档的天花板，但换来跨架构与稳定通道；适合「次热路径」。
- 第一步动作：嵌入向量内积/余弦的 CPU 路径试用 wide，与标量版出 criterion 对比再定。
- 许可证：宽松系，具体条款**待验证**（以仓库 LICENSE 为准）。
- 链接：https://github.com/Lokathor/wide

---

## 3. 并行：rayon vs 自建线程池

- 一行定位：rayon = work-stealing（Chase-Lev 双端队列，闭包/任务被偷半段）+ fork-join；自建通道池 = 固定 worker + 共享数据队列；两者是**「偷任务」与「派数据」**的机制差异（users.rust-lang.org 2024-01 讨论口径）。
- 机制差异要点：
  1. rayon 的窃取把不均匀负载摊平——文件扫描正是「有 4KB 也有 400MB」的典型不均匀场景；静态 div_ceil 分块在大文件块上留尾部等待。
  2. 任务粒度门槛：社区口径（reintech 指南、corrode.dev 2025-05 "Sharp Edges"）——单任务 ≥ 数 µs 才值得进池，>100µs 时两种方案差异趋近于零；扫描器的逐文件/逐块任务都在 µs~ms 级，处于 rayon 优势区。
  3. 自建池的真实优势在**确定性与角色固定**：块序合并、按序写盘、GPU 队列独占线程——rayon 不直接给这些。
- 档位：**吸收（rayon 于文件级并行）+ 有界（保留自建于有序相位）**。理由：混合制各取机制长处，不是二选一。
- 第一步动作：把文件遍历扇出改 rayon（`par_iter` + `Scope`），与自建池在混合大小文件语料上比 p99 耗时（判据：p99 与窗口均值，单点不算）；GPU 相位与有序合并相位保持自建。
- 许可证：rayon MIT OR Apache-2.0。
- 链接：https://corrode.dev/blog/ （2025-05 Sharp Edges）；https://users.rust-lang.org/ （2024-01 线程池机制讨论）；Chalmers 学位论文《Parallelization in Rust with fork-join and friends》（publications.lib.chalmers.se）

---

## 4. IO 与内存

### memmap2
- 一行定位：跨平台 mmap 封装（memmap-rs 的维护续命版）。
- 值得抄的机制：
  1. **mmap 不是免费午餐**：ripgrep 曾默认大文件用 mmap 后撤销默认（2018 前后，rg guide 记录）——小/中文件上 read() 系统调用更快，且 mmap 有 SIGBUS 类失败模式与页故障抖动；mmap 的真优势在随机访问与零拷贝只读。
  2. 大文件顺序扫配 `madvise(SEQUENTIAL)` / Windows PrefetchVirtualMemory（收益待验证）。
- 档位：**有界（只给大文件档）**。
- 第一步动作：以 1MB/4MB/16MB 阈值做 read vs mmap 分档基准，出拐点数字后冻结阈值（先量后改）。
- 许可证：MIT OR Apache-2.0（待核仓库）。
- 链接：https://github.com/RazrFalcon/memmap2

### zerocopy
- 一行定位：Google 出品的 derive 化安全转译（`FromBytes`/`IntoBytes`/`KnownLayout`），Android/Chromium 级生产背书。
- 值得抄的机制：`pod_read_unaligned` / slice 转换把「解析文件头」变成零拷贝且内存安全，删掉整个 unsafe transmute 面。
- 档位：**吸收**。
- 第一步动作：所有 PE/ELF/结构化头解析改 zerocopy derive。
- 许可证：Apache-2.0 / BSD-3 系双许可。
- 链接：https://github.com/google/zerocopy

### bumpalo / arena
- 一行定位：bump 指针分配器，O(1) 分配、整块回收。
- 值得抄的机制：**逐文件 arena**——每文件解析产生的小结构全部挂 arena，文件处理完整块丢，GC 式回收且缓存友好；跨文件不共享 arena（避免放大峰值内存）。
- 档位：**有界**（全工程 arena 化过度；解析热点值得）。
- 第一步动作：解析层先加 arena，用分配次数/吞吐前后对比验证（无测量不上）。
- 许可证：MIT OR Apache-2.0。
- 链接：https://github.com/fitzgen/bumpalo

### SoA 列式布局
- 一行定位：把 per-file 结构体数组换成「每字段平铺数组」（polars/arrow 的组织方式）。
- 值得抄的机制：统计批处理按列扫（熵、大小、扩展名各成数组）→ 缓存只装需要的字段；**列数组可直接作 wgpu storage buffer 上传**，CPU/GPU 数据结构零转换。
- 档位：**吸收**。
- 第一步动作：定义核心记录集（path/size/entropy/hashes/…）的 SoA schema，作为扫描产物唯一格式。
- 许可证：不适用（数据布局模式）。

### 异步文件 IO（IOCP / io_uring：compio、tokio-uring）——对扫描工具值不值
- 一行定位：compio = thread-per-core 完成型运行时（IOCP/io_uring/polling 三后端，OpenDAL 已采用，2025 活跃）；tokio-uring = 已基本沉寂（2022–2023 后无实质活动，社区视 compio 为继任）；monoio = 字节跳动 thread-per-core（io_uring 为主，Linux 侧）。
- 机制判断：异步文件 IO 解决的是「**海量并发阻塞等待**」；本地扫描器的 IO 真相是——mmap/顺序读 + 多线程早已把磁盘带宽吃满，异步不增加带宽只增加调度复杂度；Windows 上目标盘（HDD/NVMe 混合）下收益更存疑。**待验证**：NVMe 深队列（32+）+ 4KB 随机小文件海量场景下 compio IOCP 是否跑赢阻塞线程池——这是唯一可能翻身的场景。
- 档位：**不吸收（扫描内核）/ 有界（MCP 服务器外壳用 tokio 管网络并发即可）**。
- 第一步动作：不加；在「海量小文件」基准里留一个 compio 对照实验位（一眼否证/证实的判据：同语料总耗时 ±5%）。
- 许可证：compio Apache-2.0（待核）；tokio-uring MIT。
- 链接：https://github.com/compio-rs/compio ；https://github.com/tokio-rs/tokio-uring ；https://github.com/bytedance/monoio

---

## 5. GPU：wgpu 计算的适用边界（重点③）

### 边界的量化口径
- 每次 dispatch 有 µs~几十 µs 级 CPU/驱动开销；**回读（map_async + poll）比 dispatch 贵一个量级**（r/opengl 2025 讨论、Khronos 论坛案例：小负载上 compute 反而更慢的真实案例）。
- 社区共识量级：**1 万~百万级独立数据并行项**才值得上卡；数据已在 GPU 常驻（无回读）时拐点下移；PCIe/UMA 双程搬运会吃掉小批量全部收益。
- 内部锚（unified-rx 实测，单机 Windows）：批量熵计算 GPU 31–38× 于 CPU，且「IO/编排类不接」——与外部口径一致，可以沿用分相位结论。

### 扫描器相位清单（哪些值得上卡）

| 相位 | 档位 | 判据 |
|---|---|---|
| 字节直方图 / 批量熵 | **上卡** | 大批量统计、算术密度高；实测 31–38×（内部锚） |
| 单字节 XOR 穷举 | **上卡** | 256 个平行全扫，embarrassingly parallel |
| 批量 MinHash / 指纹 | **上卡（候选）** | 大文件集 n-gram 批量；需与 CPU SIMD 版对比定档 |
| 嵌入推理 | **上卡但走 ort（DirectML/CUDA EP），不自写 wgpu transformer** | 见 §7 |
| 字面量签名匹配 | **不上卡** | Hyperscan 级 CPU SIMD 已是天花板，PCIe 往返得不偿失 |
| 正则终判 | **不上卡** | 分支密集 + 数据依赖控制流 |
| 污点分析 | **不上卡** | 图遍历 + 分支密集，标量逻辑 |
| 解析/头部 | **不上卡** | 小对象、分支多、需要 CPU 侧指针 |
| IO / 编排 | **不上卡** | 与 GPU 无交集（内部锚口径） |

### KeyHog 的机制可抄
显式加速器选择 + 失败可见（不静默回退）→ 与 unified-rx「rust/gpu/cpu 三档如实上报」收敛；蓝图里固化成规则：**GPU 档缺席必须是显式状态，不是静默降级**。

- 档位：**有界（按相位）**。
- 第一步动作：GPU 档只保留「批量熵 + XOR 穷举」两个已验证相位起步；persistent buffer + 批量 dispatch + 异步回读（双缓冲），禁止逐文件回读；对每个候选相位出「1K/10K/100K/1M 元素」拐点曲线再定档。
- 许可证：wgpu MIT OR Apache-2.0。
- 链接：https://wgpu.rs ；https://lib.rs/crates/keyhog（KeyHog，2025 收录）；https://www.reddit.com/r/opengl/comments/1kpjcu5/（dispatch 开销讨论）

---

## 6. Profiling：tracing 生态 + pprof + ETW/WPA

- **tracing + tracing-chrome**：一行定位：结构化 span 树 + 导出 Chrome Trace JSON（perfetto.dev 可视化）。值得抄：扫描全程 span 化，**时间线档案 = 可复现的否证记录**（否证标签绑时间戳与窗口口径）；per-layer subscriber 让「给用户看的日志」和「给性能档案的 trace」分离。档位：**吸收**。许可证 MIT 系（tracing-chrome 待核）。链接：https://github.com/tokio-rs/tracing ；https://github.com/BeaconBrigade/tracing-chrome
- **pprof crate**：一行定位：进程内采样器出 pprof/火焰图。值得抄：生产构建可挂超采样率（低开销持续采样）。档位：**有界**——Windows 支持成熟度**待验证**（信号处理器实现以 Unix 为主）。链接：https://github.com/tikv/pprof-rs
- **samply**：Mozilla 的跨平台采样 profiler（外置进程采样，天然支持 Windows，成熟度待验证）。档位：**有界（首选试用）**。链接：https://github.com/mstange/samply
- **ETW / WPA**：一行定位：Windows 系统级真相（文件 IO、CPU 调度、页故障），xperf/wpr 录制 + WPA 分析。值得抄的机制：**mmap 扫描的页故障、磁盘队列深度只有 ETW 看得见**——它是「IO 层是否真是瓶颈」的仲裁者。档位：**有界**（工具链重，只做仲裁场景）。第一步动作：给 IO 分档基准配一份 wpr 录制脚本，把「页故障/秒」与「磁盘队列」写进基准档案。链接：Microsoft Learn WPA 文档。

---

## 7. 本地数据层与检索栈（重点②）

### rusqlite
- 一行定位：SQLite 的 Rust 绑定（bundled 编译模式零外部依赖）。
- 值得抄的机制：单文件库 = 本地优先的极致形态；FTS5 自带 `bm25()` 排序函数（官方文档口径）——**BM25 其实 SQLite 原生就有**。
- 档位：**吸收**。第一步动作：扫描产物索引库 schema 定稿（FTS5 表 + vec0 表 + 元数据表同文件）。
- 许可证：MIT。链接：https://github.com/rusqlite/rusqlite

### Tantivy（Rust 全文引擎）vs 自建 BM25
- 一行定位：Lucene 血统的 Rust 全文引擎（BM25、分段倒排、增量索引），ParadeDB/Quickwit 等生产采用。
- 与自建 BM25 的差距：自建版（unified-rx，tf-idf/BM25 + 符号加权）已满足现有语料并已测量；Tantivy 的真优势在**增量索引、分段合并、查询并发、聚合/facet**——这些是「索引会长大、会更新」才需要的能力；对「扫描后重建一次」的静态语料，优势缩水。质量差距（相关性排序）：无公开 head-to-head 数据，**待验证**（ tantivy 官方 benchmark 页 vs Lucene 的历史数字 1.5–2× 量级，未复测）。
- 生态信号：CodeIndex 等代码检索项目用 tantivy + 符号/路径 boost——与 unified-rx 的 BM25 符号加权同构，证明思路正确且可由 tantivy 承接。
- 档位：**有界**。理由：先自建对照（零依赖、已有测量），语料规模/更新频率超出自建能力时切 tantivy；两者通过 trait 切换。
- 第一步动作：定义切换判据（语料 chunk 数、增量更新频率、查询 p99），写进蓝图而不是现在二选一。
- 许可证：MIT。链接：https://github.com/quickwit-oss/tantivy

### sqlite-vec
- 一行定位：Alex Garcia 的纯 C 零依赖 SQLite 向量扩展（sqlite-vss 继任），vec0 虚表 + chunked 存储 + **brute-force 精确 KNN**，支持 float/int8/binary 量化。
- 机制要点：v0.1.0 起按 chunk 读盘做 KNN，全库不必进内存（OLTP 写入不退化）；**ANN 索引至今未落地（pre-v1，breaking changes 预期）**；存在 sqliteai/sqlite-vector 分支（2025 有独立发布，选型时区分）。
- 适用量级：brute-force + int8 量化对 **~10 万–百万级向量**够用（官方口径「runs everywhere」，具体毫秒数**待验证**——需在本机以 384/896 维 × 1e5 chunks 实测）。
- 档位：**有界（倾向吸收）**。第一步动作：1e5 × 896 维 int8 本机实测 QPS/延迟，冻结「量化维度 + 阈值」。
- 许可证：MIT。链接：https://github.com/asg017/sqlite-vec ；https://alexgarcia.xyz/sqlite-vec

### 检索栈选型结论（Q2 答案）
三候选矩阵：

| 方案 | 质量 | 维护成本 | 离线 | 判定 |
|---|---|---|---|---|
| 自建 BM25/tf-idf（现状） | 已测量、符号加权个性化 | 全自担，升级靠自己 | ✔ | 保留为对照基线 |
| Tantivy 单栈 | 成熟、增量索引 | 中（依赖重 ~百 crate 级，待核） | ✔ | 规模触发后切换 |
| **rusqlite FTS5 + sqlite-vec 单文件 + RRF 融合** | FTS5 bm25 + 向量，双路 RRF（unified-rx 已验证融合模式） | 低（两个小依赖） | ✔ | **默认档** |

- 决定逻辑：本地优先的第一性要求是「少而小的依赖 + 单文件产物」；FTS5+sqlite-vec 在同库同文件里同时给 BM25 与向量，RRF 融合已有先例；自建 BM25 不是扔掉，是降级为「FTS5 的对照与 fallback」。Tantivy 的增量索引能力在「索引常更新」场景触发切换（判据见上）。

### 本地嵌入栈：fastembed-rs / ort / candle
- **fastembed-rs**（Qdrant）：一行定位：本地 ONNX 嵌入（模型自动下载、int8 量化优先），Rust 本地嵌入事实标准，被大量项目当默认后端。档位：**吸收**。许可证 MIT（据 lib.rs 口径，待核仓库）。链接：https://github.com/Qdrant/fastembed-rs
- **ort（ONNX Runtime 绑定）**：一行定位：pyke 维护的 ORT 绑定（v2.0.0-rc 口径），支持 DirectML/CUDA EP——**Windows 上不用装 CUDA 也能吃 GPU**。机制锚：Manticore 2025 博客报告重写嵌入推理（ONNX 路线）后 **14× 提速**；社区共识（Reddit r/rust 2025）：candle「能跑但不够快不够便携」，**推理选 ort，实验选 candle**。档位：**吸收**。许可证 MIT OR Apache-2.0。链接：https://ort.pyke.io ；https://manticoresearch.com/blog/onnx-embeddings-speedup
- **candle**（HuggingFace）：纯 Rust 张量库。档位：**有界（不用于生产推理路径）**。许可证 Apache-2.0。链接：https://github.com/huggingface/candle
- 另：纯 Rust ONNX 运行时（tract 系，CPU/AVX-512/NEON，无 C++ 链接）存在，作为「零 C++ 依赖」极端档候选（成熟度待验证）。

### 2025–2026 小尺寸代码嵌入模型（本地可跑性）
| 模型 | 规模 | 许可证 | 本地判定 |
|---|---|---|---|
| **Qwen3-Embedding-0.6B**（2025-06，arXiv:2506.05176） | 0.6B / 28 层 / 32K ctx / MRL 可截断维度 | **Apache-2.0** | **默认选**；int8 后 ~0.6GB，CPU 可跑、GPU 更佳（本机吞吐待验证） |
| jina-code-embeddings-0.5b（2025，arXiv:2508.21290） | 494M / 896 维 / Qwen2.5-Coder 底座 / last-token pooling / 30+ 语言 | **CC-BY-NC（非商用）** | 不吸收为依赖；**只抄思想**（代码任务前缀指令、last-token pooling） |
| nomic-embed-text-v1.5 | 137M | Apache-2.0（待核） | 低端机档候选 |
- 机制判断：0.6B int8 在现代桌面 CPU 上「能跑但慢」，语料 1e5 chunk 级别首次索引可接受（分钟级，具体数**待验证**）；增量索引使稳态成本可控；代码检索质量上 Qwen3-Embedding 系列是 2025 后半 Apache 许可里的事实默认。

---

## 8. 构建性能与本地化

### cargo vendor
- 一行定位：把全部依赖拉平进仓内 + `.cargo/config.toml` source replacement，构建完全离线。
- 档位：**吸收**（本地优先的硬要求，零争议）。第一步动作：CI 与开发双轨锁定 vendor 目录；注意 vendor 体积与 C 依赖（sqlite3）就地编译。
- 许可证：不适用（cargo 内建）。链接：https://doc.rust-lang.org/cargo/commands/cargo-vendor.html

### sccache（自托管）
- 一行定位：Mozilla 编译缓存（默认本地磁盘；支持 GHA 缓存后端；分布式 dist server 仍是 experimental 口径）。
- 机制：按输入哈希缓存 C/C++/Rust 编译产物，跨分支/跨 worktree 复用；对「重写期高频全量构建」收益直接。
- 档位：**有界**——本地磁盘档先用；分布式档等 experimental 标签摘掉再评估。
- 第一步动作：sccache 本地磁盘档 + 构建脚本埋命中率统计（`--show-stats` 进基准档案）。
- 许可证：Apache-2.0。链接：https://github.com/mozilla/sccache

### PGO / BOLT 对扫描器二进制的收益
- 数字锚（全部有出处）：
  - rustc 官方上 PGO：**+10~15%** 编译速度（Linux ~12%、Windows ~15%，2021 rustc 上游记录）；rustc CI 现用 LTO+PGO+BOLT 优化 LLVM 本身（Kobzol 2023-07 博客）。
  - Android Rust 工具链（Google 2023-12 官方博客）：PGO **+19.8%**；叠 BOLT **合计 +24.7%**（BOLT 净贡献 ~+6.1%），体积 +10.9%。
  - 一般 Rust 二进制：社区口径 BOLT+PGO 常 **+3~5%**；个别 clang/gcc 程序 BOLT+LTO+PGO 达 34–68.5%（论坛精选数据，样本偏置，不外推）。
- 机制判断：扫描器 = 分支密集 + 热循环布局敏感，属 PGO 受益形态；**PGO 是收益/成本比最优项，BOLT 是 PGO 之后的边际项**。
- 平台注意：PGO 在 MSVC 工具链可用；BOLT 主战场是 ELF，**Windows PE 支持（待验证）**——Windows 主交付物上 BOLT 可能不可用，这是「有界」的主因。
- 档位：PGO **吸收**；BOLT **有界**。
- 第一步动作：拿真实扫描语料当 profile 工作负载，release+PGO 与基线出对比数字（判据：同语料总耗时与 p99，窗口均值口径）；BOLT 只在 Linux 交付档评估。
- 许可证：工具链层面（LLVM/BOLT Apache-2.0 系）。链接：https://kobzol.github.io/rust/rustc/2023/07/30/optimizing-rust-ci-2023.html ；https://android-developers.googleblog.com/2023/12/faster-rust-toolchains-for-android.html ；https://news.ycombinator.com/item?id=25060762

---

## 9. 2025–2026 前沿信号

1. **std::simd 仍未稳定（2026-10）**：阻塞项集中在 target-feature 语义；同期 safe `core::arch` intrinsics（1.87+）成熟，手写特化 + 运行时检测是 rg/memchr 系的事实路线。信号源：rust#86656、portable-simd#364。
2. **fearless_simd**（BurntSushi，2025 起）：便携 SIMD 的稳定通道新尝试，与 memchr 作者绑定 = 高信誉信号；成熟度待观察。
3. **completion-based IO 洗牌完成**：tokio-uring 沉寂，compio 成为事实继任（OpenDAL 采用、GitHub 2025-09 thread-per-core 讨论列其为候选）；但本地扫描器结论不变：先证明 IO 是瓶颈再说。
4. **扫描器 GPU 档出现公开同类**：KeyHog（Rust GPU 秘密扫描器，2025）确立「显式加速器选择 + 失败可见」范式——与本仓三档如实上报收敛。
5. **嵌入模型本地化定型**：Qwen3-Embedding 系列（2025-06，Apache-2.0，0.6B/4B/8B + reranker）成为本地代码嵌入默认；jina-code-embeddings（2025-08 论文）质量强但 CC-BY-NC。**推理运行时选型比模型选择更影响吞吐**（Manticore：ONNX 路线重写 14×）。
6. **PGO/BOLT 数字已官方化**：Android 工具链 PGO+BOLT +24.7%（2023-12 官方锚）是当前最佳公开参照；2025 后未见更高量级公开数据。
7. **sqlite-vec 仍 pre-v1**：ANN 未落地、breaking 预期；同时出现企业分支（sqliteai/sqlite-vector，2025 独立发布）——选型需盯上游。
8. **ParadeDB 把 tantivy 包进 Postgres**：tantivy 生产成熟度再加一层证据；代码检索项目（CodeIndex 等）已在用 tantivy + 符号 boost，验证本仓既有思路。

---

## 10. Top-3

1. **匹配层「两段式」照抄 rg 骨架**：aho-corasick(Teddy) 字面量预过滤 + memchr 罕见字节定位 + regex lazy DFA 终判；SIMD 走 stable `core::arch` + 运行时检测，std::simd/wide 都不进核心路径。这是花小钱（三个 MIT/Apache 依赖）买 rg 十年调优的最大杠杆。
2. **并行混合制 + IO 分档**：文件级不规则并行交 rayon（work-stealing 摊平不均匀负载），有序相位保留自建「分块+块序合并+sha256」；IO 按 read/mmap 双档出拐点数字后冻结阈值；异步文件 IO 不进扫描内核（唯一待翻身场景：NVMe 深队列海量小文件，留对照实验）。
3. **检索栈默认「SQLite 单文件全家桶」**：rusqlite FTS5(bm25) + sqlite-vec(int8 量化) + RRF 双路融合，嵌入走 fastembed/ort + Qwen3-Embedding-0.6B（Apache-2.0）；自建 BM25 降为对照基线，Tantivy 按「语料规模/增量更新」判据触发切换。GPU 档只保留已验证相位（批量熵/XOR），ort(DirectML) 承接嵌入推理，全部按相位拐点曲线定档。

---

### 附：本篇数字出处索引
- memchr 算法与 rg 架构：github.com/BurntSushi/memchr（README algo 节）、blog.burntsushi.net/ripgrep/
- std::simd 状态：github.com/rust-lang/rust/issues/86656、github.com/rust-lang/portable-simd/issues/364、doc.rust-lang.org/beta/unstable-book（portable-simd）
- rayon 机制与门槛：corrode.dev（2025-05）、users.rust-lang.org（2024-01）、Chalmers 论文、reintech.io
- compio/tokio-uring/monoio：github.com/compio-rs/compio、github.com/tokio-rs/tokio-uring、GitHub issue #4658（2025-09 thread-per-core 讨论）
- GPU 边界：reddit.com/r/opengl（2025 dispatch 开销线程）、lib.rs（KeyHog）、Khronos 论坛案例；内部锚「unified-rx 实测 31–38×」
- sqlite-vec：github.com/asg017/sqlite-vec、alexgarcia.xyz/sqlite-vec、marcobambini.substack.com（State of Vector Search in SQLite）、github.com/sqliteai/sqlite-vector
- tantivy：github.com/quickwit-oss/tantivy、ParadeDB/CodeIndex 生态引用；vs Lucene 数字待验证
- 嵌入栈：github.com/Qdrant/fastembed-rs、ort.pyke.io、manticoresearch.com/blog/onnx-embeddings-speedup（14×）、HF Qwen/Qwen3-Embedding-0.6B（Apache-2.0）、arXiv:2506.05176、jina.ai/models/jina-code-embeddings-0.5b（CC-BY-NC）、arXiv:2508.21290
- PGO/BOLT：HN 25060762（rustc PGO ~10–15%）、kobzol.github.io（2023-07-30）、android-developers.googleblog.com（2023-12，PGO 19.8%→+BOLT 24.7%）、users.rust-lang.org（一般二进制 3–5%）
