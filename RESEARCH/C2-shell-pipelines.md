# C2 · shell / 管道 / 结构化数据 CLI 机制解剖（ADV 扩展调研）

> 任务锚：用户 2026-10-03 指派 C2 域；方法：WebSearch/WebFetch，2025–2026 资料优先；数字带来源锚（man 页/官方发布/README 自报口径），自报数字不升格为实测结论；许可逐条标注，AGPL/专有 = 只抄思想不抄码。
> 服务对象：ADV（Rust 本地优先代码安全平台）的 cli 面（adv report / adv query）与批处理管线（adv batch / adv-bin 归档扫描）。
> 收录口径：deep 14 个（机制级）+ sweep 12 个 + 2 条查证排除；登记表 `registry/C2.jsonl` 共 26 条。

---

## 1. 机制级拆解（deep）

### 1.1 GNU parallel —— 批处理调度语义的参考实现
- **定位**：把"输入列表 × 命令"的批处理做成带台账、资源感知、输出管理的作业调度器。
- **可抄机制**：
  1. **输出三档**：默认 `--group` 把每作业输出整体缓冲、结束时按 stdout→stderr 原子吐出（man 注约 0.5ms CPU/作业代价）；`--line-buffer` 按完整行实时释放（与 `--keep-order`/`--results`/`--compress` 同用时退化走磁盘缓冲）；`--ungroup` 零缓冲、允许半行交叉。`--keep-order` 只推迟**打印次序**到作业提交序，执行仍并发——"并发执行 + 有序释放"是两件事。
  2. **台账与续跑**：`--joblog` TSV（seq/sshlogin/start/runtime/收发字节/exit/signal/command）+ `--resume`（只按 seq 续未完成，要求输入与 joblog 不变）/ `--resume-failed`（重跑非零退出行）/ `--retry-failed`（连命令都从台账取）。
  3. **分块与资源感知**：`--pipe` 默认 1M 块、`--recstart/--recend` 记录边界对齐（二进制用 `--recend ""`），man 自报吞吐约 1GB/s 入 / 100MB/s 出，`--pipe-part` 对可寻址文件约 5GB/s；`--memfree`（低于阈值一半时杀最年轻作业重排）、`--load`、`--noswap`、`--delay auto`（失败上调 30%、成功下调 10%）、`--limit` 启动前钩子。
  4. **失败策略**：`--halt now|soon,fail=X|Y%`（默认 never）分级中止。
- **退出码口径**（对照 xargs）：parallel 自身非零退出码按失败作业计数返回（0 = 全成）；xargs 是固定值 123 表示"任一子进程退出码落在 1–125"。
- **档位**：【吸收】（语义层吸收；GPL-3.0-or-later + 学术引用倡议（`--citation`/`--will-cite`），维护中。代码不进 ADV，也不建议作为运行时依赖，只抄语义自研）。
- **第一步动作**：adv batch 设计稿固化四件套——joblog（JSONL 版）+ `--resume-failed` 语义 + `--halt fail=%` + 输出三档开关。
- **链接**：https://www.gnu.org/software/parallel/man.html

### 1.2 xargs -P —— 最小并发执行器的边界
- **定位**：POSIX 系批量执行器；`-P n` 并发，其余交给用户。
- **可抄机制**：① 无输出管理：子进程共享 stdout，输出任意交叉，无保序手段（unix.SE #528542 佐证，Debian parallel 手册亦明言 "xargs has no support for keeping the order of the output"）；② 退出码契约：123 = 任一子进程退出 1–125（man 口径），126/127 = 不可执行/找不到；③ `-0` NUL 分隔是路径安全管线的事实标准。
- **档位**：【reference】（借鉴退出码与 NUL 契约；调度与输出管理全部不学）。
- **第一步动作**：ADV 退出码表里保留 126/127 执行错误口径；路径机读接口默认 NUL/JSONL，不用裸换行分隔。
- **许可/成熟度**：GPLv3+（GNU findutils），基础工具，维护中。
- **链接**：https://www.gnu.org/software/findutils/manual/html_node/find_html/xargs-options.html

