# X1 · 高性能数据结构与算法库（Rust 生态）—— perf-core 选型台账

- 日期：2026-10-04；方法：WebSearch（2024–2026 现状锚点）+ crates.io/docs.rs 记忆库交叉；registry 同步 `registry/X1.jsonl`（46 条）。
- 口径：档位 ∈ {吸收, 有界, 不吸收, watch, reference}；成熟度 ∈ {实验, 维护中, 弃维护}；说不出机制的条目不收。
- 前域复用：memchr / aho-corasick 已在检索域深拆（Two-Way/Teddy + 自动机），本域不重复建档，ADV 直接按前域结论吸收。

## Top-3

1. **句柄化 AST 存储（id-arena + slotmap 二选一原则 + 旁表）**——`Arena<T>` 返回 Copy 索引句柄（id-arena，不可删场景）；需删除/失效（增量重解析、宏展开剪枝）换 slotmap 世代索引（删槽 gen+1，旧句柄可检失效）；冷属性挪 `SecondaryMap`。与 wave-0 01 的 AstKind 方案直接拼装：节点体 = `kind: AstKind + span + 句柄`，属性外挂。
2. **lasso 字符串驻留（Rodeo + Spur/u32 Key）**——标识符密集 AST 的内存/比较双收益：每唯一标识符存一份、节点内 4B Key，相等判断从 memcmp 降为整数比较；`Splice` 支持分片驻留表归并，契合 ADV 分文件并行解析。引入判据见「重点回答②」。
3. **jiff（全仓时间统一）+ ryu/itoa（JSONL 报告数字格式化）**——jiff 以 TZ 一等语义解决 chrono 的 DST/时区供给痛点（1.x，2025-01 起）；ryu/itoa 是 serde_json 内部同款，仅在自研直写 JSONL writer 时显式引入兑现增益。

---

## 1. 哈希

### hashbrown 【吸收】
- 定位：SwissTable 实现，std `HashMap` 的内核；手写 raw table 场景（自定义 hasher、自定义布局）的直接依赖。
- 可抄机制：① 控制字节组探测——每槽 1B 存哈希高 7 位，SSE2 一次 `movemask` 比较 16 槽 / AVX2 32 槽，探测常数化；② 负载因子 7/8（87.5%），组内必有空槽保证探测终止；③ 2024 下半年起 std 构建以外默认 hasher 切 foldhash（crates feature，锚：internals.rust-lang.org「A new default Hasher for HashMap?」讨论线）。
- 第一步动作：ADV 内部 map 统一 `hashbrown` + foldhash；需要 raw_entry 式底层控制的组件（索引分片）直连 hashbrown 而非 std。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/rust-lang/hashbrown

### rapidhash 【吸收】
- 定位：wyhash 官方继任；内部指纹/分片键等"要快又要质量"的非加密哈希。
- 可抄机制：① wyhash 谱系的乘法折叠主线 + secret 参数；② v3 经 SMhasher 高吞吐（~23,789 MiB/s，锚：rurban SMhasher 表）且质量分靠前；③ 2026 新增对抗性碰撞/偏差的机器可复核分析（锚：thomasahle.com「Adversarial examples for fast hash functions」，含 rapidhash_v3_verify 脚本）。
- 第一步动作：perf-core 去重/分片哈希选 rapidhash v3，与 xxh3 做 A/B（键长分布：短键 rapidhash、长流 xxh3）。
- 许可证/成熟度：MIT / 维护中（2025–2026 活跃，Google Fuchsia/Turso DB 在用）。
- 链接：https://github.com/Nicoshev/rapidhash

### rustc-hash（FxHashMap）【吸收】
- 定位：rustc 官方速度取向 hasher；编译器内部 map 的事实惯例。
- 可抄机制：① 2.0（2024，Nethercote）把原 FxHash 重写为乘-异或-移位轮次（锚：rust-lang/rustc-hash CHANGELOG），质量较 1.x 提升且保持极快；② 短键（符号名切片）场景每哈希成本极低；③ 无雪崩、不抗 DoS——键来自外部输入的公开 map 禁用。
- 第一步动作：adv-parse/adv-index 内部短命 map 统一 `FxHashMap`（或 foldhash），外部输入键的 map 用 ahash/rapidhash。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/rust-lang/rustc-hash

