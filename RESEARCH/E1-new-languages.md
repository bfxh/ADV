# E1 · 新语言/编译器/验证栈实验调研

> 日期 2026-10-03（检索与 gh 仓库元数据查询同日）· 档位：吸收|有界|不吸收|watch|reference · 深度：机制级 15 条 + 扫视 22 条 = 37 条
> 目的：为 ADV（本地优先代码安全/质量平台，Rust 主体）找**可抄的编译器/语言服务机制**——诊断质量、增量编译/解析、类型系统设计、LSP 内建策略。不是选语言。
> 活跃度锚口径：`gh api repos/<r>` 的 `pushed_at`/`archived`（2026-10-03 查询）；版本号锚：官方 release notes / crates.io / 媒体报道（标注日期）。

---

## 0. 五个重点问题（先给结论）

### ① 诊断渲染层选型：codespan-reporting vs ariadne vs rustc 风格

| 维度 | rustc（内部 emitter）/ annotate-snippets | codespan-reporting | ariadne |
|---|---|---|---|
| 风格 | rustc 同款：primary/secondary 标签、多行 span 折叠 | rustc 同款的早期独立实现，输出朴素稳定 | 更花哨：多 span 彩色连接线、帮助/注释块 |
| 结构化输出 | `--error-format=json`（level/code/spans/children/suggestions 全结构化），渲染只是 JSON 的投影 | 无（term + graphical 渲染器） | 无 |
| 建议修复适用性 | 有（Applicability：MachineApplicable/MaybeIncorrect/…），rustfix 只自动应用 MachineApplicable | 无 | 无 |
| 维护 | rust-lang 官方系 | 0.13.1（crates.io，2025-10 列示；EPEL 2025-12 打包）；codespan 仓库 pushed 2026-02-28 | **仓库 ARCHIVED**（zesterer/ariadne，gh 2026-10-03 查：last push 2026-03-23 + archived） |
| 输出稳定性 | 以 rustc 输出为准绳 | 社区反馈格式变化少 | 官方自认：布局启发式微调会改变输出格式（对 golden 测试不利） |
| 许可证 | MIT OR Apache-2.0 | Apache-2.0 OR MIT | MIT |

**选型结论**：
- **主选 annotate-snippets**（rust-lang 维护、对齐 rustc 观感、结构化输入）。注：gh 查询 `rust-lang/annotate-snippets` 返回 404（2026-10-03），仓库路径可能迁移/改名——以 crates.io 仓库链接为准，**复核项**。
- **备选 codespan-reporting**：推翻"停滞"旧印象——0.13.1（2025-10）仍发版、输出稳定；若 annotate-snippets 路径问题无解可退到这里。
- **不吸收 ariadne**：已归档 + 格式漂移，两条都否决它作为 ADV 长期依赖。它对 codespan 的"更花哨"继承（ariadne 作者自述受 codespan 启发）可以当作风格参考，不当代码依赖。
- **关键缺口（任何渲染库都不提供）**：Applicability 概念只存在于 rustc 的诊断模型里。ADV 的"建议修复"必须在**自己的 schema 层**带 applicability（MachineApplicable/MaybeIncorrect）+ fix_id，渲染库只负责人类视图。
- miette 是另一路：应用级错误链（Diagnostic trait + cause 链 + serde JSON），适合 ADV cli 自身的运行错误 UX，与"规则报告渲染"是两个位置。

### ② 增量编译/增量解析的新做法（含 Roc 2026 现状）

