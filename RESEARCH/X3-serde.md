# X3 序列化与零拷贝（serde / rkyv / bincode / zerocopy / 行格式）

> 域：X3 · 日期：2026-10-04 · 组：W4（给 perf-core 定序列化栈）
> 锚：docs.rs / crates.io 版本号均为 2026-10-04 检索所见；机制描述绑定具体版本，不外推到未验证版本。
> R7 既定：JSONL 为报告/缓存主格式；内容寻址用 blake3。本文档在 R7 基础上回答"报告层 vs 索引层各用什么"。

---

## 重点回答（5 问）

### ① rkyv 零拷贝校验机制 —— ADV 索引用它还是 bincode？

**机制（rkyv 0.8.18，docs.rs 2026-10-04）**：
- 0.8 起校验内建，不再依赖 0.7 时代的 `bytecheck::check_archived_root`：`rkyv::access::<A>(&bytes[..])` 做全量校验（指针落界、对齐、类型合法性），O(n) 扫描后返回安全引用 `&A::Archived`；`access_unchecked` 跳过校验，是 unsafe——调用方自证三件事：字节合法、缓冲按 Archive 类型对齐要求对齐（锚：rkyv issue「Document alignment requirements for accessing byte slices」2023-06，典型 4–16 字节，常用 `rkyv::util::AlignedVec` 满足）、且为同版本布局。
- 写侧 `rkyv::to_bytes` 直接返回 `AlignedVec`（对齐容器），mmap 读侧因映射基址页对齐（≥4KB）天然满足 8/16 对齐 → `memmap2 + rkyv` 组合可全程零拷贝。

**安全边界**：`access_unchecked` 的风险仅剩"字节来自不同版本写入/被篡改"。ADV 恰好有 R7 内容寻址（blake3）这张底牌：**哈希核对通过 ⇒ 字节未被篡改**；再把 format_version 写进文件头 ⇒ 版本一致。两个前提都满足时 `access_unchecked` 的安全论证成立，且是本仓自己的论证而非外部承诺。

**结论**：索引层主选 rkyv，读侧分两档起步——默认 `access()`（校验开销与 bincode 全量反序列化同阶，但省去重建索引结构的全量分配）；blake3 核对通过 + 版本匹配后走 `access_unchecked` 快路径。bincode 2.0 保留为对照档：索引文件小（量级 MB 级）或 schema 仍在频繁变动的开发期，bincode 的 serde 生态复用和稳定性更省事。跨机器移植性（端序/指针宽度）在 0.8.18 文档列表中未见内建保证——索引本就是可重建的本机缓存，把目标环境特征揉进内容寻址键即可，哈希不匹配自动失效重建。

### ② sonic-rs 在 Windows 的可用性与对 serde_json 的兼容度

- 平台（锚：docs.rs sonic-rs 原文，2026-10-04）："Faster in x86_64 or aarch64, other architecture is fallback and maybe very slower"；启用 SIMD 需 `-C target-cpu=native` **编译期**选择并在目标机编译。docs.rs 未给出 OS 级支持矩阵——Windows x86_64 走同一 SIMD 路径**大概率可用**，但须 CI 实测（MSVC 工具链 + AVX2）确认后才可下结论（本条按"未验证"对待）。
- 兼容度：serde 双向兼容，社区直接 `use sonic_rs as serde_json` 换名即可（锚：sonic issue #42，2023-12——该 issue 同时显示 `RawValue` 再导出仍有缺口，非 100% 等价）。
- **替换收益判据**：仅当 ADV JSONL ingest 实测（serde_json 基线 vs sonic-rs，同机同语料，perf_lock 计时）提速 ≥1.5× 且目标 CPU 支持 AVX2 时才换，且以 optional feature 形式接；否则维持 serde_json。理由：报告层写多读少、人读优先，JSON 解析不构成预期瓶颈（待 R1–R9 基线数据证实，非拍板）。

### ③ ADV 数据格式定案建议