### foldhash 【吸收】
- 定位：orlp（pdqsort/glidesort 作者）2024 为哈希表设计的 hasher。
- 可抄机制：① folded-multiply 终混，质量/速度平衡点针对 hash table 使用面；② per-map 实例种子（每张表独立）；③ 自我定位"minimally DoS-resistant"——明确不是安全保证，只挡顺手碰撞不是挡攻击者（锚：crate 文档自我声明）。已并入 hashbrown 默认 feature。
- 第一步动作：作为 ADV 默认 hasher 候选与 rustc-hash 2.x 对比后二选一。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/orlp/foldhash

### ahash 【有界】
- 定位：AES-NI 路线的大 map 高吞吐 hasher。
- 可抄机制：① x86 走 AES 轮函数（aesenc 组合），其余架构 folded-multiply 回退；② 进程级随机 key（RandomState）抗碰撞注入——抗性非加密级；③ 大 map（10 万+ 键）场景吞吐领先（锚：crate 自带基准口径，复测后再引数字）。
- 第一步动作：仅在基准显示 foldhash/fx 不够时引入；不做默认。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/tkaitchuck/aHash

### nohash 【有界】
- 定位：整数键恒等哈希（hash(u)=u），键已是良分布 ID（arena 句柄、Spur）时零哈希成本。
- 第一步动作：驻留 Key/句柄做 map 键的内部表启用；键源含外部字符串的表禁用（恒等哈希=攻击者可控碰撞）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（极简近乎冻结，功能完整）。
- 链接：https://github.com/parasyte/nohash-hasher

### xxhash-rust（XXH3）【吸收】
- 定位：非加密高速指纹（内容去重、分片、校验），非哈希表默认。
- 可抄机制：① XXH3 64/128 与 secret 可配；② 流式增量 API（`StreamingXxh3`）边读边喂——大文件指纹不落全量；③ 单发吞吐 GB/s 量级（SMhasher 榜）。
- 第一步动作：文件指纹/缓存键用 XXH3-128 流式；不做对外 map 键。
- 许可证/成熟度：BSL-1.0 / 维护中。
- 链接：https://github.com/DoumanAsh/xxhash-rust

扫视：wyhash（final 4.2/4.3，作者停更自宣，rapidhash 继任；公有域/Unlicense，弃维护，https://github.com/wangyi-fudan-wyhash/wyhash 【reference】）。

## 2. 排序与 top-k

### rayon（par_sort_unstable）【吸收】
- 定位：报告行/诊断行多核排序入口。
- 可抄机制：① 并行分治（样本分割定 pivot）+ 工作窃取调度，大数组近线性核扩展；② 不稳定排序免合并缓冲；③ 与 std 同算法谱系，行为可预期。
- 第一步动作：adv-report 聚合排序统一走 rayon；小数组（<数千）退回 std 免线程开销。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/rayon-rs/rayon

### radsort 【有界】
- 定位：LSD 稳定基数排序，整数/枚举键大数组（数十万行按 severity+时间组合键）。
- 可抄机制：① key 函数逐字节（u8 通道）分桶，O(n·key_len) 无比较分支、分支预测友好；② 稳定性来自桶序保持；③ 以 key 函数替代 Ord，绕开比较器间接层。
- 第一步动作：排序热点 profile 显示比较器成本占比高时引入；默认 std/rayon。
- 许可证/成熟度：MIT OR Apache-2.0（以 crates.io 为准）/ 维护中（低频）。
- 链接：https://crates.io/crates/radsort

### top-k 模式（binary-heap-plus / std BinaryHeap 手写）【吸收】
- 定位：毫秒级 top-k（最慢文件、最热规则命中）。
- 可抄机制：① 容量 k 反向 Max-Heap 流式 push/pop：O(n log k)、内存 O(k)，n≫k 时远优于全排序；② `peek` 做准入门槛短路（新元素 ≤ 堆顶直接丢弃）；③ binary-heap-plus 支持自定义比较器与按值检索。
- 第一步动作：先 std `BinaryHeap` 手写 20 行（零依赖）；仅当需要 retrieve/泛型能力再引 binary-heap-plus。
- 许可证/成熟度：MIT（以 crates.io 为准）/ 维护中。
- 链接：https://crates.io/crates/binary-heap-plus

