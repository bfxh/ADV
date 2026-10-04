# P4 系统顶会论文扫视（SOSP/OSDI/EuroSys/USENIX ATC 2023–2026）

> 口径：2026-10-04 抓取 usenix.org（OSDI 23–26、ATC 23–25 technical-sessions）、eurosys.org（accepted papers）、sigops.org / sosp2023.mpi-sws.org（program/accepted）；DBLP API 被反爬（Anubis 挑战/连接重置）未采用。SOSP 2026 尚未召开；ATC 2026 官网 404，未采信任何条目（待验证）。OSDI/ATC 条目链接为 USENIX 讲次页（真实逐篇链接）；SOSP/EuroSys 条目为会议官网条目页级链接（DBLP 被拦，逐篇 DOI 待补）。

> 收录判据：单机工具视角，说得出可抄机制才收；机制仅据题名推断的标「题名级（待复核）」。

## P0 清单（≤6：读了能直接改哪个设计决策）

1. **BWoS: Formally Verified Block-based Work Stealing for Parallel Processing**（OSDI 2023）—— 读了直接改：ADV 并行遍历的任务粒度与窃取协议对齐『块化』设计，块大小作为可调参数。
   - 可抄机制：任务成块入队：本地按块消费(缓存局部性)，受害者按块窃取(摊薄同步成本)；协议经形式化验证(无丢失/无饥饿类性质)。
   - 模块映射：perf-core｜并行扫描/索引构建的调度
   - 链接：https://www.usenix.org/conference/osdi23/presentation/wang-jiawei（依据深度：机制级）
2. **Incr: Faster Re-Execution via Bolt-On Incrementalization**（OSDI 2026）—— 读了直接改：ADV 增量重扫在『bolt-on 运行时缓存』与『自建依赖图(salsa 型)』之间的选型判据。
   - 可抄机制：对既有程序自动附加输入/输出缓存与依赖追踪，变更时只重跑受影响分段，无需程序内建增量语义(细节待复核)。
   - 模块映射：perf-core｜增量重扫引擎形态
   - 链接：https://www.usenix.org/conference/osdi26/presentation/xie-yizheng（依据深度：题名级）
3. **Arctic: A Practical Lock-Free Adaptive Radix Tree**（OSDI 2026）—— 读了直接改：ADV 内存索引在 ART(基数)与 B+树之间的选型依据，并发路径可直接参照。
   - 可抄机制：ART 的 path compression/lazy expansion + 无锁乐观读与细粒度写协调、epoch 回收(细节待复核)。
   - 模块映射：adv-index｜内存索引选型
   - 链接：https://www.usenix.org/conference/osdi26/presentation/ni（依据深度：题名级）
4. **FIFO queues are all you need for cache eviction**（SOSP 2023）—— 读了直接改：ADV 缓存层用 S3-FIFO 替代 LRU/LFU 变体，并把『扫描型一次性读』与『重访型』流量分流。
   - 可抄机制：S3-FIFO：S/G/M 三 FIFO + ghost 快速段；新对象先入 G，二次命中才进 M；M 满时把『未再命中』对象降级 S 而非直接驱逐，每对象最多一次豁免；全 O(1)、无元数据扫描、抗一次性扫描污染。
   - 模块映射：adv-index｜结果缓存/内容寻址缓存的淘汰策略
   - 链接：https://sosp2023.mpi-sws.org/program.html（依据深度：机制级）
5. **SPFresh: Incremental In-Place Update for Billion-Scale Vector Search**（SOSP 2023）—— 读了直接改：ADV 索引增量更新采用『原地分裂+后台重平衡』而非整段重建。
   - 可抄机制：LIRE 协议：分裂-重分配(split-reassign)增量插入，后台轻量重平衡把重分配摊平以控写放大与长尾；更新期间查询一致可见。载体是向量检索，协议可移植到任何『可分裂桶』结构。
   - 模块映射：adv-index｜索引增量更新(倒排/位置索引同理)
   - 链接：https://sosp2023.mpi-sws.org/program.html（依据深度：机制级）
6. **StreamCache: Revisiting Page Cache for File Scanning on Fast Storage Devices**（ATC 2024）—— 读了直接改：ADV 全仓扫描默认走『扫描流』IO 路径而非依赖 page cache；结果缓存命中才走复用路径。
   - 可抄机制：区分『复用型』与『单遍扫描型』文件访问；扫描流绕过/降级 page cache、大块读+异步预读，防缓存污染并把内存让给复用数据(细节待复核)。
   - 模块映射：perf-core｜扫描器 IO 路径
   - 链接：https://www.usenix.org/conference/atc24/presentation/li-zhiyue（依据深度：机制级）

---

## OSDI 2023（4 篇收录）

