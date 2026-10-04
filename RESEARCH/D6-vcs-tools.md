# D6：Git/VCS 工具域调研

- 日期：2026-10-03；检索窗口：2025-01～2026-10 的公开资料（WebSearch 汇总 + 项目官方文档）。
- 服务对象：ci-ops（自托管合并队列）、cli（本地 VCS 操作）、adv-index（增量扫描变更集）、xtask-gates（hooks 门禁）。
- 结论口径说明：git 本体特性以 git-scm 官方文档为锚；第三方项目以仓库/release note 为锚；带"报道口径"的数字是二手转述，采用前需复测。

---

## 0. 2025–2026 前沿信号

1. **gitoxide 迁至 GitoxideLabs 组织**（原 Byron/gitoxide），Cargo 已用它做 fetch（生产实证）、GitButler 用它作后端并持续赞助（GitMerge 2024 演讲、GitButler 博客 2025 资助报告）；2025-12 每周 changelog（#2300）仍在推进 gix-fs / gix-transport / gix-protocol 清理——维护活跃。
2. **Git 3.0 路线定调**（官方 BreakingChanges 文档）：新仓库默认 reftable、对象格式默认 SHA-256、Rust 必需。过渡档位：Rust 支持 2.52 自动探测（2025-11）→ 2.55 默认启用 → 3.0 必需（dev.to 2026-09 转述官方文档口径）。
3. **reftable 性能拐点**：2.55 批量写 1 万分支 ref 从约 650ms 降到约 40ms、磁盘 40MB→272KB（报道口径）；2025-09 有 GitHub 议题称超大规模仓库上 reftable 已反超 files backend。
4. **Git 2.52**（2025-11-17）：新增 `git repo`（仓库特征查询）与 `git last-modified`（按目录快速定位每文件最后修改提交，比 log 遍历更快更结构化）；**2.53**（2026-02 前后）：`git maintenance is-needed`（判断维护是否真的需要）、blame 可选 diff 算法；**2.54**（2026-05）：config-based hooks——hooks 生态开始被 Git 本体收编。
5. **jj 生态信号**：作者 Martin von Zweigbergk 离开 Google（转任 ERSC CTO），但项目维持活跃；围绕 AI 代理的工具出现——jj-hunk（给 coding agent 的程序化 hunk 选择器）、tangled 平台用 change-id 做堆叠 PR。这直接印证"多 AI 会话并发"是 jj 的目标场景之一。
6. **Grit**（GitButler）：宣称 AI 辅助、以通过 C git 测试套件为验收的 Rust 重写 Git（2026 报道口径）——纯 Rust VCS 栈的潜在竞争者，未确认许可证，watch。
7. **hooks 工具 Rust 化**：pre-commit 的 Rust 重实现 prek 出现（prek.j178.dev），主打速度与单二进制；difftastic 进入 Thoughtworks Technology Radar（语法感知 diff 被行业雷达收录）。
8. **部分克隆成为 CI 默认选项**：GitLab 博客口径 blob:none 比 full clone 快 ≥50%；GitHub 博客 2020 基文仍是标准参考；2023-11 起 Azure DevOps 全面支持 treeless/blobless——主流托管端均已可用。

---

## A. Rust Git 栈

### A1. gitoxide / gix —— 纯 Rust Git 实现（重点）
- 定位：crate 级 Git 实现栈（gix-* 系列 crate + `gix` CLI），面向应用嵌入而非替代 git 二进制；强调流式读取、progress（prodash）、纯 Rust 安全性。
- 可抄机制：
  1. 分层 crate 拆解（gix-odb/gix-ref/gix-index/gix-diff/gix-revwalk/gix-url/gix-config/…）：ADV 可按需只引读路径子集，编译面小；
  2. 对象数据库流式访问（loose + pack 流式解码，不做整体物化）；ref 扫描与 reftable 读支持；
  3. progress-tree（prodash）模式：长操作（fetch/pack 生成）可挂层级进度回调——ADV cli 进度条直接可抄。
