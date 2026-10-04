# E3 · 新 VCS 与协作实验（2026-10-03）

> 方法锚：今天 2026-10-03 用 WebSearch 检索（英文为主），来源为搜索摘要 + 官方文档/博客链接；
> 检索通道多次限流（jj 查询重试两次），个别二手结论标「待核」。本域只答机制与流程，D6 已拆的
> jj 本体不重复拆（只补协作流程角度）；D10 交界（CRDT 文件同步）只从协作对照侧看一眼。
> 登记表：`RESEARCH/registry/E3.jsonl`（21 条，机制级 10 条）。

---

## 一、重点回答（5 问）

### ① pijul 的 patch 理论与冲突模型 —— 对 ADV 合并队列的冲突预判有借鉴吗

**机制**：仓库状态不是 commit 线性链，而是 **patch 的偏序集**——patch 之间以依赖图相连、
可交换（commute，应用顺序无关，等价意义下结果确定）；**冲突是仓库图里的一等对象**（显式的冲突
顶点/冲突文件），不是「合并失败」这个事件；**解决冲突本身被记录为一个 change**。

对比 git 三方合并的实质差异（文档级理解，未实测）：
- git 每次合并/变基都**重新计算**冲突，同一对 PR 在 rebase 长链上会**反复**冲突；pijul 把冲突
  变成持久对象，解决结果随 change 复用，不随重放复发。
- git 的冲突检测建立在 diff3 文本猜测上；pijul 基于图算法，冲突表示是精确的（该加粗处不糊）。
- pijul 无需「干净工作区」即可做绝大多数操作（与 jj 同一血统的工程取向）。

**对 ADV 合并队列的借鉴**（判据导向）：
1. 队列项建模从「commit 序列」换成「patch/变更的偏序图」：冲突预判 = 图上依赖与文件交集的
   增量检测，比线性逐个试合并便宜。
2. 「已解决冲突可复用」直接对应队列里同文件反复冲突的 PR 群——把人工/自动解决结果登记为
   可交换对象，理论上可消灭 churn。
3. 但 pijul 工程成熟度低（1.0 beta 拖多年，1.90 也才是 2025-09-18 的 pre-1.0；生态小），
   **不迁移存储层**，只取模型。

**档位【有界】**：冲突预判模型进合并队列设计文档；第一步动作=在 ci-ops 设计稿里写一页
「队列项 = 变更偏序图 + 冲突登记表」的可行性对照（拿 D6 仓库真实冲突样本回放）。
**许可/成熟度**：GPL-2.0（记忆口径，待核）｜实验（慢速推进，Nest 平台仍活）。
链接：https://pijul.org/ ｜ https://nest.pijul.com/pierre/pijul/discussions

### ② GitButler 的 virtual branches 机制 —— 多会话并发适配性评估

**机制**（一手来源：GitButler 官方博客 Building Virtual Branches / GitHub discussion）：
- 一个物理工作目录 + 一个 base branch；客户端持续做「工作目录 vs base」的 diff，
  维护一张 **hunk → virtual branch 的归属表**（每个 hunk 属于哪条逻辑分支）。
- 每条 virtual branch 是一条**真实 git 分支**，状态持久化在自定义 refs（`refs/gitbutler/*`），
  push 时映射为常规分支并自动 rebase；「切分支」= 改归属表，**不 checkout**。
- 文件/hunk 可事后拖拽改归属（lazily assign）。

**多会话并发适配性**（泼冷水半句）：同机多 agent 时，各会话产出可落进同一工作目录的不同
vbranch，互不 checkout 踢脚、可并行验证（HN 评价点）；但**共享同一物理工作目录**——两 agent
同写一个文件仍互踩，真正隔离还是要 worktree/容器；适合单机多线程，不适合多机多容器编排。
**对 ADV**：可抄的是「归属表 + 自定义 refs」这套持久化设计，不是产品本身。

