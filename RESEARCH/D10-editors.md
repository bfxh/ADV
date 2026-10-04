# D10 编辑器架构（文本缓冲 / 索引 / 扩展机制）

> 日期：2026-10-03。检索口径：WebSearch（英文源为主），数字均带锚（来源+日期）；说不清机制的未收录。
> 服务对象：adv-parse（tree-sitter + 归一 AST）的缓冲/解析数据结构选型；adv-index（SCIP 面）与未来 LSP/编辑器插件面的机制参照。
> 档位口径：吸收（直接抄进实现）| 有界（抄机制不抄整体）| 不吸收 | watch（留信号，定期回看）| reference（对照/背景）。

---

## 一、文本缓冲层

### 1. ropey（Rust rope 库）——【吸收】
- **定位**：Rust 生态事实上的编辑器文本缓冲库（helix、lapce 等在用），UTF-8 rope。
- **可抄机制**：
  1. **B-tree 分段**：字符存于 B-tree，叶子为连续字符串 chunk（约 1KB 量级，利于缓存局部性），节点聚合 (字节数/字符数/换行数) 元摘要 ⇒ 任意位置插入/删除 O(log n)（crates.io/lib.rs 页口径，2026-10-03 检索）。
  2. **O(1) 浅拷贝快照**：`clone()` 共享根节点（Arc），写时复制 ⇒ 可把快照发到后台线程做解析/IO，主线程继续编辑——这是"解析并发化"的先决件。
  3. **多索引视图**：byte/char/line/grapheme 统一寻址 + `Chunks` 迭代器直读连续块（零拷贝喂给 tree-sitter 输入接口）。
- **版本锚**：2.0 线已发布（lib.rs 明确标注 "this is the 2.0 version"；1.x 稳定线 1.6.x）。升级 2.0 前核 API 变更。
- **第一步动作**：adv-parse 的文件缓冲直接选 ropey 2.x；以 `rope.snapshot()` + 后台 tree-sitter 解析作为并发模型基线。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中。
- **链接**：https://github.com/cessen/ropey ；https://lib.rs/crates/ropey

### 2. crop（Rust rope 库，ropey 对照）——【watch】
- **定位**：ropey 的年轻替代者，同为 B-tree。
- **可抄机制**：显式主打"廉价快照 + 发后台线程做 IO/CPU 重活"（crates.io 页，2025-04-25 口径）；与 ropey 机制同族，差异在工程细节而非原理。
- **第一步动作**：不引入；当 ropey 2.x 在 ADV 场景出现性能/快照瓶颈时再复评。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（成熟度低于 ropey，生态采用少）。
- **链接**：https://docs.rs/crop

### 3. xi-editor / xi-rope（rope+CRDT 思想源头）——【reference】
- **定位**：Raph Levien 的实验编辑器，"rope 作为 CRDT 底层序列容器"的出处；已停开发（repo 自述 discontinued，2023 归档口径）。
- **可抄机制**：
  1. rope 作为 CRDT 的字符序列底座（engine = rope 上的 CRDT）；
  2. Levien 2020 回顾（xi-editor retrospective, 2020-06-27）记录多进程前端/插件架构的得失。
  3. 其 2022 CRDT 回顾结论：许多协作场景 CRDT 是过度设计（HN 讨论 #33826067 转述）——对 ADV "先不做协作"是权威背书。
- **第一步动作**：只读回顾文，不做代码移植。
- **许可证/成熟度**：Apache-2.0；弃维护（仅收 bug fix）。
- **链接**：https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html ；https://github.com/xi-editor/xi-editor

### 4. VS Code 文本缓冲（piece table / piece tree）——【有界】
- **定位**：VS Code 2018 年重写文本缓冲的选型记录，rope 的最强对照组。
- **可抄机制**：
  1. **piece table + 红黑树（piece tree）**：原文缓冲只读，编辑写进 append-only buffer，piece 指向 (buffer, start, len)；树节点聚合行数等元数据（"Text Buffer Reimplementation", Peng Lv, 2018-03-23）。
  2. **选型理由（带锚）**：官方 benchmark 口径下 piece tree 在内存与编辑性能上胜过所测 rope 实现——原因是原文件零拷贝、不用 per-node 字符串块元数据。
  3. 代价：**没有廉价快照**（快照需深拷贝 piece 序列或引入持久化结构）——这正是 VS Code 不做"后台线程持快照解析"的机制背景。
