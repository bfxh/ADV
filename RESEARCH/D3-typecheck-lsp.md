# D3 调研：类型检查器与 LSP 实现（类型系统机制 + LSP 工程化）

- 日期：2026-10-03；检索口径：WebSearch/WebFetch（检索日快照），优先 2025–2026 资料。
- 检索否证留档：gopls `architecture.md`（go.dev 与 golang/tools raw 均 404，疑似文档重组）、rust-analyzer book `architecture.html` 404、mypy readthedocs `incremental.html` 404、pyright `docs/background.md` 404、typescript-go/clangd 部分原文因证书问题改走 web_reader。上述对象的机制描述降为 skim 档，引用主链为准。
- 上游衔接：09 域已定 LSP 框架基调（tower-lsp 停更→选续作）；01 域已深挖 rust-analyzer/salsa。本域不重复，只补工程面。

---

## 一、重点回答（对应任务 ①–⑤）

### ① tsgo（Go 移植版 tsc）架构与性能结论（2026 现状）
- 架构：把 TS 编译器管线（scanner→parser→binder→checker→emitter）整体移植为 Go，按 goroutine 拆并行 worker；**自带 LSP server**（typescript-go 仓内 `internal/lsp`），语言服务与编译器同仓同进程模型。
- 2026 现状（锚：github.com/microsoft/typescript-go README，2026-10-03 检索）：**staging 仓已关闭（README 顶部声明，归档期 2026-09），TS 7.0 发布完成，开发回归 microsoft/TypeScript 主仓**。状态表：Program 创建 ✓、Parsing ✓（与 TS 6.0 完全相同的语法报错）、Type checking ✓（相同报错/位置/文案）、Watch/Build（project references）/Incremental ✓、**LSP：in progress（"nearly all features implemented"）**、**API：not ready**。
- 性能结论（锚：devblogs.microsoft.com，2025-03 公布口径，2026-10 检索确认）：TS 7.0 构建常约 **10x** 于 TS 6.0；VS Code 仓示例口径 ≈77.8s→7.5s（官方发布材料数字，非本方实测）。
- API 兼容档：**编辑器场景优先对齐，编译器 API 不承诺兼容**；TS 6.x（JS 版）与 7.0（Go 版）并行发布、语义共享；预览期命令 `tsgo`（npm `@typescript/native-preview`），7.0 RC 起命令回归 `tsc`；VS Code 侧开关 `js/ts.experimental.useTsgo`。

### ② pyright 类型推断缓存/服务化设计要点
- 服务化：Node 常驻进程 + 文件系统 watcher（watchdog）+ **后台分析线程**（VS Code 扩展侧 background analysis 可配线程数），编辑器交互在前台、全量检查在后台分片。
- 求值缓存：类型按需（lazy）求值并缓存于 evaluator 的 type cache；**speculativeTypeTracker**：对"猜测性求值"（如为解环而试推一个候选类型）打标并缓存，推断失败即丢弃，防止循环依赖导致指数重复计算——这是 pyright 在大仓上不炸的关键机制之一。
- 导入解析（锚：microsoft/pyright docs/import-resolution.md，2026-10-03 检索）：**纯静态、绝不执行包代码**——相对导入按导入文件路径解析；绝对导入按配置给定的 search paths 顺序（executionEnvironments root/extraPaths → venv site-packages → typeshed stubs），stub 文件优先于 .py 源；`useLibraryCodeForTypes` 允许直接解析已安装库的 .py 源内联类型（把库当"只读源"读，而非 import）。

### ③ LSP 诊断推送的节流/去抖模式（ADV 出 LSP 面直接用）
- 收：`textDocument/didChange` 声明 `textDocumentSync.change = 2`（增量），把连续 change 合并成文档最新版本号（version 单调）。
- 算：**debounce（静默期后触发）优于 throttle（固定间隔触发）**——打字流场景下 throttle 会在停手后还补一次旧计算；若算不完新的又来了，用取消（`$/cancelRequest` 或内部 cancellation token）丢弃在途计算（锚：serard.dev Reactive Pattern VIII；Jac LS changelog "debounce-driven type check" 修复案例；Twente 2025 论文谈增量同步）。
- 推：冷热分层——语法级便宜诊断即时推；类型级贵诊断延后（debounce 数百 ms 或等 didSave）；每次按文件全量 publishDiagnostics（client 端以 version 对齐），跨文件诊断置 `interFileDependencies`。