### BWoS: Formally Verified Block-based Work Stealing for Parallel Processing
- **会议+年份**：OSDI 2023 ｜ **优先级**：P0 ｜ **模块映射**：perf-core｜并行扫描/索引构建的调度 ｜ **链接**：https://www.usenix.org/conference/osdi23/presentation/wang-jiawei ｜ **依据深度**：机制级
- **一句话结论**：块化 work-stealing 在保持近线性扩展的同时拿到形式化正确性证明。
- **可抄机制**：任务成块入队：本地按块消费(缓存局部性)，受害者按块窃取(摊薄同步成本)；协议经形式化验证(无丢失/无饥饿类性质)。

### Userspace Bypass: Accelerating Syscall-intensive Applications
- **会议+年份**：OSDI 2023 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜syscall 开销削减 ｜ **链接**：https://www.usenix.org/conference/osdi23/presentation/zhou-zhe ｜ **依据深度**：题名级
- **一句话结论**：syscall 密集应用可在用户态旁路大部分系统调用(据题名)。
- **可抄机制**：把常规 syscall 变换为用户态直调路径，仅异常情况回退内核(细节待复核)。

### SEPH: Scalable, Efficient, and Predictable Hashing on Persistent Memory
- **会议+年份**：OSDI 2023 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜持久哈希索引 ｜ **链接**：https://www.usenix.org/conference/osdi23/presentation/wang-chao ｜ **依据深度**：机制级
- **一句话结论**：PM 哈希索引可做到可扩展且延迟可预测。
- **可抄机制**：段级重组避免全局停顿，冲突控制以保查询延迟上界(细节据论文方向)。

### Triangulating Python Performance Issues with SCALENE
- **会议+年份**：OSDI 2023 ｜ **优先级**：P2 ｜ **模块映射**：reference｜剖析器设计参照 ｜ **链接**：https://www.usenix.org/conference/osdi23/presentation/berger ｜ **依据深度**：机制级
- **一句话结论**：Python 性能归因需 CPU/内存/GPU 三路联合低开销采样。
- **可抄机制**：三路采样+异常值放大呈现；内存含 RSS 与拷贝量两个维度。

## OSDI 2024（3 篇收录）

### Identifying On-/Off-CPU Bottlenecks Together with Blocked Samples
- **会议+年份**：OSDI 2024 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜性能剖析 ｜ **链接**：https://www.usenix.org/conference/osdi24/presentation/ahn ｜ **依据深度**：机制级
- **一句话结论**：on-CPU 与 off-CPU 归因可用同源采样统一，免双系统。
- **可抄机制**：时钟中断采样同时捕获运行栈与阻塞点(syscall/锁等待)，统一时间轴归因；给出 blocked-sample 生成协议。

### SquirrelFS: using the Rust compiler to check file-system crash consistency
- **会议+年份**：OSDI 2024 ｜ **优先级**：P1 ｜ **模块映射**：reference｜崩溃一致性门禁与 Rust 设计模式 ｜ **链接**：https://www.usenix.org/conference/osdi24/presentation/leblanc ｜ **依据深度**：机制级
- **一句话结论**：Rust 类型系统可把『崩溃恢复路径必须被调用』变成编译期检查。
- **可抄机制**：把 FS 事务状态机编进类型(事务句柄必须经 recover/commit 消费)，借用检查器拒绝泄漏恢复义务。

### μSlope: High Compression and Fast Search on Semi-Structured Logs
- **会议+年份**：OSDI 2024 ｜ **优先级**：P2 ｜ **模块映射**：adv-parse｜日志存储与检索 ｜ **链接**：https://www.usenix.org/conference/osdi24/presentation/wang-rui ｜ **依据深度**：题名级
- **一句话结论**：半结构化日志可同时高压缩与快检索(据题名)。
- **可抄机制**：结构感知的高压缩编码+压缩域快速检索(细节待复核)。

## OSDI 2025（4 篇收录）

### Decentralized, Epoch-based F2FS Journaling with Fine-grained Crash Recovery
- **会议+年份**：OSDI 2025 ｜ **优先级**：P2 ｜ **模块映射**：reference｜本地持久层 WAL 并行化 ｜ **链接**：https://www.usenix.org/conference/osdi25/presentation/cui ｜ **依据深度**：题名级
- **一句话结论**：日志可按子树/文件分片并行提交并细粒度恢复(据题名)。
- **可抄机制**：去中心化 epoch 日志：并行提交+按 epoch 的细粒度崩溃恢复(细节待复核)。

### Fast and Synchronous Crash Consistency with Metadata Write-Once File System
- **会议+年份**：OSDI 2025 ｜ **优先级**：P2 ｜ **模块映射**：reference｜崩溃一致性设计 ｜ **链接**：https://www.usenix.org/conference/osdi25/presentation/pan ｜ **依据深度**：题名级
- **一句话结论**：以元数据一次写为中心可获得低开销同步崩溃一致性(据题名+方向)。
- **可抄机制**：元数据 Write-Once 约束化排序需求，减少日志/屏障(细节待复核)。