- **第一步动作**：不采用；把"快照能力"写进 ADV 缓冲选型判据（见§六①）。
- **许可证/成熟度**：Code-OSS 为 MIT；维护中。
- **链接**：https://code.visualstudio.com/blogs/2018/03/23/text-buffer-reimplementation

### 5. gap buffer（对照基线）——【reference】
- **定位**：最简单的缓冲结构（emacs、antirez/kilo），作为三结构对照的基线。
- **可抄机制**：单连续数组 + 游标处 gap：游标局部编辑 O(1)、跨游标移动 O(n)；无快照、无多游标、大文件重建代价高——解释了为什么"快照/多游标/协作"需求出现后必然走向 rope 或 piece table。
- **第一步动作**：仅用于文档里做对照表，不进代码。
- **许可证/成熟度**：kilo 为 MIT（示例代码）；概念无许可问题。
- **链接**：https://github.com/antirez/kilo

### 6. Zed 的 rope + sum_tree + Anchor——【吸收】
- **定位**：Zed 自研缓冲栈（不依赖 ropey），多缓冲与 CRDT 的共同地基。
- **可抄机制**：
  1. **rope 上的摘要树（sum_tree）**：按需聚合 (行数/字节) 摘要，光标移动/区间统计 O(log n)；
  2. **Anchor**：树上稳定坐标，编辑后可 O(log n) 重定位——"位置引用在编辑后仍语义有效"的通用件；
  3. 快照/版本化缓冲支撑后台解析与协作（DeepWiki Buffer Architecture 页口径）。
- **第一步动作**：把 "Anchor 稳定坐标" 模式记入 adv-index 的区间引用设计（扫描结果 → 源码区间引用，编辑后可重定位）。
- **许可证/成熟度**：zed 仓库整体 GPL-3.0（部分基础 crate 另有 Apache-2.0，逐 crate 核对后再借代码）；维护中。
- **链接**：https://github.com/zed-industries/zed （crates/rope、crates/sum_tree）；https://deepwiki.com/zed-industries/zed

---

## 二、Zed

### 7. Zed 多缓冲（MultiBuffer）——【吸收】
- **定位**：把分散片段聚合为一个可编辑视图的机制——ADV "扫描结果 → 可操作视图"的最直接参照。
- **机制拆解**：
  1. MultiBuffer（crates/multi_buffer）是**虚拟文档**：`sum_tree::SumTree<Excerpt>`，每个 Excerpt 携带源 Buffer 的 id + 起止区间（Anchor 锚定）；Summary 按 (buffer_id, 位置) 排序，光标可无缝跨 excerpt 移动（DeepWiki/源码口径）。
  2. **编辑转发**：对 multibuffer 的编辑按 excerpt 拆回各源 buffer 的对应区间；源 buffer 是唯一真值，multibuffer 只是投影 + 可写转发（zed.dev 博客 "Multibuffers – Edit Multiple Files at Once"：编辑 multibuffer 与编辑普通文件无异，改动反映到文件本体）。
  3. **Anchor 保活**：用户编辑导致行号漂移后，excerpt/诊断引用仍能重定位——静态行号引用做不到这一点。
- **对 ADV 的价值**：诊断/违规/命中片段聚合成一个"扫描结果视图"，在视图内直接修改并回写真实文件；等价物 = adv-index 产物（区间引用）+ Anchor 模式 + 转发写。
- **第一步动作**：在 adv-index 报告层设计里以 "excerpt + anchor" 为一等概念（先做成数据结构，视图壳后行）。
- **许可证/成熟度**：GPL-3.0（同上）；维护中。
- **链接**：https://zed.dev/blog (Multibuffers 篇)；https://deepwiki.com/zed-industries/zed

### 8. Zed 协作与 CRDT（含 LiveKit 现状澄清）——【watch】
- **定位**：多人协同编辑 + 音视频协作的完整实现。
- **机制要点（含事实澄清）**：
  1. 文本协同 = 自研 CRDT + 自有 collab server（"How CRDTs make multiplayer text editing part of Zed's DNA", zed.dev, 2022-12-01）；
  2. **LiveKit 只承担音视频层**，未被 Zed 收购：LiveKit 2026-01 完成 $100M C 轮（$1B 估值），独立运营；Zed 维护者在 issue #22374（2024-12）明确拒绝把 LiveKit 改为可选依赖——"协作是核心特性"。
  3. 协作与缓冲栈共享 Anchor/版本化机制（同 §6）。
