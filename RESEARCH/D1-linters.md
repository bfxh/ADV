# D1 · Linter 全家（机制级深挖）

> 调研日期：2026-10-03 · 方法：Web 检索 2025–2026 资料为主（官方文档/changelog/官方博客优先），机制描述以可引用实现为准。
> 与 wave-0 分工：`06-rust-quality-toolchain.md` 已覆盖 clippy/dylint 基础面，本文聚焦**规则工程**（表达层、生命周期、fix 分级、抑制、缓存、聚合）。
> 本文档 32 条机制级条目；registry 见 `registry/D1.jsonl`。档位体系：吸收 | 有界 | 不吸收 | watch | reference。

## 0. 五个重点问题的浓缩答案（详证见专题条目）

| # | 问题 | 一句话结论 |
|---|------|-----------|
| ① | 规则 DSL 组织模式 | 四种谱系：代码内嵌（ESLint/clippy/rubocop）、声明式规则即数据（Semgrep/markdownlint/PMD XPath）、声明+逃生舱混合（Ruff：Rust 实现但中心化规则登记表+宏生成索引）、声明+数据查询 DSL 插件（Biome GritQL）。adv-rules 的 YAML taint spec + AstKind matcher 属于第四种近亲：**80% 模式规则走声明 YAML、20% 语义规则走 Rust 回调逃生舱、全部规则共享中心化登记表（code/文档/测试/fix 元数据同源生成）**（详见 D1-032） |
| ② | --fix 安全分级 | 共识是三档语义：rustc/clang 的 Applicability（MachineApplicable/MaybeIncorrect/…）、Ruff 的 Safe/Unsafe/DisplayOnly、Biome/Rubocop 的 safe/unsafe 两档 + 独立显式开关（`--unsafe-fixes` / `-A`）。默认只跑 safe，unsafe 必须用户显式点名（详见 D1-027） |
| ③ | 聚合器架构 | golangci-lint v2：多 linter 合并为单 MetaLinter pass → 产出统一 `result.Issue` → 结果处理器管道（合并/按行去重/max-per-linter/max-same-issues/diff/fixer/printer）→ 缓存以"issue 身份"为前提（详见 D1-013/D1-014） |
| ④ | inline 抑制粒度与到期 | 粒度谱系：行内尾注（noqa）→ 行/区间/文件（ESLint）→ 符号属性（rustc allow/expect、detekt @Suppress）。**唯一原生"到期"机制是 rustc `#[expect]`（抑制未兑现即告警，Rust 1.81 稳定）**；ESLint/Ruff 都没有原生到期，靠外部工具扫描"未用抑制"（RUF100、eslint --report-unused-disable-directives）（详见 D1-029） |
| ⑤ | 增量 lint 缓存键 | 关键教训来自 golangci-lint #6428：缓存键混入绝对路径导致同内容不同路径全 miss。正确键 = 文件内容哈希 ⊕ 规则集指纹 ⊕ 引擎/规则版本 ⊕ 目标类型，**路径无关**。ESLint 提供 metadata（mtime）与 content 两种策略（详见 D1-031） |

## 1. JS/TS 阵营

### D1-001 ESLint 核心规则 API
- **定位**：JS/TS lint 事实标准；规则的工程形态（Rule 对象 + 上下文对象）被几乎所有后续工具参照。
- **可抄机制**：① 规则=纯对象（`create(context)` 返回 visitor map），`context.sourceCode` 提供 token 流/注释/作用域查询——"规则函数只拿上下文，不拿引擎"的边界设计；② 规则 `meta`（docs/fixable/messages/schema）是机器可读契约，JSON Schema 校验插件规则选项，docs.url 由登记表生成；③ 双通道诊断：`fix`（可自动应用）与 `suggestions`（需用户逐条确认），后者覆盖"能修但不能盲修"的中间态。
- **档位**：吸收（meta schema + fix/suggestions 双通道直接进 adv-rules）。
- **第一步动作**：在 adv-rules 规则 trait 里定义 `meta`（可修复性/消息模板/选项 schema），与规则登记表同源生成文档索引。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://eslint.org/docs/latest/extend/custom-rules

### D1-002 ESLint flat config 演进
- **定位**：v9（2024-04）起 flat config 为默认；2025 年完成生态收尾。
- **可抄机制**：① 配置=数组对象，按 glob 匹配叠加，后项覆盖前项——可预测的合并语义（对照 .eslintrc 时代的远程继承解析）；② 2025-03 官方把 `extends` 加回 flat config（官方博客 "Evolving flat config with extends"），承认纯组合数组的可用性代价；③ `globalIgnores`/`defineConfig` helper + 2025-05（v9.26）内置 MCP server，让 AI 助手直接查规则与结果。
- **档位**：有界（抄"数组叠加+显式 extends"的配置合并语义，不抄 JS 配置本身）。
- **第一步动作**：ADV 的 `adv.toml` 规则启用层设计成"有序片段叠加 + 路径 glob 归属"，片段可被 `extends` 引用。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://eslint.org/blog/2025/03/flat-config-extends/

