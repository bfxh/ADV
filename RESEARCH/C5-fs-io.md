# C5 文件系统与 IO 加速（fs-io）深挖

- 日期：2026-10-03；资料优先 2025–2026，机制性内容以 MS Learn / std 文档 / crate 源码为锚。
- 平台侧重：Windows 主档（NTFS + Win32），Linux 档覆盖 CI（EXT4/inotify/io_uring 对照）。
- 与 wave-0 的关系：`07-performance-localfirst.md` 已给初步结论（memmap2=有界大文件档、异步文件 IO 不进扫描内核、IO 按大小分档先量后改）；本文件将其落到「文件系统与 IO」域并给判据与实验设计。
- 档位口径：吸收 / 有界 / 不吸收 / watch / reference。许可证/成熟度：实验 / 维护中 / 弃维护（+OS API 视为稳定）。

---

## 0. 五个重点问题直答

### Q① Windows 增量扫描的最优数据源

**结论：双轨制。** 默认档 mtime+size（跨平台、零特权），elevated 档 USN Journal（需管理员 + NTFS）。MFT 直读（FSCTL_ENUM_USN_DATA）不是增量通道，是「全量快照通道」，与 USN 配套构成 Everything 范式。

| 数据源 | 通道 | 前置条件 | 覆盖 | 坑 |
|---|---|---|---|---|
| 全量遍历（ignore::WalkParallel + FindFirstFileEx） | 全量 | 无 | NTFS/FAT/网络盘 | 每次都是全量；大仓秒级～十秒级 |
| mtime+size 快照 | 增量判定 | 无 | 全平台 | mtime 会被拷贝/checkout 保留或回写；FAT 粒度 2s；NTFS 100ns 但「同秒多写」存在 |
| USN Journal（FSCTL_READ_USN_JOURNAL） | 增量事件 | 管理员 + NTFS（卷句柄 GENERIC_READ） | NTFS 卷内全部文件 | journal 被禁用/清空/溢出（USN_REASON_JOURNAL_PURGE）→ 回落全扫；记录只有 FRN+parent FRN+名字，需 FRN→路径映射 |
| MFT 直读（FSCTL_ENUM_USN_DATA / MFT_ENUM_DATA） | 全量快照 | 管理员 + NTFS | NTFS 卷内全部文件 | 卷级而非仓级；输出 FRN 树需自建路径；只读名与元数据，读内容仍要走正常文件 IO |

判据（按序判定）：
1. 卷非 NTFS 或无管理员 → mtime+size。
2. 单仓文件 <10 万且全扫 P95 在秒级 → mtime+size 够用（先量后改，阈值以 ADV 基准实测冻结）。
3. adv-index 常驻服务 + 大仓/多仓 → USN：初始全扫建 FRN 映射，之后按 NextUSN 读增量，按 parent-FRN 链过滤到仓内路径。
4. rename 在 USN 里是 OLD_NAME + NEW_NAME 两条记录；以 USN_REASON_CLOSE 为提交点判定「稳定事件」。

实现路径（Rust）：`ntfs-reader`（$MFT 内存扫描 + USN reader，MIT OR Apache-2.0，kikijiki/ntfs-reader）与 `usn-journal-rs` 已被 hyperdu-core（2025）在生产组合使用（lib.rs 依赖清单锚）；不想引库可直接 `windows` crate 的 `DeviceIoControl` 三个控制码（FSCTL_QUERY_USN_JOURNAL / FSCTL_ENUM_USN_DATA / FSCTL_READ_USN_JOURNAL，USN_JOURNAL_DATA_V2，MS Learn 锚）。

### Q② 大仓库遍历的并行度模型

**枚举 vs IO 的定位随平台变：**
- Windows/NTFS：`FindFirstFileEx` 的 WIN32_FIND_DATA（以及 FileIdExtdDirectoryInfo）**枚举内联带回 size/时间/属性 → 没有二次 stat**；NTFS 目录是 B-tree 索引，单目录枚举近顺序读。瓶颈通常在单线程内核调用路径 + 用户态过滤（CPU 侧），所以**按子树并行分治收益大**（fd/rg 的 ignore::WalkParallel work-stealing，见 C1-001/C1-002 登记）。
- Linux/EXT4：`getdents64` 只回 (ino, name)，**每文件另付一次 statx**；HDD 上并行 stat 让 NCQ 重排（收益大），SSD 上是 syscall/CPU 开销 → Linux 档并行 stat 是大头，度上限出现更晚。
- 网络 FS（SMB/NFS）：RTT 主导，低并发 + 大缓冲（FIND_FIRST_EX_LARGE_FETCH）最优，高并发恶化。

