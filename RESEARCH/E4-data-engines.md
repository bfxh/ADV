# E4 数据引擎与本地存储 — 机制盘点（为 adv-index 存储层定案供证）

- 日期：2026-10-03；口径：crates.io / GitHub API 一手查询（本日执行）+ 官方文档（sqlite.org FTS5 原文核读）+ Web 检索（检索来的数字标"检索口径"，弱锚标注）。
- 档位词：吸收 | 有界 | 不吸收 | watch | reference。成熟度：实验 | 维护中 | 弃维护。
- 本域只登记存储侧事实；向量选型已由用户拍板（不搞嵌入模型），sqlite-vec / lancedb 仅记录现状。

---

## 一、2025–2026 前沿信号

1. **DuckDB v2.0（codename cyanoptera）预览**：自研 SQL 解析器替换 Postgres 派生解析器、异步 I/O、触发器、VARIANT 类型、quack 协议客户端/服务模式（嵌入式向服务端长出一脚）；**新默认存储格式 v2.0，1.x 无法打开 2.0 文件**（2026-08 报道口径，正式发布以官方为准）。若吸收必须锁版本。
2. **sled 事实冻结**：最后稳定版 0.34.7，无 1.0；GitHub API（2026-10-03 查）主仓未 archived、最近 push 2026-04-04；issue #1444（2023-04）"项目是否死了"由维护者回应，方向指向 komora 模块化重写。Rust 嵌入 KV 社区迁移去向：redb、fjall、native_db。
3. **bincode 3.0.0 落地**（crates.io 2025-12-16）：serde 变为可选（核心 Encode/Decode derive），序列化层换代。
4. **Polars 新流式引擎成为默认**：纯 Rust 重写旧引擎（旧引擎曾基于 node.js），Polars 2.0 起全部 LazyFrame 默认走流式（out-of-core）（检索口径，2026）。
5. **Turso（前身 Limbo）**：SQLite 兼容的纯 Rust 重写，0.8.1（2026-10-02）、主仓 24.5k stars、pushed 2026-10-03（GitHub API 一手）——"SQLite 文件格式"出现第二个高热度实现，但兼容完整度未满，先 watch。
6. **嵌入式分析赛道收敛**：Kuzu（嵌入式图库）2025-10 归档（检索口径）；meilisearch 在 crates.io 的包停在 0.0.0（2022-10-12）——**服务端产品不提供库形态**是普遍事实而非个例。
7. **sqlite-vec 放缓**：主线停在 0.1.x（crate 0.1.9，2026-05-18，与仓库最后 push 同日），ANN 索引仍未交付；pre-v1 破坏性变更预期保留。

---

## 二、重点回答

### ① DuckDB 嵌入 ADV 的可行性判据（什么时候比 SQLite 值）
- **机制事实**：进程内库（零服务、零守护）、单文件 `.duckdb` 列存、向量化执行（SIMD 批处理 + morsel 并行）；`ATTACH` SQLite / 直查 Parquet、CSV.gz；MIT 许可；Windows 官方一等支持。
- **值（吸收进报表层）当**：(a) 报表是聚合/窗口/多维 group by 类负载，SQLite 行存+解释执行成为瓶颈；(b) 数据量超出内存需要 out-of-core；(c) 需要直接扫 Parquet/外部文件而不落地。
- **不值（留 SQLite）当**：(a) 只是把报告存下来再按行读回——SQLite 完全覆盖且已是既定基调；(b) 对依赖体积与构建时间敏感（duckdb crate 默认 bundled，C++ 编译可观；libduckdb 产物数十 MB 量级——弱锚，需以实际构建为准）；(c) 不想同时运维两套存储格式/两套备份。
- **风险注记**：Python 侧"死锁"是 fork 类问题（原生调度线程+互斥锁被 fork 进子进程；官方查询执行会释放 GIL）。ADV 是 Rust 主体，风险形态变为：GUI/工作线程里连接生命周期与后台任务线程的交互，管理连接创建/销毁时机即可，不是致命项。
- **档位：有界**（报表层判据满足才进；锁 1.x 版本，避开 2.0 格式断裂窗口）。

