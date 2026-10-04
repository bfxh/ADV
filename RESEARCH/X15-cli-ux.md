# X15 终端/CLI UX 库调研（ADV W4 · cli 面依赖台账）

- 日期：2026-10-04（检索窗口当日）
- 方法：WebSearch 主题检索 + crates.io API 批量元数据核验（版本 / license / 最近发布日期，均为当日口径）
- 范围：参数解析 / 配置装载与路径 / shell 补全 / 交互提示 / 着色与自适应输出 / 文本宽度与表格 / glob 模式 / Windows 参数坑
- 档位口径：**吸收**（直接进依赖）| **有界**（引入但限定使用边界）| **不吸收**（明确不用）| **watch**（跟踪不动手）| **reference**（仅对照知识）
- 成熟度口径：维护中 = 近一年内有发布或维护活动；低频稳定 = 发布间隔长但域内公认稳定；本文未发现弃维护项（最近发布最早者为 termcolor 2024-01-10，仍属稳定件）
- 共 29 个 crate（13 条机制级 + 16 条扫视），机制级条目按「定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接」展开

---

## Top-3（优先落地）

1. **clap + clap_complete**（4.6.7 / 4.6.11）——ADV CLI 主档：子命令多、help/错误上下文/补全是产品力；编译成本用 features 剪裁缓解（判据与数字见 Q1）。
2. **anstyle / anstream**（1.0.14 / 1.0.0）——C4 三态输出契约的现成机器实现：`anstream::AutoStream` 在非 TTY 时自动剥离 ANSI，把"TTY 才上色"从约定变成类型系统保证（见条目 16/17）。
3. **globset**（0.4.20）——ADV include/exclude 的模式引擎：单集合多 glob 一次性匹配、`literal_separator` 可关闭跨目录匹配的隐雷；语义裁决见 Q4。

紧随其后：unicode-width + comfy-table（CJK 表格对齐，见 Q3）、etcetera（配置路径策略）、wild/dunce（Windows 参数处理规范，见 Q5）。

---

## 五个重点回答

### Q1 clap 的编译成本与替代档（bpaf / lexopt）——选型判据

数字锚（口径不同，勿互比）：
- argparse-rosetta-rs（rosetta-rs，2026-10-04 检索 README）：lexopt 全量构建 ≈37 KiB 二进制 / ≈329 ms 编译，是该横评里最小最快档；clap 在同表属于最大档（数百 KiB 级）。
- HN 讨论（2025-07）：clap 被描述为"≈18k LOC 解析器 + ≈125k LOC 依赖，全量编译 ≈6 s"。
- usage.jdx.dev 性能页：clap debug 增量重建 ≈3.6 s（增量口径，与全量数字不可比）。

选型判据（按 ADV 的 cli 面定）：
1. **子命令规模**：≥5 个子命令或两级嵌套 → derive 生成的 help/错误上下文/`did-you-mean` 收益最大，选 clap；≤2 个平铺子命令 → 轻档才有性价比。
2. **补全需求**：要分发静态补全脚本（bash/zsh/fish/elvish/powershell）→ clap_complete 最成熟；要"参数值级动态补全"（如路径候选）→ bpaf 的动态补全由 parser 结构直接导出，是它独有卖点。
3. **编译时间预算**：若 cli crate 在全量 CI 里成为大头且无缓存缓解 → 降档到 bpaf/lexopt；有 sccache/工作区缓存则 clap 的代价被摊薄。

裁决：**ADV 主档 = clap（derive）+ clap_complete**；bpaf 列 watch（若实测 derive 编译超预算，bpaf 是唯一的"换档不换心智"候选——它同时有 derive 和组合子两套 API）；lexopt/argh 仅 reference。缓解动作：`clap = { default-features = false, features = ["std","help","usage","error-context","suggestions","derive"] }`，把 cli 拆成独立 crate 以隔离缓存失效。

### Q2 配置多源合并的优先级规范（figment 合并模型可抄部分）

figment 机制（docs.rs/figment，0.10.19）：Provider 各自产出 Dict；`Figment::merge()` 把新 provider 加到**高优先端**、`join()` 加到低优先端；取值时按"后加者先查、首个命中即用"。即**优先级 = 调用方的合并顺序，库不内置优先级表**。

ADV 采用的优先级规范（低 → 高）：

```
内建默认(Serialized::defaults) < 基础配置文件(Toml) < profile 文件 < 环境变量(Env::prefixed("ADV_")) < CLI 显式 flag
```