- 成熟度分档（截至本次检索证据）：**读路径可生产**——odb/revwalk/refs/config/attributes/mailmap/index/status/tree-diff/commit-graph 读；**网络路径有生产实证**——Cargo 用其 fetch；GitButler 用其读写；gix CLI 的 clone/fetch/push 覆盖 http/git/file/ssh（ssh 经外部 ssh 进程）。**写路径仍落后 git 本体**（历史改写、部分维护命令无对应物）。
- 档位：【吸收】（读元数据路径）；写操作回到 git CLI。
- 第一步动作：在 adv-index 用 `gix` 做 refs/odb/revwalk/status 只读接入做一次对照基准（同一仓库对 git CLI 计时），落数字再定。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（活跃，GitButler 赞助）。
- 链接：https://github.com/GitoxideLabs/gitoxide

### A2. gix 网络栈（gix-transport / gix-protocol）
- 定位：fetch/push 协议实现（v0/v1/v2），http/git/file + ssh（子进程）传输。
- 可抄机制：① 传输与协议解耦（transport trait）——ADV 自托管服务端协议测试可复用同一抽象；② capability 协商数据结构；③ pack 生成/校验（gix-pack）。
- 档位：【有界】——ADV 自托管 CI 若走 git CLI 托管传输则只取其 v2 协商结构做参考；实现服务端时另行评估。
- 第一步动作：把 v2 capability 清单整理成 ci-ops 服务端协议用例表。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://docs.rs/gix-protocol

### A3. git2-rs（libgit2 的 Rust FFI）—— 对照组
- 定位：rust-lang 官方维护的 libgit2 绑定；Rust 生态的"默认 Git 库"，但正被 gix 分流。
- 已知坑（有锚）：
  1. Windows 平台极端慢的既有报告（rust-lang/git2-rs#963，2023-06）——ADV 是 Windows 优先，这是硬伤候选；
  2. FFI 对象生命周期与 Repo 绑定、回调 panic 跨 FFI 的处理约束；内存归 C 分配器管理，长驻进程需注意释放路径；
  3. API 覆盖滞后于 git 本体（作者在 docs.rs 自述"likely lacks some bindings"）。
- 档位：【有界】——作为 gix 缺口时的兜底绑定保留，不作为主读路径。
- 第一步动作：仅在 gix 无法覆盖的功能点清单里评估 git2（而非整体引入）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/rust-lang/git2-rs

### A4. libgit2 本体
- 定位：C 实现的可嵌入 Git 库（git2-rs 与众多客户端的后端）。
- 可抄机制：① reftable 读支持已并入（1.8 起）；② ODB 后端抽象（可插拔对象存储）——ADV 若要做本地对象缓存层可参考其接口形状。
- 档位：【reference】——经 git2-rs 间接使用，不直接引 C 依赖。
- 许可证/成熟度：GPLv2 + linking exception / 维护中。
- 链接：https://github.com/libgit2/libgit2

### A5. Grit（GitButler 的 Rust 重写 Git）
- 定位：AI 辅助、以"C git 测试套件通过"为验收的 Git 重写（2026 报道口径，与 gitoxide 不同人主导、相互独立）。
- 可抄机制：以上游测试套件作为兼容性验收门——对 ADV 自研任何 VCS 子系统都是可直接复用的判据设计。
- 档位：【watch】。
- 第一步动作：每季度复查一次公开状态与许可证。
- 许可证/成熟度：未确认 / 早期。
- 链接：https://blog.gitbutler.com/

---

## B. 底层格式与协议（git 本体特性，均 GPLv2 / 官方文档锚）

### B1. packfile（pack/idx/bitmap/midx）
- 定位：Git 对象的主存储格式：.pack（delta 链，ofs-delta/ref-delta）+ .idx v2（按哈希二分索引）+ .bitmap（EWAH 位图，加速可达性枚举）+ multi-pack-index（跨 pack 索引）。
- 可抄机制：① delta 链的 window/depth 启发式（写侧压缩与读侧解压成本的权衡参数）；② bitmap 加速 `rev-list --objects` 的思路——adv-index 可用同构位图做"变更对象集"快速差分；③ midx 增量写（2.50 改进）。
- 档位：【吸收】（机制知识；adv-index 的对象缓存层设计直接受益）。
- 第一步动作：adv-index 对象缓存设计评审时把 delta 链参数与位图差分列为必答题。
- 许可证/成熟度：GPLv2（Git 本体）/ 稳定。
- 链接：https://git-scm.com/docs/pack-format , https://git-scm.com/docs/multi-pack-index

