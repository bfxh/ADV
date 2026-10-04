# X8 错误处理 / 日志 / 诊断生态调研

- 日期：2026-10-04。版本/日期锚：crates.io API 本日查询结果（crates.io/api/v1/crates?ids[]=…）；issue/讨论带各自时间。
- 上游已定决议（本文只补差量，不重开）：**E1** = 诊断渲染主选 annotate-snippets（rustc 系）；**D8/wave-0** = tracing 生态；**C4** = indicatif/console（本文略）；**C3-005** = release 档 `panic="abort"`（本文给出冲突调和）；**C2** = CLI 基础（本文补退出码）。
- 条目编号 X8-01…X8-22，与 `registry/X8.jsonl` 一一对应。

---

## 0. Top-3

1. **X8-01/06 错误三面分层 + 表驱动错误码**：库 crate 用 thiserror 具体错误（带 ADV-E#### 码），CLI 面独立渲染层，MCP 面映射 JSON-RPC 错误对象；三面共享同一张 const 码表，xtask-gates 加"码表校验门"防跳号/重码/文档漂移。
2. **X8-07 panic 隔离判据（对 C3-005 的调和方案）**：`catch_unwind` 与 `panic="abort"` 互斥，且 Cargo 不允许 per-package 覆盖 panic 设置——给出 L1（全家 unwind + 线程 catch_unwind）/ L2（不可信输入下沉 worker 进程池）双档判据，C3-005 的 abort 档先量体积再定。
3. **X8-05 annotate-snippets 仓库落点确认（补 E1 复核项）**：crates.io 显示仓库已迁至 `rust-lang/annotate-snippets-rs`（E1 当日查 `rust-lang/annotate-snippets` 404 的疑点解除）；0.12.16（2026-05-06 更新）+ cargo#15944 迁移中，rustc 系选型证据加强，miette 维持不吸收。

---

## 1. 五问定案

### ① ADV 错误分层最小规范（三面）

| 面 | 归属 crate | 错误载体 | 规则 |
|---|---|---|---|
| 库面（adv-core / adv-* 解析器） | 库 crate | **thiserror** 派生的具体错误枚举 | 每变体带错误码（ADV-E####，来自共享码表）；`#[source]` 保因果链；pub API 签名禁用 anyhow::Error；只 emit 不渲染 |
| CLI 面（adv-cli / xtask） | bin | anyhow（胶水）+ **独立渲染层** | 渲染层接收 `&dyn Error`（downcast 取码表分派）；span 类诊断交 annotate-snippets；人读走 stderr，`--json` 时结果对象走 stdout、错误仍走 stderr（rg 惯例） |
| MCP 面（adv-server MCP） | server | **JSON-RPC 2.0 error 对象** | `{code, message, data:{adv_code, severity, file, span, help}}`；标准码 -32700/-32600/-32601/-32602/-32603，自定义码落 -32000…-32099（JSON-RPC 2.0 规范的 server-error 区间） |

- 三面共享一张码表：`const CODES: &[CodeEntry]`（见 X8-06），渲染层与 MCP 序列化都是码表的投影——E1 的"结构化是本体、渲染是投影"结论在错误面同样成立。
- 最小规范一句话：**库面管"是什么错"（类型+码+source），CLI/MCP 面管"怎么呈现"（文本/JSON 投影），两层靠错误码解耦。**

### ② miette vs annotate-snippets 选型裁决：**维持 E1，选 annotate-snippets**

补证据（E1 原判之上新增，2026-10-04）：
- **维护活性**：annotate-snippets 0.12.16，crates.io updated_at **2026-05-06**，rust-lang 官方组织；miette 7.6.0，updated_at **2025-04-27**——到今天约 17 个月未发版（对长期依赖是减分项，非判死刑）。
- **官方生态背书**：rust-lang/cargo#15944（2025-09）cargo 正在向 annotate-snippets 迁移，理由是"与 rustc 输出一致"——rustc 观感对齐有了官方担保；约 1336 个 crate 依赖它。
- **架构契合度**：miette 是"错误定义框架 + 渲染 + 协议"全家桶（Diagnostic trait、ReportHandler、JSON/Narratable handler），会与本文 X8-01 的 thiserror 分层形成**两套错误抽象**；annotate-snippets 只做渲染，恰好填 ADV 分层里"渲染层"这一格，不抢库面的错误建模。
- **E1 遗留复核项解除**：crates.io 仓库字段 = `github.com/rust-lang/annotate-snippets-rs`（旧路径 404 是改名所致）。
- **不吸收 miette 但留一个参考**：其 Diagnostic trait 字段集（severity / code / labels / help / url）与 rustc JSON 诊断模型同构，是 ADV 自有诊断 schema 的现成参考（verdict: reference）。