### 1.3 jq —— JSON DSL 的事实标准与"全量解析"的教训
- **定位**：JSON 的 sed/awk；十年生态、语义稳定的过滤语言。
- **新鲜度锚**：jqlang.org —— 1.8.0 于 2025-06-01 发布，随后 1.8.1、1.8.2（含安全修复与 Windows arm64 构建）；项目 2023 年回归组织化维护（jqlang）。维护中，MIT。
- **可抄机制**：
  1. **默认整文档物化**进内存（完整解析成值树再求值）——对超大 JSON 是 O(文档大小) 内存；`--stream` 才改产出 `[path, leaf]` 事件流（可配 `tostream/fromstream` 消费），多文档靠 `input/inputs/--slurp/-n`，`--seq` 输出 RFC 7464 帧序列。
  2. **流式的生态代价有档可查**：jq→JSONL 内存密集（issue #1897）、深度限制需 `--stream` 绕行（issue #2846）；过滤器生态大多假定整树语义，`--stream` 几乎要求重写过滤器。⇒ 结论：jq 的"流式"是逃生门而非默认承诺。
  3. **语言语义稳定**是生态之本：十年兼容是 jq 能当"数据管道 Esperanto"的前提。
- **档位**：【吸收】（两条原则：JSONL-first 互换格式；流式/全量必须显式承诺并写进 --help）。详见 §3.2。
- **第一步动作**：adv report 默认 JSONL；单个大对象 JSON 用 serde_json StreamDeserializer 做流式 tokenizer；不承诺 jq 兼容（那是个语言，不是格式）。
- **链接**：https://jqlang.org/ · https://github.com/jqlang/jq

### 1.4 jaq —— 可嵌入的 jq 语义引擎（Rust）
- **定位**：jq 克隆，聚焦正确性/速度/可嵌入；MIT。
- **新鲜度锚**：v2.0（2024-11，r/rust 公告）→ 3.0 引入多格式（JSON/YAML/TOML，crates.io jaq 3.0.0）→ v3.1.1（2025-08-05，GitHub releases）。维护中。
- **可抄机制**：① **crate 拆分**（jaq-core/jaq-interpret/…）：过滤表达式引擎可以作为库嵌入宿主程序——这是对 ADV 最有价值的形态；② Rust 求值器，README 自报快于发行版 jq 构建（HN 讨论同时指出：优化构建的 jq 仍可能反超——别把自报数字绝对化）；③ 兼容性定位是"jq 子集 + 扩展"，3.0 起自带多格式。
- **档位**：【有界】（作为 adv query 的**可选**过滤引擎嵌入；--help 必须明示子集差异，避免用户拿 jq 脚本直接踩坑）。
- **第一步动作**：spike——用 jaq-core 跑 20 条 ADV 典型过滤样例，产出与 jq 1.8 的语义差异表，再决定嵌不嵌。
- **链接**：https://github.com/01mf02/jaq · https://crates.io/crates/jaq

### 1.5 mikefarah/yq —— 多格式 + 保留注释的改写器
- **定位**：YAML/JSON/XML/CSV/TOML/HCL/properties 统一处理器，K8s 生态事实标准；MIT；维护中。
- **新鲜度锚**：4.45.1（2025-01-11）→ v4.50.1（2025-12-13，TOML roundtrip）；教训锚：4.45.3 曾引入 `//` 运算符行为回归（issue #2377）——表达式 DSL 的小版本也可能破坏语义。
- **可抄机制**：① **注释/样式/锚点保留式改写**（in-place 不丢注释）——ADV 若提供"配置加固/改写"类功能直接可抄；② 多文档流逐个求值；③ 非流式：整树载入（与 jq 同策略）。
- **档位**：【有界】（需要 YAML 改写时作为外部进程调用，不内嵌）。
- **第一步动作**：不动作，挂需求位（YAML 配置改写出现时再评估）。
- **链接**：https://github.com/mikefarah/yq

