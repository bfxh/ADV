# X2 · SIMD 生态盘点（扫描 / JSONL / varint / 哈希的向量加速面）

- 日期：2026-10-04；域：X2；承接 W4 组任务，衔接 C2（命令行加速）与 R7（GPU）既有盘点
- 前提（wave-0 07 已定）：SIMD 走 **stable `core::arch` + 运行时检测**；std::simd（portable-simd）2026-10 仍 nightly，不吸收（本档复核后维持该判定，见 1.2）
- 口径：性能数字一律带来源口径（论文 / 作者自报 bench / 特定 CPU 型号）；GB/s 类数字均为单机测量，非普适结论，吸收前须在 ADV 真实语料复测
- 条目：机制级 17（含 3 坑）+ 扫视 13 = **30 条**

---

## 0. 结论速览：ADV 的 SIMD 使用准则（回答重点①）

**判据先行**（先量后改）：数据并行、分支稀少、批处理粒度 ≥ 数 KB、输出可逐位验证——四条同时满足才考虑 SIMD。

值得上的相位：

| 相位 | 结论 | 理由 |
|---|---|---|
| 扫描/预过滤（memchr 类：稀有字节、子串、定界符） | 吸收 | SIMD 收益最大的第一层；直接用 memchr crate，不手写（1.3） |
| JSONL 输入的结构定位与跳读 | 吸收机制 | simdjson 两阶段模型（1.6）；值按需物化 |
| varint/LEB128 索引编码 | 有界 | Stream VByte（1.11）；基准优于标量 ≥1.5× 才进主干 |
| 内容哈希/校验（xxh3、CRC 折叠） | 有界 | 多 lane 折叠是收益点（1.13、2.27）；哈希选型主线由 X1 已定（rapidhash/foldhash），此处仅机制参照 |
| 输出层 base64（仅当输出含二进制 blob 字段） | 有界 | 1.12；差 <1.5× 则不吸收 |

不值得的相位（记录反例，防过度工程）：
- **AST/IR 构建**：指针追逐 + 分支密集，lane 利用率低，向量化无并行度可言；
- **规则候选评估**：每批候选少，SIMD 启动/对齐开销盖过收益；
- **单条小文档解析**：批量不足，应在"相位一"（扫描）SIMD 化后让标量解析器吃结构索引。

---

## 1. 机制级（17 条）

### 1.1 std::arch + is_x86_feature_detected!（stable Rust 官方路径）【吸收】
- **定位**：stable Rust 唯一官方向量入口；x86_64 上 SSE4.2/AVX2 常用子集自 **1.27（2018-06）** 即稳定（含 AES/SHA/PCLMUL/BMI/FMA），**AVX-512 全套 `_mm512_*` 于 1.89.0（2025-08-07）稳定**；少数新指令面（部分 GFNI/VNNI 组合等）以 nightly doc 为准仍在推进。
- **可抄机制**：① `#[target_feature(enable="avx2")]` 自 **1.85（2025-02）** 起可标注 safe fn（调用者仍须保证运行时特征成立）；② `is_x86_feature_detected!` 底层 std_detect 有进程级缓存，不要自建重复检测；③ unsafe 边界收拢为一处：detect 结果 → 零尺寸 proof 类型 → 调用。
- **第一步动作**：perf-core 建 `simd/mod.rs` 惯例：SSE2 基线无条件编译 + AVX2 档 + 一次成型缓存的选型点，配逐位对拍测试。
- **许可证/成熟度**：官方 std / 稳定，维护中。
- **链接**：https://doc.rust-lang.org/core/arch/ · https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/