可抄细节：① `Env::prefixed("ADV_").map(|k| k.to_lowercase())` 做环境键映射，`split("_")` 控制嵌套；② profile 文件用 `Toml::file("adv.{PROFILE}.toml").nested()`；③ **CLI 层的坑**：不要把 `Option<T>` 直接喂给 Serialized（会序列化出 `"None"` 字面量污染合并）——CLI 只把"用户显式提供"的键收集成 `Vec<(key, value)>` 走 `Serialized::defaults` 上层合并；④ figment 的 Metadata（每个值记来源）值得抄：诊断时能答"这个值来自哪个源"。

裁决：**有界引 figment 或先自实现薄层**——若 ADV 配置源固定为上表 5 层、无 profile 需求，自实现 ordered-overlay ≈200 行即可（把 Metadata 记来源机制抄过来）；一旦要 profile/嵌套分节，直接引 figment 0.10.19（MIT OR Apache-2.0，维护中但慢节奏，2024-05 后未再发版）。

### Q3 unicode-width 对 CJK 输出的必要性（表格选型）

机制：`unicode-width`（unicode-rs，0.2.2，MIT OR Apache-2.0，维护中）按 UAX#11 计算**显示列宽**：East_Asian_Width 为 Wide/Fullwidth（CJK 及全角标点）计 2 列，其余计 1；另有 `UnicodeWidthChar` 逐字符与零宽结合字符处理。若表格按 `chars().count()` 或字节数 padding，中文路径/中文说明列必错位——ADV 的用户含中文环境，表格输出（报告、扫描结果）**必须**走显示宽度度量，这条没有替代品（已知残余误差：ambiguous-width 字符与 emoji 变体在不同终端表现不一，见 jeffquast 的终端宽度校正表文章；ADV 表格规避 emoji 单元格即可）。

comfy-table vs tabled（两者都用 unicode-width 度量，宽字符对齐同源）：
- **comfy-table 8.0.1（MIT，维护中，Nukesor）**：运行时逐格 `add_row`，约束系统（Fixed/Percentage/ContentLength + 降级策略）贴合"报告表格"场景；宽字符对齐是其明确支持项（历史上曾有 CJK 错位 issue，后改用 unicode-width 度量修复——检索口径）。注意 ANSI 着色进单元格要走 `custom_styling` feature，且**着色不影响宽度计算**是它的设计目标。
- **tabled 0.22.0（MIT，维护中，zhiburt）**：`#[derive(Tabled)]` 结构体直接出表，主题/span/rotate 功能最全，代价是 derive 宏与更多 feature 的编译成本；宽字符同样走 unicode-width。

裁决：**吸收 comfy-table + unicode-width**（ADV 报告表格是运行时动态列，derive 静态表非刚需）；tabled 列 reference。规范：含 ANSI 的单元格仅在开 `custom_styling` 时允许，否则先剥离。

### Q4 globset 的模式语义裁决（include/exclude 默认语义）