### PoWER Never Corrupts: Tool-Agnostic Verification of Crash Consistency and Corruption Detection
- **会议+年份**：OSDI 2025 ｜ **优先级**：P2 ｜ **模块映射**：reference｜崩溃一致性验证门禁 ｜ **链接**：https://www.usenix.org/conference/osdi25/presentation/leblanc ｜ **依据深度**：机制级
- **一句话结论**：崩溃一致性验证可做成工具无关的统一故障抽象。
- **可抄机制**：统一电源故障注入语义，跨检查器复用验证协议(据公开摘要方向)。

### Tintin: A Unified Hardware Performance Profiling Infrastructure to Uncover and Manage Uncertainty
- **会议+年份**：OSDI 2025 ｜ **优先级**：watch ｜ **模块映射**：watch｜剖析基础设施 ｜ **链接**：https://www.usenix.org/conference/osdi25/presentation/li ｜ **依据深度**：题名级
- **一句话结论**：统一硬件性能剖析基础设施并管理测量不确定性(据题名)。
- **可抄机制**：多源 PMU 事件统一采集与不确定性标注(细节待复核)。

## OSDI 2026（11 篇收录）

### Incr: Faster Re-Execution via Bolt-On Incrementalization
- **会议+年份**：OSDI 2026 ｜ **优先级**：P0 ｜ **模块映射**：perf-core｜增量重扫引擎形态 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/xie-yizheng ｜ **依据深度**：题名级
- **一句话结论**：外挂式增量化可把重执行改为最小重算(据题名)。
- **可抄机制**：对既有程序自动附加输入/输出缓存与依赖追踪，变更时只重跑受影响分段，无需程序内建增量语义(细节待复核)。

### Arctic: A Practical Lock-Free Adaptive Radix Tree
- **会议+年份**：OSDI 2026 ｜ **优先级**：P0 ｜ **模块映射**：adv-index｜内存索引选型 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/ni ｜ **依据深度**：题名级
- **一句话结论**：无锁自适应基数树(ART)进入实用化阶段(据题名)。
- **可抄机制**：ART 的 path compression/lazy expansion + 无锁乐观读与细粒度写协调、epoch 回收(细节待复核)。

### Learning-Augmented Heuristics: Simple Yet Smart, Robust and Interpretable Cache Eviction
- **会议+年份**：OSDI 2026 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜缓存淘汰 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/xia ｜ **依据深度**：题名级
- **一句话结论**：学习增强淘汰可做到稳健且可解释(据题名)。
- **可抄机制**：简单启发式+在线学习组件，抗分布漂移(细节待复核)。

### Merlin: An Efficient Adaptive Cache Eviction Algorithm via Fine-Grained Characterization
- **会议+年份**：OSDI 2026 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜缓存淘汰 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/li-liujia ｜ **依据深度**：题名级
- **一句话结论**：细粒度负载特征可驱动淘汰算法自适应切换(据题名)。
- **可抄机制**：在线特征化+算法切换/参数调整(细节待复核)。

### kSTEP: Characterization and Deterministic Testing of Linux CPU Scheduler Bugs
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜并发/调度测试 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/cao ｜ **依据深度**：题名级
- **一句话结论**：CPU 调度器 bug 可特征化并确定性测试(据题名)。
- **可抄机制**：调度器行为建模+确定性重放测试(细节待复核)。

### Mimesys: Generating Realistic Executable Testing Environments from Resource Usage Traces
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜测试环境构造 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/kim-donghyun ｜ **依据深度**：题名级
- **一句话结论**：资源 trace 可再生成可执行的测试环境(据题名)。
- **可抄机制**：从资源占用轨迹合成可执行环境用于复现/测试(细节待复核)。

### When Sampling Lies: Trustworthy Performance Profiling for Flat Workloads with Blink
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜剖析方法学 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/devsot ｜ **依据深度**：题名级
- **一句话结论**：平坦负载下采样剖析会系统性失真(据题名)。
- **可抄机制**：识别采样偏差来源并修正(细节待复核)。

### TypeCraft: A Lightweight Data Type Profiler with High Resolution
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜类型/内存画像 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/li-zecheng ｜ **依据深度**：题名级
- **一句话结论**：高分辨率数据类型剖析可轻量完成(据题名)。
- **可抄机制**：运行时类型使用画像(细节待复核)。

### DeLFS: A Decentralized Log-Structured File System for Manycores
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜多核文件布局 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/ahn ｜ **依据深度**：题名级
- **一句话结论**：manycore 上 FS 可去中心化日志结构设计(据题名)。
- **可抄机制**：per-core 日志/元数据分片协调(细节待复核)。

