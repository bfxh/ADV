# 09 本地优先代码检索与索引 / LSP 调研（供重写蓝图）

> 调研日期：2026-10-03。方法：约 18 次外部检索（WebSearch/WebFetch/webReader），优先 2025–2026 信息源（GitHub README/design doc、arXiv、HF 模型卡、社区维护状态帖）。
> 证据口径：能落到具体来源的写来源；查不到的标**待验证**；速度/内存数字凡未实测均标**估计**。本文件结论适用范围 = 单机本地、Rust 主体、仓库规模 1e2–1e5 文件。
> 旧栈对照：unified-rx-mcp 现为自写 BM25（符号加权）+ tf-idf 语义 + RRF 融合 + LSP 桥（pylsp/rust-analyzer）+ SCIP 只读消费。重写要回答的就是：哪些层换成成熟组件，哪些机制照抄。

---

## 0. 重点问题速答

### ① 检索栈分层设计（每层选型）

| 层 | 职责 | 首选 | 备选 | 关键判据 |
|---|---|---|---|---|
| L0 遍历 | 文件发现 + 增量基线 | `ignore` crate（WalkParallel） | walkdir（串行简单场景） | .gitignore 语义完整 + 并行，别自写 |
| L1 符号 | 定义/引用/调用 | rust-analyzer `scip` 导出 + `scip` crate 消费 | tree-sitter tags（多语言/无 Cargo 兜底） | Rust 编译器级精度优先；tree-sitter 只做兜底与 AST 切分 |
| L2 文本索引 | BM25 全文 + 子串/正则 | Tantivy（BM25、段式持久化） | 自写 trigram（照抄 Zoekt 机制，规模小再上） | Tantivy 换掉自写 BM25；符号加权用双字段近似迁移 |
| L3 向量 | 语义检索 | Qwen3-Embedding-0.6B（ONNX int8，ort/candle） + cAST chunking | jina-v2-base-code（速度档）；fastembed-rs 封装 | CPU 本地：0.6B 是质量/大小甜点；7B 代码专用款不现实 |
| L4 融合/重排 | 合并多路 | RRF 起步 → 加权融合实验 → cross-encoder 重排 top-K | Qwen3-Reranker-0.6B / bge-reranker-v2-m3 | 重排是当前最大缺口（见 §混合检索） |

### ② 索引持久化 vs 现算（增量怎么做）

三种已验证的增量模式（详见 §2 各对象的「值得抄的机制」）：
- **Zoekt 模式**：shard 不可变 → 变更仓库整 shard 重建 → 原子换名。简单可靠，适合文件级粒度；代价是重建粒度粗。
- **Tantivy 模式**：新文档进新段 + tombstone 删除 + 后台段合并。文档级增量，代价是合并与碎片管理。
- **向量层模式**：chunk 内容 hash 键控 → 只嵌入变更 chunk（删旧插新）；全库重嵌入仅在 embedding 模型版本变更时发生（模型版本必须写进索引元数据——否证标签思路：缓存命中要绑定产生它的模型与 chunk 哈希）。

取舍判据（适用范围：本地单机）：
- 单文件变更 → tree-sitter tags 现算（秒级）+ Tantivy 追加段 + 向量差分嵌入；rust-analyzer scip **不逐文件重跑**（全仓生成秒~分钟级，按 git commit 键控缓存复用）。
- 全仓/大变更 → 各层全量重建 + 原子换名（Zoekt 式），旧索引留作回滚路径。
- 一切缓存以「commit + 文件 hash 集 + 模型版本」为键，杜绝过期假命中。

### ③ 嵌入模型本地化最小可行方案

- **模型**：Qwen/Qwen3-Embedding-0.6B（Apache-2.0，2025-06 发布，MTEB Multilingual 榜首家族中的最小档；0.6B 档官方支持指令感知、32k 上下文、MRL 降维；HF 有 ONNX 版本）。
- **运行时**：`ort`（onnxruntime 绑定，Rust）跑 int8 量化 ONNX；纯 Rust 备选 `candle`（免 onnxruntime 依赖，代价是速度与覆盖度风险）；快速起步可试 `fastembed-rs`（封装 ort + 常用模型清单，是否含 Qwen3 待验证）。
- **细节**：Qwen3-Embedding 用 last-token pooling（EOS 位），查询端加 instruct 前缀；维度 1024，可用 MRL 截到 256/512 省向量索引内存。
- **预期速度**：CPU int8、单条 ~1k token，量级在百毫秒/条（**估计，待实测**）；批量并发显著摊薄；首次索引 1e4 chunk 级仓库预计分钟级（估计）。
- **备选**：jina-embeddings-v2-base-code（161M、8k ctx、官方 ONNX）为速度档；nomic CodeRankEmbed（137M）为小模型候选；nomic-embed-code（7B）本地 CPU 不现实。
- **先量后改**：上 CoIR 或自建金丝雀召回集，嵌入模型/量化/降维每一步都要过评测门。