- **第一步动作**：不引入；CRDT 仅在"多端报告协同标注"需求坐实后评估（Levien 2022 回顾结论：主从/单写场景 CRDT 常属过度设计）。
- **许可证/成熟度**：GPL-3.0；维护中。
- **链接**：https://zed.dev/blog （CRDT 篇）；https://github.com/zed-industries/zed/issues/22374

### 9. GPUI（Zed UI 框架）——【watch】
- **定位**：GPU 加速 Rust UI 框架（entity/元素树，即时+保留混合风格）。
- **可抄机制**：GPU 化渲染使大文件滚动不卡（机制层面：文本布局走 GPU，不在 CPU 逐字形布局）。
- **第一步动作**：ADV 短期不自绘 UI，不引入；若未来做本地 GUI 报告台再评估。
- **许可证/成熟度**：随 zed 仓库 GPL-3.0（部分 crate Apache-2.0）；维护中。
- **链接**：https://github.com/zed-industries/zed （crates/gpui）

### 10. Zed 语言协议（LSP + tree-sitter 混合）——【吸收】
- **定位**：语法层与语义层分工的样板。
- **可抄机制**：tree-sitter 负责高亮/折叠/结构导航（毫秒级、本地），LSP 负责诊断/hover/跳转（可能秒级、跨进程），两条管线并行、互不阻塞——低能力 LSP 服务器也不拖垮高亮。
- **第一步动作**：adv-parse（tree-sitter 面）与 adv-index/LSP 面按此分层：语法高亮不等 LSP，诊断经 LSP 推送。
- **许可证/成熟度**：GPL-3.0（同上）；维护中。
- **链接**：https://zed.dev/docs （.languages 与 LSP 章节）

### 11. Zed 扩展（WASM 沙箱）——【吸收】（对 adv-sandbox）
- **定位**：Rust 编辑器阵营的扩展沙箱样板。
- **机制拆解**：
  1. 扩展用 Rust 写、编译到 wasm32-WASI，跑在 **wasmtime** 实例里；
  2. ABI = **WIT（WebAssembly Interface Types）**；
  3. **能力供给制**：扩展无任意文件系统/网络权限，只能调 host 注入的函数（如 `zed::http_client`、受限 fs）；能做的事：注册 tree-sitter 语法、主题、**语言服务器配置**、AI 面板 slash commands、context servers（官方 docs 口径）。
- **与 VS Code 对比**见 §14；与 Lapce 互证见 §17。
- **第一步动作**：adv-sandbox 插件 ABI 以 "WIT 接口 + 显式能力注入" 为候选方向写 spike（对比进程+RPC 方案）。
- **许可证/成熟度**：扩展 API 本身随 zed（GPL-3.0）；wasmtime 为 Apache-2.0 WITH LLVM-exception；扩展系统维护中、API 面仍在扩（2025–2026 持续加 context servers 等）。
- **链接**：https://zed.dev/docs/extensions ；https://wasmtime.dev

---

## 三、helix / neovim / VS Code（对照）

### 12. helix（Rust 终端编辑器）——【有界】
- **定位**："batteries included" 的 Rust 编辑器，无插件哲学的代表。
- **可抄机制**：
  1. **selection-first**：主选区模型，多光标是一等操作原语（编辑动作一律以选区为参数）；
  2. **LSP 内建 core**（helix-lsp 管理 server 子进程，编辑器直接当 LSP 编排器）——无插件层的 LSP 集成形态；
  3. ropey 缓冲 + tree-sitter 高亮/缩进/注入开箱即用（官方 Architecture 文档：Editor 持有全部文档、视图树、语言服务器注册表）。
- **取舍记录**：维护者公开口径"不合核心目标的 feature request 会被拒，插件将来会帮上忙"（HN 讨论）——即"核心内建 vs 插件生态"的路线之争，helix 押内建。
- **第一步动作**：adv 的 LSP 客户端面（若做）参考 helix-lsp 的"无插件直连"结构；选区模型不适用于批处理工具。
- **许可证/成熟度**：MPL-2.0；维护中（发版节奏按年，25.01 线）。
- **链接**：https://helix-editor.vercel.app （Architecture）；https://github.com/helix-editor/helix