### 1.6 Miller（mlr）—— "名称索引数据"的记录流架构
- **定位**：CSV/TSV/JSON/JSONL/YAML/DKVP/PPRINT 等格式上做 awk/sed/cut/join/sort 式变换；BSD-2-Clause；维护中。
- **新鲜度锚**：Miller 6.22.0 文档在线（miller.readthedocs.io），v6 系列 2024–2025 持续维护。
- **可抄机制**：
  1. **统一记录流**：readers → 记录流 → verbs/DSL → writers，**格式只是记录的编解码层**（`--icsv --ojson` 一行换格式、逻辑不动）。⇒ ADV 结果层应同构：内部记录模型 + 多渲染器（table/jsonl/csv）分离。
  2. **v6 DSL 重写**（2023 起）：语句/函数/subroutine/`emit`/`tee`/`dump`，带类型系统——"管道内嵌 DSL"的完整先例。
  3. **流式边界披露**：多数 verb 单记录 O(1)（只滞后个别记录），sort/join/histogram 类天然全量——"哪些动词破坏流式"被写进文档心智模型。⇒ ADV 管线动词逐个标注 O(1)/O(n)。
- **档位**：【吸收】（记录流架构 + 流式分级披露 + DSL 命名空间划分）。
- **第一步动作**：adv pipeline 内部记录模型照此分层；管线动词表补"流式/全量"两档标注列。
- **链接**：https://miller.readthedocs.io/ · https://github.com/johnkerl/miller

### 1.7 qsv —— xsv 活跃续作：索引 + 数据并行 + 列式下沉
- **定位**：BurntSushi/xsv 停更后的活跃续作（dathere 维护）；100+ 子命令（README 口径）；MIT（继承 xsv 双许可口径，见仓库）；2025 高频发版，维护中。
- **可抄机制**：
  1. **index 机制**：`qsv index` 为 CSV 建行偏移索引（.idx），后续命令 O(1) 随机访问、可并行切片（该机制 xsv 时代即有：HN 原话 "a very simple index that permits random access… Parallelism is used when possible!"）。⇒ ADV 大报告可同构：扫描落 JSONL + 可选字节偏移索引，query 类命令并行切片。
  2. **Rayon 数据并行贯穿**：命令级 `--jobs`，行级分片并行——"命令默认并行、可关"的用户模型比"用户自己拼 parallel"省心。
  3. **列式下沉**：polars 系子命令（joinp 等）把 join/聚合交给列式引擎（lazy 求值，超内存工作负载）——重型聚合不写在核心 DSL 里，下沉到专用引擎。
- **档位**：【吸收】（.idx 思想 + 默认并行模型 + "聚合下沉"分层）。
- **第一步动作**：adv report `--index` 试点：JSONL 行偏移索引 + adv query 并行扫描；基准用 ADV 真实报告样例测（先量后改）。
- **链接**：https://github.com/dathere/qsv · https://qsv.dathere.com

### 1.8 ripgrep-all（rga）—— 外部工具桥接的完整样本
- **定位**：给 ripgrep 加"任意文件"前置层：PDF/DOCX/电子书/压缩包（嵌套）/字幕/OCR；crates.io：AGPL-3.0-or-later ⇒ **只抄思想**；0.10.9（约 2024，Rust 2024 edition），低频维护中。
- **可抄机制**：① **adapter 注册表**：按文件类型选适配器 → 调外部二进制（pdftotext/pandoc 等；慢适配器如 tesseract OCR 默认关闭，`--rga-adapters=+tesseract` 开）→ 抽取纯文本交 rg 搜；压缩包嵌套递归；② **抽取结果默认缓存**（对比：pdfgrep 需显式 `--cache`，ruscur 博客口径），新版配置走 `~/.config/ripgrep-all/config.jsonc`；③ **虚拟路径**：zip 内文件以 `archive.zip/inner/path` 呈现并可回指原文件。
- **档位**：【吸收】（思想）——展开见 §3.5。
- **第一步动作**：adv-bin 归档扫描设计稿按"探测→注册表→外部抽取进程→内容寻址缓存→虚拟路径"五步自研；配 `adv doctor` 检查外部依赖矩阵。
- **链接**：https://github.com/phiresky/ripgrep-all · https://crates.io/crates/ripgrep-all