---

## 1. 调研对象逐条

格式：一行定位 → 值得抄的机制 → 档位【吸收|有界|不吸收】+ 理由 → 第一步动作 → 许可证 → 链接。

### 1.1 ripgrep + ignore crate（遍历层）
- 定位：rg 是单机最快内容 grep；`ignore` crate 是它的遍历/忽略规则内核，Rust 生态 .gitignore 语义的事实标准。
- 值得抄的机制：
  1. `WalkParallel`：多线程目录遍历，gitignore 规则编译成按目录层级组合的 matcher 集，逐目录增量匹配。
  2. 完整 .gitignore/.ignore/.rgignore 语义（negation、anchoring、全局/局部叠加），外加 overrides 与回调过滤。
  3. 遍历产出自带文件类型过滤（`types`），可直接当文件清单层。
- 档位【吸收】：直接依赖，零理由自写遍历。
- 第一步：`ignore::WalkParallel` + 代码扩展名过滤 → 文件清单 + mtime/hash 基线表（增量索引的地基）。
- 许可证：MIT OR Apache-2.0（双许可）。
- 链接：https://github.com/BurntSushi/ripgrep 、https://docs.rs/ignore

### 1.2 walkdir / fd（遍历层备选）
- 定位：walkdir 是单线程串行遍历基础库；fd 是用户级 find（其并行能力同样来自 ignore crate）。
- 值得抄的机制：fd 证明了「遍历层用 ignore crate 即可达到工具级速度」——抄结论而非代码。
- 档位【有界】：需要严格顺序/极简依赖时用 walkdir；fd 是 CLI 不嵌入。
- 第一步：默认 ignore crate；walkdir 仅作 fallback 依赖。
- 许可证：均 MIT OR Apache-2.0。
- 链接：https://github.com/BurntSushi/walkdir 、https://github.com/sharkdp/fd

### 1.3 Zoekt（文本索引层，机制蓝本）
- 定位：Sourcegraph 维护的三元（trigram）倒排代码搜索引擎，Go 实现，单机设计，正则/子串精确匹配专精；README 自述"基于三元索引的快速代码搜索 + 语法感知"。
- 值得抄的机制（README + doc/design.md，2026-10-03 抓取）：
  1. **trigram → posting list**：查询从模式挑最稀有 trigram 求交出候选，正则/子串再对候选精验——「索引缩候选，regex crate 精验」的分层查询法，Rust 可原样复刻且更省（regex crate 本来就强）。
  2. **shard 架构**：索引按仓库/大小切 shard，`meta.json` 记 manifest，支持合并（merge）与原子替换；搜索逐 shard 并行——增量索引的「重建 + 原子换名」模式源头。
  3. **符号排序**：集成 universal-ctags 符号做 symbol rank、文档级 rank 定制（路径加权）——与现有项目「BM25 符号加权」是同族思想，可借其打分组合方式。
  4. indexserver 增量：按仓库更新优先级周期重索引（默认每仓库 ≤1 周全刷一次），大规模用 zoekt-archive-index 走 git archive 路径。
  5. 作为库：Go package API（indexbuilder/searcher）可直接嵌入——但**对 Rust 主体无效**，Rust 生态无成熟等价移植（待验证：是否有可用的 Rust trigram 库）。
- 档位【有界】：Go 库不能嵌进 Rust；机制（trigram/posting/shard/原子换名/symbol rank）全部值得照抄——当 L2 需要「毫秒级子串命中」且 Tantivy 觉得重时，自写 trigram 是百行级工程。
- 第一步：先用 rg/regex 精验路径兜住子串查询；当候选集过大时再按 Zoekt design doc 自写 trigram 索引（文件级 → posting list mmap）。
- 许可证：Apache-2.0（机制可抄，代码引用需留许可头）。
- 链接：https://github.com/sourcegraph/zoekt （README、doc/design.md）；深度拆解文：Thomas Tay《Exploring Zoekt》（thomastay.dev，2025-01）