### 13. neovim（对照，抄机制）——【reference】
- **定位**：Lua 生态编辑器，机制层最值得"抄点子"的对照物。
- **可抄机制**：
  1. **Extmarks**：把任意元数据钉在 (buffer, namespace, start, end) 区间上且随编辑自动漂移——诊断装饰/虚拟文本/inlay hint 的统一底座；与 Zed Anchor 同构问题的 Vim 系解法；
  2. **tree-sitter 异步化**：0.11（2025-03）引入异步高亮/折叠/注入查询迭代 + 更高效查询缓存（segmentfault 2025-03-25 口径；neovim.io News-0.11）；
  3. **vim.lsp.config**（0.11）：LSP 配置收敛进核心，第三方 lspconfig 插件职能被吸收——"标准面先于生态"的又一例。
- **第一步动作**：adv-index 的"区间附加元数据"（诊断/标注/命中）按 extmark 语义设计数据结构（namespace 隔离 + 随编辑漂移）。
- **许可证/成熟度**：Apache-2.0（部分承 Vim License）；维护中。
- **链接**：https://neovim.io （News-0.11）；https://neo.vimhelp.org （lua.txt/extmarks）

### 14. VS Code 扩展进程模型（对照）——【reference】
- **定位**：最主流的扩展模型，Zed WASM 方案的对照组。
- **机制拆解**：
  1. 扩展宿主 = **独立 Node.js 进程**（每窗口一个），扩展经 RPC 与主进程通信；崩溃隔离但**性能不隔离**（多扩展共享单 host 线程）；
  2. 扩展拥有**完整用户权限**（文件/网络无沙箱），安全边界靠商店审核 + proposed API 闸门；
  3. 文本模型至今不用 tree-sitter：TextMate 语法 + LSP semantic tokens（2026-06 第三方口径再确认）。
- **第一步动作**：仅作对照；adv 插件不复制"进程+全权限"模型（见 §六④ 对比表）。
- **许可证/成熟度**：Code-OSS 为 MIT（二进制发行版另有微软许可）；维护中。
- **链接**：https://code.visualstudio.com/api （Extension Host 概念页）

---

## 四、语法高亮增量重算

### 15. tree-sitter 增量解析 + 查询重算——【吸收】
- **定位**：编辑器语法高亮的现代标准件（Zed/helix/neovim 共用）。
- **实现要点（三个环节，缺一不可）**：
  1. **解析复用**：编辑先经 `tree.edit()`（旧树按编辑平移内部坐标），再 `Parser::parse(input, Some(&old_tree))` ⇒ 只重解析变更子树，未变子树直接复用；
  2. **查询重算范围**：`Tree::changed_ranges(&old_tree)` 给出需要重跑高亮查询的字节区间，其余区间沿用旧捕获缓存（helix `helix-core/src/syntax.rs` 的分层 layer 状态机、nvim `highlighter.lua` 均为此形状）；
  3. **异步化**：解析放后台线程 ⇒ 需要"跨线程快照"——正是 ropey/crop 的 O(1) snapshot 与 tree-sitter 的组合点（neovim 0.11 异步高亮即同类机制）。
- **第一步动作**：adv-parse 高亮管线按 1→2→3 顺序实现，增量两件（edit+changed_ranges）先行，异步随后。
- **许可证/成熟度**：tree-sitter 为 MIT；维护中。
- **链接**：https://tree-sitter.github.io/tree-sitter/ （parsing 章节）；https://github.com/helix-editor/helix （helix-core/src/syntax.rs）

### 16. tree-sitter 注入语言（injections）——【吸收】
- **定位**：混合语言文件（HTML 内 JS、markdown 代码块）的高亮机制。
- **可抄机制**：`injections.scm` 查询以 `(injection.language)` 捕获声明注入区 ⇒ 为命中区间建子 parser（同样走增量），高亮按 layer 叠加合并；`locals.scm` 决定注入区是否继承父语言作用域——各编辑器实现分歧点（tree-sitter-perl-rs epic, 2026-07, 把 highlight/locals/injection/tags/fold 口径统一列为目标）。
- **第一步动作**：adv-parse 支持 injections 查询文件加载 + 子 parser 池；locals 继承口径先跟随 helix。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://tree-sitter.github.io/tree-sitter/ （syntax highlighting / injections 章节）

---

## 五、扫视一览（机制一句 + 档位 + 链接）