### ② sled 是否弃维护（一手证据）
- **结论：事实冻结（弃维护级），不可用于新选型。** 证据链：
  1. crates.io（2026-10-03 查）：`sled` 最后稳定版 **0.34.7**（社区口径 2021-07 发布；crates.io `updated_at=2024-10-11`——yank/元数据操作也会刷新该字段，发布判定以版本号不变为准）；无 1.0，beta 状态持续多年。
  2. GitHub API（2026-10-03 查）：`spacejam/sled` 未 archived、`pushed_at=2026-04-04`、9097 stars、172 open issues——**未归档但发布停滞，push 为零星级**。
  3. issue #1444（2023-04）"Hi I wanted to know if this project is dead?"：维护者回应称 MIT/Apache 双许可仍在，重写在 **komora** 项目中以模块化方式推进——即维护者精力已转移。
- 佐证：crates.io `sled` 描述自称 "Lightweight high-performance pure-rust transactional embedded database"，但 native_db 等社区项目从 sled 迁移到其他后端、论坛选型贴普遍将其列为"不再维护"。

### ③ FTS5 bm25 调参与自定义分词器注册（代码符号检索的实际路径）
- **bm25 真相：k1、b 硬编码 k1=1.2、b=0.75，不可调**（官方文档原文核读，2026-10-03）。可调的是**列权重**：
  - 按查询：`ORDER BY bm25(email, 10.0, 5.0)`（尾参按列从左到右，缺省 1.0）；
  - 持久化：`INSERT INTO ft(ft, rank) VALUES('rank', 'bm25(10.0, 5.0)')`；`ORDER BY rank` 比 `ORDER BY bm25(ft)` 快。
- **分词器（对代码符号的关键）**：
  - `unicode61`（默认）：`tokenchars '-_'` 可把下划线/连字符并入 token、`separators` 反向拆分、`remove_diacritics`、`categories`；
  - `trigram`（3 字符滑窗）：**支持对 LIKE/GLOB 的索引加速**（子串检索不用 LIKE 全扫）；限制：子串 <3 字符查不到；`case_sensitive 1` 时禁用 LIKE（GLOB 可用）；`detail=none/column` 时查询不能含 >3 字符 token；
  - `porter`：包装型词干化（英文向）。
- **自定义分词器注册**：`SELECT fts5(?1)` + `sqlite3_bind_pointer(stmt, 1, &api, "fts5_api_ptr", NULL)` 取 `fts5_api` → `xCreateTokenizer_v2`（需 `iVersion>=3`）注册 `fts5_tokenizer_v2{xCreate, xDelete, xTokenize}`；`xTokenize` 收 `FTS5_TOKENIZE_DOCUMENT / _QUERY / _QUERY|_PREFIX / _AUX` 标志，逐 token 调 `xToken` 上报文本+字节长+起止偏移；`FTS5_TOKEN_COLOCATED` 标志做同义词（colocated token）。`xFindTokenizer` 可取已有分词器做包装。Rust 侧：rusqlite 无高级封装，需 unsafe C ABI 薄层。
- **存储侧配套**：外部内容表 `content='t1', content_rowid='a'`（索引只存词，正文留主表，配触发器维持一致，`INSERT INTO ft(ft) VALUES('rebuild')` 修复）；contentless（`content=''`，无 UPDATE/DELETE）；**contentless_delete=1（3.43.0+）** 支持删除（墓碑+`deletemerge`，官方建议新代码优先）；`prefix='2 3'` 前缀索引加速补全；`detail=full/column/none` 索引瘦身（官方数据：1636 MiB 语料 → 743 / 340 / 134 MiB）。
- **ADV 落地第一步**：`tokenize="unicode61 tokenchars '_-'"` 承担标识符分词 + 一张 trigram 表（`detail=none`）承担子串/精确符号匹配 + bm25 列权重（符号列 > 路径列 > 注释列）；不够（如 camelCase 拆词）再上自定义分词器，不必先写 C。

### ④ 2025–2026 新/存嵌入式引擎成熟度分级（crates.io + GitHub API，2026-10-03）
- **维护中（高频活跃）**：redb 4.3.0（更新 2026-10-02，主仓 push 2026-10-03）、fjall 3.1.11（2026-10-02）、gluesql 0.20.0（2026-08-30）、rocksdb 0.25.0（2026-08-16）、turso 0.8.1（2026-10-02）、lancedb 0.39.0（2026-09-17）、tantivy 0.26.2（2026-09-08）。
- **维护中（低频）**：persy 1.8.1（2026-06-30，MPL-2.0，下载 74 万）、native_db 0.8.2（2025-07-08，发布≈年更低，主仓有零星 push）、sqlite-vec 0.1.9（2026-05-18）。
- **实验级**：sqlite-vec（pre-v1 明示 breaking changes）、turso（SQLite 兼容完整度未满，非官方实现）、DuckLake（新格式）。
- **弃维护（事实冻结）**：sled（证据见 ②）。