### B2. commit-graph
- 定位：commit 元数据的旁路文件：generation numbers（corrected commit dates）、changed-path bloom filters、split commit-graph 分片写。
- 可抄机制：① generation number 使跨分支可达性判断免于全图遍历——合并队列 merge-base 判断的加速器；② bloom filter 加速 `log -- path`——adv-index 的"哪些提交碰过此路径"查询可直接对齐；③ split 分片写避免全量重建。
- 档位：【吸收】。
- 第一步动作：ci-ops 仓库初始化默认开 `gc.writeCommitGraph` + `fetch.writeCommitGraph`，并在基准里测 merge-base 提速。
- 链接：https://git-scm.com/docs/commit-graph

### B3. reftable（2026 现状）
- 定位：JGit/Gerrit 起源、GitLab 上游化的 ref 后端（2.45，2024-04/05 并入主线，含 `git refs migrate --ref-format=reftable` 迁移命令）；有序文件 + 二级索引，避免 refs 目录散文件。
- 2026 现状：格式已稳定；2.48 修了 Windows 上 racy 写导致的 I/O 错误并解耦库依赖；2.55 批量写性能数量级提升（650ms→40ms、40MB→272KB，报道口径）；**Git 3.0 将把新仓库默认切到 reftable**（官方 BreakingChanges）。gix 已有 reftable 读实现。
- 可抄机制：① ref 存储从"一 ref 一文件"到有序表的迁移路径（ADV 元数据存储同理）；② per-worktree ref 表的隔离设计。
- 档位：【吸收】（作为演进方向知识 + ADV 仓库形态决策输入）。
- 第一步动作：ADV 自用仓库在 3.0 发布前不主动迁移（工具兼容面未收齐），仅跟踪。
- 链接：https://git-scm.com/docs/reftable

### B4. 部分克隆（partial clone / promisor remote）
- 定位：`--filter=blob:none / tree:0` 拉取时省略 blob（或树），缺失对象按需从 promisor remote 惰性拉取；服务端需 `uploadpack.allowFilter`。
- 实测口径：GitLab 博客称 blob:none 克隆快 ≥50%；反面锚：GitHub 议题报告 `--depth 1 --filter=blob:none --no-checkout` 后 checkout 大量小文件时因逐对象惰性拉取极慢（网络往返放大）。
- 可抄机制：① promisor 模式（"承诺方"兜底缺失对象）——adv-index 的分层缓存可用同构设计；② filter 表达式的能力协商。
- 档位：【吸收】（ci-ops 拉取策略默认 blob:none，并配防逐对象往返的批量补齐）。
- 第一步动作：ci-ops clone 模板固定 `--filter=blob:none` 并对大仓 checkout 做一次往返实测留档。
- 链接：https://github.blog/open-source/git/get-up-to-speed-with-partial-clone-and-shallow-clone/ , https://about.gitlab.com/blog/（partial clone 文）

### B5. fetch 协议 v2
- 定位：capability 协商 + ls-refs（按需列 ref，不再全量通告）+ fetch 过滤器 + packfile-uris（CDN 卸载）+ sideband。
- 可抄机制：① ls-refs 把"列 ref"变成独立 RPC——自托管服务端实现直接对齐；② packfile-uris 思路：把重负载从协议流挪到 CDN。
- 档位：【吸收】（ci-ops 服务端协议设计基准）。
- 第一步动作：ci-ops 服务端协议 spec 引用 v2 文档并逐 capability 标记实现/忽略。
- 链接：https://git-scm.com/docs/protocol-v2