- **Roc（2026 现状）**：编译器已完成 **Rust→Zig 重写（487 天）**，宣称 **35ms 增量重编译**（Developers Digest/rtfeldman，2025–2026）；设计目标是"内容寻址 DAG"——模块/类型/中间产物全部哈希化，改动只重算子图（思想与 Unison 同源）。roc-lang/roc 活跃（pushed 2026-10-03）。注意：融资结束转社区驱动（2024）后节奏放缓，"增量一切"的兑现度以 nightly release notes 为准。【实验】
- **salsa（rust-analyzer 的查询底层）**：修订号 + 红绿依赖跟踪 + **Durability 分级**（高耐久输入变更时少重算依赖树）——现成的 Rust 增量框架，3.x 重写后极活跃（pushed 2026-10-03）。
- **rowan（rust-analyzer）**：红绿树（不可变绿树共享 + 红树父指针）+ **零失败解析**（ERROR 节点承载语法错误）——残缺/半成品代码照样有 AST 可跑分析。质量平台"边写边扫"的前提。
- **tree-sitter**：逐文件增量重解析 + 错误恢复的工业标准（编辑器生态），多语言 grammar 生态最全（MIT，活跃）。
- **快编译路线对照**：rustc_codegen_cranelift（2025-06 进度报告：476 commits）证明"换后端买编译速度"路线仍活着。
- **ADV 取舍**：salsa 的查询+耐久级模型作为 adv-parse/规则管线增量主干；rowan 树模型作为自研解析器的验收要求；Unison/Roc 的内容寻址作为缓存键设计参考（重命名/移动不失缓存）。

### ③ "工具链自带 lint/格式化"哲学（zig/gleam/roc 模式）对 ADV 的启示

- **gleam**：单二进制 `build/run/fmt/check/test/docs` + **LSP 内建在编译器里**（gleam-core），2025 年 LSP 持续增强（社区称某版 release 是 "language server 的 field day"）。
- **roc**：`check/fmt/test/docs` 内建，LSP 随工具链分发。
- **zig**：`zig fmt`/`zig build`/`zig test`/`zig ast-check`（语法级检查门）内建；LSP 外挂（ZLS）。`ast-check` 是"最小 lint 门"的极简实现——不做语义分析也能拦一类坏味道。
- **odin**：`odin check/run/test/doc` 内建，LSP 外挂（ols）——同为"内建门+外挂服务"混合。
- **对照组（外挂模式）**：ZLS/ols 证明外挂服务跟进语言演进的成本高（ZLS 长期追 comptime 的补全正确性）；gleam 内建 LSP 的漂移小得多。
- **对 ADV 的产品形态启示**：不做"插件拼装市场"，**一个 cli 子命令面**覆盖 check/fmt-等效/报告，质量门默认开启、零配置可用；LSP/JSON 报告/富文本三者消费同一份诊断数据（rustc 模式）；把"语义检查前就能跑的 ast-check 级门"分层出来，保证半成品代码也有产出。

### ④ 错误信息质量工程（错误码/解释/建议修复三件套，谁做得最好）

**rustc 做得最好**，且机制可拆解（不是文风好，是管线好）：
1. **错误码是跨版本稳定索引**（E0xxx）：文档按码检索（error-index）、跨工具引用、用户可记忆——ADV 的 rule_id 应该照此设计（稳定码，格式变更不改码）。
2. **解释与错误分离**：`rustc --explain E0308` 给长解释——错误正文保持短，深度内容放文档页。
3. **建议修复带适用性分级**：MachineApplicable/MaybeIncorrect/HasPlaceholders/Unspecified；rustfix/cargo fix 只自动应用 MachineApplicable——"能自动修"是显式承诺而非布尔值。
4. **文案与代码解耦**：诊断消息走 fluent 资源文件（#[diag] 派生），评审/审计/i18n 有据。
5. **multipart suggestion** 表达跨行修复；`--error-format=json` 让 IDE 与人类视图同源。
**小团队最佳样本是 Gleam**：编译器自带 LSP + "Rust 级错误信息"的用户口碑持续到 2025-12（Tymscar 博客）；其文案模板（短标题 + 行标注 + 下一步建议）可直接当 ADV 规则报告的文案规范。K2/FIR（Kotlin）补第三条路径：前端 IR 把解析/解析后/诊断分层，IDE 可对未解析完的代码出诊断——诊断作为"对 IR 的查询"而非编译副产品。

### ⑤ watchlist（成熟度分层）