**档位【有界】**。第一步动作=ci-ops 里做一个 hunk 归属表原型（Rust 可直接读 git diff 输出），
验证「多会话产出自动分桶」判据。**许可/成熟度**：FSL-1.1 源可用（以仓库 LICENSE 为准，待核）
｜维护中（Scott Chacon 创办，17M 美元 A 轮押注 agent-aware VCS，a16z 口径 2025）。
链接：https://blog.gitbutler.com/building-virtual-branches ｜ https://docs.gitbutler.com/overview

### ③ stacked diffs 流程对 ADV「一功能一 PR」纪律的增强可能

**机制**（Graphite 口径）：gt CLI 把一条功能链拆成小栈，**递归 rebase 自动化**；
`gt submit` 为栈内每条分支建/更新 PR；官方 merge queue 按栈序串行合并。
价值=评审粒度小、回滚粒度小、CI 只跑栈顶增量；代价=栈越深 rebase 与评审顺序越耦合
（LLVM 社区抱怨过其 merge queue 在 PR 上的评论噪音，discourse.llvm.org 锚）。
2026-08 有报道 Cursor（Anysphere）收购 Graphite、stacked-diff 模型被其 Origin 吸收
（happyrock.cloud，二手源待核）。

**对 ADV**：「一功能一 PR」不推翻，增强为「一功能 = 一栈小 PR + 增量 CI」——与 wave-0 05
阶段化同构。**档位【有界】**（纪律吸收，工具不绑定）。
第一步动作=ADV 自身开发流程先试：阶段化提交按栈评审，CI 每层只跑受影响 crate。
**许可/成熟度**：SaaS 闭源（gt CLI 许可待核）｜商业维护中。
链接：https://graphite.dev/ ｜ https://discourse.llvm.org/（merge queue 噪音讨论）

### ④ AI agent × VCS 的 2025–2026 新模式盘点（ADV ci-ops 可吸收项）

1. **worktree per agent**（已成默认模式）：Claude Code 官方文档收录「Run parallel sessions
   with Git worktrees」（issue #6947，2025-09 锚）；编排层工具 Vibe Kanban / Conductor /
   Crystal / Claude Squad 全走「一 agent 一 worktree 一分支」（BAML 播客 2025-12 口径）。
2. **checkpoint / git 作为 undo log**：Claude Code 原生 checkpoint（自动快照，可 rewind）；
   工程文章把它总结为「agent 每步落快照，人在 checkpoint 之上评审」。jj 的 operation log
   （每个操作可 undo）是同一思想的系统化版本。
3. **agent 友好的 VCS 叙事产品化**：GitButler 17M A 轮（agent-aware VCS）；社区把 jj 的
   operation-log/conflicts/revsets 写成 agent skill 直接喂给编码代理（eliteai.tools 收录）。
4. **队列层实验**：SPOQ（arXiv 2026-06）开始学术研究多 agent 排队/wave 分支；GitHub 侧
   Copilot Workspace 触发于带冲突 PR 自动解冲突（2025-04 changelog 口径）；Buildkite 2025-10
   原生支持 `merge_group`；Mergify 做依赖感知排队（Depends on 标注）。

**ci-ops 吸收清单（判据导向）**：① per-agent worktree + 分支命名规约；② 步级快照/undo log
（jj operation log 为设计参照）；③ 队列按 wave 分组假想合并测试（merge_group 思路）；
④ 依赖感知排队；⑤ 评审门用 reviewed@revision 失效语义（见 Reviewable）。

### ⑤ radicle 2026 对单人开发者的真实价值（泼冷水评估）

**机制**：git 之上加 **P2P 复制层**——Rust 实现（Heartwood 系），仓库与 issue 等作为协作对象
经 node/seed 网络扩散；1.0 已正式发布（17 个 RC 后官宣）；2025 后仍披露并修复过 2 个 node
协议关键漏洞（搜索摘要锚），属活跃维护但生态小。