### B6. 浅克隆（shallow clone）在 CI 的取舍
- 定位：`--depth`/`--shallow-since` 截断历史；最快最小，但 blame/describe/跨边界 merge-base 受损，`--unshallow` 补全成本高。
- 判据（写给 ci-ops）：合并队列必须能算 merge-base ⇒ 历史深度必须覆盖目标分支发散范围；深度不足时 `merge-tree` 预检会失真。浅克隆只适合"全量覆盖式"构建，不适合队列合并预检。
- 档位：【有界】（按任务区分：构建用浅克隆、队列预检禁用）。
- 第一步动作：ci-ops 配置里把两类任务的 clone 模板分开并注释判据。
- 链接：https://git-scm.com/docs/git-clone

### B7. sparse-checkout（cone mode）
- 定位：工作区只检出路径子集（cone 模式按目录锥匹配），index 仍全量（skip-worktree 位）。
- 可抄机制：skip-worktree 位机制——adv-index 做"大仓部分物化"时的现成参照。
- 档位：【有界】（cli 面向大仓时的可选能力）。
- 链接：https://git-scm.com/docs/git-sparse-checkout

---

## C. 独立工具

### C1. jj（Jujutsu）—— 操作日志 / 自动 rebase（重点）
- 定位：Git 兼容的 VCS（jj-vcs/jj，Apache-2.0；0.25 于 2025-02 前后，2025 下半年持续发版；版本号以 releases 页为准）。核心模型差异：工作副本即提交、无暂存区、change-id 与 commit-id 双标识、一等公民冲突。
- 机制拆解：
  1. **操作日志（op-log）**：每次仓库变更先写一个 operation（引用新的仓库视图），操作之间可并发分叉，op heads 可合并 ⇒ 天然记录"谁在何时改了什么"，`jj op log / undo / restore` 站在操作层而非提交层；
  2. **自动 rebase**：改写一个提交，后代自动重放——多会话各自演进时不产生"你 rebase 我"的协调成本；
  3. **workspace**：一个仓库多个工作区，共享对象库与 op-log，各自独立 HEAD。
- 多 AI 会话并发适配性评估（重点回答，详见 §2.③）：op-log 的分叉-合并语义与"无暂存区"设计，正好消解多会话共享仓库时 index 冲突与 HEAD 抢占两类事故面；代价是团队/工具链需整体理解 jj 模型，0.x 期 API 与 CLI 语义仍在动。
- 档位：【watch】（本体）；**op-log/auto-rebase 模型作为概念吸收进 ci-ops 审计日志设计**。
- 第一步动作：在 ADV 文档里为 ci-ops 写一页"操作日志 vs 提交历史"的双层审计设计草案（抄 jj 的 operation/view 分层）。
- 许可证/成熟度：Apache-2.0 / 维护中（活跃；作者已离开 Google 为事实，社区化推进）。
- 链接：https://github.com/jj-vcs/jj , https://docs.jj-vcs.dev/

### C2. jj 与 Git 互操作（colocated workspace）
- 定位：`jj git init --colocate` 让 .git 与 jj 仓库同目录，git/jj 命令可互换使用（rwblickhan.org 2026-07 教程口径）；`jj git fetch/push` 对接 GitHub/GitLab。
- 可抄机制：colocated 模式下 git 钩子、IDE、既有工具链全部不受影响——"新工具包旧仓"的迁移姿势。
- 档位：【watch】。
- 第一步动作：若试用，先在 ADV 的某个低风险 worktree 上 colocate 验证 xtask-gates 钩子不被破坏。
- 许可证/成熟度：Apache-2.0 / 维护中。

### C3. lazygit
- 定位：Go 写的 git TUI（jesseduffield/lazygit，MIT）。
- 可抄机制：① **不重实现 git**——全部经子进程调 git plumbing/porcelain 并解析输出（pkg/commands 层）；这与 ADV cli 的"薄封装 git"路线同构，其命令拼装/错误处理/输出解析层值得对照；② gocui 的 view/panel 事件模型（C4 域交叉）；③ 自定义命令配置面。
- 档位：【reference】。
- 第一步动作：ADV cli 的 git 调用层评审时对照其命令解析清单。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://github.com/jesseduffield/lazygit