### glidesort 【watch】
- 定位：自适应稳定排序（glide+pdq 混合，2023 新秀）。
- 机制：检测有序 run 并"滑行"合并，部分有序输入近 O(n)。
- 现状锚：最后发版 2023-02-03（crates.io versions 页），仓库休眠；思想经 driftsort 进入 std——Rust 1.81（2024-09）起新稳定排序即 driftsort。
- 第一步动作：不引 crate；自适应稳定排序需求直接吃 std 红利。
- 许可证/成熟度：MIT OR Apache-2.0 / 弃维护（2023-02 后）。
- 链接：https://github.com/orlp/glidesort

扫视：ipnsort（orlp pdqsort 后继不稳定排序，std `sort_unstable` 演进方向；std 内置已够用 【watch】，MIT OR Apache-2.0，https://github.com/orlp/ipnsort）；dmsort（分布-归并，几乎有序序列近 O(n)，增量索引重排 【watch】，MIT OR Apache-2.0 以 crates.io 为准，https://crates.io/crates/dmsort）。

## 3. 字符串 / 文本

### lasso 【吸收】（重点回答②，机制级）
- 定位：字符串驻留器；AST 标识符/限定名/字符串字面量的整数化。
- 可抄机制：① `Rodeo`：hashbrown raw_entry 定位 + 每串存一份，返回 `Spur`（默认 u32 Key，可选 u8/u16/u64）——此后比较=整数比较、节点内占 4B；② 并发谱系：`ThreadedRodeo`（并发驻留）→ 冻结后 `RodeoReader`（零争用只读）；③ `Splice` 系列（RodeoSplice 等，锚：docs.rs/lasso）支持多张驻留表合并重编号——分文件并行解析后归一符号表正好用。
- 收益判据（内存/比较/编译成本三面）：内存——当唯一标识符数 ≪ 出现次数（源码典型重复率 10–100×，单点经验待 ADV 实测复算），每节点 4B Key 替代 String（24B 结构+堆内容+分配器开销），标识符类节点占比 ≥~30% 时收益显著；比较——重命名/去重/引用解析热路径 memcmp→u32；成本——+1 依赖与 API 面积、驻留表生命周期（进程级/会话级需定）、跨线程选型（争用 vs 冻结）。
- 第一步动作：adv-parse 先在符号表层接 `Rodeo`（单线程解析、阶段末 Splice 归并），不上 ThreadedRodeo。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（0.7.x；并存替代：lasso2 fork、inturn）。
- 链接：https://docs.rs/lasso （repo https://github.com/Kixiron/lasso）

### smallvec 【吸收】（重点回答③，机制级）
- 定位：小集合栈内联，砍掉 Vec 的堆分配与指针跳转。
- 可抄机制：① 内联布局 = inline cap N×size_of::<T>() + len，溢出后单向转堆（不回迁）；② 收益三条件：内联总字节小（≤64B 量级）、元素小（≤16B）、绝大多数实例长度 ≤N（典型：运算符参数、短属性列表）；③ 反模式（何时不赚反亏）：N 过大（如 [u8;4096]）→ 每实例占满整个内联区，嵌进 AST 节点把节点撑爆（cache line 命中率下降、移动/克隆变 memcpy）；递归下降每帧多个大 N smallvec → 栈溢出（主线程 ~8MB/工作线程 ~2MB 量级）；"先大后小"访问模式无收益（降级单向）。
- 第一步动作：adv-parse 仅对 P99 长度可测且 ≤N 的字段用 smallvec（N≤8）；其余一律 Vec。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（1.x 稳定）。
- 链接：https://github.com/servo/rust-smallvec

