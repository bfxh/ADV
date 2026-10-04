# X13 存储引擎与 SQLite 工程（adv-index 域）

> 调研日期 2026-10-04。服务对象：ADV（Rust 本地优先代码安全/质量平台）的 adv-index——E4 已定 SQLite 全家桶（index+report 同库、cache 独立文件、FTS5、rusqlite），本域盘深"SQLite 工程怎么用对"与"内容寻址缓存怎么建"。
> 锚声明：SQLite 侧数字均给官方页锚（pragma.html / wal.html / limits.html / fasterthanfs.html / lang_VACUUM 等）；分块参数给 restic/borg/casync 官方文档或作者原文锚；给不出锚的数字一律不写。"默认值"均指 2026-10 口径下官方文档所载默认，版本升级需复核。

## Top-3

1. **X13-07 pragma 起步档 + "先量后改"基准设计**——ADV 库的默认配置直接定生死：WAL + synchronous=NORMAL + busy_timeout + cache_size 起步档，配合固定语料/固定指标/≥5 遍中位数的基准流程；默认档冻结，Δ<10% 不改。
2. **X13-15 大 blob 判据（100KB 线 + 溢出页机制）**——官方 fasterthanfs 口径给了可执行阈值：代码片段（远小于 100KB）进 BLOB，>100KB 走文件+句柄；机制是溢出页链，不是拍脑袋。
3. **X13-11 refinery 迁移规范**——Flyway 式 checksum 版本表 + embed_migrations! 编译期嵌入，配 ADV"只进不退、事务内 DDL、dirty 即停"三条规范，把 schema 演化变成门禁而非约定。

## 五问速答

1. **pragma 起步配置**：`journal_mode=WAL`（持久，建库时设一次）→ `synchronous=NORMAL`（WAL 下 fsync 只在 checkpoint，掉电最多丢最近已提交事务、库不损，wal.html 锚）→ `busy_timeout=5000` → `cache_size=-8000`（8MB，默认 -2000≈2MB 偏小）→ `mmap_size=0` 起步 → `page_size=4096` 保持默认（建库前才可定）→ `temp_store=MEMORY`。基准设计见 X13-07。
2. **迁移工具与规范**：选 refinery（checksum 版本表 + 编译期嵌入，MIT）；规范三条：只进不退（回滚=新迁移）、每个迁移事务内执行（SQLite DDL 可事务化，与 PG 不同）、checksum 不匹配或 dirty 即停人工介入。见 X13-11/12。
3. **大 blob 判据**：BLOB ≤100KB 进库（官方 fasterthanfs：~35% 快于文件系统、省 ~20% 空间）；>100KB 落文件、库里存 sha256+大小+引用列；代码片段典型几百字节~几 KB，进库无悬念。机制=溢出页链。见 X13-15。
4. **Git/restic 机制借鉴**：两级存储（新内容 loose 单文件、定量后后台 pack：tar+zstd 归档，**不做** delta 编码——实现贵收益薄）；restic 的 Rabin CDC 只在"语料库存历史快照"场景启用，先测版本间相似度再决定。见 X13-22~27。
5. **integrity 运维档**：quick_check（跳索引交叉校验）随启动/升级前自动跑；integrity_check（全库 O(库大小)）只在版本升级、崩溃恢复后、手动触发；无官方增量校验；修复走 .recover 或 VACUUM INTO 备份回填。见 X13-10。

---

## A. SQLite 工程域

### X13-01 rusqlite 绑定与连接模型
- **定位**：Rust 侧 SQLite 绑定事实标准，E4 已定。
- **可抄机制**：① `Connection` 不实现 `Sync`——每个连接只许单线程持有，多线程读写天然要"每线程连接"或池，编译期就挡住数据竞争；② `prepare_cached` 内置 LRU 语句缓存（默认容量 16，`set_prepared_statement_cache_capacity` 可调，docs.rs 锚），重复执行同一 SQL 不重复 prepare；③ 语句生命周期借连接，不能把 Statement 存到连接之外。
- **档位**：吸收。
- **第一步动作**：adv-index 建库入口封装"open → set pragma → migrate"三步，禁散装 open。
- **许可证/成熟度**：MIT / 维护中（crates.io 高下载量，长期活跃）。
- **链接**：https://docs.rs/rusqlite