### 1.2 portable-simd（std::simd）【不吸收 / watch】
- **定位**：`Simd<T,N>`/`Mask` 便携抽象；2026-10 仍 nightly（tracking #27731），**未见公开稳定化路线图**；2025-06 Linebender 另起外部 crate 路线（fearless_simd，见 2.23）绕开 std::simd；厂商 intrinsic 侧仍在扩张（LoongArch64 tracking #162508，2026-09）——重心不在便携抽象。
- **可抄机制**：① `Simd<T, N>` lane 常量泛型的 API 形状，先用 wide/pulp 模仿；② `Mask`/`SimdPartialOrd` 语义作为我们包装层的设计参照。
- **第一步动作**：watcher 挂 #27731，代码零依赖。
- **许可证/成熟度**：官方 nightly / 实验。
- **链接**：https://github.com/rust-lang/rust/issues/27731

### 1.3 memchr crate【吸收】
- **定位**：Rust 向量扫描事实标准（regex/aho-corasick 的 prefilter 底座），扫描相位第一选择。
- **可抄机制**：① 长针 two-way 算法 + 稀有字节预过滤；② x86_64 SSE2 基线无条件编译 + `is_x86_feature_detected` 升级 AVX2；③ 短针/无 SIMD 平台 SWAR 回退——**"基线 + 检测升级 + 便携回退"三层分级的现成样板**，ADV 自己的内核照此分层。
- **第一步动作**：扫描相位（定界符查找、注释定位、needle 预过滤）直接替换手工循环；不自研。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（burntsushi，长期活跃）。
- **链接**：https://github.com/BurntSushi/memchr

### 1.4 pulp【有界】
- **定位**：stable Rust 上"内核泛型 + 运行时分发"库（faer 线性代数作者维护，2023 起活跃，faer 内部即用它做 SIMD 层）。
- **可抄机制**：① `pulp::Arch` 一次探测 + dispatch：同一内核实现 `WithSimd` trait，按 `Simd` 类型参数在各 target（SSE/AVX2/AVX-512/NEON）下 `#[target_feature]` 特化编译；② 特征 proof 编码进类型参数，比手写 fn 表少 unsafe；③ 多架构共享同一内核源码，避免 x86/ARM 两份实现漂移。
- **第一步动作**：用 pulp 包一个 memchr 类扫描内核做基准与 API 试用；合意则采纳其 dispatch 模式，不合意也把 WithSimd 形状抄进自研惯例。
- **许可证/成熟度**：MIT OR Apache-2.0（docs.rs 口径）/ 维护中。
- **链接**：https://docs.rs/pulp

### 1.5 multiversioning 三种做法（回答重点②的做法面）【吸收】
- **定位**：Rust 无编译器级 target_clones（GCC `target_clones` 对应物至今未进语言，见 internals 讨论），运行时分发需自建。
- **可抄机制**：① **手写 fn 表（ADV 默认）**：`OnceLock<FnTable>` + `is_x86_feature_detected!` 一次选型缓存 → 调 1.85 后的 safe `#[target_feature]` fn；② **pulp WithSimd**（1.4）：内核单源；③ **multiversion crate** 属性宏（`#[multiversion]`/`#[target]`/`#[specialize]`），维护放缓，作备选。注意：target_feature fn 经 fn 指针的安全调用仍受限（tracking #69098 未完），fn 表用薄包装收敛 unsafe，不外泄裸指针。
- **第一步动作**：定 ADV 惯例：每热点 ≤2 档（SSE2 基线 + AVX2；AVX-512 仅当基准证明收益且随 SDE 测试）；选型点一次成型。
- **许可证/成熟度**：模式 n/a；multiversion crate MIT OR Apache-2.0 / 维护放缓。
- **链接**：https://docs.rs/multiversion/ · https://github.com/rust-lang/rust/issues/69098 · https://internals.rust-lang.org/t/function-multi-versioning-for-rust/6370