### D1-003 ESLint RuleTester（规则测试格式）
- **定位**：规则单元测试的标杆格式，新引擎普遍仿制。
- **可抄机制**：① valid/invalid 两组用例数组，invalid 用例声明期望 `errors:[{messageId, data, type}]` 与 `output`（期望 fix 结果）——**期望输出即回归基准**；② `messageId` 把消息文案与断言解耦，改文案不炸测试；③ 测试跑在与产线相同的解析管线（防止测试专项路径）。
- **档位**：吸收（用例格式直接进 adv-rules 的规则测试 DSL）。
- **第一步动作**：为每条 YAML 规则定义 `tests: [valid…, invalid(code, expect_diagnostics, expect_fix?)]`，xtask 生成快照回归。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://eslint.org/docs/latest/integrate/nodejs-api#ruletester

### D1-004 eslint-plugin-* 生态（react / import / security 为代表）
- **定位**：插件生态≈上千个第三方规则包；其工程约定比任何单条规则更重要。
- **可抄机制**：① 插件导出 `{rules, configs, processors}` 三件套，`recommended` 配置本身是"规则即数据"的发布物；② 2024-02 RFC 起推动插件 flat config 导出标准化（此前社区统计有 7+ 种写法——标准化失败的教训：**发布物格式必须一开始就统一**）；③ 格式类规则整体外迁 @stylistic（core 砍单的先例：规则集要有"出清"机制）。
- **档位**：有界（抄导出契约与"配置即发布物"，不集成 JS 插件）。
- **第一步动作**：定义 ADV 第三方规则包（若有）的单一导出格式，禁止多写法并存。
- **许可证/成熟度**：MIT 为主 / 维护中（个体参差）。
- **链接**：https://eslint.org/docs/latest/extend/plugins

### D1-005 typescript-eslint（类型感知 lint 的成本与缓存）
- **定位**：类型感知规则的代价控制教科书；v8（2024-09）起 `parserOptions.projectService` 稳定。
- **可抄机制**：① 分层启用：typed-linting 比非 typed 慢一个数量级，官方 troubleshooting 明确"只对需要的文件开 type-checked"（配置文件/测试关闭）——**按文件域分级付类型税**；② projectService 复用编辑器同一套 project 实例，大库上比 per-tsconfig 建 program 更省内存，且让非 typed lint 也提速；③ 类型信息不可跨 lint 调用持久缓存（TS Program 无法廉价序列化），这是它被 tsgolint/Biome 重写绕开的根本原因。
- **档位**：有界（抄"类型税分级"与"缓存不可持久→换内核"的教训）。
- **第一步动作**：ADV 规则分层标注 `requires: syntax|types`，类型税只收在声明过的目标上。
- **许可证/成熟度**：BSD-2-Clause / 维护中。
- **链接**：https://typescript-eslint.io/troubleshooting/typed-linting

### D1-006 Biome 2.x（Biotype）
- **定位**：Rust 单二进制 JS 工具链；v2.0（2025-06-17）自研类型推断 + 多文件分析。
- **可抄机制**：① 自研类型推断引擎做类型感知规则，不绑 tsc：官方明示 `noFloatingPromises` 约覆盖 typescript-eslint 版本的 75% 用例（dev.to 深测口径）——**"够用的类型推断"换 10 倍速**的取舍有公开数据；② v2 引入多文件分析（规则可跨文件查询，如 noImportCycles），且做成 opt-in 防止拖慢单文件路径；③ GritQL 作为插件规则 DSL（树查询语言写规则）+ 规则按 "domains"（框架域）分组推荐。
- **档位**：吸收（domains 分组 + opt-in 跨文件分析 + GritQL 思路）。
- **第一步动作**：adv-rules 增加规则"域"标签（框架/风险域），域级推荐集与全量集分离。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（2.4.x，450+ 规则为 2.4.x 第三方转述口径）。
- **链接**：https://biomejs.dev/blog/biome-v2/

### D1-007 oxlint / oxc + tsgolint
- **定位**：并发 Rust lint 内核；2026 现状=两条腿：Rust 快规则面 + tsgolint（Go 重写 typescript-eslint）类型感知面。
- **可抄机制**：① **AstKind matcher**：语法树节点预分类为 enum Kind，规则声明订阅的 Kind 才回调——命中路径零分支/缓存友好，是 adv-rules matcher 的直接同构物；② rayon 并发 + 每文件独立诊断收集，无共享可变状态；③ tsgolint 架构=独立 Go 二进制（typescript-go），oxlint 把需要语义的规则**委托**过去：2025-08 技术预览 → 2025-12 alpha（43 条）→ 2026-09 稳定 v7（59/61 条 typescript-eslint 类型感知规则，InfoQ 口径）；alpha 期已支持规则内联禁用与自动修复。
- **档位**：吸收（AstKind 订阅模型 + "重内核/轻内核委托"两脚架构）。
- **第一步动作**：把 adv-rules matcher 定型为 Kind-订阅派发；为将来"语义规则外部进程委托"预留协议边界。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中（tsgolint 2026-09 达稳定版）。
- **链接**：https://oxc.rs · https://github.com/oxc-project/tsgolint · https://www.infoq.com（2026-09 报道）