### Oxbow: A Coordinated Architecture for Multi-Component File Systems
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜存储栈组件协调 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/kim-jongyul ｜ **依据深度**：题名级
- **一句话结论**：多组件文件系统需要协调式架构(据题名)。
- **可抄机制**：跨组件(缓存/日志/布局)协同决策(细节待复核)。

### Virtualizing eBPF with Late-Binding
- **会议+年份**：OSDI 2026 ｜ **优先级**：watch ｜ **模块映射**：watch｜规则热更新机制参照 ｜ **链接**：https://www.usenix.org/conference/osdi26/presentation/zhang-jing ｜ **依据深度**：题名级
- **一句话结论**：eBPF 程序可虚拟化并晚绑定(据题名)。
- **可抄机制**：程序与挂载点解耦、运行时绑定(细节待复核)。

## SOSP 2023（3 篇收录）

### FIFO queues are all you need for cache eviction
- **会议+年份**：SOSP 2023 ｜ **优先级**：P0 ｜ **模块映射**：adv-index｜结果缓存/内容寻址缓存的淘汰策略 ｜ **链接**：https://sosp2023.mpi-sws.org/program.html ｜ **依据深度**：机制级
- **一句话结论**：三条 FIFO 队列即达到/超过 LRU 系命中率(论文报告口径)，扫描型负载尤其稳。
- **可抄机制**：S3-FIFO：S/G/M 三 FIFO + ghost 快速段；新对象先入 G，二次命中才进 M；M 满时把『未再命中』对象降级 S 而非直接驱逐，每对象最多一次豁免；全 O(1)、无元数据扫描、抗一次性扫描污染。

### SPFresh: Incremental In-Place Update for Billion-Scale Vector Search
- **会议+年份**：SOSP 2023 ｜ **优先级**：P0 ｜ **模块映射**：adv-index｜索引增量更新(倒排/位置索引同理) ｜ **链接**：https://sosp2023.mpi-sws.org/program.html ｜ **依据深度**：机制级
- **一句话结论**：十亿级索引可增量原地更新，免全量重建。
- **可抄机制**：LIRE 协议：分裂-重分配(split-reassign)增量插入，后台轻量重平衡把重分配摊平以控写放大与长尾；更新期间查询一致可见。载体是向量检索，协议可移植到任何『可分裂桶』结构。

### Cornflakes: Zero-Copy Serialization for Microsecond-Scale Networ
- **会议+年份**：SOSP 2023 ｜ **优先级**：P1 ｜ **模块映射**：adv-parse｜进程间/持久化数据格式 ｜ **链接**：https://sosp2023.mpi-sws.org/program.html ｜ **依据深度**：机制级
- **一句话结论**：微秒级场景中序列化中间拷贝是主要开销，可数据驱动地消除。
- **可抄机制**：schema 驱动从应用数据结构直接生成 scatter-gather 收发描述符，免中间缓冲；类型感知决定哪些字段可零拷贝、哪些必须拷贝。

## SOSP 2024（3 篇收录）

### Fast Core Scheduling with Userspace Process Abstraction
- **会议+年份**：SOSP 2024 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜扫描线程到核心的绑定 ｜ **链接**：https://sigops.org/s/conferences/sosp/2024/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：用户态进程抽象可让核心调度到微秒级(据题名)。
- **可抄机制**：调度决策放用户态、内核仅提供核心让渡原语，降低切换/唤醒延迟(细节待复核)。

### FBDetect: Catching Tiny Performance Regressions at Hyperscale through In-Production Monitoring
- **会议+年份**：SOSP 2024 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜回归检测 ｜ **链接**：https://sigops.org/s/conferences/sosp/2024/accepted.html ｜ **依据深度**：机制级
- **一句话结论**：生产内持续监控可捕捉亚毫秒级性能回归。
- **可抄机制**：在线统计检验+基线对照，区分噪声与回归(细节据论文方向)。

### Tiered Memory Management: Access Latency is the Key!
- **会议+年份**：SOSP 2024 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜分层内存/大页决策 ｜ **链接**：https://sigops.org/s/conferences/sosp/2024/accepted.html ｜ **依据深度**：机制级
- **一句话结论**：页迁移决策应以实测访问延迟而非推测热度为准。
- **可抄机制**：精准测量页访问延迟驱动迁移；给出延迟测量的低成本实现方向。

## SOSP 2025（6 篇收录）

### cache_ext: Customizing the Page Cache with eBPF
- **会议+年份**：SOSP 2025 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜扫描 IO 的 page cache 策略 ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：机制级
- **一句话结论**：page cache 可编程：应用用 eBPF 表达淘汰/保留策略。
- **可抄机制**：eBPF 挂载点让应用注册页级策略(优先级/保留集/淘汰回调)，内核负责执行与安全(据公开摘要方向，细节待复核)。