### ⑤ 报告/缓存/索引三层：一个 SQLite 文件还是分文件
- **机制依据**：WAL 模式=单写者多读者；写锁与 `-wal`/`-shm` 都是**库级**粒度；一个长读事务会顶住 checkpoint → WAL 膨胀；`busy_timeout` 只治等待不治 contention；备份粒度=文件（`VACUUM INTO` / backup API）；独立文件可各自 VACUUM/删除/随包分发。
- **判据**：
  1. **写入节奏同相**（索引批量刷、报告低频写、二者无高频小写）→ 单库，省跨库事务与一致性代码；
  2. **写入异相**（缓存高频小写 + 索引长事务/长读并存）→ 分库，否则缓存写与索引 checkpoint 互相顶牛，WAL 交替膨胀；
  3. **备份/丢弃粒度不同**（缓存要"整文件随时可删重建"，索引/报告要完整备份）→ 分库。
- **默认建议**：index 与 report 同库（跨层 join 与事务一致性收益真实），cache 独立文件（高频小写、可随时丢）。该默认可被实测推翻：判据是"锁竞争窗口是否重叠 + WAL 膨胀曲线"，非教条。

---

## 三、逐对象深挖

### 分析型

**1. DuckDB** — 定位：进程内嵌入式 OLAP（"分析界的 SQLite"）。可抄机制：①向量化执行+morsel 并行（列批+任务窃取）；②单文件列存+`ATTACH` 异源（SQLite/Parquet/CSV.gz）直查；③查询执行释放 GIL（Python 侧）与 fork 死锁类问题清单（fork 前关连接、fork 后建连接、优先 spawn）。档位：**有界**（报表层，判据见 ①）。第一步动作：在 adv-index 报表层做 1 个真实聚合查询的 SQLite-vs-DuckDB 对比（同数据、同机），过线才引依赖并锁 1.x。许可/成熟度：MIT / 维护中（41.9k stars，pushed 2026-10-02）。链接：duckdb.org、github.com/duckdb/duckdb。注：v2.0 预览见前沿信号 1。

**2. DataFusion** — 定位：Rust 的 SQL 查询执行引擎（不是存储引擎）。可抄机制：①Arrow 列式执行+谓词/投影下推；②可配置内存池——GreedyMemoryPool（按分区贪心预留）、FairSpillPool（给可溢写算子保底份额）、UnboundedMemoryPool；③溢写(spill)机制让排序/聚合超内存不死。档位：**reference**（ADV 若要"SQL over Arrow/自定义数据源"才相关；存储问题 SQLite 已解）。第一步动作：无，仅登记。许可/成熟度：Apache-2.0 / 维护中（crate 55.1.0，2026-09-11）。链接：github.com/apache/datafusion。

**3. Polars** — 定位：DataFrame 库+流式分析引擎。可抄机制：①纯 Rust 新流式引擎（out-of-core，按批过算子），Polars 2.0 起为默认（检索口径）；②GPU 流式（RAPIDS，2025-07）。档位：**reference**（Python 侧报表备选；Rust 侧嵌入无必要）。第一步动作：无。许可/成熟度：MIT / 维护中（crate 0.55.2，2026-08-06；39.9k stars）。链接：pola.rs。

**4. DuckLake** — 定位：Duck Labs（2025）的湖仓目录格式：catalog 与数据文件分离。可抄机制：①官方自己把"单文件"边界外移的方向证据；②catalog/数据分离的元数据设计。档位：**watch**。第一步动作：无。许可/成熟度：开源（license 未一手核）/ 实验。链接：ducklake.org。

### KV / 嵌入式

**5. redb** — 定位：纯 Rust 嵌入式 KV（B 树）。可抄机制：①单文件+B 树+MVCC+写时复制，读多写少场景的简洁实现参照；②ACID+保存点 API 形态。档位：**watch**（缓存层候选，等 ⑤ 判据定案后与 fjall 对比）。第一步动作：若缓存分库且 KV 形态确立，跑 redb vs fjall 写放大对比。许可/成熟度：MIT OR Apache-2.0 / 维护中（4.3.0，2026-10-02）。链接：github.com/cberner/redb。