| 层 | 用什么 | 判据 |
|---|---|---|
| 报告层（人读+流式） | **JSONL + serde_json** | 人可读、jq/grep 工具生态、逐行 append 崩溃可恢复、流式 `StreamDeserializer` 逐行读；`#[serde(default/alias/skip_serializing_if)]` 吃下演进 |
| 索引层（机器读+随机访问） | **rkyv 为主，bincode 2.0 为辅** | 需要 O(1) 访问且文件 ≥16MB 量级 → rkyv（零拷贝+免重建）；小文件/开发期/schema 未冻结 → bincode（简单稳定）；索引永远是可重建缓存，格式可激进 |
| 结果表导出 | CSV（一次性导出工具） | 表格工具/ML 生态；不作为 ADV 内部格式 |
| 列存分析 | Arrow IPC → watch | 只在有"对结果表做 OLAP 式聚合"的实测需求时再评估 |

判据落到动作：先统一 magic+版本头规范（见④），再在 perf-core 基准里量 serde_json / bincode / rkyv 三档的同语料读耗时与内存峰值，用数据取代本表中的"量级 MB"估计。

### ④ schema 演进最小规范（ADV 数据文件前后向兼容）

对照 protobuf 纪律（field number 永不复用、删除先 reserved）收敛为 6 条：
1. 每个数据文件头：magic（如 `ADVX`）+ `format_version: u32`；版本不匹配 → 按缓存策略重建，不硬读。
2. JSONL 报告层：字段**只增不删不改语义**；新增字段一律 `#[serde(default)]`；改名期间旧名 `#[serde(alias)]` 过渡一个版本；永远序列化稳定字段序（serde struct 按声明序，天然满足）。
3. 删除字段时先保留读取兼容一版（default/alias 双轨），不重用旧字段名。
4. 枚举演进：未知变体落 `Unknown(String)` 兜底而不是报错——报告由旧版写入、新版读取时不丢信息。
5. rkyv 索引层：0.8 的"高位 API 兼容"不承诺跨改动的字节兼容 → 依赖 format_version 整档失效重建；索引文件小改不值得迁移代码。
6. 非自描述二进制（bincode/postcard/rkyv）两端必须同 schema 版本，此约束写进文件头校验逻辑。

### ⑤ bytemuck/zerocopy 在"句柄化 AST 序列化"里的角色

- 前提：AST/索引结构句柄化——arena 池 + `u32` 句柄替代指针/`Rc`，字符串换成 `(offset, len)` 指向外部 blob——节点结构变成无引用 POD。
- **zerocopy 0.8**（Google，Fuchsia/Android 采用，Linux kernel 亦有 IntoBytes 相关补丁，锚：zerocopy 仓库/docs 2026-10-04）：`#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]` + `#[repr(C)]`，把"这块内存可以安全按位 reinterpret"变成**派生期布局验证**（padding/位模式/对齐编译期+宏期把关），调用点零 unsafe。0.8 重排了 trait 层级（AsBytes→IntoBytes、拆出 KnownLayout/Immutable/TryFromBytes）。
- **bytemuck 1.x**：更轻的 `cast_slice` 位转换，`Pod`/`Zeroable` 派生；无布局论证时安全责任在写代码的人。
- 角色分工：扁平节点表（`[AstNode]`）+ 句柄引用的序列化 = `zerocopy` 校验布局 + `bytemuck` 风格按位读写（二选一或搭配：结构定义用 zerocopy 派生做论证，转换走其 API）；表达力不足处（嵌套字符串/变长）留给 rkyv。**边界**：仅 POD；对齐由容器保证；padding 必须确定性写入（`#[repr(C)]` + 布局检查）。比 rkyv 更快更简单，但只适合"结构固定、扁平化已做"的表。

---

## Top-3（按行动价值排序）