### 1.4 livegrep（文本索引层，对照项）
- 定位：GitHub 系正则代码搜索（Go + C++/re2），n-gram 索引 + B-tree，Web UI 导向。
- 值得抄的机制：ngram 求交缩候选 → re2 线性正则精验；indexserver 周期重建热加载——与 Zoekt 同构，无增量信息。
- 档位【不吸收】：非 Rust 栈、面向自建搜索站、社区活跃度低于 Zoekt（提交频率**待验证**）；机制被 Zoekt 覆盖。
- 第一步：不集成；正则精确语义交给 rg/regex crate 对候选集跑。
- 许可证：Apache-2.0。
- 链接：https://github.com/livegrep/livegrep

### 1.5 Tantivy（文本索引层，主力）
- 定位：quickwit-oss 的 Rust 全文搜索引擎库（"Lucene in Rust"），BM25 + 分析器链 + 分段索引，MIT 许可，2025 生态活跃（ParadeDB pg_search、tantivy-py、Nexus 等以其为骨干）。
- 值得抄的机制：
  1. **段式增量**：新文档进新段、删除用 tombstone、后台段合并——文档级增量索引的现成实现，不用自造。
  2. **自定义 tokenizer**：可注册 camelCase/snake_case 拆分的标识符分词器——这是把现有项目「符号加权」迁进来的关键钩子。
  3. 查询侧完整：BM25 评分、短语、范围、字段 boost、多字段查询。
- 与自写 BM25 的差距：Tantivy 给的是成熟打分/持久化/并发索引；现有项目的「符号加权」需迁移为**双字段方案**（identifier 字段 + 文本字段 + 字段 boost），近似现有行为。迁移成本低于长期维护自写引擎的隐性成本（段合并、崩溃恢复都是坑）。
- 档位【吸收】：替换自写 BM25 的 L2 主力。
- 第一步：schema 定为 path/identifier/content 三字段（自定义标识符 tokenizer），用现有金丝雀语料做索引速度/体积/召回三向压测，过门再切。
- 许可证：MIT。
- 链接：https://github.com/quickwit-oss/tantivy

### 1.6 SCIP（符号层主力）
- 定位：Sourcegraph 的 protobuf 代码智能协议（LSIF 继任），预计算定义/引用/符号字符串，离线导航的事实标准；Rust 有官方 `scip` crate 读写。
- 值得抄的机制：
  1. **rust-analyzer 内建导出**：`rust-analyzer scip .` CLI 子命令直接产出 compiler-grade 符号索引（检索确认其 CLI 含 scip/lsif 导出）——Rust 侧不需要第三方 indexer。
  2. 符号字符串是稳定寻址格式（含包/版本/路径），可跨会话缓存、可 diff。
  3. 生态：scip-java/typescript/python/go/ruby 全家桶，多个 2025–2026 的 code-intelligence MCP server 直接消费 SCIP——格式即生态，MCP 时代复用面大。
- 档位【吸收】：L1 主力。现有项目「SCIP 只读消费」路线在 Rust 重写里直接升级为「rust-analyzer 产 + scip crate 消费」。
- 第一步：CI/会话内跑 `rust-analyzer scip .` 生成 .scip，读 Index 建 symbol→file/line 映射接进符号层；缓存键 = git commit。
- 许可证：Apache-2.0（proto 与 scip 库）。
- 链接：https://github.com/sourcegraph/scip 、rust-analyzer 手册 CLI 章节（rust-analyzer.github.io）

### 1.7 LSIF（符号层，legacy）
- 定位：Microsoft 前代索引格式（JSON、体积大），SCIP 明确为其继任（检索确认）。
- 档位【不吸收】：legacy；rust-analyzer 的 lsif 子命令仍在但新项目无理由选用。
- 第一步：仅保留「能读」的兼容位，不产不存。
- 许可证：协议文档许可**待验证**（无读需求则无关）。
- 链接：https://github.com/sourcegraph/scip （迁移背景）；LSIF spec（microsoft.github.io/language-server-protocol/overviews/lsif）