**6. redb2** — 定位：核实结果——**crates.io 不存在名为 redb2 的 crate**（2026-10-03 查询 NOTFOUND）。"redb2"应指 redb 2.0（2024 年 API 重写大版本线），当前主线已到 4.3.0。档位：**reference**。第一步动作：无（以 redb 条目为准）。许可/成熟度：MIT OR Apache-2.0 / 同 redb。链接：crates.io/crates/redb。

**7. fjall** — 定位：纯 Rust LSM 嵌入式 KV。可抄机制：①分区 keyspace（≈列族）；②内置 KV 分离（大 value 出 LSM 进 blob，减轻压实写放大）；③底层 `lsm-tree` crate 被 SurrealKV 复用（组件化）。v2.0（2024-09）重设计磁盘格式+miniz 压缩；轻量（编译≈3.5s、二进制≈2.2MB，atproto PDS 规划文档口径，弱锚）。档位：**有界**（缓存层首选候选：写多读少形态匹配 LSM）。第一步动作：同 redb 条目，做对比基准。许可/成熟度：MIT OR Apache-2.0 / 维护中（3.1.11，2026-10-02）。链接：github.com/fjall-rs/fjall。

**8. sled** — 定位：曾经的 Rust 嵌入式 KV 默认答案。可抄机制：仅 API 形态设计（B 树 range+事务+watch 的接口审美）影响后来者；实现不再作为依据。档位：**不吸收**。第一步动作：从候选清单划掉；文档引其 issue #1444 作为选型警示。许可/成熟度：MIT OR Apache-2.0 / **弃维护（事实冻结，证据见 ②）**。链接：github.com/spacejam/sled。

**9. LMDB（对照）** — 定位：嵌入式 KV 开销下限参照。可抄机制：①mmap 只读零拷贝读取；②单写者多读者+COW 页事务模型。档位：**reference**。HMDB：未检索到同名主流存储引擎（2026-10-03），疑为混淆，不作事实登记。第一步动作：无。许可/成熟度：OpenLDAP Public License（含署名条款，非标准开源范式）/ 维护中（低速，长期稳定）。链接：openldap.org。

**10. rocksdb（rust crate / librocksdb-sys）** — 定位：LSM 集大成者的 FFI 对照。可抄机制：WAL/MemTable/SST/分级压实+布隆过滤器的参数学（fjall/SQLite 调参时的对照坐标系）。档位：**不吸收**（C++ FFI+构建链+体积与"本地优先"相性差）。第一步动作：无。许可/成熟度：crate 声明 Apache-2.0（上游 RocksDB 为 GPLv2/Apache-2.0 双许可）/ 维护中（0.25.0，2026-08-16）。链接：github.com/rust-rocksdb/rust-rocksdb。

**11. persy** — 定位：纯 Rust 单文件事务持久引擎（段+页+事务日志，非 KV 非 SQL）。可抄机制：事务日志与页管理的极简实现参照。档位：**不吸收**（定位重叠 SQLite/redb 且生态小众）。第一步动作：无。许可/成熟度：MPL-2.0（crates.io 一手）/ 维护中（低频，1.8.1，2026-06-30，下载 74 万）。链接：github.com/applebuddy/persy（以 crates.io 指向为准）。

**12. NativeDB（native_db）** — 定位：模型优先的嵌入式库（定义 struct→自动索引→重启重载）。可抄机制：模型版本化迁移（native_model 配套）思路。档位：**watch**（社区把它当 sled 迁移去向之一，但发布节奏低）。第一步动作：无。许可/成熟度：MIT / 维护中（低频，0.8.2，2025-07-08；主仓零星 push 2026-10-03）。链接：github.com/vincent-herlemont/native_db。

**13. GlueSQL** — 定位：SQL 引擎与存储解耦（Storage trait 可插拔，默认 sled 存储）。可抄机制：自定义 KV 后端上跑 SQL 的接口分层。档位：**reference**（SQLite 已覆盖该需求）。第一步动作：无。许可/成熟度：Apache-2.0 / 维护中（0.20.0，2026-08-30；pushed 2026-10-01）。链接：github.com/gluesql/gluesql。

**14. Turso（前身 Limbo）** — 定位：SQLite 文件格式兼容的纯 Rust 重写（异步 IO 倾向）。可抄机制：SQLite 格式的 Rust 侧再实现路径；若成熟可摆脱 C 编译链。档位：**watch**（兼容完整度未满前不入生产路径）。第一步动作：季度复核一次兼容度公告。许可/成熟度：MIT / 实验→维护中过渡（0.8.1，2026-10-02；24.5k stars）。链接：github.com/tursodatabase/turso。