1. **rkyv 0.8 零拷贝 + access 校验双档**（机制级，吸收）——索引层主选；`access()` 起步、blake3+版本头放行 `access_unchecked`，安全论证落在自家内容寻址上，不欠外部承诺。
2. **serde 字段规则 → JSONL 报告层定案**（机制级，吸收）——6 条演进规范（magic+版本头/default/alias/Unknown 兜底/只增不删/两端同版本）直接写进 ADV 格式规范，成本≈0，避免日后数据迁移。
3. **bincode 2.0 cfg/编解码器抽象**（机制级，吸收）——serde 变可选、原生 Encode/Decode+derive、无 std 依赖面小；作为索引层小文件/开发期默认档，且给 perf-core 提供"紧凑格式基线"。

---

## 机制级条目（13）

### 1. serde_json
- 定位：Rust JSON 事实基线；serde 生态锚点，其他所有 JSON 实现的对标物。
- 可抄机制：① 非递归下降的 `SliceRead`/`StrRead` 字节游标解析（无 tape 结构，勿以 tape 描述它）；② `StreamDeserializer` 逐个 JSON 值迭代 → JSONL 逐行流式读的现成实现；③ `RawValue`/`&RawValue` 延迟解析（跳过不需要的子树不建 DOM）。
- 档位：【吸收】——报告层默认格式实现。
- 第一步：报告层落 JSONL + `StreamDeserializer` 行读；基线耗时进 perf-core 基准。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中（serde-rs/json，1.0.x，2026-10-04 crates.io 在活跃发版）。
- 链接：https://github.com/serde-rs/json · https://docs.rs/serde_json

### 2. serde（derive 层）
- 定位：serde 数据模型本身；X3 格式规范的演进机制载体。
- 可抄机制：① `#[serde(default)]` 新字段前后向兼容；② `#[serde(alias)]` 改名过渡；③ `#[serde(skip_serializing_if)]`+`deny_unknown_fields` 的取舍（ADV 报告**不**用 deny_unknown_fields，换取前向兼容）。
- 档位：【吸收】。
- 第一步：把 6 条演进规范落成 REPORT-FORMAT.md 的 serde 属性约定。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中。
- 链接：https://serde.rs

### 3. sonic-rs（字节跳动 SIMD JSON）
- 定位：serde_json 的 SIMD 加速替代，借鉴 sonic/simdjson/serde_json。
- 可抄机制：① x86_64 AVX2 / aarch64 NEON 双路径 + 非支持架构 fallback；② 惰性取原始片段 API（类 RawValue 但更快）；③ `-C target-cpu=native` 编译期 SIMD 选择策略（docs.rs 原文）。
- 档位：【有界】——可选 feature，须先过②节收益判据；Windows SIMD 未经 CI 实测前不下结论。
- 第一步：进 perf-core 基准矩阵（serde_json vs sonic-rs 同语料），数据出来再定 feature 开关。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中（bytedance/sonic，0.5.x）。
- 链接：https://docs.rs/sonic-rs · https://github.com/bytedance/sonic

### 4. simd-json（simd-lite）
- 定位：C++ simdjson 思想的 Rust 移植（与 FFI 绑定的 `simdjson` crate 是两回事，见扫视 19）。
- 可抄机制：① 两阶段解析（结构 stage-1 SIMD 标记 + stage-2 解析）；② x86 运行时选 SSE4.2/AVX2 kernel；③ 自有 DOM（`simd_json::Value`），serde 兼容但 trait 导入与 serde_json 有差异（锚：serenity 集成用 `json::prelude::*` 的 changelog）。
- 档位：【有界】→ 倾向【不吸收】：与 sonic-rs 解决同一问题，引入两个 SIMD JSON 会分裂基准面；sonic-rs 若过判据则 simd-json 不再考虑。
- 第一步：仅在 sonic-rs 判据失败时才评估。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中（0.14.x，Polars 等在用）。
- 链接：https://github.com/simd-lite/simd-json · https://crates.io/crates/simd-json