### 1.6 SIMDjson 相位流水线（pm2 深拆，回答重点③）【吸收（机制）】
- **定位**：JSON 解析速度标杆。两阶段法出自 Langdale & Lemire（2019，"Parsing Gigabytes of JSON per Second"）；On-Demand 设计有专文（Keiser/Lemire/Langdale，*Software: Practice and Experience* 2024）。
- **可抄机制**：① **阶段一（SIMD 重）**：按 64 字节块做字符分类——结构字符（`{}[]:,`）位集、引号位集、反斜杠转义用**奇偶游程校验**（escape = backslash_mask & ~奇数游程掩码）区分"真引号/转义内引号"；pshufb 分类表一次出整块位图；② 位集→32 位索引走 fast bitset decode（lemire 2019 博客，按 tcz 分支展开）；③ **On-Demand（阶段二）**：不建完整 tape，应用请求到哪个字段才物化——字符串跳过用 SIMD（找结尾引号时同时处理转义），数字解析走 Clinger 快路径 + Eisel-Lemire；④ UTF-8 校验独立成相（见 2.19/2.20）。
- **对 ADV JSONL 的启发**：逐行处理时把"行边界定位（memchr）→ 结构索引（SIMD）→ 值按需（标量）"三段拼起来；阶段一可完全复用、行间无缝；失败行只需标量二次确认，路径与"默认档确定性"兼容。
- **第一步动作**：ADV JSONL 输入解析器先实现"字符串跳过 + 转义位集"两件，再决定是否整段吸收。
- **许可证/成熟度**：Apache-2.0 / 维护中（v3.x 活跃）。
- **链接**：https://github.com/simdjson/simdjson · https://lemire.me/blog/2019/11/27/really-fast-bitset-decoding-for-new-intel-processors/ · 论文 DOI 见 simdjson.org

### 1.7 Vectorscan Teddy（回答重点④上半）【reference】
- **定位**：Hyperscan 系的短字面量（≤8/16 模式）SIMD 预过滤器——多模式匹配吞吐上限的认知参照。
- **可抄机制**：① 每模式位置每字符类建 64 位 bucket 掩码表，pshufb 查表；② 16 字节滑窗用 PALIGNR 取 3 个对齐视图 × 3 张表 AND/OR 得候选位图；③ popcnt 选桶后**标量 memcmp 确认**——"SIMD 过滤便宜、标量确认稀有"的分层是精髓；④ fat-Teddy（AVX2/512，16 桶）扩容。
- **第一步动作**：adv-rules 若需内置多模式预过滤，优先绑定 Vectorscan（C 绑定需过安全审）而非复刻 Teddy；机制只作上限认知。
- **许可证/成熟度**：BSD-3-Clause / 维护中（随 Vectorscan）。
- **链接**：https://github.com/VectorCamp/vectorscan · http://0x80.pl/articles/simd-strfind.html（同作者算法面）

### 1.8 Vectorscan FDR + Rose 编排（回答重点④下半）【reference】
- **定位**：大字典（数百上千字面量）匹配引擎与编排层。
- **可抄机制**：① FDR：4/8 字节"指纹"哈希 → 位图桶过滤 → literal index 确认，两级过滤、确认器只在命中时动；② Rose：字面量驱动编排——预过滤命中触发 lazy DFA/NFA 确认与状态约束；③ 工程教训：吞吐上限=内存带宽量级；**模式选择（高区分度锚点字面量）比指令技巧更影响结果**。
- **第一步动作**：adv-rules 多模式设计文档引用"预过滤→确认"分层与锚点字面量选择准则。
- **许可证/成熟度**：BSD-3-Clause / 维护中。
- **链接**：https://github.com/VectorCamp/vectorscan