### 全文 / 搜索扩展

**15. SQLite FTS5** — 定位：adv-index 全文基座（wave-0 已定）。可抄机制：见 ③ 全套（bm25 硬编码真相/列权重、unicode61 tokenchars、trigram+LIKE 加速、自定义分词器 fts5_api 注册、外部内容表+触发器+rebuild、contentless_delete、prefix、detail 瘦身）。档位：**吸收**。第一步动作：按 ③ 的组合落地索引 schema（unicode61 tokenchars '_-' 主表 + trigram detail=none 副表 + bm25 列权重）。许可/成熟度：Public Domain / SQLite 内置（3.43+ 建议）。链接：sqlite.org/fts5.html。

**16. tantivy** — 定位：Rust 全文引擎库（wave-0 09 已定"按语料/增量判据切换"方向，此处补成本面）。可抄机制：①倒排+BM25+列存 fast fields 共存；②posting 解码 SIMD 运行时分派（0.22 起，SSE2/AVX2）。成本面：纯 Rust 依赖为主但编译产物与体积可观——切换判据在 wave-0 09，不在此重复。档位：**有界**。第一步动作：遵守 wave-0 09 判据，语料阈值触发再集成。许可/成熟度：MIT / 维护中（0.26.2，2026-09-08）。链接：github.com/tantivy-dev/tantivy。

**17. meilisearch** — 定位：REST 搜索服务。可抄机制：无（服务形态）。嵌入可行性验证：**不可嵌入**——crates.io `meilisearch` 包停在 0.0.0（2022-10-12，占位），官方从未提供库形态。档位：**不吸收**。第一步动作：无。许可/成熟度：MIT / 维护中（服务端）。链接：meilisearch.com。

**18. quickwit** — 定位：面向对象存储的分布式日志检索服务（tantivy 系核心）。嵌入可行性验证：**不可嵌入**——服务集群形态；主仓未 archived（GitHub API 2026-10-03：pushed 2026-10-03、837 open issues）但产品方向仍是服务。档位：**不吸收**。第一步动作：无。许可/成熟度：Apache-2.0 / 维护中。链接：github.com/quickwit-oss/quickwit。

### 向量（只登记存储侧事实，不选型）

**19. sqlite-vec** — 定位：纯 C 零依赖 SQLite 向量扩展（wave-0 已采用）。现状：vec0 虚表存 float/int8/binary 向量；主打暴力 KNN+量化（int8/bit），**无 ANN 索引**（作者明确先做快暴力搜索，十万级向量可接受，官方博客口径）；0.1.6（2024-11）加元数据列/分区键/辅助列；crate 0.1.9（2026-05-18，repo 最后 push 同日）——发布放缓，pre-v1。档位：**有界**（已在用；注意锁版本+预留替换缝）。第一步动作：登记版本锁；关注 0.2/ANN 进展。许可/成熟度：GitHub 检测 Apache-2.0 / 实验（pre-v1）。链接：github.com/asg017/sqlite-vec。

**20. LanceDB** — 定位：嵌入式向量库（Lance 列存格式，随机访问+版本化设计）。现状：进程内 SDK 无服务，但**存储为目录+多文件，非严格单文件**（2026-10-03 核实口径）。档位：**watch**（若未来向量规模超暴力 KNN 上限再评）。第一步动作：无。许可/成熟度：Apache-2.0 / 维护中（0.39.0，2026-09-17）。链接：github.com/lancedb/lancedb。

### 缓存与序列化层

**21. bincode** — 定位：二进制序列化（X3 交界，此处不重复展开）。机制：3.0.0（2025-12-16）serde 可选化（核心 Encode/Decode derive）、varint/fixed 与端序可配。档位：**reference**。第一步动作：无（归 X3）。许可/成熟度：MIT / 维护中。链接：github.com/bincode-org/bincode。

**22. postcard** — 定位：no_std 二进制序列化（X3 交界）。机制：varint+flavor 中间修饰（COBS 等）+postcard-schema。档位：**reference**。第一步动作：无（归 X3）。许可/成熟度：MIT OR Apache-2.0 / 维护中（1.1.3，2025-07-24）。链接：github.com/jamesmunns/postcard。