### compact_str 【吸收】（机制级）
- 定位：短字符串内联（路径段、限定名、诊断短串字段）。
- 可抄机制：① 24B 表示与 `String` 同尺寸，64 位下内联至多 24B（discriminant 复用技巧）；② 超长自动落堆、`Deref<str>` 全兼容；③ 横向：smartstring 仅 23B 且 lib.rs 已标 unmaintained（2025，锚：lib.rs/crates/smartstring）；lean_string 走 16B+COW 路线（更小表示、内联更短）。
- 第一步动作：诊断消息/报告行的短串字段试点 `CompactString`，量化分配次数变化再扩面。
- 许可证/成熟度：MIT / 维护中（0.9/0.10.x，2025 活跃；Fedora 2025-04 打包 0.9.0、EPEL9 0.10.x）。
- 链接：https://github.com/ParkMyCar/compact_str

### bstr 【吸收】
- 定位：字节串（BString/BStr）——不要求合法 UTF-8 的文本操作。
- 机制：Two-Way 线性时间子串搜索 + memchr 加速；切片/迭代/比较按字节语义。
- 第一步动作：源码片段截取与诊断上下文统一 bstr，避免 String 的 UTF-8 边界 panic 面。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（1.x）。
- 链接：https://github.com/BurntSushi/bstr

### unicode-segmentation 【吸收（有界使用点）】
- 定位：UAX#29 字素簇/词/句边界。
- 机制：状态机扫边界；展示换行、重命名时词边界判断用；词法分析不用它（要自己的 lexer）。
- 第一步动作：仅 adv-report 展示层与 heck 底层涉及，不进解析热路径。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/unicode-rs/unicode-segmentation

### heck 【吸收】
- 定位：命名风格互转（UpperCamelCase/lower_snake/SHOUTY/kebab）。
- 机制：基于 unicode 词边界切词后按风格重拼；代码生成/重命名器直接用。
- 第一步动作：adv 代码生成（修 fixer 输出标识符）引入。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/withoutboats/heck

扫视：
- string-cache：Servo 编译期静态 atom（static_atoms! 生成 'static atom）+ 动态驻留，html5ever 底座；固定词表+少量动态场景 【reference】，MIT OR Apache-2.0，维护中（随 html5ever），https://github.com/servo/string-cache
- string-interner：另一主流驻留器（后端可换 String/AString），与 lasso 择一即可 【reference】，MIT OR Apache-2.0，维护中，https://github.com/robbepop/string-interner
- inturn：2024 新 lock-free 并发驻留（DaniPopes），观察 lasso 并发档替代 【watch】，MIT OR Apache-2.0，维护中（新），https://github.com/DaniPopes/inturn
- smartstring：23B 内联+惰性布局；lib.rs 2025 标 unmaintained → 被 compact_str 替代 【不吸收】，MIT，弃维护，https://github.com/bodil/smartstring
- lean_string：16B 表示+COW 内联串（2024–2025 新），观察 compact_str 替代面 【watch】，MIT OR Apache-2.0（以 crates.io 为准），维护中（新），https://github.com/ryota2357/lean_string

## 4. 句柄化集合与图

### id-arena 【吸收】（重点回答①，机制级）
- 定位：最简 arena + Copy 索引句柄；归一 AST 的默认底座候选。
- 可抄机制：① `Arena<T>` 分配返回 `Index`（u32 句柄，Copy/Eq/Hash、无生命周期参数，可存进任意结构/跨阶段传递）；② `ArenaBehavior` 定制索引宽度与分配策略；③ 与 wave-0 01 的 AstKind 呼应：节点体压缩为「kind 标签 + span + 句柄」，属性走旁表。
- 局限（选型边界）：无删除——删除即句柄悬挂且无世代防护；迭代顺序非承诺。
- 第一步动作：adv-parse 归一 AST 按「不可删场景用 id-arena」落地；需要失效语义才换 slotmap（见下）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（低频但稳定）。
- 链接：https://github.com/fitzgen/id-arena

### slotmap 【吸收（需删除场景）】（重点回答①，机制级）
- 定位：世代索引句柄化存储——删除安全的 arena。
- 可抄机制：① key = index + generation：删槽时 gen+1，旧句柄下次访问可检失效（防 ABA 复用）；② 变体谱系：`HopSlotMap`（hopscotch 散列布局，定位缓存友好）、`DenseSlotMap`（稠密数组+空闲列表，遍历快、key 稳定性靠映射）、`SparseSecondaryMap`；③ `SecondaryMap`：同一 key 空间挂旁表——冷属性剥离节点体。
- 第一步动作：符号表/跨阶段失效表（增量重解析失效传播）用 slotmap；纯 AST 用 id-arena；两者原则：默认不可删选 id-arena，需删选 slotmap。
- 许可证/成熟度：Zlib / 维护中（0.4.x）。
- 链接：https://github.com/orlp/slotmap