### ③ 单文件 panic 隔离判据（建议档）

判据三问（按序短路）：
1. **panic 源是否纯 Rust（无 FFI/unsafe）？** 否 → 必须进程隔离：catch_unwind 捕不了段错误与栈溢出（栈溢出是 abort-only panic，internals 讨论未落地），worker 进程 + 崩溃检测重启是唯一可靠路径。
2. **`panic="abort"` 是否生效？** 是 → catch_unwind 全部失效（unwinding 根本不发生），只能进程隔离。C3-005 已把 abort 吸收进 release 档 ⇒ **当前 C3 决议下线程级隔离不可用**，这是本文与 C3 的正面冲突点，调和见下。
3. **worker 与主进程有无共享可变状态？** 无（每文件独立输入）→ 线程 + catch_unwind 成本最低：线程 spawn µs 级、Windows CreateProcess 通常 ms 级（量级差 ~10³；精确值按 C1 基准法在 ADV 本机回填实测，勿引用他机数字）。有共享 Mutex → 还要处理 poisoning（catch_unwind 不清 poison，语义上反而对齐）。

对 C3-005 的调和方案（三选一，判据锚定）：
- **方案 A（推荐默认）**：release 档**暂缓 abort**（保持 unwind），线程池 + catch_unwind 隔离单文件，panic hook 落盘后该文件记失败继续扫描。判据：C3-005 实测 abort 体积收益若 <2–3%，A 胜出。
- **方案 B**：主 exe 保持 abort（C3-005 全额兑现）+ `adv-worker` 独立构建为 unwind。实现注意：Cargo 的 profile.package override **不允许设置 panic**（仅 test/bench profile 自动豁免保持 unwind），所以 worker 必须独立 cargo 构建/独立 workspace（xtask 挂构建任务）。判据：abort 收益 ≥5% 且本来就计划 worker 进程池时取 B。
- **方案 C**：全 abort + 全进程隔离（无 catch_unwind）。实现最简、语义最硬，但 spawn 成本进热路径——只在 worker 常驻进程池（请求-响应+超时 kill+重启）摊薄成本后可接受。
- **DoublePanic 补刀**：catch_unwind 回调内再 panic、或 unwind 途中 Drop 再 panic → 直接 abort，两种方案都拦不住；栈深风险用 stacker（X8-22）+ 线程 stack_size 预防，而非事后捕获。

### ④ ADV 自身日志格式与轮转规范

- **两码事原则**：扫描结果报告（机器消费，stdout JSON schema）≠ ADV 自身运行日志（人/排障消费，stderr 或文件）。本文只定后者。
- **格式**：stderr 默认**人读文本**（带 ANSI，TUI 下关闭）；文件日志 `ADV_LOG_FORMAT=json` 切 **JSONL**（一行一事件——`tracing_subscriber` `fmt().json()` 的天然产出，崩溃不损行、可 jq/rg 流式处理）。字段最小集：`timestamp / level / target / span / message / fields`；`flatten_event(true)` 把 message 提到顶层，`with_current_span(true)` 带上下文。锚：tracing-subscriber fmt json 格式文档。
- **轮转**：`tracing-appender` 0.2.5 `Rotation::DAILY`（`adv-YYYYMMDD.log` 前缀模式）+ `non_blocking` 包一层防日志 IO 拖慢扫描热路径。两个已知坑：① non_blocking 有界 channel **默认丢事件**（lossy）换吞吐，可用 builder 改阻塞，扫描器建议保持 lossy + 关键事件升级别；② 必须持 `WorkerGuard` 且让它在 main 生命周期末 drop，否则退出时丢尾部日志。
- **保留策略（appender 没有，自己补）**：tracing-appender 只有时间轮转，无 size 轮转/无压缩/无清理（tokio-rs/tracing#1940，2022-02 开至今）。本地优先规范：日志落 `%LOCALAPPDATA%/adv/logs`，默认保留 7 天、总量上限 ~200MB，**启动时按文件名日期前缀清理最旧档**（约 20 行，无需依赖）；未来需要 size/压缩时换 file-rotate / tracing-rotate / logroller 作 `MakeWriter`，或 flexi_logger（X8-18）。
- 库面纪律见 X8-14（库只 emit 不 init），兼容桥见 X8-12。