### X13-02 rusqlite 连接池（deadpool-sqlite / r2d2_sqlite）
- **定位**：把网络 DB 的池语义搬到本地单文件库，常被误用。
- **可抄机制**：① SQLite 是"单写多读"，池解决的是连接复用而非并发写——写并发被引擎本身串行化；② deadpool-sqlite 为 async 场景在 tokio `spawn_blocking` 上跑阻塞调用；③ 每个池连接各持独立语句缓存，池 build 时经 hook 统一设容量与 pragma。
- **档位**：有界——ADV 扫描写路径建议"单写连接 + 读连接按线程"，池只在多读查询场景引入。
- **第一步动作**：先用 1 写 + N 读的手工连接布局，读并发成为瓶颈再上 deadpool-sqlite。
- **许可证/成熟度**：deadpool 系 MIT/Apache-2.0、r2d2_sqlite MIT / 维护中。
- **链接**：https://docs.rs/deadpool-sqlite

### X13-03 bundled 编译与 FTS5/体积裁剪
- **定位**：libsqlite3-sys 的 bundled feature 静态编译 amalgamation（sqlite3.c 单文件），交叉编译/分发不再依赖系统 libsqlite3。
- **可抄机制**：① `rusqlite/bundled` = 静态编一份私有 SQLite，版本锁定可复现；② FTS5 由 feature 开关（背后是 `SQLITE_ENABLE_FTS5` 编译宏）启用；③ amalgamation 是全功能编译，进一步裁体积要走 `SQLITE_OMIT_*` 系宏自编 sqlite3.c——rusqlite feature 不暴露全部裁剪面，收益需对着二进制实测。
- **档位**：吸收（bundled+FTS5 定案）；`SQLITE_OMIT_*` 手工裁剪记 reference，等体积成为门再动。
- **第一步动作**：Cargo feature 固定 `bundled` + FTS5，记录启用前后二进制体积增量作基线。
- **许可证/成熟度**：rusqlite/libsqlite3-sys MIT；SQLite 本体 public domain / 维护中。
- **链接**：https://github.com/rusqlite/rusqlite（bundled 说明）

### X13-04 WAL 并发读写模型【机制级】
- **定位**：adv-index 扫描写 + 查询读并存，WAL 是前提档。
- **可抄机制**：① WAL 下读者读 `-wal`+主库快照、写者追加 `-wal`，**读写互不阻塞**，写者全局单串行；② `journal_mode=WAL` 持久化（库文件内），建库时设一次即可；③ `-shm` 共享内存索引意味着 WAL 不适用于网络文件系统——ADV 本地优先，天然匹配；④ `synchronous=NORMAL` 在 WAL 下合法：fsync 只发生在 checkpoint，掉电可能回滚最近已提交事务但库不损（wal.html 明文）。
- **档位**：吸收。
- **第一步动作**：建库入口第一条 pragma 设 WAL，文档写明"掉电可能丢最近一批提交"作为可接受档。
- **许可证/成熟度**：public domain（SQLite 3.7.0 起，2010）/ 维护中。
- **链接**：https://www.sqlite.org/wal.html

### X13-05 WAL checkpoint 与 journal_size_limit【机制级】
- **定位**：WAL 文件无限增长的唯一闸门。
- **可抄机制**：① 自动 checkpoint 触发线 `wal_autocheckpoint` 默认 1000 页（4KB 页≈4MB，pragma.html 锚），每提交后检查；② 模式 PASSIVE（默认，不阻塞读写）/ FULL / RESTART / TRUNCATE（清空 WAL 文件回收磁盘）；③ checkpoint 跑不完（有读者占着旧快照）时 WAL 继续增长——长读事务是 WAL 膨胀首因；④ `journal_size_limit` 管的是 checkpoint 后文件收缩上限（默认 -1 不收缩）。
- **档位**：吸收；主动 TRUNCATE 档先不启，观察 WAL 峰值再说。
- **第一步动作**：监控项加"WAL 文件当前字节数"，超阈值记录并告警，先量后定 journal_size_limit（候选 64MB）。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/pragma.html#pragma_wal_autocheckpoint

