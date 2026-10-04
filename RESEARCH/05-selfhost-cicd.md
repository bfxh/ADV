# 05 · 自托管 Git 平台 + CI/CD + 合并流程 + 发布工程 调研

> 调研日期：2026-10-03。方法：外部检索（WebSearch，2025–2026 资料优先），未逐条复测安装。
> 适用对象：**Windows 单人开发者 + 多 AI 会话并发**（一任务一树一分支），Rust 主体，仓库 bfxh/ADV 重写到 D:\KF\ADV。
> 口径约定：每条结论标注适用范围；查不到就写「待验证」；【吸收】= 直接用/搬进蓝图，【有界】= 只抄机制或限定场景用，【不吸收】= 给出理由后放弃。
> 所有外部主张的锚（版本号/日期）在文中括注；锚缺失处标「待验证」。

---

## 0. 一页结论（先读这里）

| 层 | 阶段 0（现在） | 阶段 1（双轨） | 阶段 2（终态） |
|---|---|---|---|
| Git 平台 | GitHub 私有仓（现状） | GitHub 主 + Forgejo 镜像 | **Forgejo 主**（单二进制 + SQLite 起步） |
| CI | GitHub Actions（云） | + Forgejo Actions / act_runner（本机 Windows） | Forgejo Actions 为主，性能门走本机 runner |
| Runner | — | 自托管 Windows runner（GitHub 仓） | act_runner / Woodpecker agent（Windows 服务化） |
| 合并纪律 | GitHub native merge queue + required checks | 同左，双轨校验 | 自制 bors 式串行队列（~1000 行 Rust）或 gitea-mq |
| 发布 | cargo-dist（GH 工作流） | release-plz/cargo-release + 脚本化 | tag → 构建（cargo-auditable）→ SBOM（cargo-cyclonedx）→ cosign 签名 → Forgejo generic registry |
| 证据 | GH OIDC keyless（SLSA L3 档） | 同左 | cosign 静态密钥（现实天花板 SLSA L1，诚实标注） |

切换的触发条件（不是日期）：① runner 在 Windows 上连续 ≥2 周门禁全绿；② 工作流只用「兼容子集」（见 §1.1）；③ Forgejo 备份恢复演练通过一次；④ GitHub 计费/配额实际痛到（2026-03 起有政策变化信号，见 §6）。

---

## 1. 自托管 Git 平台

### 1.1 Forgejo