### 1.9 VectorScan 现状（2025-2026）【watch】
- **定位**：Hyperscan 的开源继任。背景：Intel 将 Hyperscan 5.5+ 转专有授权，最后全开源版为 5.4 系；**Vectorscan 5.4.12 = 最后 100% hyperscan 兼容版**，5.5/6.0 进入大重构（维护者口径，见 #330 讨论串）。
- **可抄机制/事实**：ARM NEON/ASIMD 与 Power VSX 100% 可用，SVE2 开发中，RISC-V V 扩展为 #330 feature request（2025-05）；2025 年仍是活跃依赖（Apache Doris 2025-08 构建、Suricata 文档列为 ARM 替代）；PyPI `hyperscan` 包已改绑 Vectorscan。
- **第一步动作**：若未来绑定，钉 5.4.12 兼容基线；watch 其 5.5/6.0 的 API 变动。
- **许可证/成熟度**：BSD-3-Clause / 维护中。
- **链接**：https://github.com/VectorCamp/vectorscan · https://vectorcamp.gr/project/vectorscan/

### 1.10 StringZilla 的 SWAR 手法（回答重点⑤）【有界】
- **定位**：Ash Vardanian（Unum）；作者口径子串搜索达 16 GB/s、较标准库快 5–10×（ashvardanian.com，2023 起）；SIMD+SWAR+GPGPU 综述见 Zenodo（2025）论文；官方提供 Rust 绑定。
- **可抄机制**：① **无 intrinsic 的 SWAR**：64 位字上做异或折叠 + haszero 掩码技巧（`(x - 0x0101..01) & !x & 0x8080..80`）一次比较 8 字节——任何 64 位 CPU 可用；② 按平台选不同精确子串算法变体（two-way 族 + 稀有字节跳读，0x80.pl 文章）；③ **无运行时分发、单一便携二进制**——对 ADV 保守档（不启用特征检测也能保底快）直接对口；④ SIMD 路径（AVX-512/NEON/SVE）是增益层而非正确性前提。
- **第一步动作**：把 SWAR 手法写进 perf-core 扫描层的"便携基线"小节，与 memchr 的 SWAR 回退互为印证；是否绑定 StringZilla 本体由安全审决定（第三方 C 库）。
- **许可证/成熟度**：Apache-2.0 / 维护中。
- **链接**：https://github.com/ashvardanian/StringZilla · https://ashvardanian.com/posts/stringzilla · http://0x80.pl/articles/simd-strfind.html

### 1.11 SIMD 变长整数 / Stream VByte【有界】
- **定位**：索引 postings/倒排编码的向量编解码（LEB128/varint 家族）。
- **可抄机制**：① 控制字节 = 4×2bit 长度选择器；② Masked VByte/Stream VByte：pshufb 按掩码收集数据字节 + blend 变长写回；Stream VByte 论文口径解码 **>4×10⁹ ints/s**（Lemire/Boytsov/Kurz，IP&P 2018，i7-6700 单机口径）；③ Rust 现成 crate：stream-vbyte。
- **第一步动作**：perf-core 用 ADV 真实 postings 分布做基准（标量 varint vs Stream VByte），阈值 ≥1.5× 才吸收；位一致性测试先行。
- **许可证/成熟度**：crate MIT / 维护中（低频更新）；论文 2018。
- **链接**：https://github.com/Marwes/stream-vbyte · 论文："Stream VByte: Faster Byte-Oriented Integer Compression"（IP&P 2018）

### 1.12 SIMD base64【有界】
- **定位**：输出层二进制 blob（快照/补丁）编码吞吐。
- **可抄机制**：① aklomp/base64：pshufb 做 6-bit→字符 LUT 查表，AVX2/AVX-512 路径 encode 快于 decode，量级 10+ GB/s（仓库自报 bench 口径，具体数值随 CPU）；② SWAR 压缩 4×6bit→3 字节的重排；③ Rust 落点：base64-simd（运行时检测 + 纯 Rust 回退）。
- **第一步动作**：若 JSONL 输出含 base64 字段，基准 base64-simd vs 纯 Rust base64；差 <1.5× 不吸收。
- **许可证/成熟度**：base64-simd MIT / 维护中；aklomp/base64 BSD-2-Clause / 维护中。
- **链接**：https://github.com/aklomp/base64 · https://github.com/Nugine/simd（base64-simd）