| 对象 | 机制一句 | 档位 | 链接 |
|---|---|---|---|
| lapce（Rust 编辑器） | ropey 缓冲 + 自研 floem UI，插件走 WASI(wasmer)+RPC——WASM 插件路线的第二例证 | watch（Apache-2.0，维护中） | https://github.com/lapce/lapce |
| yrs / automerge / diamond-types | 三条现成文本 CRDT 库线（Yjs 移植 / 抗融 CRDT / 高性能序列 CRDT）——协作需求坐实前的候选池 | watch（yrs MIT；automerge MIT；diamond-types MIT，口径待核） | https://github.com/y-crdt/y-crdt ；https://github.com/automerge/automerge ；https://github.com/josephg/diamond-types |
| emacs gap buffer 实践 | 40 年量级生产验证的 gap buffer：证明单游标场景最简结构也够用——结构选型跟着交互模型走 | reference | https://www.gnu.org/software/emacs/manual/html_node/elisp/Buffer-Gap.html |
| LSP 本身（协议面） | 诊断/定义/引用/文档符号的标准传输面——"一次实现、所有主流编辑器可消费"的杠杆 | 吸收（归 LSP 域细查，此处只锚结论） | https://microsoft.github.io/language-server-protocol/ |
| Semantic Tokens（LSP） | LSP 侧语义高亮通道，tree-sitter 高亮的互补而非替代（VS Code 走的就是 TextMate+semantic tokens） | reference | https://microsoft.github.io/language-server-protocol/specifications/ |
| tower-lsp / lsp 生态（Rust） | Rust 侧 LSP server 框架起点（adv-lsp 第一步的脚手架候选） | watch（待 D 域专项核） | https://github.com/tower-rs/tower-lsp |

（累计 22 个对象：正文详述 16 + 扫视 6。）

---

## 六、重点回答

### ① ropey vs piece table 最终对比（ADV"带快照的文件缓冲"选型）

| 维度 | ropey（rope） | VS Code piece tree（piece table） |
|---|---|---|
| 编辑复杂度 | O(log n)（B-tree 路径） | O(log n) + piece 合并/分裂 |
| 内存 | 每叶子 chunk 有拷贝；树开销 | 原文件零拷贝，最省（VS Code 2018-03 官方口径胜出项） |
| **O(1) 快照** | **有**（clone 共享根，写时复制） | 无（需深拷贝 piece 序列或改造为持久化结构） |
| 后台线程解析/IO | 直接支持（快照跨线程发送） | 需要额外版本化层 |
| Rust 生态 | 现成（ropey 2.x，helix/lapce 生产验证） | 无成熟现成实现，需自研 |
| 多游标/协作演进 | 好（xi/Zed 均在 rope 上做 CRDT） | VS Code 自身也未在其上做 CRDT |

**结论：选 ropey。** 判据：ADV 的核心循环是"编辑 → 后台解析 → 报告"，快照能力是一等需求（判据时序独立：无论解析并发与否，快照都不亏）；piece tree 的内存优势是 VS Code 特定约束（超大文件 + 原文件常驻）下的度量结论（样本：其 2018 benchmark），不可外推为普遍优劣。

### ② Zed multibuffer 机制拆解 → ADV 价值
见 §7。三件套：`SumTree<Excerpt>` 虚拟文档（聚合多源片段为一个可编辑视图）；编辑按 excerpt 拆回源 buffer（源是唯一真值）；Anchor 使引用随编辑漂移仍有效。**对 ADV 的映射**：扫描结果视图 = excerpt 集合；用户在视图内的修改 = 转发写回真实文件；诊断/违规引用 = anchor 化区间。先做数据结构（excerpt+anchor），后做视图壳。

### ③ tree-sitter 增量高亮实现要点
见 §15/§16。压缩版：① 旧树 + `edit()` 平移 + 传入 parse ⇒ 子树级复用；② `changed_ranges` 圈定查询重算区间，其余沿用捕获缓存；③ injections 声明式子解析 + layer 叠加；④ 后台线程解析依赖 rope 快照。参考实现：helix `syntax.rs`、nvim `highlighter.lua`（0.11 已异步化）。

### ④ 编辑器扩展的 WASM 沙箱：Zed vs VS Code（adv-sandbox 互证）