### 1.9 moreutils（sponge / ts）—— 管道补件的两个小机制
- **定位**：Unix 管道边角补件集；GPL 系列（逐文件许可），低频维护中。
- **可抄机制**：① **sponge**：读毕全部输入再写目标文件——根治 `sort f > f` 类"边读边写自截断"；② **ts**：给每行打时间戳（-s 相对 / -i 增量）——进度/日志的可复现观测件。
- **档位**：【吸收】（sponge 语义进 adv report `--inplace`：读毕再写或临时文件+rename 原子替换）。
- **第一步动作**：写进 §3.4 规范清单第 8 条，实现时二选一（全量缓冲 vs 临时+rename）按文件大小分档。
- **链接**：https://joeyh.name/code/moreutils/

### 1.10 pv —— 管道观测与"统计走 stderr"契约
- **定位**：管道速率/进度监控（透明转发 + 速率/字节/行数/`-s` 总量进度/`-L` 限速）；GPLv3+；维护中（1.9 系列，2024–2025）。
- **可抄机制**：插进管道零侵扰——**数据走 stdout、统计走 stderr** 的契约样本；进度条绝不污染下游数据流。
- **档位**：【有界】（契约借鉴；ADV 自研进度件，adv batch `--progress` 输出强制 stderr）。
- **第一步动作**：写进规范清单第 2 条。
- **链接**：https://www.ivarch.com/programs/pv.shtml

### 1.11 sd —— "只做替换"的 sed 替代
- **定位**：sed 的查找替换专用化；chmln/sd；MIT；本次检索未见 2025 新发版信号 ⇒ 维护缓慢。
- **可抄机制**：整文档 find&replace（无 sed 的行模型）→ 天然 multiline；regex crate 语法贴近 Rust 生态；`-p` 预览 / `-w` 写回分离。
- **档位**：【reference】（ADV 批量替换自研；"预览/写回分离"的用户模型可参考）。
- **链接**：https://github.com/chmln/sd

### 1.12 fq —— jq 语法走二进制格式
- **定位**：二进制格式的 jq（README 自报 250+ 格式解码器，Go）；MIT；wader/fq 维护中。
- **可抄机制**：位/字节级解码树 + jq 式查询 + 格式互转（to_json/to_bytes）；"这是什么文件"探测与内容抽取一体。
- **档位**：【watch】（adv-bin 二进制资产探测的可选外部兜底，不进核心）。
- **第一步动作**：不动作；adv-bin 若引入外部探测兜底，fq 入候选名单。
- **链接**：https://github.com/wader/fq

### 1.13 gron —— 结构查询降维成文本查询
- **定位**：JSON → 行式赋值（`json.a.b = "x"`）→ grep/正则 → `gron -u` 回编译；tomnomnom/gron；MIT；低频维护。
- **可抄机制**：把"结构查询"降维成"文本查询"以复用整条 Unix 文本管线——ADV 的 JSONL 每行自含 `path` 字段，即同一思想的内化（不需要外部 gron 步骤）。
- **档位**：【reference】。
- **链接**：https://github.com/tomnomnom/gron

### 1.14 dasel —— 多格式统一选择/改写（v3 重写）
- **定位**：JSON/TOML/YAML/XML/INI/HCL/KDL/CSV 统一表达式选择+改写；TomWright/dasel；MIT；**v3 于 2025-12 正式发布**（官方文档 daseldocs.tomwright.me；v3.11.0）。
- **可抄机制**：v3 把"在表达式内读/解析文件"做成一等能力；跨格式统一表达式语法。
- **档位**：【watch】（v3 刚落正式版，等生态稳定；yq 已覆盖 ADV 当前需求）。
- **链接**：https://daseldocs.tomwright.me/ · https://github.com/TomWright/dasel

---