### 1.13 SIMD 哈希内部：xxh3 与 ahash-AES【有界】
- **定位**：缓存键/manifest/内容指纹哈希的向量内部结构。
- **可抄机制**：① xxh3：8×64-bit 累加器 + secret 混合，SSE2/AVX2/NEON 多实现，短输入专用路径（4/8/16B 单独处理）；② ahash：AES-NI 轮函数折叠（128-bit lane），无 AES 平台退 folded-multiply（X1-002 已登记 scan 级）；③ 教训：哈希 SIMD 的收益点在**多 lane 折叠**（一次混多块）而非单流延迟。
- **第一步动作**：不另立哈希选型（X1 已定 rapidhash/foldhash 主线）；把 xxh3 记为"内容指纹"备选（twox-hash 有 xxh3 支持）。
- **许可证/成熟度**：twox-hash MIT OR Apache-2.0 / 维护中；xxHash 参考实现 BSD-2-Clause。
- **链接**：https://github.com/Cyan4973/xxHash · https://github.com/tkaitchuck/aHash

### 1.14 Google highway【reference】
- **定位**：C++ 便携 SIMD + 动态分发标杆（libjxl/jpegli 生产级背书）。
- **可抄机制**：① `HWY_DYNAMIC_DISPATCH`：编译期生成全部 target 特化 + 运行时一次选优；② per-target 函数隔离避免编译器串染；③ Vec/Mask 类型抽象——Rust 侧包装层 API（对应 wide/pulp 的形状）可直接对照其设计。
- **第一步动作**：API 评审时对照 highway 的 Mask/SetFirstN 命名与语义；不绑定。
- **许可证/成熟度**：Apache-2.0 / 维护中（2025-2026 活跃）。
- **链接**：https://github.com/google/highway

### 1.15 坑 · 对齐与未对齐加载【吸收（准则）】
- **定位**：SIMD 正确性/性能第一坑。
- **机制**：① `_mm_load_si128` 要求 16 字节对齐，违反是**安全契约破坏（UB）**，`loadu` 无此约束；② 现代桌面 CPU（≥Haswell 口径）同行内未对齐加载代价≈0，真实代价在跨 64B 缓存行拆分；③ 实践：内核一律 loadu + 首尾标量处理；仅当基准证明收益才引入对齐分配。
- **第一步动作**：写进 perf-core SIMD 准则；代码评审门禁项。
- **许可证/成熟度**：准则 n/a；依据 Intel SDM / Agner Fog 指令表。
- **链接**：https://www.agner.org/optimize/

### 1.16 坑 · ARM NEON 与 x86 行为差【吸收（红线准则）】
- **定位**：ADV 判定字段必须机器无关——SIMD 路径必须逐位验证。
- **机制差异**：① NEON 只有 128 位宽（256 位语义需两拍拼）与标量掩码寄存器（掩码用寄存器内位图模拟）；② 浮点 FTZ/DAZ 语义差异（NEON 经 FPCR 可配，x86 SSE 默认不 flush、遇 denormal 变慢）；③ NEON 饱和算术（sqadd 等）在 x86 无对应指令——跨架构内核禁用；④ 结论：**判定路径只允许整数 SIMD**，浮点 SIMD 仅限性能敏感且不进判定字段的场合。
- **第一步动作**：CI 增加 aarch64 目标跑逐位一致矩阵（见 §3）；判定字段禁浮点 SIMD 写进红线。
- **许可证/成熟度**：准则 n/a；依据 ARM Architecture Reference Manual。
- **链接**：https://developer.arm.com/documentation/