### ④ 类型级 vs 语法级分析的责任分界（ADV 划界）
| 层 | 责任 | 典型诊断 | 延迟档 |
|---|---|---|---|
| tree-sitter（单文件语法树+错误恢复） | 结构与词法 | 括号/引号不闭合、缩进、语法错误、函数/类边界、**自建作用域表后的同名遮蔽/明显未定义引用** | ms 级，随键入 |
| 类型引擎（pyright 类实现或 ADV 自研） | 跨文件类型一致 | 类型不匹配、参数个数、重载决议、泛型实例化、narrowing/dataflow | 数百 ms+，debounce |
| 真编译器外包（tsc --noEmit / mypy / cargo check / go build） | 语言全景语义 | 编译器独有规则（借用检查、宏展开、特性门控）、规则全集 | 秒级，didSave/后台 |

判据：只依赖单文件语法树 ⇒ tree-sitter；需要跨文件类型一致性但不需要语言全景 ⇒ 类型引擎；需要类型系统全景/编译器私有规则 ⇒ 外包真编译器。rust-analyzer 的 flycheck 是"外包真编译器"的工程范型（后台跑 `cargo check`，把编译器诊断流式转成 LSP diagnostics）。

### ⑤ sourcemap 机制对 ADV 的适用点
- 格式（v3，锚：tc39.es/ecma426，"Source map format specification"，ECMA-426 标准化推进中）：`{version, sources[], sourcesContent[], names[], mappings}`；mappings 为 base64 VLQ 段 `[genCol, srcIdx, srcLine, srcCol, nameIdx?]`，生成行以 `;` 分隔、段以 `,` 分隔；index map 支持多级级联。
- ADV 适用点：当 ADV 对输入做归一化/聚合/变换后再分析（如多文件拼接、预处理展开、stub 生成），**报告行号用 sourcemap 映射回原文件**，避免"报错行号对不上源码"；消费端可用 Rust crate `sourcemap`（license 采纳时以 crates.io 页为准）。不适用点：ADV 若直接在原文件上解析（tree-sitter 原生 offset），则不需要 sourcemap——它只在"分析对象 ≠ 原文件"时引入。

---

## 二、对象明细（20 个）

### D3-01 tsgo / TypeScript 7 原生移植（typescript-go）
- 定位：TS 编译器的 Go 移植，TS 6.x（JS）与 7.0（Go）并行发布、语义共享。
- 可抄机制：(1) 编译器与 LSP 同仓一体，语言服务直接消费编译器内部数据结构；(2) 全管线 goroutine 并行；(3) staging 仓→主仓的"移植后归档"开发模式。
- 档位：**watch**（语言本体不进 ADV；等 TS 7.0 稳定后其 LSP/服务化形态可再评估）。
- 第一步动作：无（订阅 microsoft/TypeScript 的 7.0 release note 即可）。
- 许可证/成熟度：Apache-2.0（TypeScript 许可）；TS 7.0 已发布、LSP 未完（锚：README 2026-10-03）。
- 链接：https://github.com/microsoft/typescript-go ; https://devblogs.microsoft.com/typescript/typescript-7-native-port/

### D3-02 pyright
- 定位：Python 静态类型检查器 + 语言服务（TS/Node 实现），Microsoft 官方，大仓性能导向。
- 可抄机制：(1) speculativeTypeTracker 防循环推断爆炸；(2) 纯静态导入解析（配置驱动 search roots、stub 优先、库当只读源）；(3) 前台交互/后台全量分片的 service 模型。
- 档位：**吸收**（机制面，非代码）。
- 第一步动作：把"导入解析 = 配置显式 roots + 绝不执行用户代码"写进 ADV 沙箱路径策略设计稿（配合 D3-19）。
- 许可证/成熟度：MIT；维护中（Microsoft 活跃）。
- 链接：https://github.com/microsoft/pyright ; https://microsoft.github.io/pyright/

### D3-03 mypy（增量/守护进程模式，对照项）
- 定位：Python 参考级类型检查器，对照 pyright 的增量缓存路线。
- 可抄机制（skim，原文 404，机制按官方文档既有认知）：(1) `.mypy_cache` 按模块存元数据（mtime/size/hash + 依赖边），只重查脏模块；(2) dmypy fine-grained dependencies 支持"改一个符号只查受影响模块"。与 pyright 差异：mypy 是批处理+守护进程两态，pyright 是常驻服务一态。
- 档位：**reference**（对照，不采纳其缓存布局——Rust 侧有更好表达）。
- 第一步动作：无；写对照结论进 ADV 增量设计时引用本条。
- 许可证/成熟度：MIT；维护中。
- 链接：https://github.com/python/mypy ; https://mypy.readthedocs.io/