### ⑤ CLI 退出码契约表（含 Windows 差异）

| 码 | 语义 | 备注 |
|---|---|---|
| 0 | 成功（含"无超阈值发现"） | |
| 1 | 有超阈值发现（CI fail 语义） | 学 grep：把"发现"与"错误"分开，CI 只看码不需解析输出 |
| 2 | 用法/参数错误 | clap v4 参数解析失败默认即 2 |
| 3 | 运行期错误（IO/内部） | 渲染 anyhow 链 + 提示 `--log-filter` 调试 |
| 4 | 配置错误 | sysexits EX_CONFIG 思想压进小码表（sysexits 64–78 是"广泛被忽略的惯例"，4.3BSD 起源，不照搬） |
| 130 | Ctrl+C（128+SIGINT(2)，bash 惯例） | Windows 原生 Ctrl+C 默认进程以 0xC000013A（STATUS_CONTROL_C_EXIT）终结，需自行装 ctrl-c handler 才能给 130——跨平台 CI 别依赖 130 精确值 |

Windows 侧差异与避雷：
- `%ERRORLEVEL%` = `GetExitCodeProcess` 低 32 位；cmd 的 `if errorlevel N` 是 **≥N** 判断，`if %ERRORLEVEL%==N` 才是等值；PowerShell 用 `$LASTEXITCODE`，bash 用 `$?`。
- **避开 259**（STILL_ACTIVE/STATUS_PENDING）：轮询 `GetExitCodeProcess` 的调用方会永远认为进程"还在跑"（微软文档明确要求应用别用该值退出）；另避安装器惯用码 3010/1605 与负值（0xC0000xxx HRESULT 系）。
- **码表全部 ≤255**：POSIX 侧 `$?` 按 8 位截断（mod 256），>255 的 32 位码经 bash/MSYS 观察即漂移；≤255 是 bash 与 cmd/PowerShell 的公共安全区。
- 锚：rustc/cargo 内部错误 101 惯例可参考（ADV 暂不引入，码表留洞即可）。

---

## 2. 机制级条目

格式：定位 → 可抄机制 1–3 → 档位 → 第一步动作 → 许可证/成熟度 → 链接。

### X8-01 thiserror（库面结构化错误）
- **定位**：derive 宏生成 `impl Error/Display/From/Source`；错误类型是库 pub API 的一部分，调用方要能 match——库面用它、应用面用 anyhow 的社区分工已成默认惯例（thiserror→libraries, anyhow→applications；dtolnay 文档原话）。
- **可抄机制**：① `#[derive(Error)] + #[error("…")]` 每变体自带显示文本；② `#[from]/#[source]` 建因果链，`?` 自动转换；③ `#[error(transparent)]` 透传底层；④ 错误码即变体字段（`#[error("ADV-E{:04}…", self.code())]` 或 Display 委托码表）。
- **档位**：**吸收**（adv-core/adv-* 库面）。**第一步**：adv-core 建 `error.rs` + 共享码表模块，MCP/CLI 两面改为查表。
- **许可证/成熟度**：MIT OR Apache-2.0（dtolnay）；2.x 系，生态事实标准。
- **链接**：https://docs.rs/thiserror

### X8-02 anyhow（应用面动态错误）
- **定位**：type-erased `Error` + context 链，为"只报告、不 match"的 bin/胶水层省掉错误类型样板；anyhow 文档明确不建议进库。
- **可抄机制**：① `.context()/with_context()` 在边界补人类语义（含文件路径/参数名）；② `downcast_ref` 按需取回具体错误做特判（如重试类）；③ Debug 打印即整条 "Caused by" 链——渲染层可以直接吃。
- **档位**：**有界**（仅 adv-cli/xtask 的 bin 层；库 crate pub 签名禁入）。**第一步**：adv-cli `main -> Result<(), anyhow::Error>` + 顶层渲染函数。
- **许可证/成熟度**：MIT OR Apache-2.0；1.x 系，事实标准。
- **链接**：https://docs.rs/anyhow

### X8-03 std::error::Error 的 source 链工程
- **定位**：`source()` 表达"谁导致谁"，每层 Display 只说本层发生了什么；这是三面共享错误语义的 std 底座。
- **可抄机制**：① 渲染层写一个 source 链遍历器，输出 anyhow 同款 "Caused by:" 块；② `downcast_ref` 沿链找带码的错误类型做渲染分派；③ `provide`/error_generic_member_access **仍 nightly**（截至 2026-10 的 stable 版本线），ADV 不依赖，字段自携带。
- **档位**：**吸收**（机制）。**第一步**：渲染层 `render_error_chain(&dyn Error) -> String` + 单测。
- **许可证/成熟度**：std 内建。**链接**：https://doc.rust-lang.org/std/error/trait.Error.html