### petgraph 【吸收】（机制级）
- 定位：依赖图/调用图主库。
- 可抄机制：① `DiGraph` = Vec 节点 + Vec 边（链表式挂接），`NodeIndex(u32)` 句柄；② 算法全家桶：toposort、Tarjan/Kosaraju SCC、Dijkstra/A*、环检测——增量失效传播与循环依赖报告直接取用；③ 变体：`StableGraph`（删除保句柄）、`Csr`（紧凑静态图，千万边级更省内存）。
- 第一步动作：adv-index 依赖图用 DiGraph；图规模 >1e7 边时评估 Csr/自建 CSR。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（0.7 线，2024-12 起）。
- 链接：https://github.com/petgraph/petgraph

### typed-index-collections 【有界】
- 定位：`TiVec<Idx, T>`——Vec 包装仅接受 newtype 索引，编译期防「A 表索引传给 B 表」。
- 机制：Deref 到 slice + 仅 `Idx` 参数的 get/put；与句柄化配套（ExprId/PatId 各归各表）。
- 第一步动作：adv-parse 出现 ≥2 张平行属性表时启用；单表场景不值依赖。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/zheland/typed-index-collections

### indexmap 【吸收】
- 定位：保插入序哈希表——报告输出可复现（迭代序确定）。
- 机制：哈希查表 + 侧表 entries 保序，`swap_remove` O(1) 删除；底座同为 hashbrown 谱系。
- 第一步动作：诊断聚合/规则命中表的输出顺序敏感处用 indexmap 替代 HashMap+显式排序。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/indexmap-rs/indexmap

### fixedbitset 【吸收】
- 定位：定长位集——可达集、标记集、去重掩码。
- 机制：u32 块 + and/or/集合运算；petgraph 生态标配。
- 第一步动作：与 petgraph 同批进入 adv-index。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/petgraph/fixedbitset

扫视：
- bitvec：BitSlice 位级零拷贝视图（域抽象泛型重），热循环常慢于手写 u64 位运算；1.0.0（2022）后稀疏更新，2024 社区反馈 bug 响应慢（锚：users.rust-lang.org「Searching for a bitvec crate that is not abandoned…」）【reference】，MIT，弃维护（近似），https://github.com/myrrlyn/bitvec
- dashmap：分片（默认 4×核数）RwLock<HashMap> 并发 map；读多写少并发索引、跨 shard 聚合需手动 【有界】，MIT，维护中，https://github.com/xacrimon/dashmap
- arrayvec：定容量零堆集合（越界 panic/try API），有上限局部缓冲 【有界】，MIT OR Apache-2.0，维护中，https://github.com/bluss/arrayvec
- tinyvec：100% safe 小集合（Default 填充未初始化区，仅 Copy 元素），安全审计友好、开销略高于 smallvec 【有界】，MIT OR Apache-2.0 OR Zlib，维护中，https://github.com/Lokathor/tinyvec

## 5. 时间 / 数值格式化 / ID

### jiff 【吸收】（重点回答④，机制级）
- 定位：全仓时间底座（时间戳/日志/报告）。
- 可抄机制：① `Zoned` = civil datetime + offset + 时区规则三元一体，DST gap/fold 显式建模（chrono `Local` 的模糊性正是其问题起点）；② TZDB 三种供给：运行时系统 tzdb / 内嵌 `jiff-tzdb` / 编译期内嵌 `jiff-static`（锚：2026-04 前后发布，搜索锚）+ POSIX TZ 字符串解析——本地优先分发零外部依赖可选；③ `Span`（日历跨度）与 `SignedDuration`（绝对时长）分离，杜绝 chrono Duration 混用教训；RFC3339/2822、ISO8601 子集、strptime 全套；serde feature 内建。
- 2026 结论（jiff vs chrono）：ADV 新代码统一 jiff；chrono 只留生态兼容边界（依赖库输出 chrono 类型时的转换层）。依据：jiff 1.x（2025-01 起）稳定且高频发版（锚：crates.io/GitHub release 面），语义面为 chrono 已知坑（TZ 供给、DST、时长语义）的针对性修正；成本：MSRV 较新（以 Cargo.toml 为准）、+1 依赖。
- 第一步动作：perf-core 时间戳/日志模块首发引入，定 serde 互转工具函数后再全仓推广。
- 许可证/成熟度：MIT OR Apache-2.0 OR Unlicense / 维护中（1.x）。
- 链接：https://github.com/BurntSushi/jiff