## 2. Python 阵营

### D1-008 Ruff 规则生命周期（版本化 / deprecated / 预览通道）
- **定位**：900+ 规则（官方 rules index 口径）的流水线管理是业界最完整的。
- **可抄机制**：① 生命周期四态：preview → stable → deprecated → removed；所有未标注规则即 stable；② **版本奇偶通道**：偶数 minor（0.14.x）=稳定，奇数=preview；新规则至少在 preview 待一个 minor 才可晋升——用版本号本身承载通道语义，零配置成本；③ deprecated 规则必须按精确 code 选择、不再被前缀组名激活，且在下个版本移除（changelog 固定栏目）——deprecated 管理有牙齿。
- **档位**：吸收（生命周期四态 + preview 通道 + deprecated 精确选择）。
- **第一步动作**：adv-rules 登记表加 `status: preview|stable|deprecated|removed` 字段；xtask 校验 deprecated 规则只能精确引用。
- **许可证/成熟度**：MIT / 维护中（0.14.1 2025-10-16；0.16 默认启用规则集扩至 413 条，changelog 口径）。
- **链接**：https://docs.astral.sh/ruff/versioning · https://docs.astral.sh/ruff/rules

### D1-009 Ruff fix 分级 + 测试 + noqa 工程
- **定位**：fix 元数据与抑制生态最体系化的单一实现。
- **可抄机制**：① fix 按安全性元数据分级（safe/unsafe；内部另有 DisplayOnly——只出诊断不改写），CLI 以 `--unsafe-fixes` 显式解锁，`--fix-only` 支持"只应用修复不重扫"；② 抑制 `# noqa [code1, code2]` 多码语法 + RUF100 检测未用 noqa（抑制必须自证有用）；③ 测试=fixture 文件 + 快照（`crates/ruff_linter/resources/test/fixtures` + snapshots），规则实现与测试用例在 crate 内相邻，加规则即加 fixture。
- **档位**：吸收（三件套全部进 adv-rules/xtask-gates）。
- **第一步动作**：ADV 诊断输出加 `--show-fixes`（DisplayOnly 展示）与 `--fix --unsafe-fixes` 开关组合；抑制解析器支持多规则码。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://docs.astral.sh/ruff/linter/#fixes

### D1-010 Pylint
- **定位**：老牌重型 linter；对照物价值大于集成价值（Ruff 已覆盖 wave-0）。
- **可抄机制**：① message 五类前缀（C/R/W/E/F = Convention/Refactor/Warning/Error/Fatal）——**按"严重性质"而非工具来源编号**；② symbol（名字）与 id（数字码）双标识，配置里两者可互换；③ astroid 静态推断 + 插件 `load-plugins` 动态加载；`--jobs` 多进程并行。
- **档位**：reference（不集成；编号双标识可借鉴）。
- **第一步动作**：无（登记即可）。
- **许可证/成熟度**：GPL-2.0-or-later / 维护中。
- **链接**：https://pylint.readthedocs.io

### D1-011 flake8 插件生态
- **定位**：经典插件聚合器（pyflakes/pycodestyle/mccabe）；被 Ruff 取代的现状下，价值在架构模式。
- **可抄机制**：① 插件经 setuptools entry-point（`flake8.extension`）自发现——零注册表维护，但带来"装了就跑"的失控面（对照 golangci 显式 enable 的取舍）；② noqa 聚合解析在聚合器层做（插件只见 AST 不见注释），**抑制是宿主职责而非规则职责**。
- **档位**：reference。
- **第一步动作**：无。
- **许可证/成熟度**：MIT / 维护中（低频发布）。
- **链接**：https://flake8.pycqa.org

## 3. Go 阵营

### D1-012 staticcheck
- **定位**：Go 单体质量内核；SA/S/ST/QF 分类清晰。
- **可抄机制**：① 检查分类 SA（staticanalysis/缺陷）、ST（stylecheck）、QF（quickfix，供 gopls 重构复用）——**同一检查同时服务 lint 与 IDE quickfix**；② 检查选择语法 `all` + `-ST1000` 排除（默认集继承+黑名单），golangci 直接透传该 DSL；③ 内部 facts（类型检查事实）跨包缓存。
- **档位**：有界（AGPL ⇒ 只抄分类法与 all/-排除语法，不碰代码）。
- **第一步动作**：adv-rules 规则码采用 `<类前缀><序号>`（如 ADV-SA-xxx）并支持 `all -X*` 选择表达式。
- **许可证/成熟度**：AGPL-3.0（go-tools 系）⇒ 只抄思想 / 维护中（150+ checks 为第三方口径）。
- **链接**：https://staticcheck.dev/docs/checks