- **现在就能抄机制**（维护中）：rustc 诊断体系、annotate-snippets、codespan-reporting、miette、salsa、rowan/rust-analyzer、tree-sitter
- **实验/跟踪**（【实验】，活跃）：Roc、Mojo 1.0（2026 开源化）、Carbon（0.1 未达）、Hylo、Koka、Unison、egglog、Aiken、tower-lsp
- **低活跃/风险**（不依赖）：ariadne（archived）、chumsky（archived）、Vale（gh 404 待核）、Hare（gh 404）、QBE（低频稳定）、V（警示样本）

---

## 1. 机制级条目（15）

格式：定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接。

### 1.1 rustc 诊断体系
- **定位**：Rust 编译器的诊断生产/结构化/渲染三路管线，行业事实标杆。
- **可抄机制**：① 稳定错误码 + `--explain` 长解释分离；② suggestion 带 Applicability 分级，自动修复工具只应用 MachineApplicable；③ `--error-format=json` 结构化契约（level/code/message/spans primary/secondary/children/suggestions），人类渲染只是 JSON 投影；④ 诊断文案 fluent 资源文件解耦（#[diag] 派生）。
- **档位**：【吸收】——作为 ADV 报告契约蓝本（rule_id=稳定码、fix 带 applicability、JSON 双路）。
- **第一步**：定 ADV 诊断 JSON schema（对齐 rustc 字段 + rule_id + applicability + fix_id），写一版 schema 冻结测试。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（rust-lang 主体）。
- **链接**：https://rustc-dev-guide.rust-lang.org/diagnostics.html ；https://doc.rust-lang.org/error_codes/error-index.html

### 1.2 annotate-snippets（渲染层主选）
- **定位**：rust-lang 系的 rustc 风格渲染库，结构化输入（Annotation/Level/Snippet），目标是与 rustc 输出统一。
- **可抄机制**：primary/secondary 标签模型；多行 span 折叠；"来源行+列标注"极简风格——与 rustc 用户心智一致，ADV 报告观感天然可信。
- **档位**：【吸收】（cli 渲染层主选）。
- **第一步**：用 10 条 ADV 真实规则样例对比 annotate-snippets 与 codespan 的渲染宽度/断行/多行表现，钉死选型（同时核 rust-lang 仓库 404 的迁移去向）。
- **许可证/成熟度**：MIT OR Apache-2.0（crate 元数据）；维护中。
- **链接**：https://crates.io/crates/annotate-snippets

### 1.3 codespan-reporting
- **定位**：ariadne 的前身与对照（ariadne 作者自述受 codespan 启发）；files 抽象 + Label{style,range,message} 模型 + term/graphical 双渲染；codespan 家族还有 codespan-lsp（span→LSP location 换算）。
- **可抄机制**：SimpleFiles 字节 span→行列换算；输出格式稳定（对 golden 测试友好）。
- **档位**：【有界】（渲染层备选；不再作为"停滞弃维护"处理——0.13.1（2025-10）仍在发版）。
- **第一步**：并入 1.2 的对比实验；若 annotate-snippets 可用则仅借鉴其 files/LSP 换算层。
- **许可证/成熟度**：Apache-2.0 OR MIT；维护中（codespan 仓库 pushed 2026-02-28）。
- **链接**：https://github.com/brendanzab/codespan

### 1.4 ariadne
- **定位**：花哨风格诊断渲染（多 span 彩色连接线、帮助块），与 chumsky 同作者形成"解析错误→渲染"直通车。
- **可抄机制**：多报告布局思路可作风格参考；**反面机制**：布局启发式版本间漂移 + 仓库归档——golden 测试不能建在渲染库的像素上，要建在自己的 JSON schema 上。
- **档位**：【不吸收】（依赖不可入；风格可参考）。
- **第一步**：无（从选型表划掉，归档状态记入 registry）。
- **许可证/成熟度**：MIT；**弃维护（ARCHIVED，gh 2026-10-03）**。
- **链接**：https://github.com/zesterer/ariadne