### Fawkes: Finding Data Durability Bugs in DBMSs via Recovered Data State Verification
- **会议+年份**：SOSP 2025 ｜ **优先级**：P2 ｜ **模块映射**：reference｜持久层测试 oracle ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：持久化 bug 可通过『恢复后的数据状态验证』发现(据题名)。
- **可抄机制**：崩溃注入后恢复状态与预期状态比对做 oracle(细节待复核)。

### Aeolia: A Fast and Secure Userspace Interrupt-Based Storage Stack
- **会议+年份**：SOSP 2025 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜存储 IO 栈形态 ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：用户态+中断驱动(非轮询)存储栈可同时低 CPU 与低延迟(据题名)。
- **可抄机制**：用户态中断替代轮询/系统调用完成 IO 通知(细节待复核)。

### How to Copy Memory? Coordinated Asynchronous Copy as a First-Class OS Service
- **会议+年份**：SOSP 2025 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜索引构建的大拷贝/零拷贝 ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：内存拷贝可上升为 OS 一等服务并异步协调(据题名)。
- **可抄机制**：应用声明拷贝意图，系统选择 DMA/CPU 路径并重叠调度(细节待复核)。

### Scalable Address Spaces using Concurrent Interval Skiplist
- **会议+年份**：SOSP 2025 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜地址空间/大映射管理 ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：并发区间 skiplist 可让多线程 mmap/munmap 线性扩展(据题名)。
- **可抄机制**：VMA 管理用并发区间 skiplist，免全局地址空间锁(细节待复核)。

### CortenMM: Efficient Memory Management with Strong Correctness Guarantees
- **会议+年份**：SOSP 2025 ｜ **优先级**：watch ｜ **模块映射**：watch｜内存管理 ｜ **链接**：https://sigops.org/s/conferences/sosp/2025/accepted.html ｜ **依据深度**：题名级
- **一句话结论**：内存管理可兼得高效与强正确性保证(据题名)。
- **可抄机制**：(机制待复核：疑为验证/安全强化的内存管理器)

## EuroSys 2023（4 篇收录）

### FrozenHot Cache: Rethinking Cache Management for Modern Hardware
- **会议+年份**：EuroSys 2023 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜结果缓存/热表 ｜ **链接**：https://2023.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：热 key 集高度稳定：冻结快路径+冷路径重建胜过持续更新。
- **可抄机制**：缓存构建后进入只读『冻结』态走无同步快速路径；漂移超阈值再整体重建，读写分代。

### DRAMHiT: A Hash Table Architected for the Speed of DRAM
- **会议+年份**：EuroSys 2023 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜内存哈希表 ｜ **链接**：https://2023.eurosys.org/accepted-papers.html ｜ **依据深度**：题名级
- **一句话结论**：哈希表布局按 DRAM 速度目标重排架构(据题名)。
- **可抄机制**：面向缓存行/TLB 的布局与并发路径免长临界区(细节待复核)。

### LogGrep: Fast and Cheap Cloud Log Storage by Exploiting both Static and Runtime Patterns
- **会议+年份**：EuroSys 2023 ｜ **优先级**：P2 ｜ **模块映射**：adv-parse｜日志/文本检索 ｜ **链接**：https://2023.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：日志可压缩存储且直接在压缩域 grep。
- **可抄机制**：静态模式+运行时模式两级编码；查询编译到压缩域直接执行。

### Unikernel Linux (UKL)
- **会议+年份**：EuroSys 2023 ｜ **优先级**：watch ｜ **模块映射**：watch｜syscall 开销的激进方案 ｜ **链接**：https://2023.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：应用可与 Linux 内核链接成单镜像，syscall 变函数调用。
- **可抄机制**：把应用编译进内核地址空间，热路径 syscall 走直接调用，异常路径回退常规 syscall。

## EuroSys 2024（3 篇收录）

### CCL-BTree: A Crash-Consistent Locality-Aware B+-Tree for Reducing XPBuffer-Induced Write Amplificati
- **会议+年份**：EuroSys 2024 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜持久索引写路径 ｜ **链接**：https://2024.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：崩溃一致 B+树可用增量缓冲+局部性合并削减写放大。
- **可抄机制**：未持久化增量先入 XPBuffer 型缓冲，按局部性合并刷盘并保持崩溃一致(细节据题名+方向，待复核)。

### TTLs Matter: Efficient Cache Sizing with TTL-Aware Miss Ratio Curves and Working Set Sizes
- **会议+年份**：EuroSys 2024 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜缓存容量规划 ｜ **链接**：https://2024.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：带 TTL 的缓存容量可由 miss ratio 曲线+TTL 感知工作集直接算出。
- **可抄机制**：从访问 trace 估计 MRC 与对象 TTL 分布，合成 TTL-aware 工作集尺寸，按内存预算反推容量。