### X8-05 annotate-snippets（rustc 系渲染，E1 主选）
- **定位**：rustc 诊断渲染器的库化版本；输入 Message/Snippet 树（level/id/primary/secondary 标签、建议 patch），输出带 ANSI 的 rustc 风格报告。cargo 正式迁移中（#15944）。
- **可抄机制**：① `Message::level(Level::Error).id("ADV-E0001")` 挂码；② 多 Snippet + 标签做 span 标注（诊断 schema 的渲染投影）；③ 错误码 id → `adv explain ADV-E0001` 帮助链（rustc `--explain` 同型）。
- **档位**：**吸收**（E1 已定，本文补证据维持）。**第一步**：E1 的第一步不变；仓库链接按 `rust-lang/annotate-snippets-rs` 更新。
- **许可证/成熟度**：MIT OR Apache-2.0；rust-lang 官方维护，0.12.16（2026-05-06）。
- **链接**：https://docs.rs/annotate-snippets · https://github.com/rust-lang/cargo/issues/15944

### X8-06 错误码表驱动（rustc E-codes 抄法）
- **定位**：rustc 用有序 `DiagnosticMessages` 表管理 E0308 类错误码，配套 stale 检查防"码洞"；CLI 错误码一旦发布就是契约（文档、help 链接、CI 断言都依赖），必须表驱动+门禁。
- **可抄机制**：① `const CODES: &[CodeEntry]`（u16 升序 + 描述 + help 段落）+ 二分查找 `Code::explain()`；② xtask-gates 新门：唯一性、升序、跳号须显式 reserved 登记、`codes.md` 文档与表 diff 为零、每码 help URL 可达；③ 码分配规则写进表注释（段落预留：0xxx 解析、1xxx 规则、2xxx IO/配置…）。
- **档位**：**吸收**。**第一步**：xtask-gates 加 `code-registry` 门（先 10 个码跑通）。
- **许可证/成熟度**：机制移植自 rustc（MIT OR Apache-2.0）。
- **链接**：https://github.com/rust-lang/rust/tree/master/compiler/rustc_errors

### X8-07 panic 隔离（判据见 §1③）
- **定位**：扫描器对单文件 panic 的隔离决策树 + 与 C3-005 abort 的调和三案。
- **可抄机制**：① `std::panic::catch_unwind(AssertUnwindSafe(…))` + `panic::update_hook` 包默认 hook（先落盘再交还默认行为）；② worker 常驻进程池：请求-响应 + 超时 kill + 异常退出码（含 0xC0000005 系）触发重启；③ stacker 深递归兜底（X8-22）。
- **档位**：**吸收**（判据表 + 方案 A 默认，C3 回填体积数据后终裁）。**第一步**：方案 A 原型——扫描循环内 scoped thread + catch_unwind + 人为 panic 用例。
- **许可证/成熟度**：std 内建；进程池为自有机制。
- **链接**：https://doc.rust-lang.org/std/panic/fn.catch_unwind.html · https://internals.rust-lang.org/t/floating-the-idea-of-allowing-certain-abort-only-panics-to-be-unwindable（2020-10 讨论，栈溢出 abort-only）

### X8-08 退出码契约（表见 §1⑤）
- **定位**：CLI 退出码是 shell/CI 的唯一稳定接口；表驱动 + 避雷清单（259/负值/>255）。
- **可抄机制**：① `enum ExitCode` + `From` 到 `i32` 单点出口（禁散落 `process::exit`）；② clap 用法错误对齐默认 2；③ CI 判据只依赖 0/1/2/3/4，130 仅记录不承诺 Windows 精确值。
- **档位**：**吸收**。**第一步**：adv-cli 出口收敛 + 契约表进 `--help` 后的 man/doc。
- **许可证/成熟度**：自有机制；sysexits/STILL_ACTIVE 为公开惯例。
- **链接**：https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getexitcodeprocess