### 1.5 miette
- **定位**：应用级错误框架：Diagnostic trait（code+severity+help+source spans）+ cause 链 + 图形 snippet 渲染 + serde JSON。
- **可抄机制**：错误对象与渲染器解耦（插件式 renderer）；错误链与 span 标注并存——ADV cli 自身运行错误（配置错/路径错）的 UX 模板。
- **档位**：【有界】（cli 运行错误 UX；不用于规则报告）。
- **第一步**：cli 错误路径（参数/配置/IO 错误）试装 miette，与手写 eprintln 对比。
- **许可证/成熟度**：Apache-2.0；维护中（pushed 2026-06-25）。
- **链接**：https://github.com/zkat/miette

### 1.6 rust-analyzer（rowan/容错解析/LSP）
- **定位**：Rust IDE 后端：红绿树 + 零失败解析 + LSP 服务。
- **可抄机制**：① rowan 红-绿树（不可变共享 + 父指针），编辑 O(改动) 重建；② 解析永不失败，ERROR 节点承载错误——质量平台对半成品代码也能跑规则的前提；③ 诊断在 IDE 与编译器同源（消费 rustc JSON + 自产诊断）。
- **档位**：【吸收】（adv-parse 的树模型与容错验收标准）。
- **第一步**：给 adv-parse 定"零失败解析"验收样例（缺括号/半截函数/截断文件必须产出部分结果 + ERROR 节点）。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（pushed 2026-10-03）。
- **链接**：https://github.com/rust-lang/rust-analyzer

### 1.7 salsa（增量计算框架）
- **定位**：rust-analyzer 的查询底层；输入/查询/修订号/Durability 分级的增量计算框架。
- **可抄机制**：① 查询=纯函数+依赖跟踪，改哪算哪；② Durability 分级（文件内容/依赖清单/配置三档失效粒度）——正是 ADV 缓存分层要的形状。
- **档位**：【吸收】（adv-parse/规则管线的增量骨架；先抄模型，直用或仿制再定）。
- **第一步**：把 ADV 缓存按 salsa 语义分层：源文件哈希（低耐久）/依赖清单（中）/扫描配置（高），写失效矩阵。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中、极活跃（pushed 2026-10-03）。
- **链接**：https://github.com/salsa-rs/salsa

### 1.8 Roc
- **定位**：以"增量编译一切"为卖点的语言；平台抽象（语言与宿主/运行时通过 Platform 接口解耦）。
- **可抄机制**：① 内容寻址 DAG（模块/类型/产物哈希化，改动只重算子图；与 Unison 思想同源）；② 平台抽象——"语言内核"与"宿主能力"分离的接口设计；③ check/fmt/test/docs 内建工具链。
- **2026 现状**：编译器完成 **Rust→Zig 重写（487 天）**，宣称 **35ms 增量重编译**（Developers Digest/rtfeldman 博客，2025–2026）；社区驱动（2024 起融资结束）；nightly 兑现度以 release notes 为准。
- **档位**：【watch】（机制设计值得持续跟踪；团队节奏风险）。
- **第一步**：订阅 roc-lang/roc release notes；把"内容寻址缓存键"记入 adv-parse 缓存设计候选。
- **许可证/成熟度**：UPL-1.0（gh）；【实验】；活跃（pushed 2026-10-03）。
- **链接**：https://github.com/roc-lang/roc ；https://www.roc-lang.org

### 1.9 Gleam
- **定位**：BEAM 上的静态类型语言，编译目标 Erlang/JS；1.0（2024-03）后月度节奏（v1.8 2025-02 → v1.14 2025 末，InfoWorld 报道 external types）。
- **可抄机制**：① **LSP 内建在编译器 crate**（gleam-core），单二进制工具链 build/run/fmt/check/test/docs——"工具链自带质量门"完整样板；② 错误信息工程：短标题+行标注+下一步建议，2025-12 用户文（Tymscar）继续背书 "Rust-like errors"；③ 双后端策略（同一前端 lowered 到两个运行时）。
- **档位**：【吸收】（诊断文案模板 + 内建工具链哲学 → adv-rules/cli）。
- **第一步**：ADV cli 定单命令面（check/fmt-等效/report），错误文案按"≤1 行标题 + 主 span + 1 条 hint + 1 条建议修复"模板冻结 10 个样例。
- **许可证/成熟度**：Apache-2.0；维护中（pushed 2026-10-01）。
- **链接**：https://github.com/gleam-lang/gleam ；https://gleam.run