| 维度 | Zed（WASM 线） | VS Code（进程线） | Lapce（WASI 线） |
|---|---|---|---|
| 隔离单元 | wasmtime 实例（内存/CPU 可控，fuel/epoch 可中断） | 独立 Node 进程 | wasmer 实例 |
| 权限模型 | 能力供给制：无环境权威，host 显式注入（http_client、受限 fs） | 完整用户权限，靠商店审核 + proposed API 闸门 | WASI 能力制 |
| ABI | WIT（类型化接口） | 私有 RPC 协议 | RPC（volt 插件协议） |
| 性能隔离 | 实例级；WASM 开销有界 | 共享单 host 线程，弱 | 实例级 |
| 崩溃影响 | 实例挂，宿主不挂 | 进程挂可重启，主进程不挂 | 实例挂 |

**互证结论**：两个 Rust 编辑器（Zed、Lapce）独立收敛到 "WASM/WASI + 类型化接口 + 能力注入"，且 wasmtime 已提供 fuel/epoch 中断（超时治理）——adv-sandbox 插件 ABI 建议以 **WIT + 显式能力注入** 为候选，与"进程+RPC"方案做一次 spike 对比后定档。

### ⑤ ADV 的 IDE 面：先 LSP 还是先编辑器插件（判据）
**先 LSP。判据**：
1. **覆盖比**：LSP 一份实现可被 VS Code/neovim/helix/Zed/Emacs 原生消费（均为内置 LSP 客户端）；编辑器插件 = 每编辑器一份实现。比值随目标编辑器数量线性放大。
2. **能力映射零阻抗**：adv-parse 诊断 → `publishDiagnostics`；adv-index 定义/引用/符号 → `definition/references/documentSymbol`。协议现成，不需要编辑器新特性。
3. **与插件路线正交**：Zed 扩展的一等形式就是"注册一个语言服务器"（§11），VS Code 插件常见形态也是薄壳包装 LSP——LSP 稳定后插件只是 UI 增量（decorations、树视图这类 LSP 表达不了的面）。
4. **唯一反例判据**：若产品卖点压在"编辑器内交互体验"（内联 decoration、扫描视图 UI）而 LSP 无法表达，则该编辑器的薄壳插件提前——但顺序仍是 LSP 先行。
- **第一步动作**：adv-lsp 最小面 = `didOpen/didChange`（增量同步）+ `publishDiagnostics` + `documentSymbol`；框架从 tower-lsp 起评。

---

## 七、2025–2026 前沿信号
1. **Zed×LiveKit 澄清**：无收购；LiveKit 2026-01 完成 $100M C 轮（$1B 估值）独立运营，Zed 仍以自有 CRDT+collab server 为文本协同、LiveKit 只做音视频（issue #22374, 2024-12）。
2. **tree-sitter 生态 Rust 化收敛**：tree-sitter-perl-rs epic（2026-07）把 highlight/locals/injection/tags/fold 的 Rust 原生实现统一列为目标——Rust 侧 highlight 生态正在固化，adv-parse 依赖面可收敛。
3. **VS Code 不动**：至 2026-06 第三方口径仍确认 VS Code 不原生用 tree-sitter（TextMate + LSP semantic tokens）——语法高亮的"两代体系"并存仍是现状。
4. **Neovim 0.11（2025-03）**：tree-sitter 异步高亮/注入查询迭代 + `vim.lsp.config` 收编 LSP 配置——"异步化 + 标准面收编"两个方向与 ADV 路线同向。
5. **ropey 2.0 线已发布**（lib.rs 口径），crop 以"快照主打"续刷 rope 赛道（crates.io, 2025-04-25 口径）——rope 在 Rust 侧仍是活跃演进区。
6. **helix 无插件立场延续**：维护者口径"插件将来会帮上忙"仍未落地（HN）——"内建 vs 插件"之争中内建阵营持续，对 ADV 的启示：核心能力内建、扩展面后行。

## 八、Top-3
1. **ropey（§1）**：ADV 文件缓冲选型定档——O(1) 快照 + B-tree 分段 + chunks 零拷贝喂 tree-sitter，与后台解析并发模型严丝合缝。
2. **Zed multibuffer（§7）**："扫描结果 → 可操作视图"的完整机制范本（SumTree<Excerpt> + 编辑转发 + Anchor 保活）。
3. **tree-sitter 增量高亮三件套（§15/16）**：edit+old_tree 解析复用 / changed_ranges 圈定查询重算 / injections 分层——adv-parse 高亮管线的实现清单。