### 1.8 tree-sitter 符号抽取 / tags（符号层兜底 + chunking 基建）
- 定位：增量解析器家族 + 每语言查询文件（tags.scm/highlights.scm）抽取符号/引用/注释；无编译器精度但有全语言覆盖与毫秒级重解析。
- 值得抄的机制：
  1. 增量重解析：编辑只解析受影响子树——文件变更时秒级更新符号表/AST，正好喂 cAST 式 chunking。
  2. Query 捕获（@definition.function 等）产出 tags 流，语法可热换。
  3. Rust 生态成熟：tree-sitter crate + 各语言 grammar crate。
- 档位【吸收】：双重角色——非 Cargo/多语言项目的符号兜底 + 所有项目的 AST 切分基建（供 L3 chunking）。
- 第一步：接入 tree-sitter-rust；用 query 抽「顶层函数/类型/块」做 chunk 单元（见 1.13 cAST）。
- 许可证：MIT。
- 链接：https://tree-sitter.github.io 、https://github.com/tree-sitter/tree-sitter

### 1.9 stack graphs（无 LSP 的名字解析）
- 定位：GitHub 的 tree-sitter 上名字解析图框架（无 LSP 实现"跳转/查找引用"）。**2026-10-03 确认 README 顶部声明：GitHub 不再支持或更新该仓库**（建议 fork 自养）。
- 值得抄的机制：把名字解析建模为图规约、按文件增量化——思想仍先进，但维护风险改变了性价比。
- 档位【不吸收】：上游弃维护 + Rust 侧需求已被 rust-analyzer SCIP 覆盖。
- 第一步：不集成；若未来硬需求（如 JS/TS 无编译器跳转）评估 fork。
- 许可证：MIT OR Apache-2.0。
- 链接：https://github.com/github/stack-graphs

### 1.10 LSP 框架三选（tower-lsp / async-lsp / lsp-server）
- 定位（2025 社区共识，检索确认）：
  - **tower-lsp**：旧事实标准，约 3 年无实质维护（unmaintained），社区 fork 接棒。
  - **tower-lsp-server**：API 兼容的维护续作（fallow 编辑器迁移案例，解锁 pull diagnostics）。
  - **async-lsp**：tower Service 化的异步 LSP，中间件生态可复用。
  - **lsp-server**（rust-analyzer org）：同步 JSON-RPC 循环、依赖极小，rust-analyzer 自用。
- 判据（本地工具视角）：ADV 的 LSP 需求是**内嵌调用 rust-analyzer**（桥接消费），不是对外发布语言服务器 → 任何框架都不必须；只有当要向编辑器暴露自家 server 时才选型：要 tower 中间件/并发 → tower-lsp-server 或 async-lsp；要零依赖全控制 → lsp-server。
- 档位：tower-lsp【不吸收】（停更）；tower-lsp-server / async-lsp【有界】（仅在对外 server 需求成立时）；lsp-server【吸收】（对外层默认，或干脆子进程直连 JSON-RPC——现有项目的桥接模式本身成立）。
- 第一步：重写沿用「rust-analyzer 子进程 + JSON-RPC 直连」；对外 server 需求出现时再引入 lsp-server。
- 许可证：MIT / MIT OR Apache-2.0（个别 crate 具体许可**待验证**）。
- 链接：https://github.com/rust-lang/lsp-server 相关（rust-analyzer/lib）、crates.io: tower-lsp / tower-lsp-server / async-lsp

### 1.11 代码嵌入模型（L3，2025–2026 本地化格局）
| 模型 | 规模 | 代码适配 | 本地可行性 | 档位 |
|---|---|---|---|---|
| Qwen3-Embedding-0.6B | 0.6B | 通用多语含代码、指令感知、32k ctx、MRL | HF ONNX；ort/candle 可跑 | 【吸收】质量/大小/生态平衡点 |
| jina-embeddings-v2-base-code | 161M | 30+ 语言代码专用、8k ctx | 官方 ONNX，CPU 友好 | 【有界】速度档备选（2023 老，质量让位于新款） |
| EmbeddingGemma 300M | 308M | 通用多语 | ONNX-ready、MTEB <500M 段领先（HN 口碑，数字**待验证**） | 【有界】资源紧张时的通用备选 |
| CodeRankEmbed (nomic) | 137M | 代码专用小嵌入 | 可 ONNX（转换状态**待验证**） | 【有界】小嵌入 + 重排组合候选 |
| nomic-embed-code | 7B | 代码专用 | 本地 CPU 不现实（GPU 机器另议） | 【不吸收】 |
- 配套重排：Qwen3-Reranker-0.6B（与嵌入同家族）、CodeRankLLM（7B 重排，本地偏重）。
- 运行时：`ort`（onnxruntime 绑定）首选；`candle` 纯 Rust 备选；`fastembed-rs` 快速起步封装（模型清单是否含 Qwen3 **待验证**）。
- 第一步：ort + Qwen3-Embedding-0.6B int8 ONNX，离线批量嵌 1k 函数，测吞吐与抽检召回（同函数变体、跨语言对齐），过门再接全库。
- 许可证：Qwen3-Embedding Apache-2.0；jina v2 Apache-2.0；nomic 系 Apache-2.0（CodeRank 系**待验证**）。
- 链接：https://huggingface.co/Qwen/Qwen3-Embedding-0.6B 、https://huggingface.co/nomic-ai 、https://huggingface.co/jinaai/jina-embeddings-v2-base-code