### X13-06 busy_timeout 与 BEGIN IMMEDIATE 锁纪律【机制级】
- **定位**：SQLITE_BUSY 是 SQLite 并发写的第一坑。
- **可抄机制**：① `busy_timeout` 默认 0（pragma.html 锚）——撞锁立刻报错，设 5000ms 起步让写者排队而非失败；② 事务默认 DEFERRED：读时拿读锁、后转写锁，两连接同时"先读后写"会撞死锁式的 SQLITE_BUSY 升级失败——**写事务一律 `BEGIN IMMEDIATE`**（进事务就取写权，排队在 mutex 上，语义干净）；③ WAL 下写者间冲突点集中到 append 锁，busy_timeout 是唯一让步通道。
- **档位**：吸收。
- **第一步动作**：adv-index 写路径封装 `with_write_tx()`，内部强制 IMMEDIATE + 重试预算，调用点写不出裸 BEGIN。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/lang_transaction.html

### X13-07 pragma 起步档与"先量后改"基准设计【机制级】★Top
- **定位**：ADV 库的出厂配置，每个值都要给得出理由。
- **起步档**（附理由）：
  - `journal_mode=WAL`：读写不阻塞（X13-04）。
  - `synchronous=NORMAL`：WAL 下 checkpoint 才 fsync；FULL 的增量收益对本地工具链通常不值那倍写延迟——但这是待测项不是结论。
  - `busy_timeout=5000`：默认 0 太脆（X13-06）。
  - `cache_size=-8000`（8MB，负数=KiB）：默认 -2000≈2MB（pragma.html 锚）对 FTS5 索引构建/回表偏小；页缓存在写密集扫描上直接决定 B 树拆页的随机 IO 次数。
  - `mmap_size=0` 起步：官方默认多数构建为 0，mmap 读的收益平台相关、且 I/O 错误会以 SIGBUS 形式打崩进程（pragma.html 风险条目）；语料只读段稳定后再试 256MB 对比。
  - `page_size=4096` 保持默认：**建库后不可改**（改需 VACUUM 重建），FTS5 大表场景可测 8192，必须重建库量。
  - `temp_store=MEMORY`：排序/临时表落内存，配合本地大内存档。
- **基准设计（先量后改）**：① 固定语料（ADV 指定仓 commit 锁定）+ 固定扫描输入；② 三类指标：全量扫描入库耗时 p50/p95、查询 p95（路径查 / FTS 查 / JSON 元数据查）、库+WAL 峰值字节数；③ 默认档先跑 5 遍取中位数冻结为基线；④ 逐项 pragma 切换（一次只动一个），每档 5 遍中位数对比；⑤ 判据：Δ<10% 不采纳改动；每档结果落档（配置×日期×数值），否证记录是时间戳不是永久事实。
- **档位**：吸收。
- **第一步动作**：写 `scripts/bench-sqlite` 基准脚本 + 冻结默认档清单，CI 之外手动触发。
- **许可证/成熟度**：public domain（SQLite）/ 维护中。
- **链接**：https://www.sqlite.org/pragma.html

### X13-08 在线备份：VACUUM INTO / backup API【机制级】
- **定位**：adv-index 库的快照与恢复底座。
- **可抄机制**：① `VACUUM INTO 'file'`（3.27.0+，lang_VACUUM 锚）：对活库跑出一致快照到新文件，目标库不含空闲页（常比源库小），代价=一次全库顺序读；② backup API（rusqlite `backup` feature）：分页步进复制，每步可让出，适合大库不卡交互；③ SQLite **无内建增量备份**，增量=复制 WAL（Litestream 路线，X13-09）。
- **档位**：吸收——VACUUM INTO 做"报告定稿快照 + 周期备份"，backup API 暂不引入。
- **第一步动作**：report 导出路径接 `VACUUM INTO`，保留最近 N 份轮转。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/lang_vacuum.html

### X13-09 Litestream（WAL 流复制）
- **定位**：读 WAL 追加段做流式增量备份/时间点恢复的独立工具（Go）。
- **可抄机制**：① 消费 WAL 生成段文件，可还原到任意 checkpoint 点；② 证明"SQLite 增量备份=WAL 转运"这条路线可行，无需在 ADV 内自研。
- **档位**：watch——ADV 单机本地优先，若要自动备份优先抄其"WAL 段复制"思想而非引依赖。
- **第一步动作**：不入依赖；备份规范写"周期 VACUUM INTO + WAL 观察"。
- **许可证/成熟度**：Apache-2.0 / 维护中。
- **链接**：https://litestream.io/

