# D2 格式化器全家（调研）

- 日期：2026-10-03；域：D2（格式化器的机制与工程）；服务对象：ADV cli/xtask 面（自带格式化检查门）与规则引擎"可机读输出"。
- 方法：WebSearch（6+3 条）+ webReader 抓取（treefmt README 全文一手；dprint-core incremental.rs 与 prettier commands.md 的 GitHub 抓取失败/仅回导航壳，相关条目标"待核源码"）。
- 档位口径：吸收（直接采用/直接可抄）｜有界（机制吸收、平台或范围受限）｜不吸收｜watch（持续观察）｜reference（对照样本）。
- 登记表：`RESEARCH/registry/D2.jsonl`（31 行：15 机制级 + 12 扫视 + 4 机制专题）。

---

## 0. Top-3

1. **dprint（插件平台）**：host（Rust CLI）+ guest（.wasm 插件）协议化、插件 URL+checksum 固定、dprint-core 内建增量格式化（按变更区间复用打印结果）。ADV 多语言格式门的"平台形态"直接参考，也是"长尾语言兜底"候选。（机制级，见 §3.2）
2. **treefmt（多路复用 + 变更缓存）**：一个 toml 声明 command+includes glob，并行跑全部格式器，且只重格式化变更过的文件。xtask 格式门的组织模式可整套照搬。（一手 README 已核，见 §3.12）
3. **格式门验收判据三件套**：不动点 f(f(x))==f(x)（Topiary 测试法）+ AST 等价 Black-style `assert_equivalent` + `--check` 退出码 0/1/2 契约（Prettier/Biome/rustfmt 生态惯例）。ADV 格式门的验收判据可直接落。（§1.③⑤、§2.1）

---

## 1. 重点回答

### ① Prettier 的 Doc IR 机制拆解
- 结构：**AST → Doc（中间表示）→ Printer**。语言插件只负责把 AST 翻译成 Doc，打印器只消费 Doc，二者解耦（官方机制文档 `prettier/commands.md`，https://github.com/prettier/prettier/blob/main/commands.md ；理论源头 Wadler "A prettier printer" 2003，https://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf ）。
- 核心命令：`group`（组内先试 flat、超行宽整组断行，fits 探测）、`fill`（序列项间贪心取舍）、`indent`、`line/softline/hardline`、`ifBreak`（断行态才出现的文本，如逗号）、`breakParent`（硬换行向上传播；hardline 自带 breakParent）、`lineSuffix`（行尾注释类）、`join`。builders 文档见 https://prettier.github.io/prettier-printer/modules/_builders_.html 。
- 可抄机制：(a) Doc IR 本身——ADV 报告/代码片段渲染做一份同构 IR，一份结构出终端/Markdown/JSON 三种渲染，宽度策略只在打印器实现一次；(b) `ifBreak`+`breakParent` 的传播语义（断行意图局部声明、全局生效）；(c) `fill` 的贪心断行（列表/词序列场景）。
- 档位：**有界**（思想吸收进 ADV 渲染层，不引入 Prettier 本体）。第一步：mini-PoC——ADV 诊断输出用 Doc IR 在 80/120 两档终端宽下渲染比对。

### ② dprint 插件平台的增量/缓存机制
- 平台形态：dprint CLI（Rust）为 host，每个语言格式器是独立 `.wasm` 插件（guest，任意语言编译）；`dprint-core`（`wasm` feature）定义 host–guest 消息协议与格式化 trait（官方 `docs/wasm-plugin-development.md`）。插件在 `dprint.json` 里按 URL+checksum 指定，可离线、可复现；非 WASM 场景用进程插件补位。原 rustfmt WASM 插件已弃用，现行方案是 `dprint-plugin-exec` 包壳外部命令（dprint.dev 插件页）。
- 增量格式化：dprint-core 内建 IncrementalFormatter——用新旧文本 diff 圈定变更区间，仅对受影响区间重跑打印，未变更区间的已格式化文本直接复用；前提是变更区间的邻接文本/空白未被破坏。**精确判定条件本次未能抓到 incremental.rs 源码（证书链问题），标"待核源码"**；文件级缓存则由 treefmt 一类的外层复用器承担。
- 对 ADV 的可行性判断：作为"多语言格式门执行器"成本不高——MIT、单二进制、`dprint check` 非零退出即可挂门；但 ADV 自带解析器的语言（Rust 主资产）内嵌格式化逻辑更可控，dprint 更适合长尾语言（JSON/TOML/MD/YAML/JS 系）兜底。
- 档位：**有界**（机制吸收、平台引入有界）。第一步：`dprint check --json` 挂 xtask 试运行，测开销与误报面，再决定是否常驻。