### 1.10 Zig
- **定位**：系统语言；对 ADV 的价值在哲学而非语言本身。
- **可抄机制**：① **comptime**：同一语言的编译期执行（类型即值），消除宏/模板两套元语言——ADV 内部样板代码可走"普通函数+编译期参数"而非 DSL 宏；② **显式 Allocator**：所有资源显式传参、无隐式全局分配——映射为"扫描上下文/缓存/网络句柄显式贯穿调用点，禁止隐藏 IO"；③ `zig fmt`/`zig ast-check` 内建语法级质量门。
- **版本锚**：0.15（2025-08，新 std.Io Reader/Writer）→ 0.15.1（2025-08-22）→ **0.16（2026-04，io_uring/GCD 异步 I/O）**；主仓已迁离 GitHub（镜像 pushed 停在 2025-11-27），以 ziglang.org 为准。
- **档位**：【有界】（抄哲学与资源显式化，不抄语言）。
- **第一步**：写 ADV"无隐式 IO/分配"内部清单：哪些函数允许碰文件系统/网络，调用点必须显式传 context；进 code review 检查单。
- **许可证/成熟度**：MIT；维护中（0.16 已发布）。
- **链接**：https://ziglang.org ；https://github.com/ziglang/zig（镜像）

### 1.11 MLIR
- **定位**：LLVM 之下的多方言编译器基础设施。
- **可抄机制**：① 方言(Dialect)+Op+渐进式 lowering——每层只做一类变换，规则管线可按此分层（语法层→结构层→语义层→报告层）；② Pass infrastructure 的 **analysis manager**：分析结果按依赖图缓存+失效——adv-rules 的"分析复用"现成模型。
- **档位**：【reference】（不写编译器；借分层与缓存失效模型）。
- **第一步**：把 adv-rules 的规则按 MLIR 四层（语法/结构/语义/报告）归位，检查现有规则是否跨层混杂。
- **许可证/成熟度**：Apache-2.0 WITH LLVM-exception；维护中（LLVM 主干）。
- **链接**：https://mlir.llvm.org

### 1.12 Cranelift
- **定位**：wasmtime 的快速代码生成后端。
- **可抄机制**：① **ISLE DSL** 写 lowering 规则（规则即数据，可枚举/可测试）；② 每个变换后跑 **verifier** 自校验中间态不变量——映射为 ADV"每条规则声明前置/后置条件并可自检"；③ BA+学界持续验证路线（cranelift-egraph / peephole 优化的 SMT 验证）。
- **2025–2026 锚**：ISLE 迁移仍在进行（wasmtime 39.x 仍在调整 ISLE 构建项）；wasm 异常处理 proposal 落地（BA 博客 2025-11-06）；rustc_codegen_cranelift 2025-06 进度报告 476 commits。
- **档位**：【reference】。
- **第一步**：给 adv-rules 高危规则（自动修复类）补"前置条件+后置自检"字段设计稿。
- **许可证/成熟度**：Apache-2.0（WITH LLVM-exception）；维护中（wasmtime pushed 2026-10-02）。
- **链接**：https://github.com/bytecodealliance/wasmtime/tree/main/cranelift