模型：并发度 = min(2×物理核, 32) 起步，再按两个信号调：枚举线程 CPU 打满且磁盘 util 低 → CPU 瓶颈（加线程无效，减过滤开销）；磁盘 util 高 → IO 瓶颈（仅 SSD/NVMe 提并发有效）。jwalk README 自锚：排序输出场景约 4× walkdir；Rust 论坛 2019-02 基准贴：ignore 与 jwalk 并行版均显著快于 walkdir（单点旧数据，只作量级参考）。MFT 直读路线则整体绕开树形枚举，变成 O(文件数) 顺序流。

### Q③ mmap 分档最终结论表

依据：wave-0 07 已判「有界（大文件档）」+「小文件 mmap 反而慢（ripgrep 实测结论）」；rg 14.0 默认不再用 mmap（CHANGELOG 锚：BUG #922 mmap 健壮性修复，14.0 起默认走 read，15.x 未回调默认），动机是网络盘/冷页缓存/映像失败（SIGBUS、EXCEPTION_IN_PAGE_ERROR）回归。

| 档 | 文件大小 | 策略 | 触发条件 | 反例（禁用） |
|---|---|---|---|---|
| 小 | <256KB–1MB | 普通 read + BufReader(64KB) | 默认 | mmap 建立/撤销（VMA+页表+TLB）开销占比高 |
| 中 | 1–16MB | read 为主；仅当同文件会被 ≥2 趟访问（解析+二次检索）才 mmap | 重访计数 >1 | 单趟顺序扫无收益 |
| 大 | >16MB | mmap + 顺序预取（Windows PrefetchVirtualMemory / POSIX madvise SEQUENTIAL） | 只读 + 文件稳定 | 正被写、网络盘、并发截断（SIGBUS/页故障异常） |

阈值数字（1/4/16MB）是 wave-0 07 留的**待测起点**而非结论——第一步动作就是 1MB/4MB/16MB 拐点基准，出拐点数字后冻结（先量后改）。另注意 Windows 大文件映射占 pagefile 备份空间；SQLite 查询侧的 mmap 走 `PRAGMA mmap_size` 而非自己 mmap（见 C5-022）。

### Q④ 原子写/跨卷 rename 的 Windows 完整坑清单

机制底盘：`std::fs::rename` = `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`（std 文档平台行为锚）。

1. **目标被持占 → ERROR_ACCESS_DENIED**：目标被未带 FILE_SHARE_DELETE 的句柄打开（AV/索引器/自身 reader）。佐证：rust-lang issue（2024-04）"fs::rename sometimes fails on Windows due to missing FILE_SHARE_DELETE"；LLVM D13647 在 ERROR_ACCESS_DENIED 上重试循环；旧仓 MSVC rename 教训同源。修法：小步重试退避（如 1ms×2^k×10 次带抖动），或先试 `ReplaceFileW`（专为替换设计，保留 ACL/属性）。
2. **跨卷 → ERROR_NOT_SAME_DEVICE**：temp 必须与目标**同目录**；用 %TEMP% 在跨卷场景直接失败（tempfile persist 同理）。
3. **目录 fsync 缺等价物**：POSIX 惯例 fsync(dirfd)；Windows 需打开目录句柄（FILE_FLAG_BACKUP_SEMANTICS）+ FlushFileBuffers（SQLite winSync 同法）。
4. **REPLACE_EXISTING 遇只读属性/ACL 拒绝**：先清只读属性或走 ReplaceFileW。
5. **观察者视角非原子**：RDCW 可先于 MoveFileEx 返回报告 rename（MS DevBlogs 2021-10 "Renaming a file is a multi-step process"）→ watch 域不得假设事件时序与写入完成一致。
6. **残留 temp**：杀进程/断电后 .tmp 残留 → 启动清扫（名字带 pid+nonce，前缀固定如 `.adv-tmp-`）；Windows 上 O_TEMPORARY 文件被句柄占住时删不掉。
7. **大小写/短名冲突**：temp 名避免与现有文件仅大小写不同（NTFS 不敏感）。
8. **POSIX 语义 rename（FileRenameInfoEx POSIX_SEMANTICS，Win10 1703+）**：可替换「以 POSIX 语义打开」的目标，std 未采用 → watch。
9. 佐证链仍在活跃：GCC 122726（2025-11）std::filesystem::rename 的 MoveFileExW 错误码处理 bug → 平台坑是持续被主流工具链修的活题。