### ③ idempotency 的标准判据与测试格式
- 判据一（不动点）：`f(f(x)) == f(x)`。测试格式：golden test 中连跑两遍，第二遍必须零 diff（Topiary 把该法做进测试套件；https://github.com/tweag/topiary ）。
- 判据二（语义保持）：`AST(before) ≡ AST(after)`（忽略空白与位置）。Black 测试套件用 `assert_equivalent` 做 AST 比对以证明"格式化不改语义"；ocamlformat 文档记载其格式化后重解析、断言解析树等价（安全检查）。
- 判据三（确定性）：同输入多次运行输出恒定（无随机/环境依赖）。
- ADV 落法：库测层跑判据一+二（语料池全量）；门运行时只跑一遍格式化 + `--check` 比对（性能）；判据二依赖"能重新解析"，用户仓无解析器时降级为判据一。

### ④ "无配置哲学"（gofmt）vs "配置丰富"（Prettier/clang-format）的取舍
- gofmt 路线：唯一规范风格、零格式旋钮（`-s/-l/-d` 是行为开关不是风格开关）——消灭风格争论，代价是少数场景的表达力硬编码（转述 Go 社区通说，非引用；https://pkg.go.dev/cmd/gofmt ）。
- Prettier 路线：opinionated 但保留少量选项（printWidth 等）；clang-format 是配置复杂度天花板（上百选项 + `Penalty*` 惩罚权重 + `BasedOnStyle` 继承，https://clang.llvm.org/docs/ClangFormatStyleOptions.html ），风格矩阵难维护。
- rustfmt 中间态的教训（旧仓 array_width 触发重排即属此类）：width 系列选项（array_width/fn_call_width 等）在 stable rustfmt 上属 unstable、需 nightly 才真正生效，同一 rustfmt.toml 在两条工具链下行为不同；且 bare rustfmt 默认 2015 style edition，须显式 `style_edition = "2024"`（依据：rustfmt issue #2636 与 2026-08 版 rust rulebook 的口径；https://github.com/rust-lang/rustfmt ）。
- **ADV 结论：格式检查门走近零配置——唯一规范风格 + 仅暴露极少数旋钮（如 max_width）；风格演进走 style_edition 式"整档切换"（一次一档、可回滚），不做逐项开关。**

### ⑤ 格式化检查门（--check 模式）的退出码/输出契约
- 退出码三段：**0 = 干净；1 = 存在会被改写的文件（可格式化修复）；2 = 致命错误（解析失败/IO/配置错误）**。生态锚：Biome 文档口径 1=有诊断、2=致命；Prettier `--check/--list-different` 找到差异退出 1；rustfmt `--check` 输出 git 风格 unified diff 且非零退出。反例借鉴：`gofmt -l` 把"待格式化"当正常退出 0 只列文件名——ADV 不采用（CI 需要非零失败信号）。
- 输出契约：check 模式绝不改写文件；diff 默认 unified（git 可直接 patch），`--diff` 显式开关；`--format json` 时 stdout 出机器可读文件列表（stderr 留给人读）；大仓限流——最多列前 N 个文件 + 总数，防日志爆炸。
- 修复路径成对提供：`--write`（或等价 fix 子命令）+ 门失败信息里给出修复命令原文。

---

## 2. 机制专题