### C4. delta
- 定位：Rust diff 渲染器/分页器（dandavison/delta，MIT）；C1 域已覆盖性能角，此处补算法角。
- 机制：输入即 `git diff` 的 unified diff 流（解析而非自算）；语法高亮用 syntect；行内词级二次 diff；侧栏并排布局用 box-drawing。
- 算法角补注：**算法选择权在 git 侧**——`diff.algorithm` myers（默认）/patience/histogram + `--indent-heuristic`（2.14 起默认）；histogram 对代码重排更稳，2.53 起 blame 也可选算法。
- 档位：【有界】——ADV cli 的 diff 展示可直接配置 delta（外接）或抄其渲染分层；不自研渲染。
- 第一步动作：cli 的 `adv diff` 命令设计为"git diff 算法可配 + 渲染可外接 delta"。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://github.com/dandavison/delta

### C5. difftastic —— 语法感知 diff（重点）
- 定位：结构性 diff（Wilfred/difftastic，MIT；进入 Thoughtworks Radar）。
- 机制拆解（对 ADV 最有价值的部分）：
  1. 两侧文件各自经 tree-sitter 解析成具体语法树（CST，含标点与注释，非省略 AST）；
  2. diff 归约为图搜索：把"左树节点 × 右树节点"的配对建成 DAG（graph.rs），边代价衡量配对成本，Dijkstra 求最省路径（shortest_path.rs）——优化目标是"最大化匹配节点/最小化改动"，而非行编辑距离；
  3. sliders.rs 处理定界符滑移（逗号挂前项还是后项）避免误导性配对；
  4. 渲染按改动子树展示，空白/缩进/换行差异被天然忽略。
- 对 ADV"语义级变更检测"的价值（详见 §2.②）：树 diff 的**未匹配子树集合**就是天然的"语义变更范围"，可直接用于把增量扫描的文件级变更集收窄到"函数/块级"，降低重复分析量。
- 已知边界：依赖各语言 tree-sitter grammar 的版本与质量；两侧都无法解析时退化为文本 diff；大文件开销高于行 diff；"语法相同"不等于"语义相同"（同名标识符含义可变），只能作收窄信号而非最终判据。
- 档位：【吸收】（adv-index 的变更集收窄层）。
- 第一步动作：用 difftastic 对 ADV 自仓跑一次改写对比，统计"文件级变更 → 子树级变更"的收窄比，作为是否引入的判据（先量后改）。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://difftastic.wilfred.me.uk/ , https://github.com/Wilfred/difftastic

### C6. git-filter-repo
- 定位：官方推荐的历史改写工具（newren/git-filter-repo，MIT），接替 filter-branch/BFG。
- 可抄机制：① `fast-export` 流 → 中间改写 → `fast-import` 重建 pack，全程不检出工作区（与 filter-branch 的逐提交 checkout 本质区别）；② 改写账本落在 `.git/filter-repo/`：commit-map（旧→新哈希）、ref-map——**"改写必须留账"的设计正是 ADV xtask-gates 需要的审计形状**；③ `--analyze` 只读分析报告（路径/大小聚合）。
- 已知坑：多 pass 时 commit-map 会被覆盖（需自行留存）；同命令多次运行结果哈希可不同（非确定性，issue #705，2025-09）——哈希不能当永久事实引用。
- 档位：【有界】（机制吸收：账本格式；工具本体仅历史清洗时手动使用）。
- 第一步动作：把 commit-map 两列格式定为 ci-ops"改写操作"的留账格式参照。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://github.com/newren/git-filter-repo

### C7. git-lfs
- 定位：大文件扩展（git-lfs/git-lfs，MIT，Go）：树里存指针文件，对象进 LFS 存储，smudge/clean 过滤器 + pre-push 钩子上传，HTTP Batch API 传输。
- 可抄机制：① "指针文件 + 外部对象存储"的分层（adv-index 若需大产物缓存可同构）；② pre-push 钩子做上传的时机选择。
- 对 ADV 的定位：自托管平台大概率不需要 LFS（本地优先、无二进制大资产需求）；理解它主要为兼容用户仓库。
- 档位：【reference】。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://github.com/git-lfs/git-lfs

### C8. BFG Repo-Cleaner（对照物）
- 定位：旧一代历史清洗工具（Scala），被 git-filter-repo 官方口径接替。
- 档位：【不吸收】——机制面被 filter-repo 覆盖（速度快但无 commit-map 级账本与 analyze）。
- 许可证/成熟度：GPLv3 / 低频维护（弃维护边缘，以仓库为准）。
- 链接：https://rtyley.github.io/bfg-repo-cleaner/

