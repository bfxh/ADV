# X5 压缩调研（ADV 缓存 / 索引 / 快照）

> 日期：2026-10-04 ｜ 域：X5-compression ｜ 调研人：WebSearch/WebFetch，数字均带锚
> 适用对象：ADV（D:\KF\ADV，adv-rewrite 分支）的缓存、索引、快照三类落盘数据
> 范围声明：qoi 类图像格式与本域无关，略。ouch（C2 已覆盖），略。

---

## Top-3（最值得吸收）

| # | 结论 | 一句机制 | 归宿 |
|---|------|---------|------|
| 1 | **zstd 三段定档 + 小对象字典** | 解压速度与档位基本无关（zstd 1.5.6 README），所以档位只决定写成本；读多写少的缓存可放心用中高档，小 JSONL 用 `--train` 字典补冷启动 | perf-core + adv-index |
| 2 | **先哈希后压缩的顺序规范**（restic/git 同型） | 键 = hash(明文 chunk)，压缩/换档/重压缩永不改键 ⇒ 字典升级、L3→L19 冷却都是纯存储迁移，不触发索引重建 | adv-index |
| 3 | **绑定策略：zstd-safe(C) 主力，纯 Rust 兜底且设计期锁死** | zstd 字典一旦启用，纯 Rust 解码器（ruzstd）解不了带字典帧 ⇒ "要不要字典"是依赖政策级决策，不能事后改 | perf-core |

---

## ① ADV 缓存/快照压缩定档建议（判据先行）

**基准锚**（zstd dev 分支 README，lzbench @ Silesia 语料，i7-9700K@4.9GHz / Ubuntu 20.04 / gcc 9.4.0，zstd 1.5.6，2026-10-04 取读）：

| 算法 | ratio（原/压） | 压缩 | 解压 |
|------|-------|------|------|
| zstd 1.5.6 `-1` | 2.887 | 510 MB/s | 1580 MB/s |
| zstd `--fast=1` | 2.437 | 545 MB/s | 1890 MB/s |
| zstd `--fast=3` | 2.239 | 650 MB/s | 2000 MB/s |
| lz4 1.9.4 | 2.101 | 700 MB/s | 4000 MB/s |
| snappy 1.1.9 | 2.073 | 530 MB/s | 1660 MB/s |
| brotli 1.0.9 `-0` | 2.702 | 395 MB/s | 430 MB/s |
| zlib 1.2.11 `-1` | 2.743 | 95 MB/s | 400 MB/s |

README 原文锚："Decompression speed is preserved and remains roughly the same at all settings" —— **解压侧成本对档位不敏感，档位预算全部花在写路径**。

**三段定档（判据，不是默认值）**：

| 数据类别 | 判据 | 定档 |
|---------|------|------|
| 热缓存（每次扫描写、反复读） | 写频高、单次写预算紧 | zstd L3（默认档）；写延迟敏感且可接受更低比率 → `--fast=1..3` 或 lz4_flex |
| 温数据（索引段、去重后的 chunk 池） | 一次写、多次读 | zstd L9–L12（读路径免费，写只在首扫付出） |
| 冷快照/归档 | 批处理、时间不敏感 | zstd L19；大文件（> 数 MB 连续区）加 `--long` 长距窗口 |
| 裸存（algo=none） | 压缩后 ≥ 原大小 90%，或 len < ~1KB 的随机性小对象 | 存原始字节 + 对象头记 `algo=none`（省 CPU，省下的比率≈0） |

**对象头必须记算法 ID + 档位**：允许每个对象独立迁移，是"换档不改键"的前提（见 ④）。

## ② zstd 字典模式对"大量小 JSONL"的收益路径