**23. SQLite JSON vs 表（报告存储模式）** — 定位：ADV 报告落库的模式设计。可抄机制：①SQLite 3.45+（2024-01）JSONB 二进制格式（解析一次存储，内部遍历快）；②`json_extract`+**生成列建索引**把半结构化长尾拉进索引；③模式权衡：查询/过滤要用的字段拉成真实列，长尾观测字段存 JSON——索引成本由生成列显式控制，避免全 JSON 扫描。档位：**吸收**。第一步动作：报告表 schema 按"核心列 + 残余 JSONB + 生成列索引"定稿。许可/成熟度：Public Domain / SQLite 内置。链接：sqlite.org/json1.html。

**24. SQLite 单文件 vs 多文件（三层部署设计）** — 定位：⑤ 的机制依据条目（判据与默认建议见 ⑤）。可抄机制：WAL 库级单写者/checkpoint 阻塞模型、`VACUUM INTO`/backup API 的文件粒度。档位：**吸收**。第一步动作：按 ⑤ 默认（index+report 同库，cache 独立文件）出 adv-index 存储布局草案，再以实测锁竞争/WAL 曲线复核。许可/成熟度：Public Domain / SQLite 内置。链接：sqlite.org/wal.html。

---

## 四、Top-3

1. **sled = 事实冻结，排除出 Rust 嵌入 KV 选型**（一手：GitHub API pushed_at=2026-04-04、最后稳定版 0.34.7 无 1.0、issue #1444+komora 转向）；候选收敛为 redb（B 树/读多写少）与 fjall（LSM/写多读少）。
2. **FTS5 调参路径具体化**：bm25 的 k1/b 硬编码不可调，实际抓手=列权重+分词器（unicode61 tokenchars '_-'、trigram 子串加速）+detail/prefix 瘦身；自定义分词器走 fts5_api→xCreateTokenizer_v2，但 camelCase 场景可先不上 C。
3. **三层布局默认判据**：index+report 同库（事务一致/跨层 join），cache 独立文件（WAL 顶牛+备份粒度）；DuckDB 按 ① 判据"有界"进报表层并锁 1.x（2.0 存储格式破坏性变更）。

## 五、证据表（2026-10-03 一手查询）

| 对象 | crates.io | GitHub API | license 来源 |
|---|---|---|---|
| sled | 0.34.7 / updated_at 2024-10-11 | pushed 2026-04-04，未 archived，9097★ | crates.io: MIT OR Apache-2.0 |
| redb | 4.3.0 / 2026-10-02 | pushed 2026-10-03，4823★ | crates.io: MIT OR Apache-2.0 |
| fjall | 3.1.11 / 2026-10-02 | pushed 2026-10-02，2363★ | crates.io: MIT OR Apache-2.0 |
| persy | 1.8.1 / 2026-06-30 | — | crates.io: MPL-2.0 |
| native_db | 0.8.2 / 2025-07-08 | pushed 2026-10-03（零星），714★ | crates.io: MIT |
| gluesql | 0.20.0 / 2026-08-30 | pushed 2026-10-01，3132★ | GitHub: Apache-2.0 |
| tantivy | 0.26.2 / 2026-09-08 | — | MIT |
| meilisearch | 0.0.0 / 2022-10-12（占位） | — | MIT |
| sqlite-vec | 0.1.9 / 2026-05-18 | pushed 2026-05-18，8161★ | GitHub: Apache-2.0 |
| lancedb | 0.39.0 / 2026-09-17 | — | Apache-2.0 |
| bincode | 3.0.0 / 2025-12-16 | — | MIT |
| postcard | 1.1.3 / 2025-07-24 | — | MIT OR Apache-2.0 |
| rusqlite | 0.40.2 / 2026-08-08 | — | MIT |
| datafusion | 55.1.0 / 2026-09-11 | — | Apache-2.0 |
| polars | 0.55.2 / 2026-08-06 | pushed 2026-10-03，39.9k★ | GitHub: MIT |
| duckdb | 1.10506.0 / 2026-09-30 | pushed 2026-10-02，41.9k★ | GitHub: MIT |
| rocksdb | 0.25.0 / 2026-08-16 | — | crates.io: Apache-2.0（上游双许可） |
| turso | 0.8.1 / 2026-10-02 | pushed 2026-10-03，24.5k★ | GitHub: MIT |
| quickwit | —（服务端） | pushed 2026-10-03，未 archived，11.7k★ | GitHub: Apache-2.0 |

（注：tantivy 主仓 GitHub API 本次查询未返回（疑接口波动），版本锚以 crates.io 为准。）