### X13-10 integrity_check / quick_check / .recover 运维档【机制级】
- **定位**：库损坏的检出与修复档位。
- **可抄机制**：① `PRAGMA integrity_check`：全库页扫描+B 树结构与索引交叉校验，代价与库大小线性增长（全页遍历），大库上秒级到分钟级；② `PRAGMA quick_check`：跳过"索引内容 vs 表内容"交叉验证，显著更快，作为日常档；③ 无官方增量校验机制；④ 修复走 sqlite3 CLI `.recover` 抢救可读页，或回填 X13-08 的 VACUUM INTO 快照——修复预案的前提是先有备份。
- **档位**：有界——自动跑 quick_check，integrity_check 不进常驻路径。
- **第一步动作**：会话启动与版本升级钩子里跑 quick_check，失败即拒开库并提示走恢复流程。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/pragma.html#pragma_integrity_check

### X13-11 refinery 迁移★Top
- **定位**：Rust 迁移工具，设计抄 Flyway。
- **可抄机制**：① `embed_migrations!` 编译期把迁移 SQL 嵌进二进制，CLI 本地工具无需带迁移文件目录；② 版本表记版本+**checksum**，已应用迁移被手改 → checksum 不匹配即报错，挡住"历史迁移漂移"；③ 支持 rusqlite/SQLite driver，也支持用 Rust 代码而非 SQL 定义迁移。
- **档位**：吸收。
- **ADV 迁移规范（配套）**：① 迁移只进不退，不写 down，回滚=新增反向迁移；② 每个迁移在单个事务内执行（SQLite DDL 可事务化，与 PG 行为不同，这是选本地库的红利）；③ checksum 不匹配或中断留下 dirty 状态 → 拒绝继续，人工介入后修版本表。
- **第一步动作**：adv-index 接 `refinery + rusqlite`，迁移目录 `migrations/V{n}__{desc}.sql`，建库入口统一 `migrate!()`。
- **许可证/成熟度**：MIT / 维护中（crates.io ~4.7M 下载量，2026-10 检索口径）。
- **链接**：https://github.com/rust-db/refinery

### X13-12 rusqlite_migration（对照项）
- **定位**：更轻的迁移库，版本记在 `PRAGMA user_version`。
- **可抄机制**：① 无独立版本表、无 checksum（以仓库当前文档为准），迁移列表 `include_str!` 进编译；② user_version 单整数够用的场景=迁移源码与二进制同发布——ADV 正是这种形态。
- **档位**：有界——作为 refinery 的轻量备选；选 refinery 的理由是 checksum 门禁更硬。
- **第一步动作**：保留在备选清单，若嫌 refinery 重再换，切换成本=迁移表搬一次。
- **许可证/成熟度**：MIT（以 crates.io 为准）/ 维护中。
- **链接**：https://github.com/rust-db/refinery 对照 https://crates.io/crates/rusqlite_migration

### X13-13 覆盖/部分索引与 ADV 查询映射【机制级】
- **定位**：把 ADV 查询模式翻译成索引设计。
- **可抄机制**：① 覆盖索引：查询列全部在索引里 → EXPLAIN QUERY PLAN 显示 `USING COVERING INDEX`，免回表随机 IO；映射：`files(path)` 类查询把 path+常用元数据列并成一个复合索引；② 部分索引（`CREATE INDEX ... WHERE <cond>`）：只索引行子集，体积与写放大都只算子集——映射"只索引待处理/脏行"的扫描器模式，已处理行不进索引；③ 索引选择靠 `ANALYZE` 写 sqlite_stat1，缺统计的库计划器走默认启发式。
- **档位**：吸收。
- **第一步动作**：adv-index 每个 query 定义处标注目标 EXPLAIN QUERY PLAN，建库后跑 ANALYZE。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/partialindex.html