---

## D. Git 工程实践

### D1. 原生 hooks 体系（core.hooksPath / server hooks）
- 定位：客户端钩子（pre-commit/pre-push/…）+ 服务端钩子（pre-receive/update/post-receive）；`core.hooksPath` 可整体改指向；2.54 起 config-based hooks 进入主线（2026-05 口径）。
- 可抄机制：① `core.hooksPath` 指向 xtask-gates 的统一代理入口——ADV 门禁挂接点的标准做法（不碰 .git/hooks）；② pre-receive 的 ref 粒度拒绝语义（自托管队列的服务端校验位）；③ hook 的 cwd 与环境契约见 D2。
- 档位：【吸收】（xtask-gates）。
- 第一步动作：xtask-gates 安装器改为写 `core.hooksPath`，并在文档记录回滚路径。
- 许可证/成熟度：GPLv2（Git 本体）/ 稳定。
- 链接：https://git-scm.com/docs/githooks

### D2. pre-commit 框架与 GIT_* 环境变量坑（旧仓教训的机制化）
- 定位：pre-commit/pre-commit（Python，MIT）：钩子仓库按 revision 固定、stages 划分、language 隔离；Rust 重实现 prek 已出现。
- 坑的机制根源（本次检索已锚定）：**Git 只在 linked worktree 里向钩子进程导出 GIT_DIR 与 GIT_COMMON_DIR，普通检出里不导出**（GitHub 议题原文口径，2026-09）；GIT_INDEX_FILE 一旦从外层泄漏进子进程，子进程的 git 会指错 index（"EnterWorktree corrupts git index"类事故，2026-03）。旧仓的 GIT_* 变量教训属于此类：**凡在钩子/嵌套场景里再起 git 子进程，必须先清洗 GIT_DIR/GIT_COMMON_DIR/GIT_INDEX_FILE/GIT_WORK_TREE**，否则上游 git 的环境状态会改写子进程的仓库定位。
- 档位：【有界】（框架本体不吸收；环境清洗纪律吸收进 xtask-gates 的所有 git 子进程调用）。
- 第一步动作：xtask-gates 封装 `run_git()`：入口处 `env_remove` 这四项 + 文档记录判据。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://pre-commit.com , https://prek.j178.dev/

### D3. worktree 机制与多树并发坑清单（ADV 自用，重点）
- 定位：`git worktree add` 在 `.git/worktrees/<id>/` 建管理区（gitdir 反链、独立 HEAD/index），对象库与 refs 经 common dir 共享。
- 坑清单（每条带来源性质）：
  1. **分支独占**：同一分支不能在两个 worktree 同时检出（"already checked out"拒绝；绕行用 `--detach` 或新分支）——多会话抢同一分支时的第一坑；
  2. **prune 误删**：`git worktree prune` 按本机可达性判断，网络盘/可移动盘上的 worktree 会被清掉管理区——需 `git worktree lock` 保护；add+lock 与 prune 的竞态在 2.13 才补锁（官方 release notes 口径）；
  3. **移动失联**：手动 `mv` worktree 目录会断反链，恢复用 `git worktree repair`（旧自动修复代码有 bug 史）；
  4. **gc 跨树误删对象**：老版本 gc 可能清掉仅被其他 worktree index 引用的对象，现代 git 会扫全部 worktree index，但 `--prune=now` 激进参数仍应禁用；
  5. **stale lock**：中断留下的 `index.lock`、`worktrees/<id>/locked`、`gc.log` 需逐个甄别清理，脚本化时先判进程存活再删；
  6. **每树独立 index**：并发写不同 worktree 的 index 不互斥（好事），但共享对象的写入（pack/midx/commit-graph）靠各自 lock 文件串行——高并发多会话下维护命令要错峰。