## 2. 扫视（sweep：一句机制 + 档位 + 链接）

| 对象 | 一句机制 | 档位 | 链接 |
|---|---|---|---|
| csvkit | Python CSV 套件（csvsql 走 sqlite）；"每命令一进程 + 解释器启动"在批量场景开销大，作对照 | 【不吸收】 | github.com/wireservice/csvkit |
| xsv | 原版 Rust CSV 工具箱，已停更（qsv 接棒），index 机制由 qsv 续 | 【不吸收】 | github.com/BurntSushi/xsv |
| jql | Rust JSON 选择器，管道/交互两用 | 【watch】 | github.com/yamafaktory/jql |
| fx | Go TUI JSON 浏览器 + JS/jq 表达式，2025 活跃 | 【watch】 | github.com/antonmedv/fx |
| jless | Rust JSON 交互分页器（折叠/展开/结构导航），半维护 | 【watch】 | github.com/jless-org/jless |
| choose | Rust cut/awk 字段选择（负索引/范围/正则切分） | 【reference】 | crates.io/crates/choose |
| numbat | 物理单位类型化的计算语言（sharkdp 出品），CLI REPL | 【reference】 | github.com/sharkdp/numbat |
| trdsql | SQL over CSV/JSON/LTSV → 多格式输出 | 【watch】 | github.com/noborus/trdsql |
| duckdb CLI | 单文件列式引擎，原生读 parquet/csv/json；jq 管线的重型 SQL 档 | 【watch】 | duckdb.org |
| protoc 插件握手 | `protoc-gen-*` 子进程 + CodeGeneratorRequest/Response 走 stdin/stdout——"外部工具桥接"的协议化样本（与 §3.5 同族） | 【reference】 | github.com/protocolbuffers/protobuf |
| ast-grep（sg，CLI 性能角） | 主体归 D1；此处只注：sgconfig.yml 项目级规则 + tree-sitter 解析 + Rust 并行遍历，`--json` 机器输出 | 【reference】 | github.com/ast-grep/ast-grep |
| just | justfile 配方 + 依赖 DAG + 每配方 shebang；对照 D4 略 | 【reference】 | github.com/casey/just |

**查证排除**（防灌水）：intermodal 实为 torrent CLI、runpod 为 GPU 云平台 CLI——与本域"进程编排"无关，不收录。

---

## 3. 重点回答

### 3.1 GNU parallel vs xargs -P：调度/保序机制差异（adv batch 直接借鉴）
| 维度 | GNU parallel | xargs -P |
|---|---|---|
| 输出 | 默认 `--group` 每作业整体缓冲、原子吐出；`--line-buffer` 行级实时；`--ungroup` 零缓冲可交叉；`--keep-order` 只约束**打印次序**（执行仍并发） | 无任何输出管理，子进程共享 stdout 任意交叉；保序只能"每作业重定向文件"手工拼 |
| 失败 | `--halt now/soon,fail=X|Y%` 分级；退出码=失败作业计数（0 全成）；`--joblog` 全明细 | 无分级；任一子进程 1–125 ⇒ 整体 123；无台账 |
| 续跑 | `--resume` / `--resume-failed` / `--retry-failed`（基于 joblog seq） | 无 |
| 资源 | `--memfree/--load/--noswap/--delay auto/--limit` | 无 |
| 分块 | `--pipe/--pipepart --block --recstart/--recend`（记录边界对齐，支持二进制） | 只能按 `-n/-L` 计数分发，无法处理"块内记录边界" |

**ADV 结论**：并发执行与输出呈现必须解耦成两个开关。建议 adv batch 三档：`group`（默认，每任务整体缓冲后原子写）、`keep-order`（按任务序释放，确定性换尾延迟）、`line-buffer`（长流任务实时）；台账用 JSONL（含 start/duration/exit/signal/command + 任务参数哈希），配 `--resume-failed`；失败策略默认 never、CI 场景 `--halt soon,fail=5%` 类可配；退出码 0=全成 / 1=有失败（计数走 summary）/ 2=自身错误 / 130=中断——并注意扫描领域"1=有发现"的惯例与"1=有失败"冲突，**batch 类与 query 类子命令各用各的退出码表，不许混用**。