### D3-04 ty（astral-sh）
- 定位：ruff 团队用 Rust 写的 Python 类型检查器，2025 起公开开发，主打速度。
- 可抄机制：与 ADV 同为 Rust 实现的同类先例；关注其对 pyright 机制的 Rust 化重写取舍。
- 档位：**watch**。
- 第一步动作：每季度查一次 release note，观察其错误恢复与并发模型。
- 许可证/成熟度：MIT；实验（0.x 预览）。
- 链接：https://github.com/astral-sh/ty

### D3-05 gopls
- 定位：Go 官方 LSP server（x/tools），包级类型检查。
- 可抄机制（skim——architecture.md 双源 404，按既有认知与 go.dev 文档链）：(1) 三层缓存（filesystem/package/analysis）按包图组织，改动只使受影响包失效；(2) `go.work` 多模块工作区模型；(3) 诊断在包图类型检查后统一推送。
- 档位：**有界**（包图失效模型可借鉴到 adv-index 的文件→依赖失效传播）。
- 第一步动作：ADV 索引失效设计稿中对照其"包图"粒度（文件→包→反向依赖）。
- 许可证/成熟度：BSD-3（Go 工具链）；维护中。
- 链接：https://go.dev/gopls/doc/ ; https://github.com/golang/tools

### D3-06 clangd
- 定位：C/C++ LSP server（LLVM），索引工程最成熟的参考实现。
- 可抄机制（锚：clangd.llvm.org/design 与 /design/indexing，2026-10-03 原文）：(1) **背景索引 shard**：每文件索引产物写 `.cache/clangd/index/*.idx`，启动时若文件未变直接复用磁盘 shard，不重建；(2) **冷热分离**："expensive-and-rare preamble rebuilds vs cheap-and-frequent main-file rebuilds"——头文件集（preamble）很少重建，正文频繁重建，completion 走独立路径；(3) `MergedIndex` 把动态层（打开文件）叠在持久层（背景索引）上呈现统一视图；无 compile_commands 的头文件 shard 落用户缓存目录；(4) TUScheduler 每打开文件一个 ASTWorker 线程。
- 档位：**吸收**。
- 第一步动作：adv-index 设计稿引入"按文件 shard + 内容指纹判重 + 启动复用"，并给打开文件单独内存层。
- 许可证/成熟度：Apache-2.0（LLVM exceptions）；维护中。
- 链接：https://clangd.llvm.org/design ; https://clangd.llvm.org/design/indexing

### D3-07 rust-analyzer（工程面）
- 定位：Rust LSP server；salsa 增量内核已由 01 域深挖，此处只登记工程面。
- 可抄机制（skim，book 页 404）：(1) flycheck：后台进程跑 `cargo check`，把编译器诊断流式转 LSP 诊断（④ 表"外包真编译器"范型）；(2) proc-macro 展开放独立进程（隔离崩溃与构建噪音）。
- 档位：**有界**。
- 第一步动作：ADV"外包真编译器"接口设计引用 flycheck 模式（子进程 + 流式诊断 + 取消）。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://github.com/rust-lang/rust-analyzer ; https://rust-analyzer.github.io/book/

### D3-08 salsa（补登记，主责 01 域）
- 定位：rust-analyzer 的增量计算框架，2025 完成 Rust 重写（salsa-rs/salsa）。
- 可抄机制：查询式增量（输入版本→按依赖图失效重算）。
- 档位：**reference**（机制归 01 域，本域只在 LSP 缓存话题引用）。
- 第一步动作：无。
- 许可证/成熟度：Apache-2.0 OR MIT；维护中。
- 链接：https://github.com/salsa-rs/salsa

### D3-09 tower-lsp → tower-lsp-server（衔接 09 域）
- 定位：Rust LSP 框架；tower-lsp 停更，社区续作 tower-lsp-server 接管（09 域已定调，本域只确认工程衔接：并发请求处理、$/$ 通知、增量 sync 支持面）。
- 档位：**watch**。
- 第一步动作：09 域选型结论落地时，核对续作对 3.17 特性（pull diagnostics、relativePattern watcher）的覆盖。
- 许可证/成熟度：MIT；tower-lsp 弃维护，续作维护中（社区）。
- 链接：https://github.com/tower-lsp-community/tower-lsp-server ; https://github.com/ebkalderon/tower-lsp

### D3-10 LSP 生命周期/能力协商
- 定位：LSP 协议骨架：initialize（capabilities 协商、positionEncoding 默认 UTF-16）→ initialized → shutdown → exit；工作区能力按 3.17 规范逐项声明。
- 可抄机制：能力协商表 = ADV LSP 面的"档位开关"实现模板（声明什么才能用什么，客户端降级有依据）。
- 档位：**吸收**（若 ADV 出 LSP 面）。
- 第一步动作：ADV 若立项 LSP，先抄协议状态机（init 握手前拒绝一切请求）。
- 许可证/成熟度：规范文档（microsoft/language-server-protocol，许可以仓为准）；维护中（3.17 现行）。
- 链接：https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/