### 2.1 idempotency（判据与测试法）——见 §1.③。补充：非幂等的常见来源是"打印决策依赖输入的历史形态"（如引号归一、字符串拼接折行），golden 语料池两遍法可直接暴露。

### 2.2 diff 保留语义的证明（格式化不改 AST 的验证）
- 三道证明组合：(1) AST 等价比对（Black `assert_equivalent`；ocamlformat 重解析断言解析树不变——文档记载）；(2) tree-sitter 场景下比较解析树（Topiary 的设计目标之一）；(3) 上游快照差分——Biome 以 Prettier **自有测试套件**逐快照比对算出 96%+ 兼容（官方博客 2023-11-27，口径=JS/TS/JSX，https://biomejs.dev/blog/biome-wins-prettier-challenge ）。Ruff 则以 Black 为锚做兼容验证（官方文档口径：line-length 一致即开箱兼容，https://docs.astral.sh/ruff/formatter/ ）。
- ADV：兼容性/正确性声明一律"跑上游套件算分"，不口头宣称。

### 2.3 编辑器冲突（format-on-save 与 VCS 噪声）
- 病灶：全文件 format-on-save 造成 blame 噪声与 PR 混入格式行。
- 缓解三件：(1) 一次性全量格式化提交 + `.git-blame-ignore-revs`（GitHub 原生支持，rustfmt/prettier 社区通行做法）；(2) VS Code `editor.formatOnSaveMode: modifications / modificationsIfAvailable`（按编辑区间格式化，依赖格式化器支持 range format）；(3) 格式提交与功能提交分离，门端兜底。
- ADV：文档给推荐配置；门只认"整仓规范态"，不认编辑器状态。

### 2.4 增量格式化（谁做了）
- 区间级：dprint-core IncrementalFormatter（diff 圈区间、复用打印结果；判定条件待核源码）；编辑器 range-format API（语义正确性由各实现自担）。
- 文件级：treefmt——跟踪文件变更、只重格式化已变更文件（README 一手已核）。
- 底层使能：tree-sitter 增量解析（Topiary 依托）。
- ADV：先做文件级缓存（treefmt 已验证可行且简单）；区间级不急。

---

## 3. 机制级条目（15）

### 3.1 rustfmt
- 定位：Rust 官方格式化器，rustfmt.toml 配置，随工具链分发。
- 可抄机制：① stable/unstable 选项二分 + `style_edition` 整档切换（bare rustfmt 默认 2015 style edition）；② `--check` 出 unified diff + 非零退出（门契约样板）；③ `use_small_heuristics` 与 width 系列（array_width 等，stable 上不可配）联动——旧仓 array_width 重排的机制根。
- 档位：**吸收**（ADV 自身 Rust 代码门的执行器）。第一步：ADV 仓 rustfmt.toml 定 `style_edition="2024"`、不启用 unstable 选项，门内 `cargo fmt --check`。
- 许可证/成熟度：Apache-2.0 / MIT；维护中（Rust 官方）。
- 链接：https://github.com/rust-lang/rustfmt ；https://rust-lang.github.io/rustfmt/

### 3.2 dprint（重点拆解）
- 定位：Rust 写的多语言格式化平台，WASM 插件架构。
- 可抄机制：① host–guest 协议（dprint-core `wasm` feature）+ 插件 URL/checksum 固定（离线可复现）；② dprint-core 增量格式化（diff 圈区间复用打印，邻接空白前提——待核源码）；③ `dprint-plugin-exec` 进程壳兜底任意外部格式器（接 rustfmt 的现行方案）。
- 档位：**有界**。第一步：`dprint check --json` 挂 xtask 试运行测开销。
- 许可证/成熟度：MIT；维护中（活跃）。
- 链接：https://dprint.dev ；https://github.com/dprint/dprint