### 3.2 jq 的流式 vs 全量解析（对 ADV 大 JSON 报告的启示）
- jq 默认整文档物化（值树全进内存）；`--stream` 改为 `[path, leaf]` 事件流，理论上可处理超大文件（SO 有 100+GB 问答），但**过滤器生态几乎全部假定整树语义**，上流式基本等于重写过滤器，且 jq→JSONL 内存密集有 issue 存档（#1897）、深度限制绕行 #2846。
- 启示：① **别把"能流式"当默认承诺**——ADV 的 `--stream` 型接口要同时给出"哪些过滤器可用"的边界；② **JSONL 一等公民**：报告逐行落盘，天然流式、天然并行切片、天然 diff；③ 单个大对象 JSON 进 adv query 时用 serde_json StreamDeserializer 做 tokenizer 流式拉取，内存 O(栈深) 而非 O(文档)；④ 阈值判据留 TODO 锚：全量/流式切换点（MB 量级与内存预算）须用 ADV 真实报告样例实测后回填，先量后改。

### 3.3 miller/qsv 的"列式思维"对扫描结果表输出的价值
- **记录流架构**（miller）：内部记录模型与渲染器分离 ⇒ `adv report --to table|csv|jsonl` 是同一份数据的三个编解码器，不是三条代码路径。
- **索引 + 并行切片**（qsv .idx）：finding 表 schema 固定 ⇒ 扫描时顺手写行偏移索引，query 类命令（`adv query --where severity>high`）可 O(1) 随机访问 + Rayon 分片并行，避免每次全量扫。
- **聚合下沉**（qsv polars 系）：超内存的 group-by/join 不进核心 DSL，下沉列式引擎（polars/duckdb 档）——ADV 核心只保证"记录流 + 常用过滤"轻快，重型分析走可选后端。
- 对照组 csvkit：Python 每命令一进程的启动/单线程开销证明——批量管线要的是**一个进程内多阶段**（miller 的 `then` 链 / qsv 的子命令族），不是 shell 循环里反复冷启动。

### 3.4 管道工具组合契约 —— ADV CLI 接口设计规范清单
1. **退出码表**：0=成功；1=query 类=有命中 / batch 类=有失败；2=用法/环境错误；126/127 保留（不可执行/不存在）；130=SIGINT。两类子命令分开定义并在 --help 声明。
2. **数据/日志分流**：数据只走 stdout；日志、进度、统计全部 stderr（pv 契约）；`--quiet` 只压 stderr。
3. **stdin 守卫**：需要输入而 stdin 是 tty ⇒ 显式报错退出，不静默挂起；`-` 显式表示 stdin。
4. **二进制安全**：机读路径接口默认 NUL（`-0`）或 JSONL（任意字节经 JSON 转义）；人类 table 仅显示档；Windows 下输出强制 UTF-8（不落控制台 ANSI 编码）。
5. **JSONL 优先**：机器互换格式 = JSONL；每行自含 `path`/定位字段（gron 思想内化）；末行 `{"summary": ...}` 供管道收尾与 CI 断言。
6. **流式档位披露**：每个子命令 --help 标注 O(1) 流式 / O(n) 全量（miller 的动词披露心智模型）。
7. **输出确定性**：排序稳定、`--freeze` 冻结时间戳，同输入同字节输出（diff/测试可复现）。
8. **自写文件防截断**：`--inplace` 必须"读毕再写"或临时文件+rename 原子替换（sponge 语义），按文件大小分档实现。
9. **并发参数化**：`--jobs N`（默认=核数，IO 密集建议显式降档）、可选 `--keep-order`、`--halt {never|fail=N|fail=P%}`、`--timeout`、`--retries`、`--joblog` + `--resume-failed`。
10. **SIGPIPE/EPIPE**：下游关闭（`| head`）时快速静默退出——Rust 默认忽略 SIGPIPE，`println!` 会 panic，须统一处理 EPIPE。
11. **颜色/TTY**：`--color=auto` 默认 + 尊重 `NO_COLOR`；机读格式下永不出 ANSI 码。
12. **外部依赖显式化**：凡桥接外部二进制（§3.5），版本探测走 `adv doctor`，缺失时降级 skip 并在 summary 记录，不静默失败。
13. **`--version`/`--help` 机器可读**：`--version` 单行；`--help` 结构稳定（脚本可解析）。