**ADV 原子写规范（6 条）**：①temp 与目标同目录；②写完 FlushFileBuffers 再 rename；③rename 失败按坑 1 重试退避，仍败回落 ReplaceFileW；④（需要崩溃一致性的场景）目标目录 flush（坑 3）；⑤失败清理 temp，启动时清扫 `.adv-tmp-*`；⑥不承诺跨卷，跨卷需求显式走「copy+delete+标记非原子」路径并留日志。

### Q⑤ notify 后端矩阵对 ADV 索引热更新的适用性

| 平台 | 后端 | 递归 | 缓冲/上限 | 坑 | ADV 适用 |
|---|---|---|---|---|---|
| Windows | ReadDirectoryChangesW | 原生（bWatchSubtree） | notify 用 64KB 读缓冲（v5/v6 源码常量，随版本核） | 缓冲溢出→需全量重扫；目录删除后 watch 失效；rename=OLD+NEW 两事件；事件早于 rename 返回（Q④坑5） | **主档**：溢出→触发一次增量校验（走 Q① 通道） |
| Linux | inotify（inotify crate） | 自行逐目录 add_watch | max_user_watches（发行版常见 8k–512k） | walk 与 add 之间有窗口；IN_Q_OVERFLOW；move 靠 cookie 配对 | CI/Linux 档够用 |
| macOS | FSEvents | 原生 | 需要 runloop 线程 | 有 latency 合并窗口，精确性低于 inotify；文件级需 FileEvents flag | 非 ADV 重点 |
| BSD | kqueue | 逐目录 | 每目录占一个 fd | 大树 fd 耗尽 | CI 档 |
| 全平台 | PollWatcher | 轮询 | interval 可配 | 延迟与轮询开销 | 网络盘兜底 |

结论：notify（v6，维护中）直接吸收作 watch 域主库；ADV 场景是「索引失效标记」而非逐事件响应 → 100–300ms debounce + 事件分类（create/modify/rename→单文件失效，remove/溢出→目录级失效）+ 溢出回落增量扫描。watchexec 的 debounce+gitignore 过滤管线抄机制不引依赖。

---

## 1. 遍历域

### 1.1 walkdir（crate）
- 定位：跨平台串行遍历基线，迭代器式、深度优先、流式。
- 可抄机制：①`follow_links(true)` 自带 device+inode 循环检测（Windows 用 file index）；②`sort_by` 可选，代价是缓冲整层；③min_depth/max_depth 剪枝。
- 档位：**有界**（小目录/串行回退路径/并行版的对照基准）。
- 第一步动作：作为 WalkParallel 的单线程对照进遍历基准组。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（低频稳定，Simon Sapin/ripgrep 系）。
- 链接：https://github.com/rust-lang/walkdir

### 1.2 ignore（crate）
- 定位：ripgrep 系遍历内核：`.gitignore`/`.ignore`/全局/排除规则语义 + `WalkBuilder`（并行）与 `WalkParallel`（无序流式回调）。
- 可抄机制：①work-stealing 并行：每线程从共享队列取目录，天然负载均衡（fd/rg 同款）；②ignore 规则匹配在**遍历线程内**完成（过滤不下沉队列，避免脏数据）+ `-!` 排除与隐藏文件开关；③`types` 匹配器复用（按扩展名/语义过滤零成本接入）。
- 档位：**吸收**（扫描遍历主路径）。
- 第一步动作：扫描内核换 WalkParallel，回调里只做「收集路径+内联元数据」，把 ignore 语义默认开（可关）。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（ripgrep 项目背书）。
- 链接：https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore

### 1.3 jwalk（crate）
- 定位：独立并行遍历器，混合 ignore 的并行 + walkdir 的流式迭代器 API，支持排序输出与 rayon 线程池接入。
- 可抄机制：①每目录整批产出（`DirEntry::read_children`），适合「目录级批量入队」的生产者-消费者；②`parallelism(n)` 显式控制度（0=默认全核）——ADV 的并发度模型实验可直接借用此旋钮。
- 档位：**有界**（功能被 ignore::WalkParallel 覆盖；库本身维护停滞，不建议新依赖）。
- 第一步动作：不引依赖；把其「目录批量产出」思路用于自研入队批大小（256 条/批，参照 C1-002 fd 的口径）。
- 许可证/成熟度：MIT；**弃维护**（最后实质发版约 2020，以 crates.io 页为准）。
- 链接：https://github.com/Byron/jwalk