### X8-12 tracing-log 兼容桥（log → tracing）
- **定位**：依赖树里 `log` 门面 crate 的记录要进 tracing 订阅者，否则静默丢失；`fmt::init()` 自带桥，**自定义组装 subscriber 时需手动** `tracing_log::LogTracer::init()`。
- **可抄机制**：① 组装函数里 `LogTracer::try_init()`（重复 init 会 panic，用 try）；② 自测：log-only 依赖的事件在 JSONL 里可见；③ 评估依赖树 log 用量（`cargo tree`）决定是否值得。
- **档位**：**吸收**。**第一步**：subscriber 组装处补桥 + 一条 log! 事件可见性测试。
- **许可证/成熟度**：MIT OR Apache-2.0；tracing-log 0.2.0（2023-10-25 后未发版——桥是冻结功能面，无 churn 即稳定）。
- **链接**：https://docs.rs/tracing-log

### X8-13 EnvFilter 语法
- **定位**：`RUST_LOG` 风格指令 `target[span{field}]=level`，逗号分隔，按 target 前缀最长匹配，余量落 fallback——运行期调 Verbosity 的标准件（社区吐槽点也在逗号语义，要写进 --help）。
- **可抄机制**：① `adv=debug,info` 组合默认档；② span/字段过滤 `[scan{file="…"}]` 供深排障；③ `--log-filter` CLI 参数 > `ADV_LOG` env > 内置默认，三层覆盖。
- **档位**：**吸收**。**第一步**：CLI 三层覆盖实现 + help 示例两条。
- **许可证/成熟度**：MIT OR Apache-2.0（tracing-subscriber 组件，活跃）。
- **链接**：https://docs.rs/tracing-subscriber（EnvFilter 模块）

### X8-14 库内 tracing 最佳实践
- **定位**：库 crate 只 emit（宏+`#[instrument]`），**绝不 init** subscriber；级别约定 error/warn=异常路径、info=里程碑、debug=流程、trace=热路径（默认关）。
- **可抄机制**：① `#[instrument(name=…, fields(…))]` 关键函数带 span，JSONL 事件自动带上下文；② 只 dep `tracing` 本体，订阅者留在 bin；③ `NoSubscriber` 下近乎零成本，热路径再用 `max_level_off` 类编译期特性收紧。
- **档位**：**吸收**（D8 差量补全）。**第一步**：adv-core 热路径级别审计（trace/info 降档）。
- **许可证/成熟度**：MIT OR Apache-2.0；tokio-rs/tracing 活跃维护。
- **链接**：https://docs.rs/tracing

### X8-16 JSONL 日志格式定案
- **定位**：JSONL=一行一 JSON，流式可解析、崩溃不损行，是"结构化日志"在本地文件上的正确形态；整块 JSON 数组/多行 pretty 都不算。取舍：人读默认仍文本，JSONL 是文件格式与 `--json` 报告的底座。
- **可抄机制**：① `fmt().json().flatten_event(true).with_current_span(true).with_span_list(false)`（省体积）；② 字段 schema 固定进文档（ts/level/target/span/msg/fields），version 字段留升级余地；③ 日志与报告分流：stdout=报告，stderr+文件=日志。
- **档位**：**吸收**。**第一步**：定 schema 文档 30 行 + golden 单测（一条事件序列化样例）。
- **许可证/成熟度**：tracing-subscriber 内置能力。
- **链接**：https://docs.rs/tracing-subscriber/tracing_subscriber/fmt/format/index.html

### X8-17 tracing-appender 轮转
- **定位**：时间轮转（DAILY/HOURLY/MINUTELY/NEVER）+ non_blocking 写手；**无** size 轮转/压缩/保留（#1940）——轮转规范要按"时间轮转 + 自补清理"设计。
- **可抄机制**：① `rolling::daily(logs_dir, "adv-{}.log")` 前缀模式；② `non_blocking(writer)` + `WorkerGuard` 生命周期管理（见 §1④ 两坑）；③ 启动清理：按文件名日期 >7 天或总量 >200MB 删最旧。
- **档位**：**有界**（日轮转+清理吸收；size/压缩需求出现时再评估 file-rotate/tracing-rotate）。**第一步**：logs 目录 + 三件套（appender/guard/cleaner）+ 单测。
- **许可证/成熟度**：MIT OR Apache-2.0；0.2.5（2026-04-17 更新，tokio-rs/tracing）。
- **链接**：https://docs.rs/tracing-appender · https://github.com/tokio-rs/tracing/issues/1940