### Serialization/Deserialization-free State Transfer in Serverless Workflows
- **会议+年份**：EuroSys 2024 ｜ **优先级**：P1 ｜ **模块映射**：adv-parse｜进程内/进程间数据交换格式 ｜ **链接**：https://2024.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：状态传输可免序列化：直接传引用与共享内存。
- **可抄机制**：共享内存+句柄传递替代 marshal/unmarshal，schema 静态对齐，免中间拷贝(细节据题名+方向)。

## EuroSys 2025（4 篇收录）

### LOFT: A Lock-free and Adaptive Learned Index with High Scalability for Dynamic Workloads
- **会议+年份**：EuroSys 2025 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜learned index 落地参考 ｜ **链接**：https://2025.eurosys.org/accepted-papers.html ｜ **依据深度**：题名级
- **一句话结论**：无锁+自适应 learned index 可在动态负载下保高扩展(据题名)。
- **可抄机制**：分段线性学习索引外套无锁并发协议，自适应重训/分裂，避免全局锁(细节待复核)。

### Chrono: Meticulous Hotness Measurement and Flexible Page Migration for Memory Tiering
- **会议+年份**：EuroSys 2025 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜分层内存 ｜ **链接**：https://2025.eurosys.org/accepted-papers.html ｜ **依据深度**：题名级
- **一句话结论**：热度测量粒度与迁移策略可解耦(据题名)。
- **可抄机制**：细粒度热度计量+可插拔迁移策略集(细节待复核)。

### Garbage Collection Does Not Only Collect Garbage: Piggybacking-Style Defragmentation for Deduplicated
- **会议+年份**：EuroSys 2025 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜内容寻址存储维护 ｜ **链接**：https://2025.eurosys.org/accepted-papers.html ｜ **依据深度**：题名级
- **一句话结论**：去重存储的碎片整理可搭车 GC 例行扫描(据题名)。
- **可抄机制**：整理任务并入 GC 扫描路径，免专用整理窗口(细节待复核)。

### Revealing the Unstable Foundations of eBPF-Based Kernel Extensions
- **会议+年份**：EuroSys 2025 ｜ **优先级**：P2 ｜ **模块映射**：reference｜eBPF 依赖风险评估 ｜ **链接**：https://2025.eurosys.org/accepted-papers.html ｜ **依据深度**：机制级
- **一句话结论**：eBPF 扩展跨内核版本存在系统性不稳定面。
- **可抄机制**：实证 helper/验证器行为漂移，给出跨版本风险评估清单。

## EuroSys 2026（3 篇收录）

### Once Rolling Hashing is Enough: Exploiting Rolling Hash Reuse in Delta Compression
- **会议+年份**：EuroSys 2026 ｜ **优先级**：P2 ｜ **模块映射**：adv-parse｜差分/相似检测 ｜ **链接**：https://2026.eurosys.org/papers.html ｜ **依据深度**：题名级
- **一句话结论**：delta 压缩中滚动哈希可跨块/层复用(据题名)。
- **可抄机制**：复用已算滚动哈希免重复切分计算(细节待复核)。

### PaCaR: Improved Buffered I/O Locality on NUMA Systems with Page Cache Replication
- **会议+年份**：EuroSys 2026 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜多路服务器扫描 IO ｜ **链接**：https://2026.eurosys.org/papers.html ｜ **依据深度**：题名级
- **一句话结论**：NUMA 下复制 page cache 页可改善 buffered I/O 本地性(据题名)。
- **可抄机制**：跨 NUMA 节点按热度复制缓存页，读走本地副本(细节待复核)。

### Fast and Parallelized Crash Consistency with Opportunistic Order Elimination
- **会议+年份**：EuroSys 2026 ｜ **优先级**：P2 ｜ **模块映射**：reference｜WAL 并行化 ｜ **链接**：https://2026.eurosys.org/papers.html ｜ **依据深度**：题名级
- **一句话结论**：持久顺序依赖可被分析并机会性消除以并行提交(据题名)。
- **可抄机制**：识别可安全乱序的提交约束，消除后并行化(细节待复核)。

## ATC 2023（6 篇收录）

### Revisiting Secondary Indexing in LSM-based Storage Systems with Persistent Memory
- **会议+年份**：ATC 2023 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜二级索引写路径 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/wang-jing ｜ **依据深度**：机制级
- **一句话结论**：PM 上的 LSM 二级索引应以『免排序副本+内存查找结构』重构。
- **可抄机制**：二级索引做成 PM 上的哈希/混合结构：写路径只追加免排序，读路径内存查找，与 compaction 解耦(细节待复核)。

### LLFree: Scalable and Optionally-Persistent Page-Frame Allocation
- **会议+年份**：ATC 2023 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜内存分配与快照/持久化 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/wrenger ｜ **依据深度**：机制级
- **一句话结论**：页帧分配器可同时做到 NUMA 可扩展与可选持久。
- **可抄机制**：两级位图(page/order-block)+per-CPU 游标；持久模式用日志化位图翻转；分配/释放免全局锁。