### 1.4 Windows 目录枚举 API（FindFirstFileEx 家族）
- 定位：NTFS 枚举的官方通道；`FindExInfoBasic` + `FIND_FIRST_EX_LARGE_FETCH` 是性能开关。
- 可抄机制：①`FindExInfoBasic` 不回查 8.3 短名，省一趟；②`FIND_FIRST_EX_LARGE_FETCH` 加大内核目录查询缓冲，减少系统调用次数（MS 文档明示性能动机）；③WIN32_FIND_DATA 一次带回属性/size/三类时间 → 枚举即 stat。
- 档位：**吸收**。
- 第一步动作：确认 ignore/walkdir 在 Windows 是否已走 FindFirstFileEx+LARGE_FETCH（std 内部实现决定），否则扫描热点路径自封装。
- 许可证/成熟度：Win32 API（OS 稳定）；MS Learn 文档锚。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexa

### 1.5 GetFileInformationByHandleEx 批量元数据（FileIdExtdDirectoryInfo）
- 定位：在目录句柄上用枚举 class 一次拿 FileId + ReparseTag + 时间戳 + size，替代「枚举后逐个 stat」。
- 可抄机制：①`FileIdExtdDirectoryInfo`/`FileIdExtdDirectoryInformation` 把 file_id 一并带回 → 硬链接去重与 reparse 分类零额外系统调用；②与 `FILE_FLAG_BACKUP_SEMANTICS` 打开的目录句柄配合；③部分 class 有最低 OS 版本约束，落地按目标档验证（MS 文档为准）。
- 档位：**吸收**（Windows 主路径的元数据来源；std 不可达处走 windows crate）。
- 第一步动作：扫描内核「枚举即元数据」改造 + 非 NTFS/异常路径回落逐个 stat。
- 许可证/成熟度：Win32 API。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex

### 1.6 NTFS MFT 直读路线（Everything 范式）
- 定位：卷级全量快照：FSCTL_ENUM_USN_DATA（MFT_ENUM_DATA）按 MFT 序吐出全卷 (FRN, parent FRN, name, 属性)，不经目录树；Everything 官方口径：首索引约 1 秒/12 万文件、约 1 分钟/百万文件（Everything FAQ/介绍页表述，量级参考）。
- 可抄机制：①枚举缓冲循环 + 逐条解析 USN_RECORD（parent FRN 链重建路径树）；②Everything 后续用 USN journal 维护（见 1.7）；③Rust 现成件 `ntfs-reader`（$MFT 内存扫描 + USN reader，处理未文档化记录类型）与 `ntfs` crate（纯 Rust 只读 NTFS 解析器，Colin Finck 系）。
- 档位：**有界**（仅 adv-index 常驻全局服务可选；CLI 单仓扫描不吸收——仓级问题用仓级遍历）。
- 第一步动作：不起码就先不做；若做，先用 ntfs-reader 做原型（全卷索引 + parent 链过滤到 ADV 管辖仓）。
- 许可证/成熟度：ntfs-reader = MIT OR Apache-2.0（crates.io 锚），维护中；ntfs crate = MIT OR Apache-2.0（以 crates.io 为准）；Everything SDK = 专有免费（闭源，仅参考）。
- 链接：https://crates.io/crates/ntfs-reader ; https://github.com/kikijiki/ntfs-reader ; https://www.voidtools.com/

### 1.7 USN Journal 变更追踪
- 定位：NTFS 变更日志（$Extend\$UsnJrnl:$J 环形文件），增量扫描的 Windows 神器（判据与实现路径见 Q①）。
- 可抄机制：①FSCTL_QUERY_USN_JOURNAL 取 NextUSN/当前 journal id → 断点续读；②FSCTL_READ_USN_JOURNAL 按 (journal id, NextUSN) 增量读，Reason 位过滤（CREATE/DELETE/DATA_OVERWRITE/DATA_EXTEND/TRUNCATION/RENAME_*）；③以 USN_REASON_CLOSE 为提交点；④journal 被清空（JOURNAL_PURGE）或 id 不符 → 回落全扫。
- 档位：**吸收**（elevated 档增量主通道；默认档保留 mtime+size 回落）。
- 第一步动作：adv-index 服务原型：初始全扫建 FRN→路径映射 + 定时读增量 + 仓路径过滤。
- 许可证/成熟度：Win32 API（稳定）；`usn-journal-rs`（license 待核，hyperdu-core 生产使用锚）。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_read_usn_journal