### 1.13 Koka
- **定位**：微软研究系效果系统语言。
- **可抄机制**：① **效果行多态**：函数类型携带效果集合——ADV 规则/扫描器声明效果（读缓存/写报告/碰网络），调度器据此做并行与沙箱决策；② **Perceus** 精确引用计数+原地复用（资源确定性释放的研究样本）。
- **档位**：【watch】（效果标注思想进 adv-rules 规则元数据候选）。
- **第一步**：规则注册表加可选 `effects: [read-files|net|exec]` 字段设计（与 E8 沙箱域对表）。
- **许可证/成熟度**：MIT-0 系（仓库自定义 LICENSE，gh NOASSERTION，待核）；【实验】；活跃（pushed 2026-10-01）。
- **链接**：https://github.com/koka-lang/koka

### 1.14 Hylo
- **定位**：可变值语义研究语言（默认值语义、无隐式共享）。
- **可抄机制**：**参数约定即所有权**——`inout`/`sink`/subparam 传参约定替代借用标注，所有权由值语义推出而非逐处标注：类型系统设计对照组（说明"防误用"可以换一种根基，不必全抄借用检查）。
- **档位**：【watch】。
- **第一步**：无（跟踪 Hylo-lang 仓库即可；pushed 2026-09-28）。
- **许可证/成熟度**：Apache-2.0；【实验】。
- **链接**：https://github.com/Hylo-lang/Hylo ；https://hylo-lang.org

### 1.15 Unison
- **定位**：内容寻址代码的函数式语言（定义=AST 语义哈希；改名不改哈希；ability=效果）。
- **可抄机制**：**代码身份与文件系统解耦**——ADV 缓存键对照：规则文件/解析产物按语义哈希寻址，重命名/移动不失缓存、同一规则多副本自动去重。**反面**：社区批评内容寻址会"杀掉 API 级抽象"（Lobste.rs 讨论）——引用粒度要选在"产物"层而非"API"层。
- **档位**：【watch】。2025 状态：活跃（pushed 2026-10-02），已有语言服务器；LWN 2025-08-21 效果系统深文再引其 ability 设计。
- **第一步**：adv-parse 产物缓存键从"路径+mtime"改为"内容哈希"的实验分支评估。
- **许可证/成熟度**：MIT（仓库自定义 LICENSE，gh NOASSERTION，待核）；【实验】。
- **链接**：https://github.com/unisonweb/unison ；https://www.unison-lang.org

---

## 2. 扫视（22 条：一句机制 + 档位 + 许可证/成熟度 + 链接）