### 3.5 ripgrep-all 的"外部工具桥接"模式 → adv-bin / 归档扫描可否复用
- **模式五步**（从 rga 抽象）：① 类型探测（扩展名+魔法数）→ ② adapter 注册表（类型→外部抽取命令映射，慢适配器默认关）→ ③ 子进程抽取纯文本 → ④ **内容寻址缓存**（键含路径+大小+mtime/内容哈希，二次扫描免抽取）→ ⑤ **虚拟路径**（`归档/内部路径` 回指原文件，让下游引擎无感）。
- **复用可行性**：是——adv-bin 归档扫描（zip/7z/tar 内源码/文档）可桥 unzip/7z/pdftotext/tesseract 等；同族先例还有 protoc 插件握手（stdin/stdout 传结构化请求/响应），说明该模式可做成**协议**而非一次性胶水。
- **约束与风险**：rga 为 AGPL-3.0-or-later ⇒ 只抄思想、代码零接触；外部依赖矩阵存在版本漂移/缺失风险 ⇒ 优雅降级（skip+记录）+ `adv doctor` 体检；缓存失效要保守（宁可重抽，不可陈旧命中——安全工具的正确性优先于扫描速度）。
- **ADV 落点**：`adv-bin` 归档桥接层自研五步；缓存键与失效策略先写 RFC 再实现（第一性：安全工具缓存错 > 慢）。

---

## 4. Top-3（对 ADV 直接可抄）
1. **GNU parallel 的批处理语义四件套**（JSONL 台账 + resume-failed + halt 分级 + 输出三档释放）→ `adv batch` 骨架。
2. **JSONL-first + 流式/全量显式分级**（jq 的全量教训、miller 的动词披露、qsv 的 .idx 并行切片）→ `adv report/query` 数据层三件：记录流渲染器分离、可选字节索引、流式 tokenizer。
3. **rga 式外部工具桥接五步**（探测→注册表→外部抽取→内容寻址缓存→虚拟路径）→ `adv-bin` 归档扫描（思想自研，AGPL 不抄码）。

## 5. 2025–2026 前沿信号
- **jq 复活**：1.8.0（2025-06-01）→ 1.8.1 → 1.8.2（安全修复 + Windows arm64/Docker arm 构建），jqlang.org 口径——jq 不是弃维护对象，兼容基准应以 1.8 为准。
- **jaq 3.x**：多格式化（JSON/YAML/TOML）+ 持续发版（v3.1.1，2025-08-05）；"过滤表达式引擎作为可嵌入库"路线被验证。
- **yq v4.50.1**（2025-12-13，TOML roundtrip）；4.45.3 的 `//` 回归（issue #2377）提醒：表达式 DSL 小版本也会破坏语义 ⇒ ADV 内嵌 DSL 必须锁版本 + 回归样例。
- **dasel v3**（2025-12 正式发布）：多格式统一表达式进入新一轮竞争。
- **Miller 6.22.0** 在线文档，6.x 线持续；**qsv** 2025 高频发版、polars 系子命令扩张——CSV 工具箱向"列式下沉"演进。
- **rga** 0.10.9（约 2024，Rust 2024 edition）+ config.jsonc——桥接层配置化。
- **生态横向**：jq 替代品密度上升（gojq/zq/fx/jsongrep/jaq…，HN/Reddit 多帖），"引擎可插拔、格式互通（TOML/YAML/JSON 一套表达式）"成为 2025 共识方向——ADV 的 adv query 设计应假定表达式引擎将来会换/会多。