- 档位：【吸收】（ADV 自身就是多 worktree 用户；此清单进 ci-ops 与 CONTRIBUTING）。
- 第一步动作：把 6 条坑落成 ADV 仓库的 `git worktree` 操作 SOP（含 lock/repair/unlock 命令序列）。
- 许可证/成熟度：GPLv2 / 稳定。
- 链接：https://git-scm.com/docs/git-worktree

### D4. stash 内部机制
- 定位：stash 是普通提交：WIP 提交（父=原 HEAD）+ index 提交（第二父）+ 未跟踪文件提交（第三父，2.32 起 `-u` 统一走第三父）；挂在 refs/stash 的 reflog 上；apply 走 merge 机制（merge-ort）。
- 可抄机制：① "临时状态也是提交"——多会话工作流里比 stash 更稳的是直接建临时分支/提交；② reflog 作为操作追溯层（免费、本地）。
- 档位：【reference】（机制理解为主）。
- 许可证/成熟度：GPLv2 / 稳定。
- 链接：https://git-scm.com/docs/git-stash

### D5. merge-ort 与 `git merge-tree --write-tree`（合并队列核心）
- 定位：merge-ort 是 2.34 起的默认合并策略（重写的目录改名检测、更好的冲突标注）；`git merge-tree --write-tree`（2.38+）提供**无工作区的真实合并预演**：直接写出一个 tree 对象并返回冲突清单，退出码区分干净/冲突/错误。
- 可抄机制：① 合并队列的"能否自动合并"预检 = 一次 `merge-tree` 调用，不需要 checkout、不碰工作区、可并发（对象库写入有 lock 兜底）——这是 ci-ops 合并队列预检的标准原子操作；② basename 驱动的目录改名检测减少了误判面。
- 档位：【吸收】（ci-ops 预检主路径）。
- 第一步动作：ci-ops 队列原型第一件事就是封装 merge-tree 预检 + 退出码三分支处理。
- 链接：https://git-scm.com/docs/git-merge-tree

### D6. push --atomic 与 force-with-lease（CAS 推送）
- 定位：`--atomic` 让多 ref 推送在服务端全成全败；`--force-with-lease=<ref>:<expect>` 是以期望值做 compare-and-swap 的安全强推——两者合并即是"合并队列写入"的原子提交原语（需服务端支持，主流托管端已支持）。
- 可抄机制：CAS 语义直接映射为队列租约：队列先写 ref（atomic+lease），再触发后续流水线。
- 档位：【吸收】（ci-ops）。
- 第一步动作：ci-ops 推送封装固定 `--atomic --force-with-lease` 组合，并记录服务端能力探测命令（`git push --dry-run` 探测）。
- 链接：https://git-scm.com/docs/git-push

### D7. git maintenance / fsck（后台维护）
- 定位：`git maintenance` 把 gc/commit-graph/prefetch/loose-objects/incremental-repack/pack-refs 组织成后台任务（foreground/hourly/daily 档）；2.53 加 `is-needed` 判断是否真的需要维护。
- 可抄机制：① 任务分档 + is-needed 短路——adv 后台维护调度的直接模板（避免为维护而维护）；② cruft pack 与 MIDX 增量写（2.50/2.51 改进口径）。
- 档位：【有界】（ci-ops 仓库后台维护策略引用之）。
- 第一步动作：ci-ops 节点配置 `git maintenance start` 前先用 is-needed 网关包一层。
- 链接：https://git-scm.com/docs/git-maintenance

---

## 2. 重点回答

### ① gitoxide 成熟度与 gix/git2 选型
- 可生产（读）：odb（loose+pack 流式）、revwalk、refs（files + reftable 读）、config/attributes/mailmap、index、status、tree-diff、commit-graph 读。证据锚：Cargo 用 gix 做 fetch（网络读路径的生产实证）、GitButler 全量后端使用 + 赞助（GitButler 博客 2025）；仓库活跃（2025-12 changelog #2300）。
- 仍依赖 git CLI：历史改写、`worktree` 管理区维护、部分维护命令；写路径覆盖低于 git 本体。
- 选型判据（ADV）：**元数据读 → gix**（纯 Rust、无 FFI 内存归 C 的释放问题、git2 在 Windows 有慢的既有报告 #963）；**缺的功能点 → git CLI 子进程**（而非引入 git2），只有当某功能点既高频又 gix 缺口时才评估 git2 兜底。判据落点：同一仓库 gix vs git CLI 读路径基准（数字留档后再定）。