| # | 名称 | 一句机制 | 档位 | 许可证/成熟度 | 链接 |
|---|---|---|---|---|---|
| 16 | Mojo | MLIR 之上的 Python 系系统语言；**1.0 已发布（2026-08 前后）且编译器/工具链 Apache-2.0 全开源（2026）**——2025-04 时仍捆绑 Max，兑现了开源承诺 | watch | Apache-2.0 WITH LLVM-exception；【实验】（商用绑定 Modular）；活跃（pushed 2026-10-03） | https://github.com/modularml/mojo |
| 17 | Carbon | C++ 互操作实验语言；**0.1 里程碑未达**（2025 透明度报告 2026-01-12："working towards 0.1 (next year, hopefully!)"）；其"年度透明度报告"机制本身值得学（工程进度可审计） | watch | Apache-2.0 WITH LLVM-exception；【实验】；活跃 | https://github.com/carbon-language/carbon-lang |
| 18 | Vale | 区域借用/生成器检查（无 GC 手工内存的第三条路）思想；近年低活跃、gh 仓库 404（路径待核） | reference | 待核；【实验】低活跃 | https://vale.dev |
| 19 | Odin | 显式手工内存管理 + `odin check/run/test/doc` 内建工具链（"门内建、LSP 外挂"混合模式） | reference | Zlib（gh）；维护中（pushed 2026-10-03） | https://github.com/odin-lang/Odin |
| 20 | Futhark | 函数式 GPU 语言（数组融合→CUDA/OpenCL）；ADV GPU 档（R7 交界）的"纯函数+编译器自动并行"对照 | reference | ISC；维护中（pushed 2026-10-02） | https://github.com/diku-dk/futhark |
| 21 | QBE | 轻量 SSA 后端（非 LLVM 替代，Hare 采用）；证明"小而够用的后端"路线可行 | reference | MIT（c9x.me 标注）；低频稳定 | https://c9x.me/compile/ |
| 22 | tree-sitter | 逐文件增量重解析 + ERROR 节点错误恢复，多语言 grammar 生态最全；adv-parse 多语言备选（若未被其他域占用） | watch | MIT；维护中（pushed 2026-10-03） | https://github.com/tree-sitter/tree-sitter |
| 23 | chumsky | 组合子解析 + 一等错误恢复（1.0 alpha 重写）；**仓库 ARCHIVED**（gh 2026-10-03）→ 机制可引用，依赖勿入 | reference | MPL-2.0；弃维护 | https://github.com/zesterer/chumsky |
| 24 | egglog | Datalog + e-graph 等价饱和（规则即数据 + 不动点收敛）；adv-rules 规则去重/收敛/冲突检测的思想库 | watch | MIT；维护中（pushed 2026-10-02） | https://github.com/egraphs-good/egglog |
| 25 | CIRCT | MLIR 的硬件方言生态；dialect 模型可外溢的活证据 | reference | Apache-2.0 WITH LLVM-exception；维护中 | https://github.com/llvm/circt |
| 26 | Verus | Rust 程序验证（状态机 DSL + SMT）；**E6 域主责，此处仅登记**；活跃（pushed 2026-10-03） | reference | MIT；维护中 | https://github.com/verus-lang/verus |
| 27 | Idris2 | 定量类型理论（类型级资源计数）；依赖类型对照，不展开 | reference | 仓库 NOASSERTION 待核；维护中（低频，pushed 2026-09-18） | https://github.com/idris-lang/Idris2 |
| 28 | Agda | 证明助手对照（类型即命题）；不展开 | reference | 仓库 NOASSERTION 待核；活跃 | https://github.com/agda/agda |
| 29 | ZLS | Zig 外挂 LSP；长期追 comptime 正确性补全——语言设计直接影响工具可行性的信号源 | reference | MIT；维护中（pushed 2026-10-03） | https://github.com/zigtools/zls |
| 30 | ols | Odin 外挂 LSP；外挂跟进成本对照组（与 gleam 内建对照） | reference | MIT；维护中（pushed 2026-10-01） | https://github.com/DanielGavin/ols |
| 31 | tower-lsp | Rust LSP 服务框架（tower 中间件式）；ADV 若出 LSP 服务的框架候选 | watch | Apache-2.0；维护中（pushed 2026-09-11） | https://github.com/tower-lsp-community/tower-lsp |
| 32 | Aiken | Cardano 智能合约语言；小团队错误信息工程的正面样本（类型+错误提示）；活跃 | watch | Apache-2.0；维护中（pushed 2026-10-03） | https://github.com/aiken-lang/aiken |
| 33 | Wuffs | Google 的"不可信输入解码器"语言（边界检查/循环约束可证明）；安全平台对"安全语言"的参照系 | reference | Apache-2.0（仓库 NOASSERTION）；维护中（pushed 2026-09-29） | https://github.com/google/wuffs |
| 34 | V (vlang) | 宣称（自举/速度/功能）与工程现状长期有差距的警示样本；方法论不采 | 不吸收 | MIT；活跃（pushed 2026-10-03） | https://github.com/vlang/v |
| 35 | Hare | 极简系统语言（无泛型、手工内存）；小社区低活跃；gh 路径 404 待核 | reference | MIT（官网）；低活跃 | https://harelang.org |
| 36 | Kotlin K2/FIR | 前端 IR（FIR）把解析/解析后/诊断分层：**诊断成为对 IR 的查询**，IDE 可对未解析完的代码出诊断——诊断架构第三条路 | reference | Apache-2.0；维护中（K2 随 Kotlin 2.x 出货） | https://github.com/JetBrains/kotlin |
| 37 | rustc_codegen_cranelift | 用 Cranelift 后端快速编译 Rust（调试构建速度路线）；2025-06 进度报告 476 commits | reference | Apache-2.0 WITH LLVM-exception；维护中 | https://github.com/rust-lang/rustc_codegen_cranelift |