### 3.3 Prettier
- 定位：JS/TS 及多语言事实标准，opinionated，AST→Doc→printer。
- 可抄机制：① Doc IR 命令集（group/fill/ifBreak/breakParent…）与 printer 解耦；② `--check/--list-different` 输出契约；③ 插件体系（语言插件各自产 Doc）。
- 档位：**有界**（Doc IR 思想进 ADV 渲染层）。第一步：报告渲染 mini-PoC（80/120 双宽比对）。
- 许可证/成熟度：MIT；维护中（3.x 线 2025 年仍活跃发版，以 changelog 为准）。
- 链接：https://github.com/prettier/prettier/blob/main/commands.md ；https://prettier.io/docs/options

### 3.4 Biome
- 定位：Rust 单二进制 JS/TS/JSON/CSS/GraphQL 一体式 format+lint。
- 可抄机制：① 一体式架构（format 与 lint 共享解析器/AST——与 ADV 规则引擎同构）；② 兼容性验证法（跑 Prettier 自有套件逐快照比对，96%+，2023-11-27 官宣）；③ 退出码约定（1=诊断，2=致命）。
- 档位：**有界**（校验法吸收）。第一步：把"上游套件跑分"写进 ADV 兼容声明模板。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（2025 年 2.x 线推进多文件分析，日期以官方博客为准）。
- 链接：https://biomejs.dev/blog/biome-wins-prettier-challenge

### 3.5 Black
- 定位：Python 事实标准格式化器，确定性承诺。
- 可抄机制：① 魔法尾部逗号（尾逗号=用户声明拆行意图，格式化器当语义信号尊重；Ruff 同口径，可关）；② 测试套件 AST 等价断言（`assert_equivalent`，判据二样板）；③ 确定性 + `--check`。
- 档位：**吸收**（"尾逗号=拆行意图"约定进 ADV 格式门文案）。第一步：ADV 门文档固化该约定。
- 许可证/成熟度：MIT；维护中。
- 链接：https://black.readthedocs.io ；https://docs.astral.sh/ruff/formatter/ （Black 兼容口径）

### 3.6 Ruff format
- 定位：Rust 写的 Python 格式化器，以 Black 输出为锚。
- 可抄机制：① 兼容策略=对锚工具差分验证（line-length 一致即开箱兼容，官方口径）；② linter+formatter 同仓共享管线（对 ADV 规则引擎↔格式门架构直接可抄）；③ `skip-magic-trailing-comma` 开关的设计（约定可关但要显式）。
- 档位：**吸收**。第一步：规则引擎输出与格式门共用 AST/文本区间基础设施的设计评审。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://docs.astral.sh/ruff/formatter/

### 3.7 gofmt
- 定位：Go 官方格式化器，无配置哲学鼻祖。
- 可抄机制：① 零配置 + 唯一规范风格（`-s/-l/-d` 是行为开关非风格开关）；② `-l` 列文件模式（反例：退出 0，见 §1.⑤）；③ "格式化器即社区规范"路线。
- 档位：**吸收**（哲学）。第一步：ADV 格式门默认零配置并文档化。
- 许可证/成熟度：BSD（Go 项目）；随 Go 维护。
- 链接：https://pkg.go.dev/cmd/gofmt

### 3.8 goimports
- 定位：gofmt + import 分组/增删。
- 可抄机制：警示样本——它改 import 集合（AST 级内容），超出"纯格式"契约；格式门是否允许触碰语义内容必须明示。
- 档位：**有界**。第一步：ADV 格式门契约写死"只动空白与换行，不改 token 流"。
- 许可证/成熟度：BSD；维护中（x/tools）。
- 链接：https://pkg.go.dev/golang.org/x/tools/cmd/goimports

### 3.9 gofumpt
- 定位：gofmt 之上的更严规范层，同样无配置。
- 可抄机制："规范演进 = 换工具版本"（与 style_edition 整档切换同思路，规则不分叉成选项）。
- 档位：**有界**。第一步：观察其新增规则是否值得进 ADV 风格档。
- 许可证/成熟度：BSD-3；维护中。
- 链接：https://github.com/mvdan/gofumpt