### ② difftastic 机制拆解 → 增量扫描变更集收窄
- 机制链：tree-sitter 双侧解析 CST → 节点配对 DAG（graph.rs）→ Dijkstra 最小代价路径（shortest_path.rs）→ 定界符滑移修正（sliders.rs）→ 按改动子树渲染。
- ADV 收窄路径：adv-index 先行文件级 diff（git/内部比较）→ 命中文件做 difftastic 式树 diff → **未匹配子树 = 语义变更范围** → 以"函数/块"粒度喂给增量分析器，替代行号区间近似。可用性判据：先实测收窄比（改动文件数 vs 改动子树数）与误收率（语法同但语义变），两项数字达标再进主路径。边界：语法同≠语义同（标识符重绑定会漏），需叠加名称解析校验；两侧解析失败退化为行 diff。

### ③ jj 的 op-log / 自动 rebase 对"多 AI 会话并发"的适配性
- 适配点（机制对应）：a) op-log 天然并发分叉 + op-heads 合并 ⇒ 多会话各自的仓库变更互不覆盖，且审计可回放（对 ADV 的"多会话留账"是直接的概念来源）；b) 无暂存区 ⇒ 消除 index 抢占冲突面（多会话共享仓库的经典事故）；c) 自动 rebase 后代 ⇒ 会话 A 改写基础提交不会让会话 B 的栈断链；d) workspace + colocated ⇒ 每会话一工作区、git 工具链照常。
- 保留意见：0.x 演进期语义仍会动（版本号以 releases 页为准）；引入成本是仓库级而非文件级；ADV 现有钩子/门禁均在 git 语义上建。结论：**本体 watch；op-log 双层审计（operation 层 + commit 层）作为概念吸收进 ci-ops**。

### ④ worktree 多树并发坑清单
见 D3 的 6 条（分支独占 / prune 误删 / 移动失联 / gc 跨树 / stale lock / index 与维护命令的锁面）。ADV 自用的最小纪律：每树锁分支不共享；移动必 repair；脚本删 lock 前判进程存活；`--prune=now` 禁用；维护命令错峰。

### ⑤ ci-ops 合并队列的最小 Git 接口（原子操作清单）
- 读：`git for-each-ref --format`（含 symref/upstream）/ `git rev-parse` / `git merge-base` / `git cat-file --batch-check`（批量存在性）/ `git status --porcelain=v2`；元数据读走 gix（①）。
- 预检：`git merge-tree --write-tree --name-only`（退出码 0 干净 / 1 冲突 / 其他错误）——无工作区、可并发。
- 写：`git commit`（经门禁代理钩子）→ `git push --atomic --force-with-lease=<ref>:<expect>`（队列租约 CAS）。
- 同步：`git fetch --porcelain=v2 --prune`（机器可读变更清单）。
- 并发与卫生：`git worktree add --detach` 建任务树；所有 git 子进程入口清洗 GIT_DIR/GIT_COMMON_DIR/GIT_INDEX_FILE/GIT_WORK_TREE（D2）。
- 维护：`git maintenance run --task=...` + `is-needed` 网关；`gc.writeCommitGraph`/`fetch.writeCommitGraph` 开启。
- 设计原则：每步是"可独立失败与重试"的原子操作，队列状态只依赖 ref 状态 + 操作日志（抄 jj 的双层留账），不依赖工作区状态。

---

## 3. Top-3

1. **gitoxide/gix 读路径（吸收）**——ADV 元数据读的主库选型：读路径有生产实证（Cargo/GitButler），写回 git CLI；第一步是 gix vs git CLI 的读基准留档。
2. **difftastic 语法 diff（吸收）**——"文件级变更 → 子树级变更"的收窄器，直接服务 adv-index 增量扫描；先量收窄比与误收率再定集成深度。
3. **merge-tree 预检 + atomic/force-with-lease 写入（吸收）**——ci-ops 合并队列的两端原子原语：预演不碰工作区、提交是 CAS；配 jj 式操作日志做双层留账。