### 1.12 cAST（AST-aware chunking）
- 定位：2025-06 CMU/Augment 论文（arXiv 2506.15655）《Enhancing Code Retrieval-Augmented Generation with Structural Chunking via Abstract Syntax Tree》。
- 值得抄的机制：
  1. 递归分割：自顶向下遍历 AST，超 token 预算的节点在结构边界切开，兄弟子树保持完整——保证 chunk 语法完整。
  2. 相比定长 chunk 在仓库级补全/QA/检索召回均有增益（论文实验口径）。
  3. 与 tree-sitter 天然配套：节点类型 → 切分点表，百行级实现。
- 档位【吸收】：机制简单、有论文背书、直接补现有项目「文本切块」的结构盲区。
- 第一步：tree-sitter query 抽顶层定义/块为首选切分单元，超预算递归下钻；与定长 chunk 做金丝雀集消融（先量后改）。
- 许可证：论文思想自由使用；官方代码仓许可**待验证**（未确认存在维护的实现仓）。
- 链接：https://arxiv.org/abs/2506.15655

### 1.13 混合检索 SOTA（L4，RRF 之外还差什么）
- 2025 实验口径要点（多篇 arXiv/工程复盘，样本各异，结论带范围）：
  1. **RRF vs 加权分数融合**：Recall@100 近似持平；加权融合（需 per-retriever 归一化 + 调参）在 nDCG@10 上更好——值得用自家评测集做对照实验，而非默认 RRF 最优。
  2. **cross-encoder 重排是最大单项杠杆**：某基准上 hybrid + Cohere Rerank 带来 +17.2pp（个案，范围有限，别外推）；本地对应物 = Qwen3-Reranker-0.6B / bge-reranker-v2-m3（ONNX）。
  3. **反例存在**：社区有 hybrid+rerank 使效果变差的案例——两个含义：归一化与权重必须调；评测集（金丝雀语料）是前提。
  4. 学习稀疏（SPLADE 类）本地推理成本高，第一优先级之外。
  5. 现有项目已做 RRF ⇒ **缺口排序：重排层 > 加权融合实验 > 分数归一化 > 查询扩展（符号级 query rewrite，可选）**。
- 档位：cross-encoder 重排【吸收】（Qwen3-Reranker-0.6B ONNX，top-50→top-10）；加权融合【有界】（实验门控，赢了才切）；SPLADE【不吸收】。
- 第一步：把重排接进现有 hybrid 管道尾端，金丝雀集上量化增益再定档。
- 许可证：Qwen3-Reranker Apache-2.0；bge-reranker-v2-m3 Apache-2.0（**待验证**）。
- 链接：arXiv 混合检索融合对比（2026-08 检得）；https://www.digitalapplied.com/blog/hybrid-search-bm25-vector-reranking-reference-2026 ；reddit.com/r/Rag hybrid 反例帖

### 1.14 向量索引存储（L3 补位，简查）
- 定位：向量层需要 ANN 存储；本地单机候选 = usearch（Rust 绑定）、hnsw_rs、instant-distance，或干脆暴力余弦（≤1e5 向量时毫秒级，先量后改）。
- 档位【有界】：规模未到前用暴力扫描 + 内存映射；到 1e6 再换 ANN。
- 第一步：向量存为 `.npy` 式平坦文件 + 暴力点积；压测决定是否引入 usearch。
- 许可证：usearch Apache-2.0（**待验证**）。
- 链接：https://github.com/unum-cloud/usearch

---