**机制**：压缩算法靠"前面已见的数据"学习统计（熵表 Huff0/FSE + 匹配历史）；单个几 KB 的对象在帧开头没有"过去"可学 ⇒ 冷启动税。字典把学习结果提前物化，压帧前熵表/历史已就位。README 锚：官方样例 github-users（~10K 条、每条 ~1KB），训练字典后 ratio 与压缩/解压速度**同时**改善；"Dictionary gains are mostly effective in the first few KB"。

**训练语料怎么选（判据）**：
1. **每 schema 一个字典**——README 原文 "no universal dictionary"，字典越贴合数据类型越有效。ADV 的 JSONL 按记录类型分桶（诊断记录/符号表/告警事件各一）。
2. 样本 = 该 schema 的**真实记录**：JSONL 按行（或每 N 行）切块，"一样本一文件"喂 `zstd --train`。
3. 量级（社区口径，zstd issues/博客综合，非官方硬数）：≥100 样本起步，几百到几千封顶，总量为数倍字典体积；过少过拟合，过多无增益。产出后**必须在 held-out 样本上对比有/无字典的 ratio** 再定稿（否证留档）。
4. 工具链：默认 `--train-fastcover=d=8,steps=4`（Oracle zstd man page）；追求更优用 `--train-cover`（更慢）；`--maxdict` 默认 112640 B（110KB），解码端字典要常驻内存/L3——ADV 场景可缩到 32–64KB 换取多字典共存。
5. 部署：zstd 帧内嵌 4B dictID 自动选字典；字典本体版本化入库；换字典不改任何寻址键（④）。

## ③ 纯 Rust 解码器 vs C 绑定（依赖政策取舍）

| 维度 | zstd-sys/zstd-safe（C 绑定） | ruzstd（纯 Rust 解码） | lz4_flex（纯 Rust 双向） |
|------|------|------|------|
| 功能 | 全：字典/训练/`--long`/多线程 | 仅解码；**不支持 trained-dictionary 帧**（社区口径，docs.rs/ruzstd） | block+frame 双格式，与 lz4 CLI 帧兼容 |
| 性能 | C 级（上表） | 社区口径比 C 慢约 2–5×（非正式基准，无权威发布，引用需复测） | 宣称最快纯 Rust LZ4；无 C 但率比 zstd 低（上表 lz4 行） |
| 安全 | FFI + cc 构建依赖 | 100% safe Rust | 默认无 unsafe（可选 unsafe 特性） |
| 许可 | libzstd: **BSD OR GPLv2**；crate: MIT OR Apache-2.0 | MIT OR Apache-2.0（社区口径） | MIT（Pascal Seitz） |
| 成熟度 | 参考实现，生产级（Meta 部署，README 原文） | 中等；小众维护，Cargo 生态在用 | 较成熟；libarchive_oxide 用作可移植 codec 并做过帧兼容验证 |

**ADV 取舍判据**：默认 `zstd-safe`（功能与性能全覆盖）；仅当构建环境无 cc（交叉编译/wasm）或依赖政策禁 C 时降级：热缓存用 lz4_flex 双向、已有 zstd 流用 ruzstd 解码——**且此路径不得含字典帧**。⇒ 是否启用字典 = 设计期决策，锁定后写入对象头算法 ID，降级路径只在 `algo=zstd-nodict` 的分区生效。

## ④ 压缩 × 内容寻址的顺序规范（吸收 restic/git 同型）

- **restic 0.14+（repo v2）**：内容定义分块 → **chunk ID = SHA256(明文 chunk)** → zstd 压缩 → 加密（restic 官方 docs，Tuning Backup Parameters / 0.14 changelog）。键与压缩器版本、档位无关。
- **git**：blob 键 = SHA-1(明文)，zlib 只在存储层 —— 同型，多年生产验证。
- **ADV 规范**（建议）：`键 = hash(明文 chunk)`（哈希算法由索引域定）→ 明文上去重 → 压缩（对象头记 algo ID+档位+dictID）→ 读路径解压后 re-hash 对键校验。**禁止压缩后哈希**。
- 收益链：换档（L3→L19 冷却）、换字典、重压缩 = 纯后台迁移任务；去重不受压缩率波动影响；压缩器 bug 可回滚到裸存重压。