### X13-14 JSON1/JSONB 与表达式索引
- **定位**：扫描元数据（半结构化）入 JSON 列后的查询路径。
- **可抄机制**：① 3.38.0 起 JSON1 内建不再需要编译开关；3.45.0 起 JSONB 二进制格式更快更省（官方 release notes 口径）；② 表达式索引：`CREATE INDEX idx ON t(json_extract(meta,'$.rule'))`，`json_extract(meta,'$.rule')=?` 走索引——只给**会查的键**建，不是给 JSON 全量；③ 生成的列（GENERATED ALWAYS AS）+ 普通索引是同效的显式写法，计划器行为更直观。
- **档位**：吸收。
- **第一步动作**：元数据表先裸 JSON 列，查询稳定后按实测慢查询补表达式索引。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/json1.html

### X13-15 大 blob 判据：100KB 线与溢出页机制【机制级】★Top
- **定位**：代码片段/中间产物"进 BLOB 还是进文件"的判据。
- **可抄机制**：① 官方 fasterthanfs：10KB 量级小 blob 直接进 SQLite 比文件系统读写快约 35%、省约 20% 空间，优势区间到 ~100KB 为止（fasterthanfs.html / internalversusextblob.html 锚）——快的原因是省掉 open/stat/目录查找的系统调用链，不是 IO 本身更快；② 机制：record 超页容量 → 落**溢出页链**（overflow pages），读一个大 blob = 沿链随机读，blob 越大链越长，B 树查询连带劣化；③ 判据落地：BLOB ≤100KB 进库（代码片段典型几百字节~几 KB，全进）；>100KB 落文件，库里存 sha256+size+引用列，配孤儿清理（引用计数或定期 fsck）。
- **档位**：吸收。
- **第一步动作**：adv-index cache 库 schema 定"snippet BLOB 列 + oversize 文件句柄列"双通道，阈值 100KB 进常量并写进 schema 文档（带锚）。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/fasterthanfs.html

### X13-16 增量 blob I/O API
- **定位**：大 BLOB 的流式读写接口（rusqlite `blob` feature）。
- **可抄机制**：`Blob` 句柄按偏移分块读写，不必整块载入内存——只对"确实要经手大 BLOB"的场景有意义。
- **档位**：有界——X13-15 判据下 ADV 不该有大 BLOB 入库，此 API 备而不用。
- **第一步动作**：不启用；若 oversize 通道将来要进库再启用。
- **许可证/成熟度**：MIT（rusqlite）/ 维护中。
- **链接**：https://docs.rs/rusqlite（blob feature）

### X13-17 事务批写 / WAL 的 group commit【机制级】
- **定位**：扫描器批量入库的吞吐核心。
- **可抄机制**：① 单事务 N 条语句 = WAL 一次顺序追加 + **一次 fsync**（NORMAL 档延到 checkpoint），逐语句自动提交则是 N 次——批写差出数量级是官方语义的直接推论（WAL 是顺序文件追加）；② 批大小调的是"内存中的脏页与锁持有时长"与 fsync 次数的折中，判据看 p95 不是均值；③ 批写与 busy_timeout/IMMEDIATE 组合（X13-06）才完整。
- **档位**：吸收。
- **第一步动作**：扫描入库固定 `with_write_tx` 包批，批大小 1000 条起步进基准（X13-07）扫描指标。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/wal.html

### X13-18 预编译语句缓存
- **定位**：热路径 SQL 的解析开销消除。
- **可抄机制**：`prepare_cached` LRU（默认 16，docs.rs 锚）——扫描循环里同一 INSERT 反复执行时免重复 parse+plan；跨连接不共享，每连接各自缓存。
- **档位**：吸收。
- **第一步动作**：扫描/查询热路径全部走 `prepare_cached`，池化时在连接 hook 统一调容量。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://docs.rs/rusqlite

## B. 性能实证与对照