**单人开发者真实价值：低。** 理由：① 没有协作对端时 P2P 层是纯开销（要跑 node、依赖 seed）；
② 托管配套（issue/CI/浏览）齐备度远低于 GitHub，CI 只有社区实验件；③ 网络效应税——协作者
也得装 node。真实价值只剩三点：抗审查的异地镜像备份、跨组织非信任协作实验、主权自托管
偏好。对 ADV 气质相合（本地优先），工程上**不吸收**。顺带信号：jj 官方仓库 2025 已迁到
tangled.org（ATProto P2P forge）——P2P forge 在借大项目续命。
**档位【不吸收】**（watch 其漏洞披露节奏作 P2P 复制层成熟度指标）。
**许可/成熟度**：开源（heartwood 仓库 LICENSE 为准，本次未确认细节）｜维护中。
链接：https://radicle.xyz/

---

## 二、机制级档案（10 条，格式：定位→可抄机制→档位→第一步→许可/成熟度→链接）

### 1. pijul —— patch 理论 VCS
定位：可交换 patch + 冲突一等对象的实验 VCS。可抄机制：①变更偏序图建模 ②冲突登记为
持久对象、解决可复用 ③无干净工作区操作。档位【有界】。第一步：合并队列冲突预判设计页 +
D6 真实冲突样本回放。许可/成熟度：GPL-2.0（待核）｜实验。
链接：https://pijul.org/

### 2. jj（jujutsu）—— 协作流程角度（本体 D6 已拆）
定位：Git 兼容、冲突一等公民的现代 VCS（jj-vcs 组织，2024-12 独立；0.45.1 口径）。
可抄机制：① operation log——每个操作可 undo/可回放，即系统化的 agent undo log；
② 冲突不阻塞任何操作（`jj resolve` 后补），CI/队列可对「含冲突状态」建模；
③ `jj workspace` 多工作副本共享一仓，天然 per-agent 布局。档位【有界】。
第一步：ci-ops 的 undo log 设计对照 jj operation log 写一页；ADV 仓试 colocated 模式跑
workspace。许可/成熟度：Apache-2.0｜维护中（高频发版）。
链接：https://github.com/jj-vcs/jj ｜ https://martinvonz.github.io/jj/latest/git-comparison

### 3. GitButler —— virtual branches
见重点回答②。可抄机制：①hunk→分支归属表 ②自定义 refs 持久化（`refs/gitbutler/*`）
③归属事后改。档位【有界】。第一步：归属表原型验证多会话自动分桶。
许可/成熟度：FSL-1.1（待核）｜维护中。链接：https://blog.gitbutler.com/building-virtual-branches

### 4. Graphite —— stacked diffs + merge queue + Diamond AI 评审
定位：stacked-PR 平台（gt CLI 递归 rebase；merge queue 栈序合并；Diamond 在人评审前先出
AI 评审）。可抄机制：①栈式小 PR + 增量 CI ②队列栈序串行 ③AI 评审前置到人之前。
档位【有界】。第一步：ADV 阶段化提交按栈试跑一轮。许可/成熟度：SaaS 闭源（CLI 待核）｜
商业维护中（2026-08 Cursor 收购报道待核）。链接：https://graphite.dev/

### 5. GitHub merge queue —— merge_group 假想合并
定位：平台级合并队列：把排队 PR 组成**假想合并分支**（merge_group）一起测，过测才进 main，
消灭「合并竞赛」。可抄机制：①组内批量假想测试 ②批内一挂全组重排。档位【reference】
（ADV 自建队列的对标基线）。第一步：ci-ops 队列设计以 merge_group 语义为对照。
许可/成熟度：平台闭源｜GA 维护中。链接：https://docs.github.com/（merge queue 文档）