### D1-013 golangci-lint（聚合器：合并/去重/裁剪）
- **定位**：多 linter 聚合器范本；v2（2025 系，v2.14.0 2026-09-24）架构稳定。
- **可抄机制**：① **MetaLinter**：把所有启用的 linter 合并为单个 pass 执行（消灭 N 个 linter = N 次解析/类型检查的浪费）；② 统一 `result.Issue` 结构 + 结果处理器管道：processor 链依次做合并（同位置同文本折叠）、uniq-by-line 去重、`max-per-linter`/`max-same-issues` 配额裁剪、git diff 过滤、fixer、printer——**聚合器的本质是 issue 的清洗流水线，不是并发跑 N 个工具**；③ issue 身份（identity）是去重与缓存的前提（官方 architecture 文档明确）。
- **档位**：吸收（MetaLinter 思想 + 结果管道 + 配额裁剪直接进 xtask-gates）。
- **第一步动作**：ADV 内部先统一诊断结构（含 rule/pos/related/fix），再实现去重与配额处理器链。
- **许可证/成熟度**：GPL-3.0 / 维护中。
- **链接**：https://golangci-lint.run/docs/contributing/architecture

### D1-014 golangci-lint 缓存与增量
- **定位**：聚合器缓存的反面教材 + 正面设计并存。
- **可抄机制**：① 两级缓存：facts cache（类型检查事实，源自 x/tools analysis facts）与 issues cache；② 缓存键设计坑：**键含绝对路径** ⇒ 同内容不同 worktree 全 miss（issue #6428，官方确认行为）——ADV 缓存键必须路径无关；③ "related information"陈旧条目随缓存存活的 bug（golangci-lint-action #420）⇒ 修复后必须让依赖条目失效，否则门禁出鬼。
- **档位**：吸收（键设计教训直接进 ADV 棘轮门）。
- **第一步动作**：定义 ADV 缓存键 = f(content_hash, rule_set_fingerprint, engine_version, target_kind)，路径无关；单测用两个同内容异路径 worktree 验证命中。
- **许可证/成熟度**：GPL-3.0 / 维护中。
- **链接**：https://github.com/golangci/golangci-lint/issues/6428

### D1-015 go vet / x/tools go/analysis 框架
- **定位**：Go 官方分析器框架；linters 的公共底座（staticcheck 也跑在其 pass 模型上）。
- **可抄机制**：① `pass` 对象（Fset/Files/TypesInfo/Report）定义分析器的最小接口——分析器之间不共享状态，靠框架编排；② **facts API**：分析器可声明产/消费 facts（如"此函数是纯的"），跨包传递并进缓存——跨文件语义信息的持久化通道；③ vet 随 `go test` 默认跑，工具粘性来自寄生宿主。
- **档位**：有界（pass 接口 + facts 概念进 adv-rules 的跨文件信息通道设计）。
- **第一步动作**：ADV 定义内部 pass 接口与 fact 注册（先只做"文件→诊断"，facts 留接口）。
- **许可证/成熟度**：BSD-3-Clause（Go 官方）/ 维护中。
- **链接**：https://pkg.go.dev/golang.org/x/tools/go/analysis

## 4. Rust 阵营

### D1-016 clippy（lint 分类 / group / --fix）
- **定位**：Rust 质量主 lint；wave-0 已覆盖基础，此处补规则工程。
- **可抄机制**：① 834 条 lint（官方 lint 列表页口径）分 9 组（correctness/style/pedantic/nursery/perf/cargo/complexity/suspicious/default）——组=默认级别集合，nursery（实验区）与 pedantic（严格区）隔离激进度；② 单 lint 级别可覆盖组级别（`-W clippy::pedantic -A clippy::module_name_repetitions`），clippy.toml 提供每 lint 配置项与 MSRV 感知；③ `cargo clippy --fix` 只应用 rustc 标记为 MachineApplicable 的修复——**修复可应用性是编译器级元数据**，不是 lint 工具自说自话。
- **档位**：吸收（组/单 lint 覆盖 + MachineApplicable 元数据进 xtask-gates/cli）。
- **第一步动作**：ADV 诊断带 `applicable: machine|maybe|none`，`adv fix` 默认只吃 machine。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中。
- **链接**：https://doc.rust-lang.org/clippy/

### D1-017 rustc 内置 lints 与 `#[expect]`
- **定位**：编译器级 lint；抑制机制的全部灵感来源。
- **可抄机制**：① lint level（allow/warn/deny/forbid）+ cap-lints 机制（依赖 crate 统一降级）——**供应链噪音隔离**；② `#[expect]`（Rust 1.81 稳定，2024-09）：声明"预期触发"，未触发即 `unfulfilled_lint_expectations` 告警——**原生到期语义**，防永久豁免的标杆；③ rustfix 的 Applicability 枚举（MachineApplicable/MaybeIncorrect/HasPlaceholders/Unspecified）。
- **档位**：吸收（`#[expect]` 语义 = ADV 抑制到期的设计基准；cap-lints 进依赖扫描面）。
- **第一步动作**：ADV 抑制语法设计为 `# adv: expect(rule)`——未触发即报，替代"永久 allow"。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中。
- **链接**：https://doc.rust-lang.org/rustc/lints/