### 1.8 inotify（Linux 对照，附 FSEvents/kqueue）
- 定位：Linux 遍历与监听机制基线，用于 CI 档覆盖与并行度模型对照。
- 可抄机制：①getdents64 只回 (ino,name) → statx 批量/并行补元数据（与 Windows「枚举即 stat」构成 Q② 的平台差异）；②inotify 每目录一个 wd，递归监听需自行维护 walk+add（有竞态窗口）；③max_user_watches 与 IN_Q_OVERFLOW 上限回落策略。
- 档位：**吸收**（Linux CI 档的实现约束清单）。
- 第一步动作：CI 基准里加「并行 stat vs 串行 stat」对照组。
- 许可证/成熟度：内核机制（用户态无关）；inotify crate = MIT（以 crates.io 为准），维护中。
- 链接：https://man7.org/linux/man-pages/man7/inotify.7.html

---

## 2. 读写域

### 2.1 memmap2（crate）
- 定位：跨平台 mmap 封装（memmap-rs 续护版）。分档结论表见 Q③。
- 可抄机制：①mmap 真优势在随机访问与零拷贝只读，不在顺序扫（rg 14.0 默认弃 mmap 互证）；②Windows 侧失败模式是 EXCEPTION_IN_PAGE_ERROR 而非 SIGBUS，需按档捕获；③PrefetchVirtualMemory 可对已映射区间显式预热（Win8+，收益待验证——wave-0 锚）。
- 档位：**吸收**（仅大文件档）。
- 第一步动作：1/4/16MB 拐点基准，冻结阈值。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://github.com/RazrFalcon/memmap2

### 2.2 FileExt read_vectored / ReadFileScatter
- 定位：分散读接口；Windows 上 std 的 read_vectored 基本是单段顺序读的退化路径，ReadFileScatter 要求页对齐 + NO_BUFFERING。
- 可抄机制：机制本身是「一次 syscall 填多个缓冲」，仅在「读即分块消费」管线有省 syscall 意义。
- 档位：**不吸收**（扫描内核读路径单一，收益不出现在本域瓶颈上）。
- 第一步动作：无；保留 reference。
- 许可证/成熟度：std 内建。
- 链接：https://doc.rust-lang.org/std/os/fd/trait.ReadVectored.html

### 2.3 BufReader 缓冲预算
- 定位：缓冲读是「小文件直读」档的默认形态；默认 8KB 对哈希/扫描偏小。
- 可抄机制：①顺序哈希用 64KB–1MB 缓冲换 syscall 次数；②过大缓冲污染 CPU 缓存与页缓存——预算值随「读后消费模式」（逐块 hash vs 全文解析）变化；③`BufReader::with_capacity` 显式化，禁裸默认值进热点。
- 档位：**吸收**。
- 第一步动作：遍历基准中加 8K/64K/256K/1M 缓冲对照（perf_lock 下测量）。
- 许可证/成熟度：std/pattern。
- 链接：https://doc.rust-lang.org/std/io/struct.BufReader.html

### 2.4 预读提示（posix_fadvise / FILE_FLAG_SEQUENTIAL_SCAN / PrefetchVirtualMemory）
- 定位：向内核声明访问模式，换取预读策略。
- 可抄机制：①POSIX：FADV_SEQUENTIAL（加倍预读）/ WILLNEED（异步预热）/ DONTNEED（单遍大扫后还页缓存）；②Windows：FILE_FLAG_SEQUENTIAL_SCAN 在打开时声明（缓存管理器加大预读，MS 文档机制）；PrefetchVirtualMemory 是 WILLNEED 的 mmap 侧近似物；③Windows 无 fadvise 等价 → 「用完还缓存」只能靠 NO_BUFFERING 或接受页缓存压力。
- 档位：**有界**（哈希大文件路径吸收 SEQUENTIAL_SCAN；fadvise 仅 Linux 档）。
- 第一步动作：哈希路径开 SEQUENTIAL_SCAN 的 A/B 基准（ETW 看队列深度与页故障，wave-0 07 的 wpr 脚本可复用）。
- 许可证/成熟度：POSIX 规范 / Win32 API。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea （flag 说明）

### 2.5 O_DIRECT 类（FILE_FLAG_NO_BUFFERING）
- 定位：绕过页缓存直读；约束：缓冲/偏移/长度按扇区对齐。
- 可抄机制：唯一高价值用例 = 单遍海量哈希不冲刷页缓存（避免把热数据挤出）；反例 = 小文件反而慢（无缓存命中可吃）+ 对齐管理复杂。
- 档位：**watch**（仅当基准证明哈希路径对页缓存是负外部性再启用）。
- 第一步动作：无（挂起）；在基准档案里预留「页缓存驱逐量」指标。
- 许可证/成熟度：Win32/POSIX API。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering

### 2.6 compio（completion-based 运行时）
- 定位：IOCP/io_uring/polling 三后端的 thread-per-core 完成型 IO（OpenDAL 已采用；2025 活跃——wave-0 07 锚）。
- 可抄机制：完成型 API 把缓冲所有权交给内核 → 天然配零拷贝管线；但 wave-0 判断已成立：异步文件 IO 解决「海量并发阻塞等待」，扫描器真实瓶颈在元数据枚举+CPU，阻塞小线程池即可打满磁盘带宽；异步只增调度复杂度不增带宽。
- 档位：**watch**（唯一待翻身场景：NVMe 深队列(32+) + 4KB 随机海量小文件）。
- 第一步动作：对照实验设计（定案后跑）：固定线程池阻塞 read vs compio IOCP，同 NVMe、10 万小文件，测墙钟/CPU/队列深度，出拐点再议。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（2025 活跃）。
- 链接：https://github.com/compio-rs/compio

### 2.7 tokio-uring
- 定位：tokio 系 io_uring 绑定；**仅 Linux**（Windows 无后端）且基本沉寂（wave-0 07 锚：2022–2023 后无实质活动，社区视 compio 为继承者）。
- 可抄机制：无新增（io_uring 机制认知已在 2.6 覆盖）。
- 档位：**不吸收**（主平台 Windows 不覆盖 + 维护状态）。
- 第一步动作：无。
- 许可证/成熟度：MIT；弃维护（实质停滞，以仓库动态为准）。
- 链接：https://github.com/tokio-rs/tokio-uring

---

## 3. stat/元数据域

### 3.1 硬链接 / 符号链接 / junction 处理
- 定位：扫描语义正确性与去重收益的交叉点。
- 可抄机制：①FileId 去重：BY_HANDLE_FILE_INFORMATION 的 nFileIndex 或 GetFileInformationByHandleEx(FileIdInfo) 识别同 inode（Windows 上 hard link 共享 FileId）→ 哈希缓存按 FileId 键控可省重复哈希；②reparse point 三分类：symlink（读不需特权、建需开发者模式/特权）、junction（目录、免特权）、云占位符（OneDrive 等）→ 默认不跟随 reparse，白名单显式开启防环与防拉取海量云端数据；③循环防护：路径栈深度 + FileId 访问集。
- 档位：**吸收**。
- 第一步动作：扫描内核加 reparse 分类开关与 FileId 缓存键（默认关闭跟随）。
- 许可证/成熟度：Win32/POSIX API。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points