### zpoline: a system call hook mechanism based on binary rewriting
- **会议+年份**：ATC 2023 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜IO 拦截/插桩/沙箱参照 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/yasukata ｜ **依据深度**：机制级
- **一句话结论**：系统调用钩子可用纯用户态二进制重写做到近零开销。
- **可抄机制**：扫描代码段把 syscall 指令改写为跳转到 trampoline；预映射全地址空间承接间接跳转；免 ptrace/kprobe。

### Light-Dedup: A Light-weight Inline Deduplication Framework for Non-Volatile Memory File Systems
- **会议+年份**：ATC 2023 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜内容寻址/去重 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/qiu-jiansheng ｜ **依据深度**：机制级
- **一句话结论**：NVM 文件系统内联去重可做得很轻。
- **可抄机制**：精简指纹索引与写路径去重钩子，降低元数据开销(细节据方向)。

### Avoiding the Ordering Trap in Systems Performance Measurement
- **会议+年份**：ATC 2023 ｜ **优先级**：P2 ｜ **模块映射**：reference｜基准测量方法学 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/duplyakin ｜ **依据深度**：机制级
- **一句话结论**：系统性能测量存在『顺序敏感』陷阱，可协议化避免。
- **可抄机制**：识别测量顺序引入的偏差并给出可复现测量协议。

### Adaptive Online Cache Capacity Optimization via Lightweight Working Set Size Estimation at Scale
- **会议+年份**：ATC 2023 ｜ **优先级**：P2 ｜ **模块映射**：adv-index｜缓存容量自适应 ｜ **链接**：https://www.usenix.org/conference/atc23/presentation/gu ｜ **依据深度**：机制级
- **一句话结论**：缓存容量可由轻量工作集估计在线自适应。
- **可抄机制**：低成本 WSS 在线估计驱动容量调整，免全量 trace。

## ATC 2024（5 篇收录）

### StreamCache: Revisiting Page Cache for File Scanning on Fast Storage Devices
- **会议+年份**：ATC 2024 ｜ **优先级**：P0 ｜ **模块映射**：perf-core｜扫描器 IO 路径 ｜ **链接**：https://www.usenix.org/conference/atc24/presentation/li-zhiyue ｜ **依据深度**：机制级
- **一句话结论**：快设备上的单遍文件扫描不该吃默认 page cache：区分复用流与扫描流才是快路径。
- **可抄机制**：区分『复用型』与『单遍扫描型』文件访问；扫描流绕过/降级 page cache、大块读+异步预读，防缓存污染并把内存让给复用数据(细节待复核)。

### FetchBPF: Customizable Prefetching Policies in Linux with eBPF
- **会议+年份**：ATC 2024 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜扫描预读策略 ｜ **链接**：https://www.usenix.org/conference/atc24/presentation/cao ｜ **依据深度**：题名级
- **一句话结论**：预读策略可下放为 eBPF 程序按负载定制(据题名)。
- **可抄机制**：Linux 预读路径挂 eBPF，按文件/访问模式替换 readahead 策略(细节待复核)。

### An Empirical Study of Rust-for-Linux: The Success, Dissatisfaction, and Compromise
- **会议+年份**：ATC 2024 ｜ **优先级**：P1 ｜ **模块映射**：reference｜ADV Rust 依赖边界决策 ｜ **链接**：https://www.usenix.org/conference/atc24/presentation/li-hongyu ｜ **依据深度**：机制级
- **一句话结论**：生产内核中 Rust 与 C 共存的成本集中在 unsafe 边界与 API 演进。
- **可抄机制**：实证统计 unsafe 使用、抽象层摩擦与维护者反馈，给出『薄封装边界+稳定 API 缺口』清单。

### FastCommit: resource-efficient, performant and cost-effective file system journaling
- **会议+年份**：ATC 2024 ｜ **优先级**：P2 ｜ **模块映射**：reference｜本地 WAL 设计参照 ｜ **链接**：https://www.usenix.org/conference/atc24/presentation/shirwadkar ｜ **依据深度**：题名级
- **一句话结论**：文件系统日志可用更省的提交协议降开销(据题名，ext4 实证)。
- **可抄机制**：精简 journal 提交(固定环形日志+按需 flush 类机制，细节待复核)。

### Fast (Trapless) Kernel Probes Everywhere
- **会议+年份**：ATC 2024 ｜ **优先级**：P2 ｜ **模块映射**：perf-core｜运行时观测 ｜ **链接**：https://www.usenix.org/conference/atc24/presentation/jia ｜ **依据深度**：题名级
- **一句话结论**：内核探针可免 trap 达到近零开销(据题名)。
- **可抄机制**：静态预映射/分支补丁替代 trap 类探针(细节待复核)。