### D1-018 dylint（自定义 Rust lint 路线）
- **定位**：Trail of Bits 维护的动态库 lint 机制；不改 clippy 源码加自定义 lint 的正统路线。
- **可抄机制**：① lint 编译为动态库（dylint-link 包装链接），`cargo dylint` 按目录/工作区发现加载；② 每个 lint 库自带版本与 toolchain 匹配要求（rustc 私有 API 跨版本不稳定 ⇒ **驱动器强制版本对齐**）；③ 可经 clippy driver 集成运行。
- **档位**：watch（wave-0 已登记；ADV 自研引擎不依赖 rustc 私有 API，无需此路线；观察其版本对齐设计）。
- **第一步动作**：无（仅当 ADV 需要临时 Rust 语义 lint 时复访）。
- **许可证/成熟度**：MIT OR Apache-2.0 / 维护中。
- **链接**：https://github.com/trailofbits/dylint

## 5. 其他语言

### D1-019 clang-tidy
- **定位**：C/C++ 检查组织与 FixIt 机制的鼻祖级实现。
- **可抄机制**：① 检查按模块前缀组织（modernize-/bugprone-/performance-/readability-/cppcoreguidelines-…，500+ checks 为第三方口径），前缀=模块=目录=注册命名空间三合一；② check 实现 = ClangTidyCheck 基类 + AST Matcher 声明式匹配（`registerMatchers` 绑定 matcher，`check` 收 MatchResult）——**声明式匹配器 + 回调处理**与 adv-rules YAML+AstKind 同构；③ FixItHint 与诊断并列输出（Diagnostics.h：diagnostic 含 notes 与 fixes），`-fix` 批量应用、`export-fixes` 导出 YAML 供外部工具（如 clang-apply-replacements）合并应用——**修复生成与应用分离**。
- **档位**：有界（前缀组织 + 修复生成/应用分离抄思想；C++ 域本身不集成）。
- **第一步动作**：ADV 的 fix 在诊断内生成（数据），由独立 `adv apply` 模块应用（逻辑），中间可落 diff 文件。
- **许可证/成熟度**：Apache-2.0-LLVM（含 UIUC 例外）/ 维护中。
- **链接**：https://clang.llvm.org/extra/clang-tidy/

### D1-020 ShellCheck
- **定位**：shell 静态分析孤品；"方言差异即规则"的样本。
- **可抄机制**：① 自研 Haskell 解析器（Parser.hs→AST.hs）把脚本解析成带唯一 token Id 的 AST，分析器走 AST 而非正则——bash/sh/dash/ksh 多方言同一内核，方言差异进诊断依据；② SC 编号 + severity（error/warning/info/style）四级；③ directives 体系（`# shellcheck disable=SCxxxx source=… shell=…`）行/区间生效，且 directive 本身被解析校验（写错会被告知）。
- **档位**：有界（directives 校验思想进 ADV 抑制解析器；shell 域若做集成则吸收其 SC 分类）。
- **第一步动作**：ADV 抑制注释解析后回显校验结果（未知规则码/写法错误必须报）。
- **许可证/成熟度**：GPL-3.0 ⇒ 只抄思想 / 维护中（0.11.0，2025-08-03）。
- **链接**：https://github.com/koalaman/shellcheck/releases

### D1-021 dart analyze
- **定位**：语言内核自带 lint 的代表（analyzer 在 SDK 内，非外部工具）。
- **可抄机制**：① 规则集以版本化包发布（`lints` core/recommended、`flutter_lints`），analysis_options.yaml 一行 include——**规则集=依赖包，升级即变更**；② `dart fix --apply` 批量应用 quick fix（官方文档明确 bulk application 支持）；③ 2025-12（Flutter 3.38）新 analyzer plugin 体系（AST visitor + 自定义 lint + fix），取代旧 isolate 插件；分析服务器常驻（IDE 与 CLI 同内核）。
- **档位**：有界（规则集=包 + fix 批量应用抄思想）。
- **第一步动作**：ADV 规则集按语义版本发 "preset 包"（default/strict/security），preset 间显式 include。
- **许可证/成熟度**：BSD-3-Clause（Dart SDK）/ 维护中。
- **链接**：https://dart.dev/tools/diagnostic-messages · https://github.com/dart-lang/lints