### 5. rkyv
- 定位：零拷贝反序列化库；archived 表示即文件布局，读=cast+校验。
- 可抄机制：① `access()`/`access_unchecked` 双档（见重点①）；② `AlignedVec` 对齐容器 + mmap 基址天然对齐的组合；③ `#[derive(Archive, Serialize, Deserialize)]`，archived 字段是偏移/内联值而非指针。
- 档位：【吸收】——索引层主选。
- 第一步：索引文件头（magic+version）+ rkyv `access()` 读路径 PoC，与 bincode 对照基准。
- 许可/成熟度：MIT · 维护中（djkoloski/rkyv，0.8.18，2026-10-04 docs.rs）。
- 链接：https://docs.rs/rkyv · https://github.com/rkyv/rkyv

### 6. bincode 2.0
- 定位：紧凑二进制 serde 格式的现代重构；serde 变为可选依赖。
- 可抄机制：① 原生 `Encode`/`Decode`/`BorrowDecode` trait + `bincode-derive`，绕开 serde 的重量；② `serde` feature 才启用 serde 兼容层（cfg 隔离）；③ `no_std` 可用（alloc 可关）→ perf-core 里依赖面可控。
- 档位：【吸收】——索引层辅档 + perf-core 紧凑基线。
- 第一步：与 rkyv 同基准对比（吞吐/内存/代码量）。
- 许可/成熟度：MIT · 维护中（bincode-org/bincode，2.x）。
- 链接：https://docs.rs/bincode · https://github.com/bincode-org/bincode

### 7. postcard
- 定位：嵌入式风格 serde 格式：varint + 非自描述。
- 可抄机制：① 整数 varint 压缩规则（platform-sized 映射定宽）；② flavor 抽象（Cobs/heap/crc 可叠加）——"格式=核心编码+装饰层"的分层思路值得抄；③ 无分配 no_std 序列化。
- 档位：【有界】——ADV 文件场景收益有限（rkyv/bincode 已覆盖），但其 flavor 分层思想可借鉴到自家 codec；不引入依赖。
- 第一步：仅抄机制；如 perf-core 需要超紧凑 IPC 再回看。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中（jamesmunns，2019 起向 1.0 演进，锚：作者博客 2022-05）。
- 链接：https://postcard.rs · https://github.com/jamesmunns/postcard

### 8. zerocopy
- 机制详见重点⑤。
- 档位：【吸收】——句柄化 AST/节点表的布局论证工具。
- 第一步：对第一个句柄化 POD 结构上 `#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]` + `#[repr(C)]` 跑通。
- 许可/成熟度：BSD-3-Clause（另附 Apache-2.0 兼容 notice）· 维护中（Google，0.8.x）。
- 链接：https://docs.rs/zerocopy · https://github.com/google/zerocopy

### 9. bytemuck
- 机制详见重点⑤。
- 档位：【有界】——若 zerocopy 派生已覆盖转换需求则不引入第二个同类依赖；仅在需要 `cast_slice` 轻量转换、且团队接受自证安全时用。
- 第一步：与 zerocopy 在 PoC 中二选一，保留一个。
- 许可/成熟度：Zlib OR MIT OR Apache-2.0 · 维护中（1.x）。
- 链接：https://docs.rs/bytemuck

### 10. bytes
- 定位：tokio 系引用计数字节缓冲。
- 可抄机制：① `Bytes` = Arc 指针+区间，`slice()` 零拷贝切分 → 报告行/片段共享底层缓冲；② `Buf`/`BufMut` trait 抽象读写游标；③ `BytesMut` 复用分配（写 JSONL 缓冲）。
- 档位：【吸收】——perf-core 管道内缓冲与"片段视图"统一类型。
- 第一步：扫描/报告管道的中间缓冲统一为 `Bytes`，消灭逐行 `Vec<u8>` 拷贝。
- 许可/成熟度：MIT · 维护中（tokio-rs/bytes，1.x）。
- 链接：https://docs.rs/bytes

### 11. memmap2
- 定位：mmap 的安全薄封装（memmap-rs 的社区续作）。
- 可抄机制：① 只读映射大索引文件 + 切片即零拷贝读；② 对齐事实：映射基址页对齐 → 满足 rkyv/zerocopy 对齐要求；③ **安全边界**：映射期间文件被外部改写属未定义行为风险——ADV 场景用"文件写完后 rename 原子发布 + 只读映射"规避。
- 档位：【有界】——只在索引层走 mmap 路径；报告层 JSONL 用普通流式读写。
- 第一步：rkyv PoC 里以 `memmap2::Mmap` 作读侧容器之一对比 `AlignedVec` 全量读。
- 许可/成熟度：MIT OR Apache-2.0 · 维护中（0.9.x）。
- 链接：https://docs.rs/memmap2