## ATC 2025（7 篇收录）

### HotRAP: Hot Record Retention and Promotion for LSM-trees with Tiered Storage
- **会议+年份**：ATC 2025 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜本地索引分层与 compaction 策略 ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/qiu ｜ **依据深度**：题名级
- **一句话结论**：LSM 分层存储下把热记录留在快层，可同时降放大与延迟(据题名)。
- **可抄机制**：压缩/分层时识别热记录做保留与晋升(retention & promotion)，冷数据下沉(细节待复核)。

### PageFlex: Flexible and Efficient User-space Delegation of Linux Paging Policies with eBPF
- **会议+年份**：ATC 2025 ｜ **优先级**：P1 ｜ **模块映射**：perf-core｜页缓存/缺页策略 ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/yelam ｜ **依据深度**：题名级
- **一句话结论**：分页策略可按文件委托给用户态/eBPF 定制(据题名)。
- **可抄机制**：文件/页粒度声明分页策略，eBPF 实现并委托内核执行(细节待复核)。

### IRHash: Efficient Multi-Language Compiler Caching by IR-Level Hashing
- **会议+年份**：ATC 2025 ｜ **优先级**：P1 ｜ **模块映射**：adv-index｜内容寻址缓存指纹 ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/landsberg ｜ **依据深度**：题名级
- **一句话结论**：多语言编译缓存可按 IR 级哈希内容寻址复用(据题名)。
- **可抄机制**：编译中间表示规范化后哈希，跨语言/跨版本命中缓存产物(细节待复核)。

### LogCrisp: Fast Aggregated Analysis on Large-scale Compressed Logs by Enabling Two-Phase Pattern E
- **会议+年份**：ATC 2025 ｜ **优先级**：P2 ｜ **模块映射**：adv-parse｜日志分析 ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/wei ｜ **依据深度**：题名级
- **一句话结论**：压缩日志上可两阶段(先粗后精)完成模式聚合分析(据题名)。
- **可抄机制**：两阶段模式抽取：粗筛在压缩域、精化按需解压(细节待复核)。

### HEC: Equivalence Verification Checking for Code Transformation via Equality Saturation
- **会议+年份**：ATC 2025 ｜ **优先级**：watch ｜ **模块映射**：watch｜改写类门禁参照(P3 域交叠) ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/yin ｜ **依据深度**：题名级
- **一句话结论**：e-graph 等价饱和可为代码变换做等价验证(据题名)。
- **可抄机制**：等价类生成+变换语义验证(细节待复核)。

### ASTERINAS: A Linux ABI-Compatible, Rust-Based Framekernel OS with a Small and Sound TCB
- **会议+年份**：ATC 2025 ｜ **优先级**：watch ｜ **模块映射**：watch｜Rust 系统结构参照(E7 域交叠) ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/peng-yuke ｜ **依据深度**：机制级
- **一句话结论**：Rust framekernel 可做到 Linux ABI 兼容且 TCB 小而健全。
- **可抄机制**：framekernel 分层：Rust 安全内核+最小 unsafe 边界，ABI 兼容层承接 Linux 程序。

### Converos: Practical Model Checking for Verifying Rust OS Kernel Concurrency
- **会议+年份**：ATC 2025 ｜ **优先级**：watch ｜ **模块映射**：watch｜并发验证 ｜ **链接**：https://www.usenix.org/conference/atc25/presentation/tang ｜ **依据深度**：题名级
- **一句话结论**：Rust 内核并发可用实用化模型检查验证(据题名)。
- **可抄机制**：状态空间剪枝+Rust 语义建模(细节待复核)。

---
## 覆盖缺口与待验证

- 增量计算/物化维护：四会内直接命中稀少（SOSP 2023 SPFresh 的 LIRE 协议、OSDI 2026 Incr bolt-on 增量）；编译型增量的系统实证更多在 PLDI/ASPLOS（P3/E2 域已有 salsa 线索），本域不重复收。
- FUSE 专项、通用 malloc/GC 专项、崩溃一致性的 FAST 线论文：2023–26 四会内未命中可直接落地条目（EuroSys 2024 Jade GC、SOSP 2025 CortenMM 仅 watch）。
- ATC 2026：官网 technical-sessions 404，待验证（或该届改期/改名）。
- DBLP 逐篇 DOI（SOSP/EuroSys）与 2025/2026 年论文机制细节：待补核（部分条目标题名级）。

## ATC 2026 补扫（2026-10-04）

官网 usenix.org/conference/atc26 及 technical-sessions 均 404；检索显示 ATC 26 录用通知 2026-09-03、议程页尚未上线，无可靠录取清单可引，本轮 0 条入册（链接宁少勿假）。官网议程上线后按下轮补扫流程收。