### 3.10 clang-format
- 定位：C/C++ 系格式化器，配置复杂度天花板。
- 可抄机制：① Penalty* 惩罚打分 + 搜索的断行决策（与 Prettier group/fits 并列的两大断行流派）；② `BasedOnStyle` 继承（配置继承机制的样本）；③ 上百选项的维护代价本身是反面教材。
- 档位：**不吸收**（配置面）；断行流派对照 reference。
- 第一步：无（保留流派对照笔记）。
- 许可证/成熟度：Apache-2.0（LLVM exception）；维护中（LLVM）。
- 链接：https://clang.llvm.org/docs/ClangFormatStyleOptions.html

### 3.11 Topiary
- 定位：Tweag 的 tree-sitter 查询驱动格式化引擎。
- 可抄机制：① 语言适配≈写 tree-sitter query（声明节点前后换行/空格），不写解析器；② 幂等与解析树保持是显式设计目标（判据一测试内建于套件）；③ 2025-01-30 官方教程显著降低写格式器门槛。
- 档位：**watch**（版本未到 1.0；ADV 长尾语言若走 tree-sitter 路线是现成引擎）。第一步：盯 release；引 tree-sitter 时评估 topiary-core 作库。
- 许可证/成熟度：MIT；实验倾向（pre-1.0，维护中）。
- 链接：https://github.com/tweag/topiary ；https://tweag.io/blog/2025-01-30-topiary-tutorial/

### 3.12 treefmt
- 定位：格式器多路复用器（README 一手已核）。
- 可抄机制：① `treefmt.toml` 按 `[formatter.<name>] command+options+includes(glob)` 声明，一命令跑全部；② 并行执行；③ 跟踪文件变更、只重格式化已变更文件（文件级缓存）；④ 统一 CLI 与输出（多格式器异构输出的归一化）。
- 档位：**吸收**（xtask 多语言门的组织模式整套照搬）。第一步：xtask format-gate 按 treefmt 模式建 config+并行+变更缓存。
- 许可证/成熟度：MIT；维护中（v2 重写为 Rust，numtide）。
- 链接：https://github.com/numtide/treefmt

### 3.13 taplo
- 定位：TOML 的 LSP+格式化一体工具。
- 可抄机制：① `taplo format --check` 直接可挂门；② LSP 与 CLI 共享同一格式化内核（编辑器与门零分歧——ADV cli/xtask 面同构诉求）。
- 档位：**吸收**（ADV TOML 配置文件格式门）。第一步：xtask 门加 `taplo format --check`。
- 许可证/成熟度：MIT；维护中（节奏放缓）。
- 链接：https://taplo.tamasfe.dev

### 3.14 shfmt
- 定位：POSIX/mksh/bash shell 格式化器。
- 可抄机制：真 shell 解析器（mvdan/sh）打底——heredoc/引号语义正确，`-l/-d` 门模式，EditorConfig 支持。
- 档位：**有界**。第一步：ADV 支持资产面含 shell 时启用并挂门。
- 许可证/成熟度：BSD-3；维护中。
- 链接：https://github.com/mvdan/sh

### 3.15 StyLua
- 定位：Lua 格式化器（full-moon AST）。
- 可抄机制：stylua.toml 选项面（列宽/引号/括号）+ `--check` 门模式；小众语言格式器的"解析器+选项表"最小形态。
- 档位：**reference**。第一步：无。
- 许可证/成熟度：MPL-2.0；维护中。
- 链接：https://github.com/JohnnyMorganz/StyLua

---

## 4. 扫视条目（12）