### 12. flatbuffers
- 定位：schema 驱动零拷贝（vtable+offset），对照 rkyv 的"无 schema 也能零拷贝"。
- 可抄机制：① vtable 字段缺省省空间（未写字段=默认值，零存储）→ 索引稀疏字段省盘；② 构建期反向拼装（buffer 从尾往前长）；③ schema 文件+代码生成 = 跨语言演进纪律的现成样板。
- 档位：【reference】——Rust 单仓场景引入 schema 代码生成不划算，抄其字段缺省思想即可。
- 第一步：无；把 vtable 缺省省空间的思路记入索引布局设计备忘。
- 许可/成熟度：Apache-2.0 · 维护中（google/flatbuffers）。
- 链接：https://github.com/google/flatbuffers

### 13. capnp（capnproto-rust）
- 定位：schema 驱动 + arena 分配 + pointer/offset 布局 + RPC 能力（对照项）。
- 可抄机制：① arena 内零拷贝读（message reader 视角）；② packed 编码变体（去零字节）；③ schema 演进规则文档（field number/reserved 纪律的权威表述）。
- 档位：【reference】——只需其演进纪律文档与 rkyv 互补，不引入运行时。
- 第一步：无；演进规范（重点④）已吸收其纪律。
- 许可/成熟度：MIT · 维护中（dwrensha/capnproto-rust）。
- 链接：https://github.com/capnproto/capnproto-rust

---

## 扫视条目（一句机制+档位+链接）

14. **serde 字段规则以外的 `#[serde(flatten)]`** — 嵌入动态段（报告的扩展块）进 struct 而不破坏静态 schema — 吸收（报告扩展块用）— https://serde.rs/attr-flatten.html
15. **flexbuffers**（flatbuffers 项目内 crate）— 无 schema 扁平二进制：map 存 key 向量+偏移、值带 1 字节类型标签，可随机访问且半自描述 — reference（备选"半自描述缓存"，暂无场景）· Apache-2.0 · 维护中 — https://github.com/google/flatbuffers/tree/master/rust/flexbuffers
16. **rmp（MessagePack）** — 1 字节类型标签 + varint 长度的自描述紧凑格式，rmp/rmp-serde 双 crate 分层 — reference（自描述需求出现时再评估）· MIT · 维护中 — https://github.com/3Hren/msgpack-rust
17. **prost / protobuf 纪律** — field number 永不复用+reserved 的 schema 演进权威做法，作为 ADV 6 条规范的上游依据 — reference · Apache-2.0 · 维护中 — https://github.com/tokio-rs/prost
18. **simdjson（SunDoge）** — 对 C++ simdjson 库的 FFI 绑定（与 simd-json 移植版区分）：C++ 工具链依赖重 — 不吸收 · 许可以 crates.io 页为准 · 维护中 — https://github.com/SunDoge/simdjson-rust
19. **bytecheck** — rkyv 0.7 时代的 archive 校验框架；0.8 校验已内建进 `access()`，新代码不再直接依赖 — reference · MIT · 维护中 — https://docs.rs/bytecheck
20. **JSONL 行格式** — 逐行自包含 JSON：append 友好、崩溃丢最后半行可弃、jq/grep/wc 生态、`StreamDeserializer` 流式读；代价是无键级随机访问（需索引层补） — 吸收（报告层定案，R7 复核）— https://jsonlines.org
21. **CSV** — 结果表一次性导出格式：表格/ML 工具生态；类型推断弱、转义烦，不做内部格式 — reference（导出工具用）— https://docs.rs/csv
22. **自定义行协议** — 自造行分隔二进制：省解析但丢全部工具生态、需自维护行解析与转义 — 不吸收（JSONL 生态收益碾压省的那点解析）— （无单一链接，判据见③）
23. **Arrow IPC（arrow-rs）** — 列存 record batch + Feather V2 文件/流：对"结果表做聚合分析"有潜力，引入代价是重依赖+列式思维 — watch（等 OLAP 需求实证）· Apache-2.0 · 维护中 — https://arrow.apache.org/docs/format/IPC.html · https://github.com/apache/arrow-rs
24. **simdutf8** — SIMD UTF-8 校验（std 现实现是标量回退）：JSONL ingest 前置校验的加速点 — watch（ingest 基准显示 UTF-8 校验进热区才引入）· 许可以 crates.io 页为准 · 维护中 — https://docs.rs/simdutf8
25. **memchr** — 子向量 SIMD 搜索：JSONL 行切分/`\n` 定位的底座，serde_json 内部亦用同类实现 — 吸收（已在依赖树内则直接用，不新增）· MIT OR Apache-2.0 · 维护中 — https://docs.rs/memchr
26. **rkyv AlignedVec** — 为对齐访问特化的 Vec（重分配时保持对齐）：非 mmap 路径下承载索引缓冲的标配 — 吸收（随 rkyv 一并引入）· MIT · 维护中 — https://docs.rs/rkyv/latest/rkyv/util/struct.AlignedVec.html