### X8-20 MCP 面错误对象（JSON-RPC 2.0）
- **定位**：MCP 承载在 JSON-RPC 2.0 上，错误面 = `{code, message, data?}`；标准码五件套（-32700/-32600/-32601/-32602/-32603），-32000…-32099 是实现自定义区间。
- **可抄机制**：① 内部 ADV-E#### 映射：协议层错误用标准码，业务错误统一 -32000 + `data.adv_code`；② `data` 固定 schema：`{adv_code, severity, file, span, help}`（与诊断 schema 同源）；③ `message` 一行人话，细节全进 data。
- **档位**：**吸收**（adv-server）。**第一步**：错误映射函数 + 3 条 golden（用法错/内部错/规则命中）。
- **许可证/成熟度**：JSON-RPC 2.0 规范（公开标准）。
- **链接**：https://www.jsonrpc.org/specification

### X8-21 stdout/stderr 契约
- **定位**：stdout=数据（可管道可重定向，机器消费），stderr=诊断/进度/日志（人消费）；`--json` 只改 stdout 的报告形态，**错误诊断仍走 stderr**（rg/jq 同款惯例，见 C2 域）。
- **可抄机制**：① 进度条/日志全走 stderr（C4 的 indicatif 默认即 stderr）；② `--json` 时 stdout 只出结果对象，一次写入避免半截流；③ 管道自检：stdout 非 TTY 时关 ANSI（`anstream`/console 自动）。
- **档位**：**吸收**。**第一步**：adv-cli 输出路径审计（grep 掉 stdout 上的 eprintln/progress）。
- **许可证/成熟度**：POSIX 惯例 + rg/jq 先例。
- **链接**：https://github.com/BurntSushi/ripgrep（--json 与 stderr 分流先例）

---

## 3. 扫视条（一句机制 + 档位 + 链接）

| # | 条目 | 机制一句 | 档位 | 链接 |
|---|---|---|---|---|
| X8-04 | miette | Diagnostic trait（severity/code/labels/help/url 字段集）+ 可插拔 ReportHandler（Graphical/JSON/Narratable）——字段集抄作 ADV 诊断 schema 参考，框架本身与 thiserror 分层重复 | **不吸收**（参考其 schema 思想） | https://docs.rs/miette |
| X8-09 | eyre / color-eyre | anyhow 同位的 Report 抽象 + 彩色回溯（color-spantrace/section）；eyre 0.6.14（2026-08-11）活跃、color-eyre 0.6.5（2025-05-30） | **不吸收**（与 anyhow 同位，二选一即 anyhow；调试期临时可入 dev-deps） | https://github.com/eyre-rs/eyre |
| X8-10 | codespan-reporting | E1 已列备选：0.13.1 仍发版、输出朴素稳定；annotate-snippets 路径若出问题再启用 | **reference** | https://crates.io/crates/codespan-reporting |
| X8-11 | ariadne | E1 已否决：仓库归档 + 布局启发式导致输出漂移（golden 测试不利） | **不吸收** | https://github.com/zesterer/ariadne |
| X8-15 | fern | `log` 门面老牌 logger（0.7.1，2024-12-15 后未动），功能与 tracing 栈完全重叠 | **不吸收** | https://github.com/daboross/fern |
| X8-18 | flexi_logger | log 门面但 size+age 轮转/清理/压缩齐全（0.31.10，2026-08-07 活跃）——若将来要 size 轮转且有 log 兼容需求时再评估 | **watch** | https://github.com/emabee/flexi_logger |
| X8-19 | tracing-rotate / file-rotate / logroller | 以 `MakeWriter` 身份给 tracing 供给 size/时间轮转与压缩，保住 span 上下文 | **reference**（X8-17 的升级备件） | https://docs.rs/tracing-rotate |
| X8-22 | stacker | 栈增长（grow stack on demand）防深递归/深解析栈溢出——治"捕不了的 abort"靠预防 | **有界**（解析 worker 路径） | https://docs.rs/stacker |

（X8 领域内 indicatif/console 按 C4 决议略，不设新条目。）

---

## 4. 交叉引用与待回填

- 对 **C3-005**：abort 与 catch_unwind 互斥 + Cargo 不支持 per-package panic 覆盖 ⇒ C3-005 定档前先跑方案 A 体积对照（§1③），把"体积收益 % 写回 C3-005 判据"。
- 对 **E1**：仓库路径复核项已解除（`rust-lang/annotate-snippets-rs`），miette 增补两条否决证据（17 个月未发版、架构重复）。
- 对 **C2/xtask**：退出码表 + code-registry 门进 xtask-gates。
- 待回填锚（本机实测）：① Windows 进程/线程 spawn 成本比值；② abort vs unwind 体积差 %；③ non_blocking lossy 在扫描峰值下的丢事件率。