## 2. 索引持久化复用 vs 现算：统一增量方案（重点②展开）

分层各自的增量化路径（机制来源标注在 §1）：

1. **L0 清单**：每次检索会话起步跑 WalkParallel，产出 path→mtime/hash；与上次清单 diff 得变更集。（成本：秒级）
2. **L1 符号**：Rust 项目 → `rust-analyzer scip .` 全量生成（秒~分钟级，按 git commit 键控缓存；无 commit 变更直接复用）。非 Cargo/多语言 → tree-sitter tags 按**文件级**现算（毫秒/文件）。两者并存：SCIP 是精度层，tags 是覆盖层。
3. **L2 文本**：变更文件 → Tantivy 追加新段 + 旧文档 tombstone；后台段合并交给 Tantivy。全仓重建走「新索引目录建完 → 原子 rename 换入」（Zoekt 式），旧目录留作回滚。
4. **L3 向量**：chunk 内容 hash 键控；变更 chunk 才重嵌（差分嵌入）。索引元数据记录：模型 id + 量化档 + chunk 哈希算法版本——模型换版 = 全量重嵌的显式决策点（禁止静默混用不同版本向量）。
5. **L4 重排**：无状态，不需持久化。

复用 vs 现算的判据（单机、秒级交互预算）：
- 交互内必须现算的只有：L0 清单 diff、tree-sitter tags（未缓存文件）、查询与融合。
- 一切秒级做不完的（SCIP、Tantivy 重建、全量嵌入）都必须持久化 + 增量 + 原子换名 + 缓存键绑定产生条件（commit/哈希/模型版本）——与现有项目「否证标签是时间戳」「判据时序独立性」同纪律。

---

## 3. 2025–2026 前沿信号

1. **cAST（2025-06）**：AST 结构感知 chunking 进入代码 RAG 主流讨论，后续工作（如 "Practical Code RAG at Scale"）跟进——chunk 质量成为检索质量的第一杠杆之一。
2. **Qwen3-Embedding 家族（2025-06）**：0.6B/4B/8B 三档，8B 登顶 MTEB Multilingual；0.6B + ONNX 成为本地代码/混合语检索的默认甜点，配套 Reranker 形成「嵌入+重排」套装。
3. **小模型 ONNX 化趋势**：EmbeddingGemma 300M（2025）等把 MTEB 竞争压到 <500M 段——CPU-only 本地向量检索可行性大幅上升。
4. **SCIP 成为 MCP 时代符号基建**：2025–2026 多个 code-intelligence MCP server 直接消费 SCIP 索引（定义/引用/影响面/调用图）；rust-analyzer 是 Rust 侧官方产源。
5. **LSP 框架换代完成**：tower-lsp 停更 ~3 年，tower-lsp-server / async-lsp 接棒成为 2025 年新项目默认；lsp-server（rust-analyzer org）守住零依赖路线。
6. **stack-graphs 弃维护（GitHub 归档式声明）**：「无 LSP 做跳转」的自研路线风险上升——名字解析应押编译器/语言服务器产物（SCIP），不是图规约框架。
7. **无 GPU 代码检索成独立课题**：arXiv 2025《Searching for Code Context When You Have No Spare GPU》把 Zoekt 类精确检索作为 agent 上下文供给的低成本基线——与本项目本地优先目标同频。
8. **混合检索结论收敛**：重排 > 融合函数选择；RRF 是稳健默认，加权融合是 nDCG 潜力选项；所有增益主张必须过自家评测集（社区反例明确存在）。

---

## 4. Top-3

1. **检索栈骨架换成四层成熟组件**：`ignore`（遍历）+ Tantivy（BM25 文本，符号加权迁移为 identifier/content 双字段）+ rust-analyzer `scip`（符号，`scip` crate 消费）+ Qwen3-Embedding-0.6B ONNX int8（向量）——自写 BM25/tf-idf 只保留「融合层」与评测集。
2. **统一增量机制**：内容 hash 键控 + 「重建→原子换名」（Zoekt 机制）+ Tantivy 段追加 + 向量差分嵌入；所有缓存键绑定 commit/哈希/模型版本（时序独立性纪律落到索引层）。
3. **两个低成本高确定性增量**：cAST 式 tree-sitter 结构 chunking（直接决定向量层质量上限）+ Qwen3-Reranker-0.6B 重排层（混合检索当前最大缺口）；两者都以金丝雀评测集为验收门（先量后改）。