- **一行定位**：Gitea 的社区分支（Codeberg 主导），自带 Actions（兼容 GitHub Actions 语法的 runner），2026 年处于双轨发布、节奏最快的成熟期。
- **2026 现状锚点**：v14.0 于 2026-01-15 发布（[forgejo.org/2026-01-release-v14-0](https://forgejo.org/2026-01-release-v14-0)）；发布计划显示 15.0 为 LTS（2026-04）、16.0（2026-07）、17.0（2026-09），即截至 2026-10 已到 16/17 线（[release schedule](https://forgejo.org/docs/v15.0/admin/release-schedule)）。Forgejo Runner 独立发版，2026-09-17 仍有一次发版（[releases 页](https://forgejo.org/releases)）。
- **值得抄的机制**：
  1. **Actions = 「熟悉而非兼容」的明确官方口径**（[docs](https://forgejo.org/docs/v15.0/user/actions/github-actions)）——语法层兼容，但 artifact API 与 GitHub 的 `actions/download-artifact` **不做完全兼容且无计划兼容**（2026-05 月报）。这直接决定迁移成本清单：工作流必须收敛到「兼容子集」（checkout/cache/upload-artifact 用 Forgejo 侧 fork，避免 GH 专属 API）。
  2. **LTS + 每年双发**的版本策略：单人自托管可以钉 LTS，升级压力可控。
  3. 内置 package registry（container/generic/cargo 等），release 工件有现成落点。
- **短板（2026 实测口径）**：没有 GitHub 式 environment 保护（讨论中，[codeberg/forgejo/discussions#440](https://codeberg.org/forgejo/discussions/issues/440)）；**native merge queue 仍是 feature request**（[forgejo#5102](https://codeberg.org/forgejo/forgejo/issues/5102)）；marketplace 生态远小于 GitHub。
- **档位**：【吸收】——单人 + Rust 主体下，「单二进制 + Actions 兼容语法 + 内置 registry」的组合迁移成本最低，且社区活跃度有 2026 年连发版本背书。
- **第一步动作**：在一台 WSL2/小 Linux 机上以 SQLite 跑 Forgejo，开 pull mirror 同步 GitHub 仓，先只验证 Actions 兼容子集跑通 build+clippy+test 三件套。
- **许可证**：MIT（沿 Gitea）。
- **链接**：[forgejo.org](https://forgejo.org) · [v14.0 公告](https://forgejo.org/2026-01-release-v14-0) · [迁移实测博客（2026）](https://danubedata.ro/blog/migrating-from-github-to-forgejo-2026)

### 1.2 Gitea

- **一行定位**：Forgejo 的上游本尊，商业公司（Gitea Ltd）背书，Actions 同源，版本号略保守但 2026 年也进入快车道。
- **2026 现状锚点**：1.26.0 于 2026-04-18 发布，含 Actions 强化与 3 个安全修复（[blog](https://blog.gitea.com/release-of-1.26.0)）；1.27.x 已跟进（1.27.3 含 Actions/API 安全修复）。官方 runner 在 2026-05-05 出了 1.0.0（[blog 第 2 页](https://blog.gitea.com/page/2)），并加了内置 checkout、修复 composite action 里的 actions/cache（[runner releases](https://gitea.com/gitea/runner/releases)）。
- **值得抄的机制**：
  1. **PR 自动合并（auto-merge when checks pass）**——单人场景的「穷人版合并队列」，单一 PR 时等价于队列；两个 PR 并发时的竞态它不管（这正是 bors 存在的理由，见 §4）。该能力自 Gitea 1.21 引入（待验证具体版本号）。
  2. runner 管理面（admin API 更新全局 runner，1.26 起）——多 AI 会话并发时 runner 池的可观测性有现成 API。
- **与 Forgejo 怎么选（单人口径）**：治理差异对单人用户几乎不可感知；Gitea 对 Windows 官方下载更全（Gitea 官方支持 Windows 二进制，Forgejo 需要自编译/第三方构建——待验证 Forgejo 官方 Windows 产物现状，这**是**选型时要在阶段 1 实测的第一件事）。
- **档位**：【有界】——作为 Forgejo 的备胎保持关注；若 Forgejo 无 Windows 服务器二进制且不愿跑 WSL2，Gitea 上位。
- **第一步动作**：确认 Forgejo / Gitea 各自的 Windows 服务端产物与安装文档现状（一次网页核对即可）。
- **许可证**：MIT。
- **链接**：[gitea.com](https://gitea.com) · [2026-04 发布报道](https://linuxiac.com/gitea-1-26-released-with-security-fixes-and-actions-upgrades) · [两仓活跃度对比（2025-05）](https://honeypot.net/2025/05/14/gitea-vs-forgejo-development-activity.html)

### 1.3 Radicle

- **一行定位**：Rust 写的 P2P 代码协作栈（git + DHT 种子节点，无中央服务器），主权叙事很强，工程成熟度仍在爬坡。
- **2025–2026 锚点**：2025-06-13 发了 Radicle Desktop（[radicle.dev](https://radicle.dev/2025/06/13/radicle-desktop.html)）；项目活跃，但检索未见到大规模采用指标与内置 CI 的正式化路线（Radicle CI 历史上是 node 侧实验特性，待验证）。
- **值得抄的机制**：
  1. **「仓库即等价副本」**——本地 git 仓直接 push 到 p2p 网络即备份，不需要平台方。
  2. issue/PR 状态用本地优先数据结构存，collaboration 数据也在对等网内。
- **对单人开发者的泼冷水**：你的痛点是「多 AI 会话并发的门禁与合并纪律」，Radicle **解决不了其中任何一条**（无内置 CI、无 merge queue、review 生态小）；它解决的是「不信任任何托管方」——而 GitHub + Forgejo 镜像 + git bundle 离线备份已经用低得多的运维成本覆盖了备份与主权诉求。P2P 网络还需要种子节点常开（Windows 单机上多一个常驻服务）。
- **档位**：【不吸收】（作为每日工作流）——问题不匹配；若未来想要「平台无关的备份通道」，把 git push 到 radicle 种子当作阶段 2 之后的可选实验。
- **第一步动作**：不设。归档本节结论即可。
- **许可证**：MIT / Apache-2.0。
- **链接**：[radicle.dev](https://radicle.dev) · [LWN 深度文](https://lwn.net/Articles/966869)

---

## 2. CI 引擎与 Runner

### 2.1 Forgejo Actions / act_runner（与 §1.1 同栈）

- **一行定位**：Forgejo 内置的 Actions 引擎 + Go 写的 runner，语法上是 GitHub Actions 的「熟悉子集」。
- **值得抄的机制**：workflow 语法兼容让「阶段 0→1 迁移」主要是**收敛动作**（删掉 GH 专属 API 用法）而不是重写。
- **档位**：【吸收】。
- **第一步动作**：写一份 `ADV` 仓库的「Actions 兼容子集」清单（用的每个 action 逐个标注 Forgejo 侧等价物）。
- **许可证**：runner MIT（待验证，沿 Gitea runner）。链接：[docs](https://forgejo.org/docs/v15.0/user/actions/github-actions)

### 2.2 Woodpecker CI

- **一行定位**：Drone 0.8 的社区分叉（Apache-2.0），server/agent 分离，YAML 简洁，对 Windows agent 支持是开源 CI 里最实的之一。
- **2025–2026 锚点**：3.0.0（重命名大版本）→ 3.15（`depends_on` 可选化、cron 时区、pipeline path 配置）（[3.15 报道](https://linuxiac.com/woodpecker-ci-3-15-released-with-smarter-pipeline-dependencies)）；官方支持 Windows：agent 走 WSL2+Docker Desktop 或原生 Windows 容器（[supported platforms](https://woodpecker-ci.org/docs/next/administration/installation/supported-platforms)）；Windows agent 实操指南存在（[Medium](https://medium.com/@vgamebox/how-to-install-woodpecker-agent-on-windows-3497b3aecd12)）。
- **值得抄的机制**：
  1. **pipeline 依赖可选化**（`depends_on` 可关）——单人仓库 job 少，默认并行反而省心。
  2. **agent/server 解耦**——server 可放任何小盒子，Windows 只跑 agent；与 Forgejo 并存做「重量级性能门」专用 CI 不冲突。
  3. pipeline replay（3.0 起 CLI 从 server 重放）——性能门复现实验直接可用。
- **档位**：【有界】——若 Forgejo Actions 在 Windows 上跑出问题（容器依赖是最大嫌疑），Woodpecker 是第一顺位替补；不主动上两套 CI。
- **第一步动作**：仅在 Forgejo Actions 验证失败时启动：Windows agent 安装 + 一个 Rust 构建 pipeline 走通。
- **许可证**：Apache-2.0。链接：[woodpecker-ci.org](https://woodpecker-ci.org)

### 2.3 Concourse

- **一行定位**：声明式 pipeline 老牌引擎（资源/校验和模型很优雅），但运维模型重（Postgres + worker，历史上 BOSH 部署）。
- **2026 现状**：本次检索未命中 2025–2026 活跃度证据——**待验证**；社区多年走低是普遍印象（不作为结论）。
- **值得抄的机制**：`resource` 抽象（版本化输入输出，天然适合「基线棘轮」这类有状态门禁的思想）。
- **档位**：【不吸收】——单人 Windows 下运维面（Postgres+worker 常驻）远超收益；棘轮思想直接在自己脚本里实现。
- **许可证**：Apache-2.0。链接：[concourse-ci.org](https://concourse-ci.org)

### 2.4 Dagger

- **一行定位**：把 pipeline 写成「容器编排代码」（Dagger Functions），engine 可跑在本地或任何 CI 上；2026 年的叙事恰好打中你——「AI agent 产出速度超过 CI 吞吐」。
- **2026 锚点**：v0.21.9（2026-08）/v0.21.10（2026-09）活跃发版；Rust SDK 属 **Community 档**（crates.io `dagger-sdk`，全史 ~116k 下载，仍在更新，非官方档）；官方档 SDK 是 Go/Python/TS 等（[dagger.io](https://dagger.io)）。2026 主题文：[The Great CI Bottleneck of 2026](https://dagger.io/blog/the-great-ci-bottleneck-of-2026)。
- **值得抄的机制**：
  1. **「pipeline 即代码、可本地复跑」**——性能门/god 门这类需要本地精确复现的门禁，用 Dagger 的模型（同样的函数本地与 CI 各跑一遍）比 YAML 更贴你的纪律。
  2. engine 抽象：CI 引擎可替换，pipeline 不变。
- **档位**：【有界】——Rust SDK 非官方档 + 引入容器引擎常驻（Windows 上要 WSL2）对单人偏重；但其思想值得在自家门禁脚本里实现（本地与 CI 共用同一 Rust 二进制跑门禁）。
- **第一步动作**：把现有性能门封装成 `cargo run --bin gate` 的单一入口，本地与 CI 都调它——这是 Dagger 模型的零依赖版。
- **许可证**：核心 Apache-2.0（CLI 部分许可口径待验证）。链接：[dagger.io](https://dagger.io)

### 2.5 Earthly（已死，重要反面信号）

- **一行定位**：Earthfile 式构建工具 + CI——**2025-04 公司宣布收缩，2025-07-16 停止开源维护、Earthly Cloud（含 Satellites）关停**（[Reddit r/devops 公告转述](https://www.reddit.com/r/devops/comments/1k0p57f/earthly_shutting_down_earthfiles)；最后 release 注明 Cloud shutdown）。
- **值得抄的机制**：Earthfile 的「recipe + target 依赖」思想（与 Just/Make 同族）。
- **档位**：【不吸收】（选型层面）——新项目引入即接盘弃儿；作为** survivorship 证据**保留：2023 年它还是各种推荐榜常客，两年就凉。
- **许可证**：Apache-2.0（停止维护）。链接：[earthly.dev](https://earthly.dev)

### 2.6 GitHub self-hosted runner（Windows）——过渡/混合方案

- **一行定位**：在 GitHub 现有工作流里挂自己的 Windows 机器当 runner，是阶段 0→1 的最低成本桥。
- **2025–2026 硬锚点**：
  - **最低版本强制**：2026-03-16 起 GitHub 将阻断 < v2.329.0（2025-10-15 发布）的自托管 runner，之后 2026-07/09 分批扩大（[community discussion #186520](https://github.com/orgs/community/discussions/186520)、[devops.com](https://devops.com/github-actions-gets-serious-about-self-hosted-runner-versions)）→ runner 自动升级策略要在阶段 1 就定好。
  - **2025 年供应链事故**：tj-actions/changed-files 事件（CVE-2025-30066）证明第三方 action 是最大攻击面（[Safeguard 综述](https://safeguard.sh/resources/blog/securing-self-hosted-github-actions-runners)）→ **所有第三方 action 按 SHA 钉死**。
  - 攻击模式：runner 被武器化为持久后门、走可信通道难检测（[Sysdig](https://www.sysdig.com/blog/how-threat-actors-are-using-self-hosted-github-actions-runners-as-backdoors)）。
- **Windows 坑清单（实操口径，标注置信度）**：
  - `shell: bash` 依赖 runner 自带的 git-bash；PowerShell 桌面版 vs pwsh 的默认差异（经验项，待验证最新默认值）。
  - Windows runner 上没有 Linux 容器：硬编码 `docker run` 的 action 大量失效（除非 WSL2）（经验项）。
  - **持久工作区状态**：self-hosted runner 两次 job 间状态保留（[actsense](https://actsense.dev/vulnerabilities/self_hosted_runner)）→ 对你的棘轮门禁这既是特性（缓存可加速）也是风险（污染），门禁脚本必须自己声明输入输出而不是信工作区。
  - CRLF / 长路径 / 杀软实时扫描拖慢 node 启动（经验项）。
- **安全纪律（检索背书）**：私有仓才可用 self-hosted runner（公开仓的 fork PR 会拿到你机器的执行权，[Wiz](https://www.wiz.io/blog/github-actions-security-guide)）；GitHub 官方也推荐 ephemeral 化缓解持久化（[community #205363](https://github.com/orgs/community/discussions/205363)）。
- **档位**：【吸收】（仅限阶段 0→1 过渡，私有仓 + SHA 钉死 + 定期清理工作区）。
- **第一步动作**：现在就可以在现有仓挂一台自托管 Windows runner，把最慢的 1–2 个性能门 job 打上 `runs-on: self-hosted`，其余留在云上。
- **许可证**：runner 客户端 MIT（[actions/runner](https://github.com/actions/runner)）。

---

## 3. 合并队列

**为什么单人 + 多 AI 会话反而需要它**：GitHub merge queue 的原始动机是「PR 各自绿 ≠ 合并后还绿」；多会话并发时两个 AI 会话几乎同时合 PR 的竞态是真实高频事件。Edd Barrett 从 bors-ng 迁去 GitHub 原生队列的经验文对机制取舍有完整对比（[theunixzoo, 2023-11](https://theunixzoo.co.uk/blog/2023-11-16-migrating-to-gh-merge-queues.html)）。

### 3.1 GitHub native merge queue

- **一行定位**：branch protection + required status checks 之上，把多个 PR 组成推测性批次（group）在临时分支上测试，绿了才合入 main。
- **值得抄的机制**：① 分组/合并测试；② 失败后的「跳过嫌疑 PR 重跑」（jump/bisect 逻辑）；③ 队列并发上限——单并发串行就是最小可行版。
- **档位**：【吸收】（阶段 0）；机制整体抄进自制队列（阶段 2）。
- **第一步动作**：给 main 设 branch protection + required checks + merge queue（私有仓此功能可用性待验证——GitHub 部分队列特性绑公共仓/付费档）。
- **许可证**：专有服务。链接：[docs](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)

### 3.2 GitLab merge trains

- **一行定位**：同思想的开山鼻祖，但 **merge trains 是 Premium 付费特性**，自托管 CE 拿不到。
- **值得抄的机制**：train 可视化（队列里每个位置及其推测性状态可查）——自制队列值得做个 `/mq status` 输出。
- **档位**：【不吸收】（付费墙）；机制已并入 3.1。
- **第一步动作**：无。**许可证**：专有。链接：[docs](https://docs.gitlab.com/ee/ci/pipelines/merge_trains.html)

### 3.3 rust-lang/bors（Rust 重写版）

- **一行定位**：Rust 官方把 Homu 全量重写为 Rust 的新 bors，2025 年经 GSoC 补齐 merge 功能并在 rust-lang/rust 上线（[repo](https://github.com/rust-lang/bors)、[Kobzol 2025 总结，2026-01-05](https://kobzol.github.io/rust/rustc/2026/01/05/my-rust-contributions-in-2025.html)、[GSoC 页](https://open-in-three.vercel.app/projects/gsoc-2025-implement-merge-functionality-in-bors)）。
- **值得抄的机制**：① `try` 分支（不打扰队列地试验合并结果）；② 队列状态机（approve → queued → testing → success/fail → merge/drop）直接可抄成 Rust 枚举；③ bot 无状态化，状态全在 forge 侧（labels + status）。
- **档位**：【有界】——它绑 GitHub API，不能直接用；但**是自制队列的最佳参考实现**（同语言、开源、生产级）。
- **第一步动作**：clone 读它的 state machine 与 webhook 处理两个模块，产出自制版设计稿。
- **许可证**：Apache-2.0 / MIT（Rust 官方惯例，待验证仓库 LICENSE 标头）。链接：[bors.rust-lang.org](https://bors.rust-lang.org/help)、[Rust Forge 文档](https://forge.rust-lang.org/infra/docs/bors.html)

### 3.4 bors-ng

- **一行定位**：经典 bors 思想的 Elixir 实现，GitHub 系，2025–2026 维护节奏放缓（待验证最新 commit）。
- **档位**：【不吸收】——GitHub 专用 + Elixir 栈单人难维护；机制参考 rust-lang/bors 更现代。
- **许可证**：Apache-2.0。链接：[bors-ng](https://github.com/bors-ng/bors-ng)、[bors.tech](https://bors.tech)

### 3.5 gitea-mq（Mic92）——最接近目标的现成品

- **一行定位**：**支持 Gitea/GitHub/Forgejo 的合并队列，串行化 PR 合并保 main 常绿，且没有 bot 命令**（label/规则驱动）（[repo](https://github.com/Mic92/gitea-mq)）。
- **值得抄的机制**：① 「无命令」设计——用 PR 属性（label/milestone）代替 `bors r+` 评论，多 AI 会话场景下少一个「会话忘了喊命令」的故障模式；② 串行即最小正确性。
- **档位**：【吸收】（阶段 2 首选现成品，自制前先试它）。
- **第一步动作**：读源码 + 跑一次本地 Forgejo 实例集成测试。
- **许可证**：待验证（Mic92 的仓库通常 MIT，需查 LICENSE）。

### 3.6 Mergify

- **一行定位**：SaaS 合并自动化（规则引擎 + 队列 + 优先级），专有。
- **值得抄的机制**：queue rule 作为**配置而非代码**——自制队列把队列参数（并发数、required checks 列表）外置成仓库内 YAML。
- **档位**：【不吸收】（专有服务，机制并入自制版）。**第一步动作**：无。**许可证**：专有。链接：[mergify.com](https://mergify.com)

### 3.7 「homer」

- **待验证**：本次检索未找到名为 homer 的 Forgejo/Gitea 合并队列工具（可能记忆有误或过小众）。不出现在结论里。

---

## 4. 发布工程

### 4.1 cargo-dist（现名 dist 生态）

- **一行定位**：Rust 二进制发布全自动化（版本号→artifact 矩阵→安装器→release 页），2025 年经历公司关停后由原作者恢复维护。
- **时间线锚点**：0.28.x（2025-07）后一度沉寂 → Astral fork 并占走 `dist` 名 → **0.29.0/0.30.0（2025 下半年）由 mistydemeo 复活，0.29.0 移除了对 Axo Releases（公司托管服务）的支持并合并 Astral fork 的特性**（[CHANGELOG](https://github.com/axodotdev/cargo-dist/blob/main/CHANGELOG.md)、[PSA: cargo-dist is not dead, r/rust](https://www.reddit.com/r/rust/comments/1noozk7/psa_cargodist_is_not_dead)）。
- **值得抄的机制**：① artifact 矩阵声明式生成（平台 × 格式 × 安装器）；② checksum 文件与 release 的绑定方式；③ `--announce` 版本公告聚合。
- **对自托管的现实**：它的 CI 集成以 GitHub Actions 工作流为主体——切到 Forgejo 后这部分价值归零，剩下的价值在**本地能跑的部分**（manifest 生成、安装脚本生成）。
- **档位**：【有界】——阶段 0 继续用（GH 上它是最佳体验）；阶段 2 保留思想、换成自己的发布脚本（§8），并且每次升级前查一次活跃度（它刚证明过一次「公司死掉」）。
- **第一步动作**：在 GH 现状下 pin 到 0.30.x 并记录 escape hatch（纯 `cargo build --release` + 手写安装脚本的降级路径）。
- **许可证**：Apache-2.0 / MIT。链接：[docs](https://axodotdev.github.io/cargo-dist)

### 4.2 release-please

- **一行定位**：Google 的 Conventional Commits 驱动发版机器人，自动 changelog + release PR。
- **值得抄的机制**：release PR 模式——发版动作本身是个 PR，过同一套门禁（好设计，直接抄）。
- **档位**：【有界】——绑 GitHub；单人 Rust 仓用 cargo-release/release-plz 更顺。
- **许可证**：Apache-2.0。链接：[github.com/googleapis/release-please](https://github.com/googleapis/release-please)

### 4.3 changesets（及 Rust 端 cargo-changeset / Sampo）

- **一行定位**：「人类意图文件」流——改动时手写 changeset 文件，发版时汇总 bump + changelog；Rust 端有 cargo-changeset（crates.io）与 2025 年新出的 Sampo（[Sampo 介绍](https://goulven-clech.dev/2025/introducing-sampo)、[r/rust 讨论](https://www.reddit.com/r/rust/comments/1osesrv/introducing_sampo_automate_changelogs_versioning)）。
- **值得抄的机制**：**显式发版意图** vs 「从 commit 猜版本」——对多 AI 会话场景尤其对：AI 生成的 commit message 不可信为版本依据，显式文件是抗噪信号。
- **档位**：【吸收】（抄机制）——单人仓起步可以先用 release-plz/git-cliff 的自动 changelog，等仓变成多 crate workspace 再上 changeset 文件。
- **许可证**：MIT（changesets）；cargo-changeset/Sampo 各自待验证。
- **链接**：[changesets](https://github.com/changesets/changesets)

### 4.4 cargo-release / release-plz / git-cliff

- **一行定位**：cargo-release = crate-ci 出的极简 bump+publish；release-plz = Rust 原生的发版 PR 机器人（git-cliff 生成 changelog，社区对比中的热门中庸项，[release-plz](https://github.com/release-plz/release-plz)、[对比讨论](https://github.com/MarcoIeni/release-plz/discussions/1019)）；git-cliff = Conventional Commits changelog 生成器。
- **值得抄的机制**：release-plz 的「发版 PR + 独立 publish」两段式，与 release-please 同思想；git-cliff 可脱离任何平台本地跑（自托管友好）。
- **档位**：【吸收】（release-plz 或 cargo-release 二选一 + git-cliff；release-plz 对 Gitea/Forgejo 的支持现状待验证，一次网页核对）。
- **第一步动作**：`cargo install cargo-release git-cliff`，先手动跑通一次 dry-run。
- **许可证**：均 MIT/Apache-2.0 系（待验证逐个）。链接：[release-plz.dev](https://release-plz.dev)

---

## 5. 签名与 Provenance

### 5.1 Sigstore / cosign

- **一行定位**：工件签名与透明日志（Fulcio 签证书 + Rekor 日志）；cosign 是 CLI 事实标准。
- **关键现实（自托管口径）**：**keyless 模式需要 OIDC issuer 被公共 Fulcio 信任**——自托管 Forgejo 签发的 token 公共 Sigstore 不认；要走 keyless 就得自建 Fulcio+Rekor（或私有 Sigstack），对单人过重。社区实操路径：① GH Actions 里用 OIDC keyless（云阶段最优）；② 自托管阶段用 **cosign 静态密钥**（`cosign sign-blob`）或退到 minisign（更简单、无基础设施）（[Sigstore 官方 overview](https://docs.sigstore.dev/cosign/signing/overview)、[自托管 GitLab CI keyless 实操文](https://hogin.pro/posts/sigstore-cosign-keyless-ci)、[2026 综合指南](https://kokil.com.np/blog/sign-and-verify-artifacts-with-sigstore-cosign)）。
- **值得抄的机制**：① 签名与 attestation（`cosign attest` 产 in-toto JSON）统一 CLI；② 透明日志防抵赖的思想（单人场景可降级为「发布清单入 git 历史」）。
- **档位**：【吸收】（工具）+【有界】（keyless/私有 Sigstore 实例：不吸收）。
- **第一步动作**：本地生成 cosign 密钥对，对一次 release 的 checksum 清单跑 sign-blob + verify-blob 闭环。
- **许可证**：Apache-2.0。链接：[sigstore.dev](https://www.sigstore.dev)

### 5.2 gitsign

- **一行定位**：Sigstore keyless 的 git commit 签名——同样卡在「需要受信 OIDC」，自托管单人下不如 **git 自带 ssh-key 签名（`gpg.format ssh` + allowed_signers）** 务实（后者零基础设施，经验项）。
- **档位**：【不吸收】（自托管阶段）；git ssh 签名作为替代吸收进蓝图。
- **第一步动作**：配置 git ssh commit 签名并在 Forgejo 开验证（Forgejo 支持 ssh 签名验证，待验证版本）。
- **许可证**：Apache-2.0。链接：[sigstore/gitsign](https://github.com/sigstore/gitsign)

### 5.3 in-toto / SLSA（等级在自托管下的可达性）

- **一行定位**：SLSA 是供应链完整性分级（L1 有出处脚本可复现 → L2 托管构建平台 → L3 加固隔离）。
- **自托管单人的现实天花板（诚实标注）**：**L1 可达且值得**（发布脚本产 in-toto attestation，写明 builder=本机 runner、材料哈希）；**L2/L3 在自托管 Windows 单机上不现实**（需要抗篡改的托管构建/隔离平台）——SLSA 官方历来把 self-hosted runner 视为无法直接达 L3（此句为文档级共识，本次未逐条核原文，**待验证**）。补强方向是密钥管理（TPM/离线 age 密钥）而非虚标等级。
- **档位**：【吸收】（L1 + attestation 格式）；L2/L3 明确不做并写进蓝图（防止未来某个 AI 会话许诺做不到的等级）。
- **链接**：[slsa.dev](https://slsa.dev) · [in-toto](https://in-toto.io)

### 5.4 GUAC

- **一行定位**：供应链关系图谱（把 SBOM/SARIF/advisory 归并成图查询），Kusari/Google 系。
- **值得抄的机制**：`artifact → builder → source` 三元组模型——自制发布闭环里用一张极简表格记录这三个字段即可获得八成价值。
- **档位**：【不吸收】（全套服务对单人明显过重）；三元组模型吸收。
- **第一步动作**：无（模型已并入 §8）。**许可证**：Apache-2.0。链接：[guac.sh](https://guac.sh)

---

## 6. 2025–2026 前沿信号（时间线）

1. **Earthly 全线关停（2025-04 宣布，2025-07-16 停维护）**——build-tool 两年内从热门到弃养，自建栈选型必须附「退出路径」。（r/devops）
2. **cargo-dist 惊魂（2025）**——公司停摆→Astral fork 占名→原作者复活 0.29/0.30。教训：Rust 工具链公司化项目同样会断档，任何发布工具都要有纯脚本降级路径。（CHANGELOG + r/rust PSA）
3. **GitHub 自托管 runner 进入强制治理（2025-10 → 2026-09）**——最低版本 v2.329.0 分批强制；同时 2026-03 起 GitHub 计费政策变化被多家归因为自托管迁移动力上升（wolkig.it，具体口径**待验证**）。对你：混合方案的 runner 版本管理不是可选项。
4. **Forgejo 进入双轨快发期（2026-01 v14 → 2026 年内 15 LTS/16/17）** + Gitea 1.26/1.27——自托管 forge 的「成熟窗口」2026 年是真实打开的。
5. **rust-lang/bors 生产化（2025 GSoC 补齐 merge，2026-01 Kobzol 年报确认已在 rust-lang/rust 运行）**——bors 思想有了生产级 Rust 参考实现，自制队列的抄写成本显著下降。
6. **Dagger 的「CI 瓶颈」叙事（2026）**——AI agent 产出速度 > CI 吞吐，与你多 AI 会话工作流完全同构；这给「合并队列 + 快速本机门禁」提供了战略优先级依据，而不只是洁癖。
7. **Forgejo 官方明确「熟悉而非兼容」+ artifact API 不做完全兼容（2026-05 月报）**——迁移成本清单从「谣言」变成「官方文档」，阶段 1 的兼容子集可以据此精确圈定。

---

## 7. 推荐栈与分阶段路线（问题①）

**推荐终态**：Forgejo（主仓，SQLite 起步）+ act_runner（Windows 服务，工作区每次 job 清理）+ 自制/gitea-mq 合并队列 + release-plz/cargo-release + git-cliff + cargo-auditable + cargo-cyclonedx + cosign 静态密钥 + Forgejo generic registry。Gitea 为备胎，Woodpecker 为 CI 备胎，Earthly/Radicle/Concourse/GUAC/Mergify 明确不进。

**阶段 0（现在，零新增运维）**：
- 现有 GitHub 仓：main 上 branch protection + required checks + native merge queue（可用性待验证）。
- 挂一台自托管 Windows runner 跑最慢的性能门（私有仓前提 + 第三方 action SHA 钉死 + runner 自动升级开启）。
- cargo-dist pin 0.30.x + 记录降级路径。

**阶段 1（双轨，触发条件：性能门在自托管 runner 上稳定 2 周）**：
- WSL2/小盒子跑 Forgejo + pull mirror；工作流收敛到兼容子集（逐 action 对照 §1.1 清单）。
- 产出 `RESEARCH` 侧清单：哪些 job 留 GH（需要 keyless 签名/OIDC 的），哪些搬 Forgejo。
- 备份：`forgejo dump` + git bundle 异地（这一步通过恢复演练才算数）。

**阶段 2（切换，触发条件：兼容子集全绿 + 恢复演练通过）**：
- Forgejo 转主仓；GH 降为只读镜像；队列上 gitea-mq（先试现成）→ 不满足再自制（§8）。
- 发布闭环切 §8 方案；SLSA 停在 L1 并在发布清单里诚实标注。

**迁移成本量级（单人口径）**：阶段 1 ≈ 2–3 个工作日（大头是 action 逐个对照与 Windows runner 服务化）；阶段 2 ≈ 2–3 天（备份恢复 + 队列 + 发布脚本），前提是兼容子集清单已经做完。

---

## 8. 合并队列的最小可行实现（问题②）

**bors 思想核心**：测试「合并后的 main」而不是「PR 分支本身」。最小件清单（对应 rust-lang/bors 的状态机，全部有 Forgejo API 支撑）：

1. **入口**：webhook 监听 PR 事件；入选条件 = required checks 绿 + 满足规则（label / 无 bot 命令，学 gitea-mq）。
2. **推测分支**：服务端把队头 PR merge 到 `staging`（或 `main` 的临时快进分支）并 push——Forgejo API 有 commit status + branch API。
3. **等待**：订阅 staging 头部 commit 的 required status（你的门禁 job 上报）。
4. **落盘**：全绿 → fast-forward main + 删临时分支；任一失败 → 弹出该 PR + 自动评论失败原因，队头回到下一候选。
5. **（可选）批量**：一次测 2–3 个 PR 的组合，失败时二分弹出——**单人并发 2–3 会话规模下，串行版已覆盖 90% 痛点，批量是锦上添花，不做不减分。**

**工作量量级**：串行版 ≈ 600–1200 行 Rust（webhook + reqwest + shell git 调用），1–2 个工作日含测试；批量版再 +2–3 天。**先试 gitea-mq，自制是 fallback**——这也是为什么它被标【吸收】。
**最低成本替代（若一个 PR 合并时其余会话能等待）**：Forgejo/Gitea 的 auto-merge（checks 绿自动合，§1.2）+ 人工保证「同一时刻只有一个待合 PR」——即用 AGENTS.md 的「一任务一树一分支」纪律把并发压到 1，机器人都不用写。队列的价值只在纪律失效时兑现。

---

## 9. release + 签名 + SBOM 的最小闭环（问题③）

```
tag (git tag vX.Y.Z, ssh 签名)
 → release-plz / cargo-release bump（或 release PR，抄 release-please 模式）
 → cargo build --release + cargo auditable（把依赖清单嵌进二进制）
 → cargo cyclonedx 产 CycloneDX SBOM
 → sha256sum 清单
 → cosign sign-blob（静态密钥；密钥存 OS 凭据保护/离线 age 备份）
 → cosign attest：in-toto attestation（builder=<runner id>, source=<commit>, artifact=<sha256>）
 → Forgejo generic registry 上传 + git-cliff 生成 release notes + 清单提交回 git（防抵赖的穷人版透明日志）
 → verify 脚本：verify-blob + SBOM 与上一版 diff（依赖棘轮，你已有同族纪律）
```

- **SLSA 定位**：闭环达标 L1，attestation 里如实写 `builder: self-hosted-windows-runner`；L2/L3 不追求（§5.3）。
- **多 AI 会话适配**：闭环每一步都是独立脚本 + 可本地复跑（§2.4 的门禁入口思想），任何一个会话都能验证而不是只能信任上次运行的输出。

---

## 10. Top-3

1. **Earthly 死了（2025-07）、cargo-dist 断档后复活（2025 下半年）**——自托管栈选型的第一原则从「哪个最好」变成「哪个死了你活得下来」：每个组件必须带纯脚本降级路径（Earthly 教训）和 pin + 退出计划（cargo-dist 教训）。
2. **合并队列对「单人 + 多 AI 会话」是真实需求而非装饰，且最小实现出乎意料地便宜**——先试 gitea-mq（支持 Forgejo、无 bot 命令设计），自制串行版 1–2 天，rust-lang/bors 提供生产级参考；最低成本兜底是 auto-merge + 把并发压到 1 的纪律。
3. **阶段化迁移而非一步到位**：GitHub 自托管 runner（阶段 0，注意 2026-03 起的版本强制）→ Forgejo 镜像跑兼容子集（阶段 1，官方明确 artifact API 不完全兼容）→ Forgejo 主仓 + 自制队列 + cargo-auditable/cyclonedx/cosign 静态密钥的 L1 发布闭环（阶段 2）——每步切换由触发条件（稳定性/恢复演练）而非日期决定。