### D1-022 detekt
- **定位**：Kotlin 静态分析主工具；规则集工程与类型解析分层的样本。
- **可抄机制**：① 规则集（potential-bugs/complexity/style/naming/performance/coroutines/compose…）各自独立配置节，每规则一文档页；② type resolution 为可选增强：启用后部分规则才可用（需编译 classpath），未启用则降级——**同一规则引擎内显式声明规则的能力前提**；③ `@Suppress("RuleName")` 符号级抑制 + .editorconfig 承载配置（format 规则集兼容 ktlint 面）。
- **档位**：有界（规则能力前提声明抄进 adv-rules 规则元数据）。
- **第一步动作**：ADV 规则登记表加 `requires: syntax|symbol|types` 字段并在未满足时静默降级+汇总提示。
- **许可证/成熟度**：Apache-2.0 / 维护中。
- **链接**：https://detekt.dev

### D1-023 rubocop
- **定位**：autocorrect 安全分级的先行者（safe/unsafe 二分 2018-10 起系统化，Meta Redux 博文锚）。
- **可抄机制**：① cop 双属性：`Safe`（规则本身是否可能误报）与 `SafeAutoCorrect`——**规则可靠性与其修复可靠性分离声明**，cop unsafe 则其修复必然 unsafe（`rubocop -a` 只做 safe，`-A` 才做 unsafe）；② LSP 默认按 safe 档自动修，客户端可配置解锁 unsafe——编辑器内更保守；③ `--auto-gen-config` 生成"待办排除清单"（todo 模式）：存量违规一次性豁免+持续收敛，`--disable-uncorrectable` 把修不了的规则暂时关掉并记录。
- **档位**：吸收（SafeAutoCorrect 分离 + todo 模式直接进 xtask-gates 棘轮设计）。
- **第一步动作**：ADV 提供等价 `adv gate --autogen-exclusions`（生成可复审的存量豁免文件，diff 可审计）。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://docs.rubocop.org/rubocop/latest/usage/autocorrect.html

### D1-024 markdownlint
- **定位**：纯声明式规则集的极简样本。
- **可抄机制**：① MD001–MD05x 编号规则（50+），每规则=名称+别名+默认级别+是否 fixable 的薄描述；② 配置用别名或编号、可整体关闭某类；③ 规则可测性极强（纯文本变换），fixable 子集覆盖大多数规则。
- **档位**：reference。
- **第一步动作**：无。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://github.com/DavidAnson/markdownlint

### D1-025 PMD
- **定位**：多语言统一规则引擎的老样本；"规则即 XML 数据"路线的极限测试。
- **可抄机制**：① XPath 规则（规则即数据）覆盖简单模式，复杂规则退回 Java 实现——数据路线表达力天花板的存在证明；② 2025 roadmap（docs.pmd-code.org）：把 type-resolution 专属规则与普通规则合并，非 TR 环境自动 fallback——**类型解析降级路径的正面设计**；③ 多语言共享一个引擎 ⇒ 单条规则难以利用语言特有语义，跨语言复用率低于预期。
- **档位**：watch（为 adv-rules 的"声明式表达力边界"提供对照，暂不集成）。
- **第一步动作**：无（adv-rules 若出现大量"YAML 写不出"的规则时回看本条）。
- **许可证/成熟度**：BSD 风格 / 维护中。
- **链接**：https://docs.pmd-code.org/latest/

### D1-026 Semgrep
- **定位**：规则即数据的现代样本（YAML pattern/taint），与 adv-rules YAML taint spec 直接同构。
- **可抄机制**：① 规则四要素：pattern（含 metavariable）/pattern-either/pattern-not 组合逻辑 + taint 的 sources/propagators/sanitizers/sinks 四段式声明；② 规则发布物=YAML 文件仓库（semgrep-rules），规则可独立版本化、独立测试（rules 内嵌 `tests:` 块含 bad/good 配对）——**测试跟着规则走**；③ 社区版引擎 LGPL ⇒ 只抄格式。
- **档位**：吸收（bad/good 内嵌测试块与 taint 四段式进 adv-rules spec 格式）。
- **第一步动作**：adv-rules YAML schema 增加内嵌 `tests` 键与 taint 四段式字段名对齐（降低未来互相移植成本）。
- **许可证/成熟度**：LGPL-2.1（社区版引擎；规则仓库同）⇒ 只抄思想与格式 / 维护中。
- **链接**：https://semgrep.dev/docs/writing-rules/rule-syntax

## 6. 通用机制专题

### D1-027 专题A：--fix 安全档位设计
- **对比**：
  | 工具 | 档位 | 解锁方式 |
  |---|---|---|
  | rustc/clippy | MachineApplicable/MaybeIncorrect/…（Applicability 枚举） | `cargo clippy --fix` 只吃 machine |
  | clang-tidy | 无分级（FixIt 与诊断并列） | `-fix`；修复导出/应用分离 |
  | Ruff | Safe/Unsafe（内部另含 DisplayOnly：仅展示 diff 不参与 --fix） | 默认 safe；`--unsafe-fixes` 显式解锁 |
  | Biome | safe/unsafe | `--write` 默认 safe；`--write --unsafe` 解锁 |
  | RuboCop | Safe cop/SafeAutoCorrect 双属性 | `-a` safe / `-A` unsafe；LSP 默认 safe |
  | ESLint | fix 与 suggestions 双通道 | fix 自动；suggestion 需逐条确认 |
  | Dart | quick fix/assist 分离 | `dart fix --apply` 批量 |