- **ocamlformat**（OCaml）：文档记载其格式化后重解析、断言解析树不变的"安全检查"——判据二在 ML 系的实例。｜reference｜MIT｜https://github.com/ocaml-ppx/ocamlformat
- **dprint-plugin-prettier**：把 Prettier 包进 dprint 平台——格式化平台互操作的桥样本。｜reference｜MIT｜https://github.com/dprint/dprint-plugin-prettier
- **dprint-plugin-exec**：进程壳跑任意命令（接 rustfmt 的现行方案）。｜reference｜MIT｜https://github.com/dprint/dprint-plugin-exec
- **Prettier（markdown 打印器）**：prose wrap（preserve 为默认）+ 内嵌代码块联动格式化——markdown 作为"半自由文本"的格式化折衷样本。｜reference｜MIT｜https://prettier.io/docs/options
- **markdownlint**：lint 而非 formatter，`--fix` 修复——markdown 世界"lint+fix"与"format"两派分工的代表。｜reference｜MIT｜https://github.com/DavidAnson/markdownlint
- **mdformat**：CommonMark 严格口径的 Python markdown 格式化器 + 插件（含 mdformat-rustfmt 等联动）。｜reference｜MIT｜https://github.com/executablebooks/mdformat
- **SwiftFormat**：规则多且规则应用顺序敏感——格式器非幂等/顺序依赖的风险样本。｜watch｜MIT｜https://github.com/nicklockwood/SwiftFormat
- **CSharpier**：Prettier 理念（含 Doc IR 思路）移植 C#——Doc IR 跨语言可迁移的例证。｜reference｜MIT｜https://csharpier.com
- **sqlfluff**：SQL linter+fixer，方言/templater 感知——"格式化撞方言"复杂度样本。｜watch｜MIT｜https://github.com/sqlfluff/sqlfluff
- **yamlfmt**（Google）：YAML 格式化器，断行/折行策略可配。｜reference｜Apache-2.0｜https://github.com/google/yamlfmt
- **terraform fmt**：HCL 官方格式化，无配置 + `-check` 模式——IaC 侧的 gofmt 哲学。｜reference｜MPL-2.0（随 Terraform）｜https://developer.hashicorp.com/terraform/cli/commands/fmt
- **ormolu**（Haskell）：唯一风格 + 无配置（gofmt 哲学在 Haskell 的移植）。｜reference｜BSD-3｜https://github.com/tweag/ormolu

---

## 5. 2025–2026 前沿信号

1. **style_edition 2024 落地**（Rust 2024，2025-02 起）：格式风格随 edition 整档演进成为官方路线；"bare rustfmt 默认 2015 style edition"是新配置陷阱（依据 2026-08 版 rust rulebook 与 rustfmt 文档口径）。
2. **Biome 2.x（2025）**：一体式 Rust 工具继续推进多文件分析；对 Prettier 的兼容跑分（96%+，2023-11-27 口径）仍是"兼容性用上游套件度量"的最佳范例。
3. **Topiary 教程化（2025-01-30）**：tree-sitter query 驱动写格式器的门槛被显著拉低——长尾语言格式覆盖的可行路径，pre-1.0 仍需观察。
4. **treefmt v2（Rust 重写，numtide）**：多格式器复用 + 文件级变更缓存的产品化，xtask 门直接对标。
5. **AI 编辑器时代的格式噪声**（判断，非事实锚）：LLM 生成代码 + format-on-save 放大 VCS 噪声，`.git-blame-ignore-revs` 与"格式提交分离"的通行做法价值上升；待 2026 年内找一手数据锚再升档。
6. **Prettier 3.x 线 2025 年仍活跃发版**（3.5/3.6 节奏，插件 API 逐步开放；以官方 changelog 为准）——"老牌格式器未让位"，ADV 渲染层借鉴对象稳定。

## 6. 与 ADV 的落点小结

- **xtask-gates**：treefmt 组织模式（config+并行+变更缓存）+ rustfmt/taplo 具体门 + 判据三件套（§1.③）+ 退出码 0/1/2 契约（§1.⑤）。
- **cli**：dprint 平台形态与 checksum 固定（有界引入）；Doc IR 渲染 mini-PoC；"只动空白不改 token"契约写进门文案。
- **watch**：Topiary（tree-sitter 路线）、SwiftFormat/sqlfluff（非幂等与方言风险样本）。
- 旧仓教训归档：array_width 重排根因 = width 系列属 unstable、stable/nightly 行为分叉 + `use_small_heuristics` 联动——ADV 自身门固定 `style_edition="2024"` 且不启用 unstable 选项即规避。