### 6. Reviewable —— reviewed@revision 评审状态机
定位：代码评审 SaaS，评审状态精确到「文件 × revision × 人」。可抄机制：①文件 reviewed
状态绑定 revision，新 revision 推送即对旧评审人失效；②delta 视图只看「自上次评审以来」
的变化（delta stats）；③完成条件=全文件最新版已读 + 全线程 resolved（docs.reviewable.io 口径）。
档位【吸收】。第一步：ADV 评审门实现「revision 失效 + delta 重审」语义（这是合并队列
防「评审过期」的直接判据）。许可/成熟度：SaaS 闭源｜维护中。
链接：https://docs.reviewable.io/files ｜ https://docs.reviewable.io/reviews

### 7. Claude Code worktrees + checkpoints —— agent×git 默认模式
定位：官方背书的并行 agent 模式：一 agent 一 worktree 一分支；产品级 checkpoint 自动快照、
可 rewind（issue #6947，2025-09 锚；parallelcode.dev 2026 版指南；dev.to 2026-09「what broke」
复盘）。可抄机制：①worktree 隔离规约 ②步级快照作 undo log ③合并前 review gate。
档位【吸收】。第一步：ADV 仓的 CLAUDE/AGENTS 约定文件里固化 worktree-per-agent 命名与
快照判据。许可/成熟度：闭源产品｜维护中。链接：https://github.com/anthropics/claude-code（issue 6947）

### 8. Vibe Kanban / Conductor / Crystal / Claude Squad —— agent 编排层
定位：多编码 agent 的看板/编排器，底层统一是 worktree 隔离 + 状态面板（BAML 播客 2025-12；
lmsa.app 2026-05 对比口径）。可抄机制：①任务→worktree→agent 生命周期映射 ②多 agent
结果聚合评审。档位【有界】。第一步：ci-ops 里定义「agent 任务单」结构（对齐 worktree 生命周期）。
许可/成熟度：开源（Vibe Kanban 许可以仓库为准，待核）｜实验→早期维护。
链接：https://github.com/BloopAI/vibe-kanban

### 9. Sapling / EdenFS —— 可扩缩性与虚拟文件系统
定位：Meta 从 Mercurial 分叉的 Git 兼容 VCS；三件套=可扩缩服务端 + `sl` 客户端 + EdenFS
虚拟文件系统（**按需物化**文件，只落地真正访问的文件，jyn.dev 2025-12 引用口径）。
可抄机制：①按需物化思想（对应 ADV 大仓缓存/索引的懒加载）②原生 stack 提交工具。
档位【watch】（ADV 规模用不上 VFS，先记着）。第一步：无（规模触发器：仓库 >数万文件再评估）。
许可/成熟度：GPL-2.0｜维护中（Meta 内部驱动）。链接：https://sapling-scm.com/

### 10. Radicle —— P2P 托管
见重点回答⑤。定位：git 上层 P2P 协作网（Rust）。可抄机制：①仓库作为协作对象的 gossip
复制（作自托管分发的对照思路）。档位【不吸收】。第一步：无。
许可/成熟度：开源（待核）｜维护中。链接：https://radicle.xyz/

---

## 三、扫视（一句机制 + 档位 + 链接）

- **Mergify 依赖感知队列**：排队 PR 按依赖排序（Depends on 标注+侧栏），被依赖者先行——
  【reference】（ci-ops 队列排序对照）。https://docs.mergify.com/
- **Buildkite `merge_group` 支持**：CI 按 merge queue 事件建/撤构建（2025-10 changelog）——
  【reference】。https://buildkite.com/changelog
- **Copilot Workspace 解 PR 冲突**：冲突 PR 触发自动解决（2025-04 口径，效果未实测）——
  【watch】。https://github.blog/
- **SPOQ（arXiv 2026-06）**：多 agent「专家编排排队」论文，讨论 wave 分支 vs 共享队列——
  【watch】（wave-0 学术参照）。https://arxiv.org/