### X13-19 SQLite 规模上限与"何时换引擎"判据
- **定位**：官方上限口径 vs 社区实感，避免"听说大数据要换库"式提前优化。
- **可抄机制**：① 官方硬上限：页数 SQLITE_MAX_PAGE_COUNT 默认 4294967294 × 最大页 64KiB ≈ 281TB（limits.html 锚）——容量远不是 ADV 的约束项；② 真约束是**单写者**（X13-04/17）：写并发冲突率与单写队列延迟，不是行数；③ 判据格式：当"写入 p95 延迟"或"WAL 峰值"超预算且 pragma/批写已调尽 → 才谈分库（index/report 已同库，cache 已独立文件）或换引擎。
- **档位**：reference。
- **第一步动作**：基准脚本（X13-07）里加"行数 × 写延迟"扫描点，画出 ADV 语料量级下的实测曲线再谈上限。
- **许可证/成熟度**：public domain / 维护中。
- **链接**：https://www.sqlite.org/limits.html

### X13-20 DuckDB 报表对照：写路径劣势
- **定位**：E4 对照补充——列式 OLAP 引擎做 ADV "report 分析查询"的候选面。
- **可抄机制**：① 官方 concurrency 口径：单进程可读写、多进程只读 attach——多进程写不在其模型内；② 列存+向量化对聚合/扫描强，但事务提交路径重，OLTP 式小事务写是其明确弱项（其文档定位即 OLAP）；③ 可行组合：adv-index SQLite 保持写路径，报表分析时导出/attach DuckDB 只读跑重聚合。
- **档位**：watch——报表若只是"出 JSON/表格"，SQLite 自身聚合够用；出现跨表重分析再评估。
- **第一步动作**：不做；在 report 需求变重时拿真实查询集实测对比。
- **许可证/成熟度**：MIT / 维护中（活跃迭代，版本演进快）。
- **链接**：https://duckdb.org/docs/connect/concurrency

### X13-21 LMDB mmap 模型对照【机制级】
- **定位**：嵌入式 KV 的 mmap 极简路线，作为 SQLite 的思想对照（E4 已拆 sled/redb/fjall，此处只取对照思想）。
- **可抄机制**：① COW B+tree：写时复制页、父页随路径复制，旧根仍完整可读——**读者永不阻塞写者**且读到一致快照；② 全库 mmap 只读映射，读路径零系统调用（无页缓存两次拷贝）；③ 代价：写路径放大（COW 逐层）、单写者互斥、无二级索引/SQL 语义。
- **档位**：reference——借鉴两点进 ADV：SQLite 只读场景可试 mmap_size（X13-07），以及"快照一致性 = 旧版本页保留"的思想与 X13-08 备份互补。
- **第一步动作**：不动；mmap 实验并入 X13-07 基准档位。
- **许可证/成熟度**：OpenLDAP Public License 2.8 / 维护放缓但极稳（上游多年少改动）。
- **链接**：https://git.openldap.org/openldap/openldap/-/tree/libraries/liblmdb

## C. 内容寻址存储

### X13-22 Git 两级对象库：loose + packfile【机制级】
- **定位**：ADV 内容寻址缓存的形态参照（存对象内容，不存文件名）。
- **可抄机制**：① 两级：新对象写 **loose**（zlib 全量压缩，sha1 前 2 位目录 + 后 38 位文件名），小写延迟、单文件即写即读；**packfile** 由 gc 后台批量合并（`gc.auto` 默认 ~6700 个 loose 触发，git-scm docs 锚），内做 delta 压缩；② 写路径永远是"loose 单文件原子落盘"，打包异步化——读路径两级都能定位；③ 取舍：delta 编码要对象按类型/大小排序+二叉 diff，实现贵；ADV 第一版 pack 只做"tar+zstd 归档"，**不抄 delta**。
- **档位**：吸收（两级形态）；delta 机制不吸收。
- **第一步动作**：cache 层定"新片段 = sha256 前缀目录单文件，计数超阈值触发后台合并归档"草案。
- **许可证/成熟度**：Git GPLv2（作为机制参照，不引依赖）/ 维护中。
- **链接**：https://git-scm.com/book/en/v2/Git-Internals-Packfiles

### X13-23 packfile idx 的 fanout 定位
- **定位**：大对象集合里 O(1)~O(log n) 按 sha 定位。
- **可抄机制**：idx 文件头 256 项 fanout 表按首字节前缀计数，直接跳到目标 sha 区间再二分——纯数组布局、无树结构、mmap 友好。
- **档位**：有界——若 ADV cache 归档后要"不解包查询"，抄 fanout 表布局；第一版解包遍历即可。
- **第一步动作**：记录设计，等 pack 合并功能落地时再决定是否需要 idx。
- **许可证/成熟度**：Git GPLv2（机制参照）/ 维护中。
- **链接**：https://git-scm.com/docs/packformat