---

## 汇总表

| # | 对象 | 档位 | integration | 许可 | 成熟度 |
|---|---|---|---|---|---|
| 1 | serde_json | 吸收 | adv-index | MIT OR Apache-2.0 | 维护中 |
| 2 | serde | 吸收 | adv-index | MIT OR Apache-2.0 | 维护中 |
| 3 | sonic-rs | 有界 | perf-core | MIT OR Apache-2.0 | 维护中 |
| 4 | simd-json | 有界（倾向不吸收） | perf-core | MIT OR Apache-2.0 | 维护中 |
| 5 | rkyv | 吸收 | adv-index | MIT | 维护中 |
| 6 | bincode 2.0 | 吸收 | perf-core | MIT | 维护中 |
| 7 | postcard | 有界 | reference | MIT OR Apache-2.0 | 维护中 |
| 8 | zerocopy | 吸收 | perf-core | BSD-3-Clause | 维护中 |
| 9 | bytemuck | 有界 | perf-core | Zlib OR MIT OR Apache-2.0 | 维护中 |
| 10 | bytes | 吸收 | perf-core | MIT | 维护中 |
| 11 | memmap2 | 有界 | adv-index | MIT OR Apache-2.0 | 维护中 |
| 12 | flatbuffers | reference | reference | Apache-2.0 | 维护中 |
| 13 | capnp | reference | reference | MIT | 维护中 |
| 14 | serde flatten | 吸收 | adv-index | （serde 一体） | 维护中 |
| 15 | flexbuffers | reference | reference | Apache-2.0 | 维护中 |
| 16 | rmp | reference | reference | MIT | 维护中 |
| 17 | prost | reference | reference | Apache-2.0 | 维护中 |
| 18 | simdjson FFI | 不吸收 | reference | 以 crates.io 为准 | 维护中 |
| 19 | bytecheck | reference | reference | MIT | 维护中 |
| 20 | JSONL | 吸收 | adv-index | 格式规范 | — |
| 21 | CSV | reference | reference | — | — |
| 22 | 自定义行协议 | 不吸收 | reference | — | — |
| 23 | Arrow IPC | watch | watch | Apache-2.0 | 维护中 |
| 24 | simdutf8 | watch | watch | 以 crates.io 为准 | 维护中 |
| 25 | memchr | 吸收 | perf-core | MIT OR Apache-2.0 | 维护中 |
| 26 | AlignedVec | 吸收 | adv-index | MIT | 维护中 |

条数：26（机制级 13 + 扫视 13）。所有"量级 MB/≥16MB/1.5×"均为待 perf-core 基准证实的**工作假设**，不是结论；下一步动作见各条"第一步"。