### 1.17 坑 · 掩码 API 的可读性成本【吸收（包装层准则）】
- **定位**：掩码风格代码的维护成本被普遍低估。
- **机制**：① AVX-512 mask 寄存器 API（`_mm512_mask_blend` 族）表达力强，但移植到 AVX2 需 blendv 重写——写法即绑档位；② core::arch 无统一掩码抽象，nightly std::simd 的 `Mask<T,N>` 才是理想形状（1.2 未吸收）；③ 对策：裸 `__m256i` 不出模块边界；每个 SIMD 内核配**同语义 scalar 参考实现**做逐位对拍，掩码逻辑用命名帮助函数（如 `classify(byte) -> [bool; 32]`）承载语义。
- **第一步动作**：perf-core 规定"SIMD 内核 + scalar 对拍"为出厂标配（与 §3 测试矩阵同一件套）。
- **许可证/成熟度**：准则 n/a。
- **链接**：https://doc.rust-lang.org/core/arch/

---

## 2. 扫视（13 条：一句机制 + 档位 + 链接）

1. **simd-json**（Rust 移植 simdjson）——两阶段 + tape/DOM 风格，On-Demand 完整度弱于 C++ 原版；档位：有界（JSONL 原型期候选）。MIT OR Apache-2.0，维护中（simd-lite）。https://github.com/simd-lite/simd-json
2. **simdutf8**（Rust）——std::arch 实现的 UTF-8 校验（simdjson 算法变体），比通用 simdutf 更 Rust 原生；档位：watch（输入层预检）。MIT OR Apache-2.0，维护中。https://github.com/rusticstuff/simdutf8
3. **simdutf**（C++）——Unicode 校验/转码，lookup4/8 有限状态 pshufb 表，10+ GB/s 量级（作者口径）；档位：watch。Apache-2.0，维护中。https://github.com/simdutf/simdutf
4. **wide**（Lokathor）——core::arch 上的便携宽类型（f32x8 等）+ 无 SIMD 回退，API 形状最近 std::simd；档位：reference。Zlib，维护中。https://github.com/Lokathor/wide
5. **simdeez**——特征泛型 trait 写一次全平台（SSE2..AVX-512/NEON）+ 运行时 select，2.0 起以 const generics 复活；档位：watch。MIT，维护中（曾长期停滞，注意轮换风险）。https://github.com/jackmott/simdeez
6. **fearless_simd**（Linebender）——2025-06 "A plan for SIMD" 的落地：std::simd 之外的外部便携路线，实验期 API；档位：watch。Apache-2.0 OR MIT，实验。https://github.com/linebender/fearless_simd
7. **aho-corasick**——多字面量自动机 + memchr 预过滤，无 intrinsic 依赖，与 Teddy 互为参照；档位：吸收（adv-rules 多字面量默认底座）。MIT OR Apache-2.0，维护中。https://github.com/BurntSushi/aho-corasick
8. **regex**（rust-lang）——元自动机 + memchr prefilter，非 intrinsic 路线；档位：吸收（判定引擎既有底座，不动）。MIT OR Apache-2.0，维护中。https://github.com/rust-lang/regex
9. **lexical-core**——数字解析/格式化双栈，SIMD 友好算法而非 intrinsic；档位：reference。MIT OR Apache-2.0，维护中。https://github.com/Alexhuszagh/rust-lexical
10. **CRC 折叠（PCLMULQDQ / crc32 指令）**——SSE4.2 `crc32` 指令与 PCLMUL 折叠（zlib-ng 路线），块校验/manifest 场景；档位：有界。机制 n/a；Rust 落点 crc32c/crc32fast 等 crate（license 以 crates.io 为准）。https://github.com/rawrunprotected/crc32f （参考实现面）
11. **raw-cpuid**——no_std 细粒度 CPUID 查询（std 检测宏之外的口子）；档位：reference。MIT，维护中。https://github.com/gz/rust-cpuid
12. **Intel SDE**——无硬件模拟 AVX-512/新指令集路径，CI 矩阵验证件；档位：reference（流程内使用）。Intel EULA（免费、非 OSI），维护中。https://www.intel.com/content/www/us/en/developer/articles/tool/software-development-emulator.html
13. **qemu-user**——aarch64 用户态模拟跑跨架构逐位一致矩阵（§3 载体）；档位：吸收（进 CI 基建）。GPL-2.0，维护中。https://www.qemu.org/