## ⑤ 级档判据：CPU 时间 vs 大小的实测口径

1. 语料：ADV 真实产物抽样（缓存对象 + 快照 chunk + 小 JSONL 批），≥100MB，语料清单+哈希存档（复算口径）。
2. 工具：`zstd -b`（内置 lzbench 同源）或 lzbench；固定 zstd 版本、单/多线程分开记录。
3. 每档采集三元组：ratio / 压缩 MB/s / 解压 MB/s；画 knee 曲线；判据 = **写路径预算内取最高档**（读侧免费，见 ① 锚）。
4. 小 JSONL 单独跑：有/无字典两组，held-out 验证（② 第 3 条）。
5. 结论格式：`档位 X 在语料 Y（哈希 Z）、zstd vW、N 线程下：ratio A、写 B MB/s`——不写无锚的"zstd 很快"。

---

## 机制级条目

### 1. zstd 参考实现（facebook/zstd）
- **定位**：缓存/快照的主压缩器；zlib 级速度 + 更高比率，格式已标准化（RFC 8878）。
- **可抄机制**：① 负档位 `--fast=N` 把速度推到 lz4 区间（2.239 ratio @ 650 MB/s，Silesia 锚）——同一 API 覆盖快慢两端；② 解压速度档位无关 ⇒ 定档只看写预算；③ 长距匹配 `--long`（大连续文件的窗口外重复）。
- **档位**：见 ① 三段表。
- **第一步动作**：adv-index 落盘 API 统一走 `zstd-safe` 封装层，L3 起步，对象头带 algo ID。
- **许可证/成熟度**：BSD OR GPLv2；参考实现，生产级。
- **链接**：https://github.com/facebook/zstd

### 2. zstd 字典模式（`--train` / fastcover / cover）
- **定位**：小 JSONL/小对象批量的专用路径；解决帧开头冷启动税。
- **可抄机制**：① 每 schema 一字典 + 帧内 dictID 自动路由；② 语料 = 真实记录切块（JSONL 按行）；③ `--maxdict` 缩容换多字典共存。
- **档位**：单对象 < ~4KB 且同构 ⇒ 字典；> 几十 KB 字典增益趋零（增益集中在前几 KB，README 锚）。
- **第一步动作**：从 ADV 缓存抽取 1–2 个 schema 各 500 条真实记录，`--train` 出 64KB 字典，held-out 对比 ratio 后定档。
- **许可证/成熟度**：随 zstd（BSD OR GPLv2）；官方特性。
- **链接**：https://github.com/facebook/zstd （README "The case for Small Data"）；man page：https://docs.oracle.com（zstd(1)）

### 3. zstd-sys / zstd-safe（Rust 绑定）
- **定位**：ADV 主绑定；zstd-rs 生态的 `-sys`/safe 封装层，暴露 libzstd 全特性（流/字典/多线程/训练）。
- **可抄机制**：① `zstd::bulk::Compressor` 带字典的复用型压缩器（省每次初始化）；② feature 门控制多线程与长窗；③ stream API 直通 Read/Write。
- **档位**：n/a（透传 ①）。
- **第一步动作**：adv-index 依赖表加 `zstd-safe`（默认特性减去不需要的），封装一层 `CompressAlgo` 枚举屏蔽 ③ 的降级路径。
- **许可证/成熟度**：crate MIT OR Apache-2.0；底层 libzstd BSD OR GPLv2；绑定层广泛使用。
- **链接**：https://crates.io/crates/zstd-safe