- **设计结论**：共识=**默认只跑安全档 + unsafe 需显式开关 + 存在"仅展示"档**。adv-rules 建议：`applicable ∈ {machine, maybe, display}` 三档 + `adv fix` 默认 machine、`--unsafe-fixes` 解锁 maybe、display 档进 `adv gate` 报告供人审。
- **档位**：吸收。**许可证/成熟度**：—（专题）。**链接**：见各工具条目。

### D1-028 专题B：规则文档与测试格式
- **机制**：① ESLint RuleTester：valid/invalid+messageId+output 期望（见 D1-003）；② Ruff：fixture 文件+快照，规则与 fixture 相邻（见 D1-009）；③ Semgrep：规则 YAML 内嵌 bad/good 测试对（见 D1-026）；④ clippy：compiletest UI 测试（.stderr/.stdout 快照随 rustc 版本更新）；⑤ 共同点：**期望值必须随用例落盘、与规则同 PR 变更**；差异点：ESLint 的断言是结构化字段，快照系是整段文本 diff。
- **给 adv-rules**：结构化断言（字段级）优于全文快照（rustc 升级/引擎改动时不炸）；YAML 规则内嵌 bad/good 对；文档页由登记表生成保证不烂尾。
- **档位**：吸收。**第一步动作**：xtask `adv-rules test` 跑内嵌用例；CI 门要求新规则必须带 ≥1 bad + ≥1 good。
- **许可证/成熟度**：—（专题）。

### D1-029 专题C：inline 抑制的粒度与到期（防"永久豁免"）
- **对比**：
  | 工具 | 粒度 | 到期/自证 |
  |---|---|---|
  | Python/Ruff | 行尾 `# noqa [codes]` | RUF100 检测未用 noqa（自证有用）；无原生到期 |
  | ESLint | 行/下一行/区间/文件四粒度，可列规则码 | `--report-unused-disable-directives`；无原生到期 |
  | rustc | 符号属性 `#[allow]`/`#[expect]` | **expect 未触发即告警（Rust 1.81）＝原生到期** |
  | ShellCheck | 行/区间 directive，可指定 shell/source | directive 被解析校验，写错即报 |
  | detekt | 符号 `@Suppress` | 无 |
  | rubocop | disable/enable 配对（嵌套需配平） | `--auto-gen-config` todo 清单可复审 |
- **给 adv-rules**：四粒度（行/区间/符号/文件）+ 每条抑制必须带规则码与原因 + **默认 expect 语义**（未触发即报）+ 到期时间戳可选字段，到期扫描进 xtask-gates；旧仓 noqa 尾注教训=无规则码、无原因、无到期 ⇒ 全部补齐。
- **档位**：吸收。**第一步动作**：先定抑制语法 v1（含 expect 语义），写 `adv gate --stale-suppressions`。
- **许可证/成熟度**：—（专题）。

### D1-030 专题D：编辑器集成（LSP 内嵌 vs 独立进程）
- **机制**：① 内核常驻型：dart analysis server（IDE 与 CLI 同一 analyzer 内核，最彻底）、ruff server（Rust LSP，随 IDE 插件分发）、biome/rubocop 内置 LSP；② 外挂型：eslint daemon（独立 Node 进程 + `--stdin` 单文件诊断通道，CI 与编辑器共享规则但不同进程）；③ `--stdin`/`--stdin-filename` 是把 lint 嵌进任意宿主的最小协议。
- **给 ADV（本地优先）**：单常驻进程（adv daemon）+ stdin 单文件通道为底线；watch 域复用同一增量索引，避免每文件 spawn（spawn 成本在本地工具链实测中是主要延迟项——07 域口径）。
- **档位**：有界（stdin 协议与 daemon 模式抄思想）。
- **第一步动作**：adv CLI 提供 `adv lint --stdin`；watch 用 daemon 复用解析缓存。
- **许可证/成熟度**：—（专题）。

### D1-031 专题E：增量 lint 的缓存键设计（对 ADV 棘轮门的价值）
- **机制**：① golangci-lint：issues cache + facts cache，键=包内容+配置+工具版本，**但含绝对路径导致同内容异路径 miss（#6428）**；修复相关条目必须连带失效（#420 的 related-information 陈旧条目教训）；② ESLint `--cache`：`metadata` 策略（mtime+配置哈希，快但可能漏）vs `content` 策略（内容哈希，慢但稳）——**两种正确性档位供用户选**；③ Ruff 自带缓存（内容+配置+版本键，`--no-cache` 关闭）；④ clippy 复用 cargo 增量缓存，跨 toolchain 失效。
- **给 ADV 棘轮门**：棘轮（基线只减不增）要求"未变更文件的历史违规不重报"，缓存键必须 = f(内容哈希, 规则集指纹, 引擎版本, 目标类型)，**路径无关**（worktree/CI 检出路径变化不影响命中）；规则集指纹=登记表内容哈希（规则改了=全部失效，宁可全量不可漏报）。
- **档位**：吸收。**第一步动作**：`adv gate` 缓存模块先写键函数与两个异路径同内容 worktree 的单测。
- **许可证/成熟度**：—（专题）。