### ryu + itoa 【吸收】（重点回答⑤，机制级）
- 定位：数字→文本最快格式化；JSONL 报告 writer 的吞吐底座。
- 可抄机制：① ryu（Adams 2018，PLDI）：f64 最短往返表示，10 幂表 + 定宽算术 O(1)，无 fmt 状态机；② itoa：整数直接十进制生成写缓冲，零分配；③ serde_json 浮点/整数内部正是 ryu/itoa——已用 serde_json 时显式引入无增益；增益只在自研直写 writer（每行手拼 Vec<u8>，字段级 itoa/ryu 直写）兑现，绕开 `format!`/fmt 机制。
- 收益判据：字段级 profile 显示数字格式化进热点（指标行/耗时行占比高的报告）才自研 writer；否则 serde_json 已含同款，不动。
- 补充锚：zmij（2025–2026 新 f64→string，qsv 已从 ryu 换 zmij 提浮点序列化速度——锚：qsv CHANGELOG）为升级观察位。
- 第一步动作：adv-report 若手写 JSONL writer，引入 itoa+ryu 直写缓冲；同步 watch zmij。
- 许可证/成熟度：ryu BSL-1.0、itoa MIT OR Apache-2.0 / 均维护中（多年稳定）。
- 链接：https://github.com/dtolnay/ryu ・ https://github.com/dtolnay/itoa

### chrono 【有界】
- 定位：老牌时间库，仅生态兼容层。
- 机制/坑：0.4.x 低速稳态；TZ 供给二选一——chrono-tz（编译期烘焙全表：编译慢、数据冻结于发版）或 iana-time-zone（运行时系统 tzdb）；DST/本地语义历史坑多。
- 第一步动作：不进新代码；依赖边界处 chrono↔jiff 转换。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（低频）。
- 链接：https://github.com/chronotope/chrono

### rand 0.9 【吸收】（机制级）
- 定位：采样/模糊种子/jitter 的随机源。
- 可抄机制：① 0.9（2025-01/02，锚：GitLab rand 0.9 升级 MR 2025-01-28）：`thread_rng()`→`rng()`、`gen_*`→`random_*`（trait 重命名）；② rand_core 0.9：`from_entropy` 移除改 `from_os_rng`、`CryptoRngCore` 拆分；③ MSRV 1.85（edition 2024 系）。
- 第一步动作：新依赖统一锁 rand 0.9 线；下游未跟进 0.9 的 crate 隔离在特性门后（0.8/0.9 双版并存是 2025–2026 常态）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/rust-random/rand

### uuid 【吸收】
- 定位：会话/发现项 ID。
- 机制：v4 随机 122bit（getrandom 底座）。
- 第一步动作：基础组件直接引入，无选型争议。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（1.x）。
- 链接：https://github.com/uuid-rs/uuid

### lexical 【watch】
- 定位：lexical-core 数字双向（parse+format）全精度 + 自定义 format 规格。
- 机制：双向全精度算法 + 格式规格化；编译成本与体积高于 ryu/itoa。
- 第一步动作：仅当需要解析外部大数字文本（非标准 JSON 数字格式）再引。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/Alexhuszagh/rust-lexical

扫视：zmij（新 f64→string，qsv 从 ryu 换入；观察浮点格式化升级面 【watch】，MIT OR Apache-2.0 以 crates.io 为准，维护中（新），https://crates.io/crates/zmij）。

## 6. 概要统计 / 随机补充