### D3-11 LSP 增量文本同步
- 定位：`textDocument/didChange` 增量模式（SyncKind=2），range+text 补丁、version 单调。
- 可抄机制：服务端维护文档影子副本 + 版本号单调校验，乱序/回退版本直接丢弃（锚：spec 3.17；Twente 2025 论文）。
- 档位：**吸收**。
- 第一步动作：与 D3-13 的去抖器同层实现（收增量→合并→定时触发分析）。
- 许可证/成熟度：同 D3-10。
- 链接：同 D3-10。

### D3-12 LSP 诊断推送节流/去抖
- 定位：didChange→诊断的节流管线（③ 的落地件）。
- 可抄机制：(1) debounce 静默期触发；(2) 在途计算可取消；(3) 冷热分层推送（语法即时/类型延迟）；(4) 按文件全量 publish + version 对齐。
- 档位：**吸收**。
- 第一步动作：在 ADV LSP 面设计稿中固化为默认管线（参数：debounce 窗口、取消粒度）留实测定标。
- 许可证/成熟度：模式类，无独立许可；实践锚见③。
- 链接：https://essay.utwente.nl/ （2025 论文，增量同步实践） ; https://serard.dev/ （Reactive Pattern VIII）

### D3-13 LSP pull diagnostics（3.17）
- 定位：`textDocument/diagnostic` 拉取模式，支持 `interFileDependencies` 与 `workspaceDiagnostics`；与 push 并存。
- 可抄机制：pull 模式把"何时算"的决定权交回客户端，配合 push 做"编辑器可见文件 push、其余 pull"的混合档。
- 档位：**watch**（客户端覆盖面待核）。
- 第一步动作：ADV LSP 面若立项，capabilities 里同时声明 push+pull。
- 许可证/成熟度：同 D3-10。
- 链接：同 D3-10。

### D3-14 didChangeWatchedFiles / 文件监听
- 定位：服务端注册 glob watcher（3.17 支持 relativePattern），客户端代为监听并推 `workspace/didChangeWatchedFiles`。
- 可抄机制：把"文件系统监听"责任交给客户端可省一份轮询；但**不可只依赖它**——客户端不在线期间的变更要靠服务端自己重扫兜底（两种来源都要接）。
- 档位：**吸收**。
- 第一步动作：ADV 的 watch 组件（workspace/watch 域）设计里把 LSP watcher 作为可选来源、自建 watcher 为基线来源。
- 许可证/成熟度：同 D3-10。
- 链接：同 D3-10。

### D3-15 多 root workspace
- 定位：`workspaceFolders` 数组（3.16+ 起 standard），配置/搜索路径按 folder 分层。
- 可抄机制：配置作用域按 folder 叠加（folder 覆盖全局），pyright 的 executionEnvironments 是单仓内再分层的先例（见 D3-19）。
- 档位：**吸收**。
- 第一步动作：ADV 沙箱/项目配置 schema 预留 per-folder 覆盖层。
- 许可证/成熟度：同 D3-10。
- 链接：同 D3-10。

### D3-16 hover/completion 缓存（冷热分离机制）
- 定位：编辑器高频查询的缓存策略（clangd preamble 冷热分离 + pyright 求值缓存的交叉结论）。
- 可抄机制：(1) 把"包含关系/前置解析"冻结为冷缓存（clangd preamble 模式），高频小改动只动热缓存；(2) 查询结果按 (位置, 文档版本) 键控，版本变更即失效。
- 档位：**有界**（ADV 有 LSP/completion 面才启用）。
- 第一步动作：与 D3-06 shard 设计合并进 adv-index 缓存分层稿。
- 许可证/成熟度：机制类；锚 clangd.llvm.org/design/indexing。
- 链接：同 D3-06。

### D3-17 tree-sitter 做诊断的边界（ADV 划界）
- 定位：增量语法解析器（错误恢复好），只给语法级；④ 表的"快层"。
- 可抄机制：增量解析 + ERROR 节点即诊断源；符号表需自建（tree-sitter 不给作用域语义）。
- 档位：**吸收**（划界结论进 ADV 诊断分层设计）。
- 第一步动作：把④表三列判据写进 ADV 诊断路由（哪些 rule 走哪层）。
- 许可证/成熟度：MIT；维护中。
- 链接：https://github.com/tree-sitter/tree-sitter