### D1-032 专题F：规则 DSL 组织模式对比（给 adv-rules 表达层的总输入）
- **谱系**：
  1. **代码内嵌**（ESLint JS 规则/clippy Rust 规则/rubocop cop 类）：表达力无上限、类型安全；规则=代码 ⇒ 批量管理/文档生成/跨语言复用全靠纪律。
  2. **规则即数据**（Semgrep YAML/markdownlint/PMD XPath）：可批量审查/版本化/市场分发；表达力有天花板（PMD 教训），复杂语义退回宿主语言。
  3. **混合+中心登记表**（Ruff）：实现是 Rust，但规则码/类别/文档/修复元数据集中在一个登记表里由宏/脚本生成索引——**代码表达力 + 数据可管理性**两头占。
  4. **声明 + 查询 DSL 插件**（Biome GritQL）：外部贡献者可写规则而无需碰引擎代码；DSL 本身成为兼容面（学习成本=门槛）。
- **给 adv-rules**：已选 YAML taint spec + AstKind matcher（谱系 2/4 之间）；建议补齐谱系 3 的中心登记表（code、status 生命周期、requires 能力前提、fix 元数据、文档、测试同源生成），并保留 Rust 回调逃生舱（谱系 1）应对 YAML 写不出的 20%。规则表达力分布先量化（YAML 命中率），再决定是否扩 DSL，避免重蹈 PMD 覆辙。
- **档位**：吸收。**第一步动作**：登记表 schema 定稿（合并本域各条的字段建议），作为 adv-rules 的第一份代码。
- **许可证/成熟度**：—（专题）。

## 7. 2025–2026 前沿信号

1. **tsgolint 达稳定（2026-09，v7）**：覆盖 typescript-eslint 61 条类型感知规则中的 59 条（InfoQ 口径；2025-12 alpha 为 43 条）——"用 typescript-go 重写而非绑定 TS 编译器"证明类型感知 lint 的内核可以被替换，速度不再是不可逾越的税。
2. **Biome 2.0（2025-06-17）自研类型推断**：首个不依赖 tsc 的 JS 类型感知规则（noFloatingPromises 约覆盖 75% 用例，dev.to 实测口径）；Vercel 加入合作（官方博客）——"够用推断"路线与 tsgolint "换内核"路线正面竞争。
3. **ESLint 收尾与扩张**：flat config `extends` 回归（2025-03）；v9.26 内置 MCP server（2025-05）——lint 工具开始原生服务 AI 助手查询面。
4. **Ruff 0.16 默认规则集扩至 413 条**（changelog 口径；规则总量 900+，官方 rules index）——默认集扩张+奇偶版本通道并行，规则工程的流水线样本成熟。
5. **golangci-lint v2 系列**（v2.14.0，2026-09-24）MetaLinter + 结果管道定型；缓存键路径依赖问题（#6428）官方确认——聚合器缓存的头号教训。
6. **clippy 834 条 lint**（官方列表页口径）/ 9 组，MSRV 感知与 clippy.toml 每规则配置常态化。
7. **Dart 新 analyzer plugin 体系**（Flutter 3.38，2025-12，社区教程锚）：插件从 isolate 模型换为 SDK 内体系——语言内核自带 lint 的路线继续强化。
8. **PMD roadmap 2025**：type-resolution 规则与非 TR 规则合并、自动 fallback——类型解析降级路径被正式化。

## 8. Top-3（按对 ADV 的杠杆排序）

1. **D1-017 rustc `#[expect]` 抑制到期语义 + D1-029 抑制四要素设计**——直接解决旧仓 noqa 尾注教训（无码/无因/无到期），是棘轮门"豁免不固化"的地基。动作：定抑制语法 v1（expect 默认 + 到期字段）+ `adv gate --stale-suppressions`。
2. **D1-027 fix 安全三档（machine/maybe/display）+ D1-023 RuboCop SafeAutoCorrect 分离 + todo 模式**——`adv fix` 默认只吃 machine 档，存量豁免走可审计的 autogen 清单。动作：诊断结构加 applicable 字段，xtask-gates 加 autogen-exclusions。
3. **D1-031 缓存键路径无关设计（golangci #6428 教训）+ D1-013 结果管道**——ADV 棘轮门与 watch 域共用的增量底座；键写错会在 worktree 场景静默全量重扫或漏报。动作：键函数单测（双 worktree 验证）先行，再实现去重/配额处理器链。