### hyperloglogplus 【watch】
- 定位：基数估计——扫描结果去重文件数/符号数近似计数，内存恒定。
- 机制：m=2^p 寄存器存 ρ(hash) 最大前导零+1，调和平均 + Ertl 偏差修正，误差 ≈1.04/√m（p=12→~0.8% 量级）；合并 = 逐槽 max → 分片结果可归并。
- 注：领域盘点输入称 pprof-rs 内含 HLL 实现，本次未复核——采纳前先 rg 其 src 确认。
- 第一步动作：报告出现"去重计数"需求且允许 ~1% 误差时引入。
- 许可证/成熟度：MIT（以 crates.io 为准）/ 维护中（低频）。
- 链接：https://github.com/jedisct1/rust-hyperloglog

### sketches-ddsketch 【watch】
- 定位：相对误差保证的分位数草图（p99 免全量排序）。
- 机制：对数桶 + 计数，合并安全——多分片扫描耗时分布聚合。
- 第一步动作：adv-report 聚合延迟分布时与手写直方图二选一。
- 许可证/成熟度：Apache-2.0 / 维护中。
- 链接：https://github.com/DataDog/sketches-rust

### count-min sketch 【reference】
- 定位：频率估计（海量告警键频次 top-k 预过滤）。
- 机制：d×w 计数器矩阵 + 成对独立哈希，查询取 min（保守低估），空间 O(dw)。
- 现状：主流 crate 零散且多不活跃；~30 行手写可行——先手写。
- 第一步动作：出现频率统计需求时手写实现 + 基准，不引依赖。
- 许可证/成熟度：以所选实现为准 / 多为实验。
- 链接：https://crates.io/crates/count-min-sketch

---

## 五个重点回答（决策摘要）

1. **AST/符号存储句柄化**：归一 AST 首选「arena + typed handle」：默认不可删 → id-arena（`Index` u32 句柄）；需删除/失效（增量重解析、死代码剪枝）→ slotmap 世代索引；冷属性一律 `SecondaryMap` 旁表，节点体保持「AstKind + span + 句柄」小结构（与 wave-0 01 拼装）；平行表 ≥2 时加 typed-index-collections 防混用；标识符载荷用 lasso `Spur`。
2. **lasso 驻留收益判据**：唯一标识符数 ≪ 出现次数（文本重复率高）且标识符类节点占比高（≥~30% 量级，ADV 实测复算后定档）→ 内存省（4B Key 替代 String 三元组）+ 热路径比较 memcmp→u32；成本面：驻留表生命周期、跨线程变体选型（ThreadedRodeo 争用 vs 阶段末 Splice 归并）、+1 依赖。分片解析后 Splice 合并与 ADV 并行架构同构，直接采用。
3. **smallvec 适用与反模式**：赚——内联总字节 ≤64B、元素小、P99 长度 ≤N（N≤8 量级）；亏——大 N（节点体膨胀/cache line 破坏/移动变 memcpy）、递归热路径上每帧大 N（栈溢出）、单向降级使"先大后小"模式无收益。不满足判据一律 Vec；固定上限缓冲用 arrayvec。
4. **时间库 2026 结论**：全仓统一 jiff（TZ 一等语义 + DST 显式 + Span/SignedDuration 分离 + TZDB 三供给含 jiff-static 2026-04 编译期内嵌；1.x 稳定）；chrono 仅留依赖边界转换层，不进新代码。
5. **ryu/itoa 对 JSONL 的贡献**：serde_json 内部已用同款——不换 writer 则无显式增益；自研直写 writer（字段级 itoa/ryu 写 Vec<u8> 缓冲）在数字密集报告行可兑现吞吐增益（qsv 为浮点序列化专门换 zmij 是该环节敏感度的实测侧锚）；先 profile 后引入，zmij 列观察位。

## 条目台账索引

46 条已同步 `registry/X1.jsonl`（X1-001…X1-046；deep 14 条：hashbrown、rapidhash、rustc-hash、foldhash、lasso、smallvec、compact_str、id-arena、slotmap、petgraph、jiff、ryu+itoa、rand、xxhash-rust 之外的扫视条目一律 scan）。数字均带锚（SMhasher 表、crates.io versions、lib.rs 标注、GitLab MR、社区帖）；单点结论标注了复算口径，未升格普遍结论。