globset 机制（docs.rs/globset，0.4.20，Unlicense OR MIT，BurntSushi/ripgrep 系）：① `GlobBuilder::literal_separator(true)` 使 `*`、`?`、`[...]` **不跨 `/`**；默认 false 时它们连 `/` 都匹配（隐雷：`*.rs` 会匹配到子目录里的文件）；② `**` 递归匹配仅在三个合法位置：glob 以 `**/` 开头、以 `/**` 结尾、中间含 `/**/`；③ `GlobSet` 多模式一次匹配（内部按字面/简单/正则自动混合策略）；④ `case_insensitive`、`backslash_escape`（Windows 上默认关——`\` 是路径分隔符，不作转义）可配。

裁决：ADV include/exclude 采用 **glob 语义（非 .gitignore 语义）**，理由与细则：
- .gitignore 语义（`ignore` crate 的 gitignore 模块）包含锚定规则、目录排除即剪枝、`!` 重新包含等一整套"仓库过滤"心智，面向"我在排除别人的仓库内容"；ADV 的 include/exclude 是**用户显式清单**，用户心智是 glob（与 rg --glob 同源）。
- `literal_separator(true)`：`*.rs` 只匹配单层；跨目录要显式 `**/*.rs`。
- **exclude 优先于 include**：先匹配 exclude 集合，命中即排除，再看 include（include 为空 = 全含）。
- **不引入 `!` 否定前缀**：与 Windows/PowerShell 的引号与历史展开坑叠加风险大，需要否定语义时用户写两条集合（exclude 里再加规则）。
- 大小写：默认 `case_insensitive(false)` 保证跨平台一致；Windows 用户可用 flag 显式打开。
- 输入归一：用户模式与路径统一 `\` → `/`（配合 dunce 规范化，见 Q5）。

### Q5 Windows 参数坑聚合（ADV CLI 的 Windows 参数处理规范）

1. **通配符不展开**：cmd.exe/PowerShell 都不展开 `*.rs`（展开是 Unix shell 的 libc 层职责）→ Windows 上 ADV 必须自展开。用 **wild 2.2.1**（Apache-2.0 OR MIT，BurntSushi）：`wild::args()` 替代 `std::env::args()`，仅 Windows 生效、其他平台直通；clap 官方文档把 wild 列为 Windows 通配符的标准配套。
2. **引号解析规则差异**：Windows 进程收到的是单条 UTF-16 命令行串，按 MSVCRT 规则拆（`2n+1` 个反斜杠 + 引号 → n 个反斜杠 + 引号字面量）。Rust `std::process::Command` 已按该规则转义；但**不要用 shell-words 拆 Windows 命令行**——它是 POSIX 语义。shell-words 的正确定位：拆"配置文件里存的命令字符串"（如编辑器命令）。另注：PowerShell 7.3 起 `$PSNativeCommandArgumentPassing` 改变了向原生命令传引号的方式（MS 文档口径），ADV 文档需注明 PS 5.1 与 7.3+ 行为差异。
3. **`\?\` 前缀污染**：`std::fs::canonicalize` 在 Windows 返回 `\\?\C:\...` verbatim 前缀，破坏显示、拼接与长度敏感 API → 用 **dunce 1.0.5**（CC0-1.0 OR MIT-0 OR Apache-2.0，kornelski）：`dunce::canonicalize` 仅在路径确实需要前缀（超长路径/UNC）时保留。
4. **非法 Unicode 参数 panic**：`std::env::args()` 对无法转 UTF-8 的参数会 panic（std 文档口径）→ 统一走 `args_os()` + `to_string_lossy`（wild::args 同样处理了此层）。
5. **大小写与分隔符**：NTFS 大小写不敏感、`/` 与 `\` 混用 → 路径比较前先 dunce 规范化再统一分隔符，globset 配 `backslash_escape` 的 Windows 默认。
6. **控制台编码与 ANSI 使能**：老 conhost 需要 VT 使能与 UTF-8 代码页；console/anstream 已封装 SetConsoleMode 使能，ADV 不自写。
7. **补全生态断层**：cmd.exe 无补全生态；PowerShell 走 clap_complete 静态脚本；nushell 走官方 clap_complete_nushell（见条目 11）。

---

## 条目清单

### 机制级（13 条）

**01. clap** —— 定位：Rust CLI 参数解析事实标准（derive + builder 双 API）。可抄机制：① derive 宏编译期从 struct 生成 parser（无运行时反射，成本在编译不在运行）；② `error-context`+`suggestions` 特性提供"did you mean"与错误定位；③ `ArgAction::Append/Count` 等动作语义与 `--` 分隔转义。档位：**吸收**（主档，判据见 Q1）。第一步：建 `cli::Args` 单一入口 + features 剪裁（default-features=false）。许可证：MIT OR Apache-2.0；成熟度：维护中（rust-cli org，4.6.7，2026-09-14 发布）。链接：<https://docs.rs/clap>

**02. bpaf** —— 定位：组合子派生双 API 的新一代解析器，动态补全卖点。可抄机制：① 纯组合子"构造即文档"，parser AST 反推 help；② shell 补全由 parser 结构直接导出，**不需要额外 crate**；③ 同库提供 derive 面向习惯 clap 的人。档位：**watch**（ADV 若实测 clap derive 编译超预算，这是唯一候选换档）。第一步：无（在 X15 台账登记复核点，半年后再看）。许可证：MIT OR Apache-2.0；成熟度：维护中（0.9.28，2026-09-17 发布）。链接：<https://docs.rs/bpaf>

**03. lexopt** —— 定位：极简零依赖 POSIX 风格 argv 解析（对照档）。机制：手写状态机逐参数消费，无 derive、无自动 help/补全，全量构建 ≈37 KiB/≈329 ms（argparse-rosetta-rs 口径）。档位：**reference**（ADV 不用：补全与 help 需自建，产品力受损）。许可证：MIT；成熟度：维护中（低频，0.3.2，2026-02-28 发布）。链接：<https://docs.rs/lexopt>

**05. figment** —— 定位：多源配置合并库（Rocket 血统）。可抄机制：见 Q2（merge/join 优先级 = 合并顺序；Provider/Dict 抽象；Metadata 来源追踪）。档位：**有界**（先抄模型自实现薄层，出现 profile/嵌套需求再引入）。第一步：配置装载层先按 Q2 规范写 ordered-overlay，值带来源元数据。许可证：MIT OR Apache-2.0；成熟度：维护中（慢节奏，0.10.19，2024-05-17 发布——近两年多无新版，但无 issue 积压爆仓迹象，属稳定件）。链接：<https://docs.rs/figment>

**07. etcetera** —— 定位：配置/数据/缓存路径策略抽象。可抄机制：① `choose_base_strategy()` 返回平台策略对象（Windows: %APPDATA% 系；Unix: XDG 系），`config_dir()/data_dir()/cache_dir()` 统一出口；② 策略可注入（测试时换临时目录）；③ "unopinionated"——不替你决定 app 名层级，`config_dir().join("adv")` 由 ADV 自定。档位：**吸收**（ADV 配置文件位置策略直接用它，比 dirs/directories 多一层可注入的策略抽象）。第一步：`etcetera::choose_base_strategy()?` + `config_dir()/adv/adv.toml` 定位配置。许可证：MIT OR Apache-2.0；成熟度：维护中（0.11.0，2025-10-28 发布）。链接：<https://docs.rs/etcetera>

**10. clap_complete** —— 定位：clap 的 shell 补全生成器。可抄机制：① 从 clap Command 树静态生成 bash/zsh/fish/elvish/powershell 脚本（`generate()` 一次性输出，build.rs 或运行时 `--generate` 子命令皆可）；② 动态候选补全走 `complete()` 回调注册（仅部分 shell 支持）；③ `Shell` 枚举可枚举遍历批量生成。档位：**吸收**（随 clap 主档）。第一步：build.rs 为 5 个内置 shell 生成补全进 `completions/`。许可证：MIT OR Apache-2.0；成熟度：维护中（4.6.11，2026-09-15 发布）。链接：<https://docs.rs/clap_complete>

**13. dialoguer** —— 定位：行内交互提示（console-rs 系）。可抄机制：① Theme 抽象统一渲染风格；② Confirm/Select/MultiSelect/Input 基于 `console::Term` 行内重绘（非 TUI，不接管整屏）；③ `interact_opt()` 把 Ctrl-C/D 返回 `None` 而非 panic——程序化嵌入时的关键口。档位：**有界**（ADV 是非交互优先平台：交互仅限 TTY fallback 场景；C4 三态契约下非 TTY 必须禁用，由 anstream/anstyle-query 的 TTY 探测门控）。第一步：仅在 `--interactive` 显式 flag 下启用。许可证：MIT；成熟度：维护中（0.12.0，2025-08-23 发布）。链接：<https://docs.rs/dialoguer>

**16. anstyle** —— 定位：零依赖 ANSI 样式定义/解析（clap v4 默认着色后端，2026 主流）。可抄机制：① `Style/Color/Effects` 编译期常量构造，无运行时解析成本；② `anstyle-query` 检测 NO_COLOR/CLICOLOR/TERM=dumb 环境约定；③ 与 ratatui 的 Style 互转（crossterm/anstyle 双轨时的桥）。档位：**吸收**。第一步：着色统一从 anstyle 定义，禁止散落硬编码 `\x1b[`。许可证：MIT OR Apache-2.0；成熟度：维护中（1.0.14，2026-03-13 发布）。链接：<https://docs.rs/anstyle>

**17. anstream** —— 定位：自适应输出流（anstyle 官配）。可抄机制：① `AutoStream::choice()`/包装把"非 TTY 自动剥离 ANSI"做成 IO 层行为——**C4 三态输出契约（人读/机器读/CI 读）的机器实现点**；② `StripStream/ColorChoice` 显式覆盖（`--color=always/never/auto` 语义的标准实现）；③ stdout 锁定与 buffer 策略。档位：**吸收**。第一步：所有输出经 `AutoStream::auto(std::io::stdout())`，`--color` flag 映射 ColorChoice。许可证：MIT OR Apache-2.0；成熟度：维护中（1.0.0，2026-02-11 发布）。链接：<https://docs.rs/anstream>

**20. unicode-width** —— 定位：UAX#11 显示宽度计算（表格对齐/进度条/CJK 输出的度量基准）。可抄机制：① `UnicodeWidthStr::width()` 对 str 直接求显示列宽；② `UnicodeWidthChar::width()` 逐字符（区分 None=控制字符）；③ 供 comfy-table/tabled 内部使用的同一实现，ADV 自绘行（进度条、单行状态）也用它 pad。档位：**吸收**（判据见 Q3）。第一步：封装 `display_width(&str) -> usize` 作为唯一宽度口径。许可证：MIT OR Apache-2.0；成熟度：维护中（0.2.2，2025-10-06 发布）。链接：<https://docs.rs/unicode-width>

**21. comfy-table** —— 定位：运行时动态宽表格渲染。可抄机制：① 约束系统（Fixed/Percentage/ContentLength）+ 超宽降级策略，贴报告场景；② 样式表（UTF8_FULL/ASCII_MARKDOWN 等）一行切风格——机器态输出切 ASCII、人读态切 UTF8 正是 ADV 需要的；③ 宽字符与 ANSI 度量内置 unicode-width（详见 Q3）。档位：**吸收**。第一步：报告层封装 `render_table(rows, style)`，机器态禁 UTF8 框线。许可证：MIT；成熟度：维护中（8.0.1，2026-09-25 发布）。链接：<https://docs.rs/comfy-table>

**23. globset** —— 定位：ripgrep 系 glob 引擎。可抄机制：见 Q4（literal_separator、`**` 三合法位、GlobSet 多模式混合策略匹配、case/backslash 选项）。档位：**吸收**。第一步：`GlobSetBuilder` + Q4 语义封装成 `include_exclude.rs`，模式归一 `\`→`/`。许可证：Unlicense OR MIT；成熟度：维护中（低频稳定，0.4.20，2026-08-04 发布）。链接：<https://docs.rs/globset>

**26. wild** —— 定位：Windows 命令行通配符展开。可抄机制：① `wild::args()` 直替 `std::env::args()`，仅 Windows 展开、Unix 直通（展开职责补位，见 Q5-1）；② 无匹配时保留原样参数（不静默吞）；③ 与 clap 配合为官方推荐组合。档位：**吸收**。第一步：cli 入口用 `wild::args_os()` 喂 clap。许可证：Apache-2.0 OR MIT；成熟度：维护中（低频稳定，2.2.1，2024-01-27 发布）。链接：<https://docs.rs/wild>

**28. dunce** —— 定位：Windows 路径规范化（去 verbatim 前缀）。可抄机制：① `dunce::canonicalize` 直替 `std::fs::canonicalize`，仅在超长路径/UNC 时保留 `\\?\` 前缀（智能判断，非暴力去前缀）；② 处理 `\\?\UNC\server\share` 到 `\\server\share` 的还原；③ 跨平台 API 同签名。档位：**吸收**（见 Q5-3）。第一步：所有 canonicalize 调用点替换为 dunce。许可证：CC0-1.0 OR MIT-0 OR Apache-2.0；成熟度：维护中（低频稳定，1.0.5，2024-08-04 发布）。链接：<https://docs.rs/dunce>

### 扫视（16 条）

**04. argh** —— derive 轻量解析（Google/Fuchsia 系，BSD-3-Clause，0.1.19，2026-03-16，维护中低频）：derive 生成简单匹配代码、无错误上下文与补全生态；ADV 不用的对照档。reference。<https://docs.rs/argh>

**06. config** —— 多格式分层配置（MIT OR Apache-2.0，0.15.27，2026-09-30，维护中）：`Source` 逐层 merge、顺序即优先级，机制与 figment 同类但格式 feature 更重、remote 源是占位实现；figment 已覆盖，不引入。不吸收。<https://docs.rs/config>

**08. directories** —— 平台标准目录常量（MIT OR Apache-2.0，6.0.0，2025-01-12，维护中）：基于 XDG/Apple/Windows 规范的 mid-level 路径出口；被 etcetera 的策略抽象覆盖。reference。<https://docs.rs/directories>

**09. dirs** —— 平台标准目录只读简版（MIT OR Apache-2.0，7.0.0，2026-09-05，维护中）：`dirs::config_dir()` 一族函数，无策略切换；同上被 etcetera 覆盖。reference。<https://docs.rs/dirs>

**11. clap_complete_nushell** —— nushell 补全生成器（MIT OR Apache-2.0，4.6.2，2026-08-11，维护中，nushell 团队官方）：不在 clap_complete 主 crate 内，需单独引入；Windows/nushell 用户才值得带。有界。<https://docs.rs/clap_complete_nushell>

**12. clap_mangen** —— clap Command → roff man 页（MIT OR Apache-2.0，0.3.3，2026-08-12，维护中）：CI 生成 man 的常规配套；ADV 本地优先、无 Unix 包管理分发计划前价值低。watch。<https://docs.rs/clap_mangen>

**14. inquire** —— 交互提示对照档（MIT，0.9.4，2026-02-24，维护中）：crossterm 后端、validator/Formatter/page-size 更可定制，API 与运行成本也更高；dialoguer 已覆盖 ADV 的有限交互面。reference。<https://docs.rs/inquire>

**15. demand** —— 新一代极简提示库（mitsuhiko，MIT，2.3.0，2026-09-25，维护中但单维护者风险）：console 系底座 + builder 风格极简 API；API 稳定性与长期维护待观察。watch。<https://docs.rs/demand>

**18. console** —— 终端抽象底座（MIT，0.16.6，2026-09-10，维护中）：Term 抽象、TTY 探测、Windows VT 使能（SetConsoleMode），是 dialoguer 的传递依赖；ADV 直接用点限于 TTY 探测（anstyle-query 可能已覆盖，落地时二选一）。有界。<https://docs.rs/console>

**19. termcolor** —— 着色输出老牌（BurntSushi，Unlicense OR MIT，1.4.1，2024-01-10，低频稳定，ripgrep 在用）：`WriteColor` trait、无 NO_COLOR 自动化；anstream 已覆盖并多做环境约定检测。reference。<https://docs.rs/termcolor>

**22. tabled** —— derive 表格对照档（MIT，0.22.0，2026-09-05，维护中，zhiburt）：`#[derive(Tabled)]` 结构体直出表、主题/span 功能最全，derive 编译成本高于 comfy-table；ADV 动态列场景非刚需（见 Q3）。reference。<https://docs.rs/tabled>

**24. wildmatch** —— 极简 `*`/`?` 通配符匹配（MIT，2.6.1，2025-11-14，维护中）：无 `**`/字符类/花括号，仅字符串语义；需要"组件名简单匹配"（如忽略文件名单条规则）时可用，globset 已覆盖主场景。reference。<https://docs.rs/wildmatch>

**25. glob** —— 经典 Unix shell 风格路径匹配（MIT OR Apache-2.0，0.3.4，2026-07-21，维护中）：递归遍历文件系统返回命中路径，**语义不可配**（无 literal_separator 开关），与 globset 的隐雷同源但更不可控；ADV 只要匹配不要遍历。不吸收。<https://docs.rs/glob>

**27. shell-words** —— POSIX shell 词法拆分（MIT/Apache-2.0，1.1.1，2025-12-10，维护中）：按 POSIX 规则拆带引号字符串；仅用于"配置里存的命令串"场景，**不可**用于解析 Windows 原生命令行（见 Q5-2）。reference。<https://docs.rs/shell-words>

**29. path-absolutize** —— 路径绝对化（MIT，4.0.1，2026-07-11，维护中）：基于 CWD 的 `PathBuf` 扩展；ADV 用 std + dunce 已覆盖（absolute → canonicalize → dunce），多一层依赖无增益。不吸收。<https://docs.rs/path-absolutize>

---

## 风险与遗留

- figment 近两年无新版（2024-05-17 锚）：不是弃维护信号（无 bug 积压证据），但 ADV 若长期依赖应在台账加半年复核点；自实现薄层是规避路径。
- demand/console-rs 系单维护者集中（mitsuhiko / console-rs org）：交互面 ADV 用量小，风险可接受。
- Q3 的表格 CJK 度量残余误差（ambiguous-width/emoji）未逐一实测：落地 comfy-table 后用中文+emoji 混排用例测一轮再定输出规范。
- PowerShell 5.1 vs 7.3+ 传参差异（Q5-2）与 Windows 补全矩阵（cmd 无 / PS 静态 / nushell 官方侧 crate）需进 ADV 用户文档，未实测部分不写死结论。
- 交互库对比一次检索遇限流未深挖（dialoguer/inquire/demand 的机制为熟知口径，版本/日期来自 crates.io API 当日锚）。