### 4. ruzstd（纯 Rust zstd 解码器）
- **定位**：构建兜底（无 cc/交叉/wasm）时的**只读**解码路径；不承担主写路径。
- **可抄机制**：① 纯 safe Rust 解码状态机（Huff0/FSE 表构建的可读实现，审计友好）；② streaming frame 逐块解码；③ 无 FFI ⇒ 远程构建免工具链。
- **档位**：仅解码；**不支持 trained-dictionary 帧**（社区口径）⇒ 与 ② 互斥。
- **第一步动作**：不进默认依赖；进 CI 一个"纯 Rust 降级"feature 组合做编译验证（`--no-default-features` 矩阵），仅在 `algo=zstd-nodict` 分区启用。
- **许可证/成熟度**：MIT OR Apache-2.0（社区口径）；中等，解码慢 C 约 2–5×（社区口径，未复测）。
- **链接**：https://crates.io/crates/ruzstd

### 5. lz4_flex（纯 Rust LZ4）
- **定位**：写延迟极敏感的热缓存备选；纯 Rust 双向（编码也在 Rust 侧，ruzstd 做不到）。
- **可抄机制**：① block 格式（无帧头，嵌入自管容器，ADV 对象内裸块）；② frame 格式（流式，与 lz4 CLI 互通，libarchive_oxide 验证过兼容）；③ 默认 safe、可选 unsafe 提速。
- **档位**：Silesia 锚（C 实现）：2.101 ratio / 700 MB/s 写 / 4000 MB/s 读——比 zstd `--fast=3` 快 8% 写、2× 读，但比率低 6%。**仅当写延迟实测卡预算才启用**。
- **第一步动作**：基准竞品（⑤ 口径）里加 lz4_flex 行；不赢不出场。
- **许可证/成熟度**：MIT；较成熟（crates.io 0.11–0.14 线）。
- **链接**：https://crates.io/crates/lz4_flex ；https://github.com/Nullus157/lz4-flex

### 6. restic 的压缩×寻址设计（模式来源）
- **定位**：④ 顺序规范的权威参照（去重备份工具，与本域"快照"同构）。
- **可抄机制**：① ID=SHA256(明文 chunk)，压缩在其后、加密之前；② repo v2 对象头带压缩标记（zstd/off），旧仓库无标记可读；③ `--compression auto/off/max` 三档对齐 ① 的定档思路。
- **档位**：auto = 按数据可压性跳过（等价 ADV 的 90% 判据）。
- **第一步动作**：把 ④ 规范写进 adv-index 的对象格式 RFC（对象头字段清单）。
- **许可证/成熟度**：restic BSD-2-Clause；生产级，0.14（2022）引入压缩。
- **链接**：https://restic.readthedocs.io （Tuning Backup Parameters / 100_references）

### 7. zstd Seekable Format（随机访问压缩流）
- **定位**：大快照文件的随机读（不解压全量）；每帧独立可跳。
- **可抄机制**：① seek table 索引帧边界（帧起点偏移+解压大小）；② 按帧 lazy 解压；③ 与 ④ 内容寻址兼容（帧边界写进对象头即可校验局部 hash）。
- **档位**：适用 > 数 MB 的单快照文件；小对象批量场景无收益。
- **第一步动作**：watch 到快照格式 RFC 期再定；先用"多 chunk 多对象"替代（内容寻址天然分段）。
- **许可证/成熟度**：随 zstd（BSD OR GPLv2）；官方 contrib 文档，非核心 API，采用者中等。
- **链接**：https://github.com/facebook/zstd/tree/dev/doc/seekable_format

### 8. async-compression（tokio/futures 流式压缩适配层）
- **定位**：watch 域的异步流压缩（若 ADV 管道改 async）；现在同步路径不需要。
- **可抄机制**：① 双轴 feature：runtime（tokio/futures-io）× codec（zstd/brotli/gzip/xz…）；② `bufread::ZstdEncoder` 包装 AsyncRead；③ 档位透传有限（部分封装不暴露 level）⇒ 直用 ③ 的绑定更可控。
- **档位**：n/a。
- **第一步动作**：不进依赖；watch 域（async 化）立项时复评。
- **许可证/成熟度**：MIT OR Apache-2.0（crates.io）；0.4.x，TiKV/Vector 在用，成熟。
- **链接**：https://docs.rs/async-compression