### D3-18 sourcemap v3 / ECMA-426
- 定位：变换代码→原码行号映射的标准格式；ECMA-426 标准化推进中（锚：tc39.es/ecma426 原文，2026-10-03）。
- 可抄机制：见⑤（VLQ mappings 结构、index map 级联、sourcesContent 取舍）。
- 档位：**有界**（仅在 ADV"分析对象≠原文件"的管线引入）。
- 第一步动作：在 ADV 报告层预留 `origin_map` 字段（格式对齐 v3），暂不实现生成端。
- 许可证/成熟度：ECMA 标准（推进中）；消费 crate 采纳时核 license。
- 链接：https://tc39.es/ecma426/ ; https://github.com/mozilla/source-map（历史参考实现）

### D3-19 pyrightconfig 与"源即包解析"（沙箱路径策略借鉴）
- 定位：pyright 的配置分层：pyrightconfig.json / pyproject.toml [tool.pyright]，executionEnvironments 按目录划执行环境（各自 root/extraPaths/规则档），多配置文件变体支持。
- 可抄机制：(1) 搜索根显式化 + 默认只信任工作区目录；(2) 外部依赖只读映射（venv/typeshed），解析靠读源/stub 不靠执行；(3) 规则严格度按目录分层（severity 档）。
- 档位：**吸收**。
- 第一步动作：ADV 沙箱配置 schema 起草：importRoots 显式列举 + 外部依赖只读 + per-dir 规则档。
- 许可证/成熟度：同 D3-02。
- 链接：https://microsoft.github.io/pyright/ （configuration/import-resolution 章节）

### D3-20 vscode-languageserver-node
- 定位：Microsoft 的 LSP 参考实现（client + server 库，JSON-RPC 层），多数 LS 实现的协议行为基准。
- 可抄机制：diagnostics/versioning、cancellation、progress token 的参考语义（协议细节的判例库）。
- 档位：**reference**。
- 第一步动作：无（协议争议时查其实现当判例）。
- 许可证/成熟度：MIT；维护中。
- 链接：https://github.com/microsoft/vscode-languageserver-node

---

## 三、2025–2026 前沿信号
1. **TS 7.0（tsgo）落地**：staging 仓 2026-09 关闭、开发回归 microsoft/TypeScript；编辑器场景接近完备，**编译器 API 兼容明确"not ready"**（锚：typescript-go README，2026-10-03 检索）——"原生重写换 10x"这条路线已被 TS 走通，对 ADV 是路线背书。
2. **ECMA-426 sourcemap 标准化**推进中（tc39.es/ecma426），格式在 v3 上收敛，ADV 报告层可按 v3 预留。
3. **Rust 侧类型检查器竞争**：astral-sh 的 ty 2025 公开开发（0.x 实验），pyright 机制正在被 Rust 化重写——ADV 若自研类型引擎，ty 是最接近的实现参考。
4. **tower-lsp 停更 → tower-lsp-server 社区续作**（09 域基调），Rust LSP 框架层 2025–2026 完成代际交替。
5. **basedpyright**（pyright 社区分叉）活跃，扩展 pyright 未收的配置面——pyright 生态单点风险有限。

## 四、Top-3（本域对 ADV 价值最高）
1. **clangd 背景索引 shard + 冷热分离**（D3-06/D3-16）：按文件 shard 落盘、指纹判重、启动复用；preamble"贵而少"vs 正文"廉而频"——adv-index 直接抄。
2. **pyright 沙箱式导入解析 + 配置分层**（D3-02/D3-19）：配置显式 search roots、stub 优先、纯静态不执行、per-dir 规则档——ADV 沙箱路径策略的完整蓝本。
3. **LSP 诊断推送节流管线**（D3-11/D3-12）：增量同步 + debounce（优于 throttle）+ 在途取消 + 冷热分层推送——ADV 若出 LSP 面的默认模式。

## 五、检索锚点汇总（主要引用）
- https://github.com/microsoft/typescript-go （README，2026-10-03 检索）
- https://devblogs.microsoft.com/typescript/typescript-7-native-port/ （10x 口径，2025-03 公布）
- https://raw.githubusercontent.com/microsoft/pyright/main/docs/import-resolution.md （2026-10-03 检索）
- https://clangd.llvm.org/design ; https://clangd.llvm.org/design/indexing （2026-10-03 检索）
- https://tc39.es/ecma426/ （2026-10-03 检索）
- https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
- https://essay.utwente.nl/ （2025，LSP 增量同步论文）；https://serard.dev/（debounce/throttle/sample）