- **gh-stack / spr 类 stack CLI**：栈式 PR 的标记与逐层 submit，社区小工具——【reference】
  （许可以各自仓库为准）。https://github.com/timothyandrew/gh-stack
- **Cursor Origin 吸收 stacked-diff**：大变更拆小栈模型进 IDE（2026-08 报道，二手待核）——
  【watch】。https://happyrock.cloud/
- **Syncthing**：BEP 协议块级文件同步、版本化冲突副本，自托管分发通道的对照物——
  【reference】（对 ADV「本地优先分发」的对照，D10 交界）。https://syncthing.net/
- **CRDT 文件同步（Automerge / Diamond types）**：字符级 CRDT 使并行编辑可确定性合并，
  文件级实践仍不成熟——【watch】（D10 交界，本域不展开）。https://automerge.org/
- **tangled.org**：ATProto 上的 P2P forge，jj 官方仓库现宿主（2025 迁入）——【watch】
  （P2P forge 活体样本）。https://tangled.org/
- **GitButler CLI/agent 集成**：无分支切换的多 agent 并行提交管理 + Claude Code skill
  （2025-11 企业版集成口径）——【reference】。https://blog.gitbutler.com/
- **jj-workflow agent skill**：把 operation-log/conflicts/revsets 文档化喂给编码代理——
  【reference】（VCS 文档即 agent 接口的样本）。https://lobehub.com/

---

## 四、2025–2026 前沿信号

1. **agent 编排层爆发**：worktree-per-agent 从民间技巧升为官方文档模式（2025-09 起可见）；
   Vibe Kanban/Conductor/Crystal/Claude Squad 一年内成类（2025-12 BAML 口径）。
2. **资本押注 agent-aware VCS**：GitButler 17M 美元 A 轮（a16z，2025）——「多 agent 并行
   使单分支模型失效」成为投资叙事。
3. **stacked-diff 被 IDE 巨头吸收**：Cursor 收购 Graphite 的报道（2026-08，二手待核）——
   小栈评审模型从 SaaS 走进 IDE。
4. **队列生态化**：Buildkite 原生 merge_group（2025-10）、Mergify 依赖感知、Copilot 自动解
   冲突（2025-04）——合并队列从「测完就合」演进到「依赖排序 + 组测试 + 自动修复」。
5. **学术进场**：SPOQ（arXiv 2026-06）开始形式化多 agent 排队与 wave 分支——wave-0 05
   阶段化方向有学术同行者。
6. **P2P forge 的活体实验**：jj 迁宿 tangled.org（ATProto）；Radicle 1.0 后仍靠漏洞披露
   证明活跃——P2P 协作网在「能活」与「能养生态」之间仍偏前者。
7. **pijul 仍未 1.0**：1.90（2025-09-18）只是 pre-1.0——patch 理论正确的工程化成本可见一斑。

---

## 五、Top-3（对 ADV 最值得动）

1. **Claude Code worktree-per-agent + checkpoint（吸收）**：ADV 自身开发流程与 ci-ops 的
   并发基座，零新依赖、今天可执行——固化命名规约 + 步级快照判据。
2. **Reviewable 的 reviewed@revision 状态机（吸收）**：评审门「新 revision 即失效旧评审 +
   delta 重审」语义，是 ADV 合并队列防评审过期最直接可抄的判据。
3. **GitButler 的 hunk→分支归属表（有界）**：多会话产出自动分桶的持久化设计，给 ci-ops 做
   归属表原型，验证「同工作区多逻辑分支」在 ADV 的适配边界。

---

## 六、登记

21 条已写入 `RESEARCH/registry/E3.jsonl`（字段：id/domain/name/kind/verdict/mechanism/
integration/license/link/depth/date；python 追加，ensure_ascii=False）。
检索局限声明：本轮检索通道限流频繁，jj 与 Sapling 的 2025 版本细节主要来自摘要快照；
pijul/Radicle/部分许可字段标注「待核」，复核时以一手仓库与官网为准。