---

## 3. 2025–2026 前沿信号（带锚）

1. **Roc Rust→Zig 重写完成（487 天）**，宣称 35ms 增量重编译（Developers Digest / rtfeldman 博客，2025–2026）；roc-lang/roc 活跃（pushed 2026-10-03）。
2. **Mojo 1.0 发布（2026-08 前后）+ 编译器/工具链 Apache-2.0 全开源（2026）**；2025-04 时仍捆绑 Max——开源承诺已兑现（openai-hub/Linuxiac/HackerNoon 报道）。
3. **Carbon 0.1 未达**：2025 年度透明度报告（2026-01-12）明言 "working towards 0.1 (next year, hopefully!)"——透明度报告机制本身可学。
4. **Zig 0.15（2025-08，新 std.Io）→ 0.16（2026-04，io_uring/GCD 异步）**；主仓迁离 GitHub（镜像 pushed 停在 2025-11-27）——本地优先/自托管基础设施的一个行业信号。
5. **Gleam 月度节奏贯穿 2025**：v1.8（2025-02）→ v1.14（2025 末，external types）；LSP 内建持续增强；Gleam Weekly #75（2025-12-18）。
6. **诊断渲染库分水岭**：codespan-reporting 0.13.1（2025-10）仍发版；**ariadne 与 chumsky 仓库 ARCHIVED**（gh 2026-10-03）——zesterer 系两个库同时归档，直接改变渲染层选型。
7. **Cranelift**：ISLE 迁移进行中（wasmtime 39.x）；wasm 异常处理落地（BA 博客 2025-11-06）；BA+学界持续做 peephole/egraph 验证；rustc_codegen_cranelift 2025-06 报告 476 commits。
8. **salsa 3.x 极活跃**（pushed 2026-10-03）——Rust 增量计算的现成底座。
9. **Unison**：活跃（pushed 2026-10-02）、已有语言服务器；LWN 2025-08-21 效果系统专文再引其 ability 设计（Koka/Unison 的效果系统在 2025 进入主流技术媒体视野）。
10. **Verus 活跃**（pushed 2026-10-03）——Rust 验证栈热度的锚（细节归 E6）。

## 4. Top-3（对 ADV 最值得立刻动手的三个）

1. **rustc 诊断契约 + annotate-snippets 渲染**：rule_id 稳定码 + --explain 式长解释页 + 建议修复带 Applicability + JSON/富文本双投影（渲染只是 JSON 的投影）。ariadne 归档进一步坐实此选型。（1.1/1.2，§0-①④）
2. **rust-analyzer 双件套进 adv-parse**：rowan 红绿树（零失败解析、ERROR 节点）+ salsa（查询+Durability 三档失效）——"残缺代码可分析、改哪算哪"是本地优先平台的命门。（1.6/1.7，§0-②）
3. **"工具链自带质量门"产品形态**：学 gleam/roc/zig——ADV 一个 cli 出 check/fmt-等效/report，零配置默认开启；错误文案冻结"≤1 行标题+主 span+1 条 hint+1 条建议修复"模板（gleam 是小团队正面样本）；把 ast-check 级"语法门"分层出来保证半成品代码有产出。（1.9/1.10，§0-③）

## 5. 一句话总账

37 条：吸收 5（rustc 体系、annotate-snippets、rowan/rust-analyzer、salsa、gleam 模式）、有界 3（codespan-reporting、miette、zig 哲学）、不吸收 2（ariadne 归档、vlang 警示）、watch 10、reference 17。机制级 15 条。所有 license/活跃度带 2026-10-03 的 gh/crates.io 锚；唯一未决复核项：rust-lang/annotate-snippets 仓库路径 404（可能迁移）与 Vale/Hare 仓库路径 404。