### 3.2 Windows 大小写不敏感与路径规范化
- 定位：索引键正确性。NTFS 默认大小写保留+不敏感（$UpCase 表），同仓不可能有两个仅大小写不同的文件。
- 可抄机制：①查重/合并键必须大小写折叠——用 Unicode 大小写折叠而非 ASCII（NTFS $UpCase 是全 Unicode 大写表），且在 OsStr/UTF-16 层做，避免 to_lowercase 的locale 陷阱；②`Path::canonicalize()` 返回 `\\?\` 前缀 + 解析后大小写 → 用 dunce 规避前缀（多数工具链同法）；③8.3 短名（PROGRA~1）可能出现在枚举结果里（FindExInfoBasic 可关）→ 索引键用长名。
- 档位：**吸收**（adv-index 键规范化规范）。
- 第一步动作：索引键统一「dunce 化 + Unicode 大小写折叠」写入 ADV 索引规范。
- 许可证/成熟度：std + dunce = MIT；维护中。
- 链接：https://github.com/mvdnes/... (dunce: https://crates.io/crates/dunce)

### 3.3 批量 stat 策略
- 定位：把「每文件一次 stat」压成 0（Windows）或并行化（Linux）。
- 可抄机制：①Windows：枚举内联元数据（1.4/1.5）→ stat 趋零；②Linux：并行 statx + 目录级批量（getdents 后分组 stat）；③跨平台兜底：仅对「size 或时间存疑」的文件补 stat（与 3.4 三档缓存联动）。
- 档位：**吸收**。
- 第一步动作：Linux CI 基准加并行 stat 对照（Q② 信号表联动）。
- 许可证/成熟度：OS API。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ne-minwinbase-file_info_by_handle_class

---

## 4. 变更监听域

### 4.1 notify（crate）
- 定位：跨平台监听事实标准；后端矩阵与 ADV 适用性见 Q⑤。
- 可抄机制：①后端选择按平台特性 flag 编译期定；②`notify-debouncer-*`（官方去抖器，Debian 打包佐证生态地位）提供事件合并与时间窗；③Windows 后端 64KB 缓冲溢出时 notify 会报错/要求重扫 → 必须接「溢出→增量校验」回调而不是丢弃。
- 档位：**吸收**。
- 第一步动作：adv-index 热更新管线：notify → debounce(200ms) → 事件分类 → 失效标记 → USN/mtime 增量校验。
- 许可证/成熟度：MIT（以 crates.io 为准）；维护中（v6；生态 fork（rolldown-notify 等）提示有人要定制性能，属正常分叉信号）。
- 链接：https://github.com/notify-rs/notify

### 4.2 watchexec
- 定位：基于 notify 的工程化 CLI/库：去抖、过滤（ignore 语义）、进程派发。
- 可抄机制：①事件去抖 + 「project origin」归并（多根工作区）；②过滤管线与遍历过滤复用同一套 ignore 语义（与 C5-002 联动）；③失败重试与监听重建（目录被删后 watch 失效的工程处理）。
- 档位：**有界**（抄机制不引依赖——ADV 自建 watch 管线）。
- 第一步动作：读其 debouncer 参数默认值，作为 ADV debounce 常量的对照锚。
- 许可证/成熟度：Apache-2.0；维护中（2025 活跃）。
- 链接：https://github.com/watchexec/watchexec

### 4.3 文件哈希缓存三档（mtime vs size vs hash）
- 定位：增量扫描的跨平台兜底数据源 + 索引陈旧度防线。
- 可抄机制：三档判据——
  - L1 stat 快照：(size, mtime) 不变 → 跳过（NTFS 100ns；FAT 2s 粒度；拷贝/checkout 可保留旧 mtime → 有漏检风险，条件限定：接受该风险的默认档）。
  - L1.5 身份扩展：加 FileId/硬链接计数（Windows nFileIndex、POSIX ino）→ 捕捉「路径没变内容换了」的硬链接替换类操作。
  - L2 首块校验：内容前 4KB 哈希 + L1 元组 → 大文件快速负判。
  - L3 全量哈希：兜底与首次建库。
  - 失效条件表：mtime 回写/时钟跳变/跨机同步 → 降级到 L2/L3 复检。
- 档位：**吸收**。
- 第一步动作：定三档常量与降级触发条件，进索引 schema（version 字段随判据演进）。
- 许可证/成熟度：pattern（无依赖）。
- 链接：参考 git/fsmonitor、Watchman 的 stat 判据（Facebook Watchman 文档）。

---

## 5. 存储层域（IO 角度，与 X13 只交界）

### 5.1 SQLite IO 特性（WAL / immutable / readonly / mmap_size）
- 定位：扫描缓存与检索库的 IO 形态决定读写并发与崩溃恢复成本。
- 可抄机制：①WAL：读不阻塞写、写不阻塞读（WAL 文档机制）→ 扫描器单写 + MCP 查询多读的理想形态；WAL 下 `synchronous=NORMAL` 是常见平衡点（崩溃只丢最近 checkpoint 后的事务，不损库完整性）；②`mode=ro` + `immutable=1`：声明库永不变化 → 免锁免恢复，查询侧最快路径（适用「每次全扫完成后冻结的快照库」——快照轮换，不适用热库）；③`PRAGMA mmap_size`：让 SQLite 自己管理映射页（Q③ 表外的一个例外：索引查询侧读 SQLite 用它，不自 mmap）；④checkpoint 时机与扫描批次对齐，避免扫描中途写放大。
- 档位：**吸收**。
- 第一步动作：定缓存库形态：热库=WAL+NORMAL；发布快照=immutable+ro；查询侧 mmap_size 开启（数值待基准）。
- 许可证/成熟度：SQLite = Public Domain；rusqlite = MIT OR Apache-2.0，维护中。
- 链接：https://www.sqlite.org/wal.html ; https://www.sqlite.org/pragma.html

### 5.2 tempfile（crate）
- 定位：临时文件工程标准件（NamedTempFile/TempDir，进程退出清理）。
- 可抄机制：①Windows 上临时文件带 FILE_ATTRIBUTE_TEMPORARY（内核倾向驻缓存、延迟落盘，适合短生命周期）；②`persist()`=同卷 rename（跨卷失败，见 5.3 坑2）；③TempDir 的清理在 panic/正常退出路径触发，Windows 上句柄未闭时删除会失败（ERROR_SHARING_VIOLATION 系）→ ADV 用启动清扫兜底而非依赖库清理。
- 档位：**吸收**。
- 第一步动作：原子写工具函数统一基于 tempfile（同目录 NamedTempFile），禁直接 %TEMP%。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://github.com/Stebalien/tempfile

### 5.3 原子写规范（write-temp-then-rename 的 Windows 完整坑）
- 定位：坑清单与 ADV 规范见 Q④（9 坑 + 6 条规范），此处不重复。
- 可抄机制（外部实现参照）：①SQLite 的 winWin32Open/ winSync：目录句柄 flush 先例；②LLVM D13647：ERROR_ACCESS_DENIED 重试循环先例；③ReplaceFileW：保留目标 ACL/属性的替换原语（MS 文档明示用于「替换」场景）。
- 档位：**吸收**（作为 ADV 写路径硬规范）。
- 第一步动作：规范落成 `adv-fs::atomic_write` 单一入口 + 单测覆盖坑 1/2/6/7。
- 许可证/成熟度：pattern；先例均为维护中的知名项目。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilea

---

## 6. 2025–2026 前沿信号

1. **ntfs-reader + usn-journal-rs 组合在生产工具链落地**：hyperdu-core（2025，lib.rs 依赖锚）用二者做 Windows 全量文件枚举/去重内核 → 「MFT 快照 + USN 增量」范式在 Rust 生态已有可抄实现，且 ntfs-reader 处理未文档化 MFT 记录类型的坑已被踩过。
2. **rg 14.0 默认弃 mmap**（CHANGELOG 锚，15.x 维持）→ 与 wave-0 07「mmap 有界、小文件反例」互证；下游工具（缓存扫描类）显式加 --no-mmap 的案例出现 → mmap 分档已从「性能优化」变成「失败模式管理」。
3. **rename 平台坑仍是活跃修复区**：GCC 122726（2025-11）、rust-lang rename/FILE_SHARE_DELETE issue（2024-04）→ 原子写规范需要「错误码→重试/回退」表，不能只写 happy path。
4. **notify 生态分叉**：rolldown-notify、martensite-notify 等 fork 出现（crates.io 锚）→ 高吞吐场景对 notify 缓冲/事件粒度有定制需求；ADV 若遇瓶颈，fork 路径已有人趟过。
5. **scankit（crates.io 2026-04）**：walk + watch + filter 一体化目录树扫描器，定位桌面应用共享扫描件 → 「遍历与监听共用过滤语义」的方向被独立验证（机制细节未核实，watch 不入登记表）。
6. **compio 2025 持续活跃**（OpenDAL 采纳，wave-0 锚）+ Windows 侧 IOCP 后端成熟 → Q②/2.6 的对照实验有了低成本执行路径（同一 crate 切后端即可对比）。

## 7. Top-3

1. **扫描主路径**：`ignore::WalkParallel` + Windows `FindFirstFileEx(FindExInfoBasic | FIND_FIRST_EX_LARGE_FETCH)` + 枚举内联元数据（免 stat）+ reparse 不跟随；并行度按 Q② 模型起步并进基准（预期收益最大：天花板常在串行段，遍历是第一个串行段）。
2. **增量双轨**：默认档 mtime+size(+FileId) 三档缓存；elevated 档 USN Journal（NextUSN 断点 + CLOSE 提交点 + 仓路径过滤），MFT 直读仅 adv-index 常驻服务可选项；任何通道溢出/失效都回落全扫，保证「陈旧可检测」。
3. **两项硬规范先冻结**：原子写 9 坑/6 规范（Q④，含旧仓 MSVC 教训的通用化）+ mmap 分档表（Q③，阈值留基准待测）。二者成本低、错一遍代价高（数据损坏/崩溃），先于性能优化落地。

## 8. 对照实验清单（留给 wave-1）

| 编号 | 问题 | 设计 | 判据 |
|---|---|---|---|
| E1 | mmap 分档拐点 | 1/4/16/64MB 桶 × read/mmap × 顺序/随机 | 墙钟 + 页故障/秒（ETW），拐点冻结阈值 |
| E2 | 并行遍历度 | WalkParallel 并发 1/2/4/8/16/32 × NTFS SSD/HDD/网络盘 | 墙钟 + CPU util + 磁盘 util（Q② 信号表） |
| E3 | USN vs mtime 增量 | 10 万文件仓，扰动 1%/5%/20% 后增量扫描 | 墙钟 + 漏检数（对照真值集） |
| E4 | 异步文件 IO 是否翻身 | 阻塞线程池 vs compio IOCP，NVMe 深队列海量小文件 | 墙钟/CPU/队列深度；无显著差 → 维持不吸收 |
| E5 | 缓冲预算 | 8K/64K/256K/1M BufReader × 哈希/解析 | 吞吐 + CPU 缓存未命中率 |