---

## 扫视条目（一句机制+档位+链接）

9. **lz4（参考 C 实现）**——帧/块双格式，速度上限锚 700/4000 MB/s @ 2.101（Silesia 1.9.4）；比 zstd fast 段快但比率低，ADV 有 zstd-fast 时价值有限。BSD-2；生产级。https://github.com/lz4/lz4
10. **brotli**——RFC 7932 固定格，q0–11；强项 web 静态资源小文件（q10–11 高率但编码极慢）；Silesia 上 q0 不敌 zstd -1（2.702@395 vs 2.887@510）。MIT；生产级。https://github.com/google/brotli —— ADV 无 web 资产域，reference。
11. **snappy**——Google framed format，为高吞吐而生；Silesia 2.073@530/1660，被 zstd 同速更率覆盖（同速档 zstd 率高 ~40%）。BSD-3；生产级。https://github.com/google/snappy —— 不吸收。
12. **bzip2**——BWT 块排序，率高（xz 之下）但编解都慢（数十 MB/s 级）；解码慢这点就否决读路径。bzip2 许可（BSD 系）；成熟但演化停滞。https://sourceware.org/bzip2 —— 不吸收。
13. **lzma-rs / lzma-rust（纯 Rust LZMA）**——xz/7z 高率低速率对照项；纯 Rust 许可友好（lzma-rs: MIT OR Apache-2.0），仅当冷归档要压过 zstd L19 时复评（编码慢一个量级）。https://github.com/gendx/lzma-rs —— watch。
14. **zram（内核 RAM 压缩层）**——思想参照：内存页 lzo/lz4/zstd 后端压缩成"压缩内存盘"，把 RAM 变大 2–3×；ADV 无内核态，可借的是"内存表满 → 先压再落盘"的层级思想。GPL-2（内核）；生产级。https://docs.kernel.org/admin-guide/blockdev/zram.html —— reference。
15. **NTFS 压缩（LZNT1）**——16-cluster 压缩单元（4KB cluster ⇒ 64KB），弱 LZ77 变体、写入 CPU 税、压缩单元级碎片化、cluster >4KB 直接不支持（Windows Internals / Profolus 2023-11 锚）。**ADV 数据目录禁用**，自管显式压缩可控档位与分块。https://learn.microsoft.com（compact/fsutil 文档）—— 不吸收（负结论有价值）。
16. **CompactOS（XPRESS4K/8K/16K/LZX）**——Win10+ 对可执行文件的现代 OS 级压缩，4KB 对齐、免 LZNT1 碎片；思想=对"只读代码"压、对热数据不压。仅可作为开发者机器省盘手段，与 ADV 产品逻辑无关。https://learn.microsoft.com —— reference。
17. **透明压缩 vs 显式档（应用模式）**——FS 层透明压缩（NTFS/btrfs/zram）不可控档位/分块、隐藏 CPU 成本、跨平台不一致；app 层显式（ADV 选择）每对象带 algo ID+档位，可迁移可跳过（=90% 判据）。restic/git 均为 app 层实证。—— 显式吸收，透明不吸收。

---

## 边界与未决

- ruzstd 的 2–5× 减速与"无字典帧"限制是**社区口径**（搜索综合，无权威基准页）；若走降级路径必须先复测（⑤ 口径）。
- 字典样本量（≥100、几百到几千）是社区经验值，非官方硬数；ADV 定稿以 held-out 对比为判据。
- 高档位（L19–22）的绝对速度数字未取锚（README 只给 -1 到 fast 段的表）；⑤ 实测时补。
- zstd `--train` 在 ADV 真实 JSONL 上的增益幅度未测（官方只锚了 github-users 样例"显著"），落地前必须出数。