### X13-24 restic 的 Rabin 内容定义分块（CDC）【机制级】
- **定位**：CDC 的工业实现参照（Rust 生态另有 rustic 复刻同一仓库格式）。
- **可抄机制**：① 滚动窗口算 Rabin 指纹，指纹低位命中掩码即切块边界——**块边界由内容决定**，文件头部插入/偏移后只有受影响块重算，后续边界稳定，这是去重对"常改头部"的源码文件有效的原因；② 默认参数：min 512KiB / 平均 1MiB / max 8MiB，小于 512KiB 的文件整块不切（restic 官方论坛与 rustic 文档锚），多项式为每仓库随机的 53 次不可约式（防刻意构造碰撞）；③ 块按内容 sha 寻址进仓库，与 X13-22 两级形态叠加。
- **档位**：有界——机制吸收，启用与否取决于 X13-27 判据。
- **第一步动作**：不动代码；在 cache 设计文档写明"CDC 启用条件"占位。
- **许可证/成熟度**：restic BSD-2-Clause / 维护中；rustic（Rust 实现）GPL-3.0?（以仓库为准）/ 维护中。
- **链接**：https://github.com/restic/restic（doc/design.rst Chunking 节）

### X13-25 borg 的 buzhash 分块对照
- **定位**：CDC 另一流派（buzhash 滚动哈希）的参数对照。
- **可抄机制**：默认 `buzhash,19,23,21,4095` = min 512KiB / 平均 2MiB / max 8MiB，小文件（<512KiB 默认阈值）整块（borg 官方文档锚）——与 restic 同区间不同哈希，说明平均块大小 1–2MiB 是"备份语义"的共识档，不是算法决定。
- **档位**：reference。
- **第一步动作**：无；仅作 X13-24 参数选择的旁证。
- **许可证/成熟度**：BSD-3-Clause / 维护中。
- **链接**：https://borgbackup.readthedocs.io（chunker-params）

### X13-26 casync 的小块 chunk store
- **定位**：CDC 用于"系统/容器镜像分发"的参照（作者 Lennart Poettering）。
- **可抄机制**：默认平均 64KiB 块（作者博客原文锚），块粒度比备份工具小两个数量级——用途是 HTTP 按块拉取的 CDN 友好分发；说明**块大小跟着分发/访问模式走**，不是普适值。
- **档位**：reference。
- **第一步动作**：无。
- **许可证/成熟度**：LGPL-2.1+ / 上游基本停更（实验项目定位，机制仍有参照价值）。
- **链接**：https://0pointer.net/blog/asynchronous-http-downloads-for-c.html

### X13-27 CDC 对 ADV 语料库的价值判据【机制级】
- **定位**：决定 ADV 内容寻址缓存要不要上 CDC。
- **可抄机制**：① 收益模型：版本间"块级重合率"决定 CDC 净收益——源码改动常集中在文件局部，CDC 边界稳定使重合块可复用；fixed-size 分块则边界漂移、重合率塌掉（X13-24 机制的直接推论）；② 代价模型：CDC 切块本身有 CPU 成本（滚动哈希逐字节），小块（casync 档）成本更高；③ 判据落地：先对 ADV 目标语料的两个相邻快照跑"同算法分块重合率"实测——重合率显著高于 zstd 全量重压的等效收益才启用；否则 plain zstd + 整文件寻址。
- **档位**：有界。
- **第一步动作**：cache 设计文档记录判据公式与所需实测脚本名，等语料库版本化需求出现再量。
- **许可证/成熟度**：n/a（本域自研判据；机制锚 X13-24/25/26）。
- **链接**：https://github.com/restic/restic（design.rst）

---

## 附：登记说明
- 本域共 27 条，标题标注【机制级】13 条、registry depth 口径 14 条（X13-11 refinery 机制级未上标题）；registry 见 `registry/X13.jsonl`。
- 档位口径：吸收=直接进 ADV 实现计划；有界=进设计文档带判据，条件触发才动；不吸收=明确不用；watch=等外部信号；reference=思想参照不引入。