---

## 3. 运行时分发（multiversioning）与测试矩阵（回答重点②）

**分发标准做法（ADV 默认第 1 种，其余备选）**：
1. 手写 fn 表：`OnceLock` 缓存一次选型 → safe `#[target_feature(enable="avx2")]` fn（1.85 起）→ unsafe 只收敛在"detect 结果 → 调用"一处；
2. pulp `WithSimd`（1.4）：内核单源、proof 入类型参数，多架构免漂移；
3. multiversion crate 属性宏：维护放缓，仅备选。
- 档位策略：x86_64 基线 SSE2 无条件编译；热点 ≤2 档（+AVX2）；AVX-512 仅当基准证明收益（1.89 起可写 stable，但需 SDE 覆盖测试）。

**测试矩阵（判据 = 逐位一致，对应"默认档确定性"红线）**：
1. **scalar 对拍**：每个 SIMD 内核配同语义 scalar 参考实现，同输入逐字节 diff——这是判定字段"机器无关"的主判据；
2. **强制特征路径**：同一 binary 依次走 SSE2 基线 / AVX2 /（可选 AVX-512 经 Intel SDE 模拟），输出两两逐位 diff；
3. **跨架构**：aarch64 目标（qemu-user 或 CI 硬件）跑同一矩阵，与 x86 结果逐位 diff；
4. **拒收浮点 SIMD 于判定字段**（1.16 的 FTZ/DAZ 差异）；若性能场合必须用，锁 FTZ/DAZ 并写进档位说明。
- 绿=SKIP 不算绿：SDE/qemu 不可用时该格记 SKIP 并显式暴露，不许静默降级为绿。

## 4. 重点专拆索引
- ③ SIMDjson 相位流水线 → 1.6（六步机制 + ADV JSONL 三段拼装）；
- ④ Vectorscan Teddy/FDR → 1.7 / 1.8 / 1.9（上限认知：预过滤吞吐≈内存带宽量级，锚点选择 > 指令技巧）；
- ⑤ StringZilla SWAR → 1.10（无 intrinsic 便携加速，保守档直接对口）。

## 5. Top-3（对 ADV 价值排序）
1. **memchr + std::arch 三层分级惯例**（1.1/1.3）：基线无条件编译 + 检测升级 + SWAR 回退——扫描相位第一天就用，零自研风险；
2. **SIMDjson 两阶段 / On-Demand 模型**（1.6）：JSONL 输入解析的机制蓝本，"结构索引先行、值按需"与确定性红线路径兼容；
3. **StringZilla 的 SWAR 手法**（1.10）：无 intrinsic、无分发的便携加速——保守档（无特征检测也保底快）的机制来源。

## 6. 锚与口径
- Rust 1.27（2018-06）std::arch 稳定（SSE..AVX2）；1.85（2025-02）target_feature 1.1（safe fn）；1.89.0（2025-08-07）AVX-512 intrinsics 稳定——均出自官方 release/博客；
- portable-simd：tracking #27731 截至 2026-10 无稳定化路线图；LoongArch64 intrinsics tracking #162508（2026-09）；
- Vectorscan：5.4.12 = 最后 100% hyperscan 兼容版、5.5/6.0 重构中（维护者口径，2025）；RISC-V #330（2025-05）；
- Stream VByte：>4×10⁹ ints/s 解码（IP&P 2018 论文口径，i7-6700）；StringZilla 16 GB/s（作者口径，ashvardanian.com）；aklomp/base64 10+ GB/s 量级（仓库自报 bench）；simdjson GB/s 级（Langdale & Lemire 2019 / Keiser 2024 SP&E）；
- 以上 GB/s 均为单机/特定 CPU 口径，非普适结论；ADV 吸收任何一条前须在本域语料复测（先量后改），否则只进 watch。
