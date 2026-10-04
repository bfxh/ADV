# D9 · 本地 CI 与 runner 工程（机制级深挖）

> 调研日期：2026-10-03 · 方法：Web 检索 2025–2026 资料为主（官方文档/issue/release 页优先），机制描述以可引用实现为准；平台重点 Windows。
> 与既有分工：`05-selfhost-cicd.md` 已定自托管阶段化路线（GH runner 过渡→Forgejo 主仓+合并队列）；C5 已覆盖文件 notify，本文只看"CI 触发角"；R3/wave-0 08 与本文在 Windows Job Object 处交界（本文从 CI runner 视角切）。
> 本文档 27 条条目，其中 16 条机制级（deep）；registry 见 `registry/D9.jsonl`。档位体系：吸收 | 有界 | 不吸收 | watch | reference。
> 口径提醒：所有"维护状态/版本/价格"都标注了锚（仓库/文档/日期），引用前按锚复核；未锚数字一律不写。

## 0. 五个重点问题的浓缩答案（详证见专题条目）

| # | 问题 | 一句话结论 |
|---|------|-----------|
| ① | act_runner（Forgejo Runner）Windows 2026 可用性 | 可用且活跃维护（Codeberg e.V.，独立发布线 2024 v3→v5、2025 v6+，锚：code.forgejo.org/forgejo/runner/releases），但**隔离模型在 Windows 缺口**：`docker://` 标签走 Docker Desktop/WSL2 只能跑 Linux 容器（`runs-on: windows-*` 无容器等价物），Windows 门实际只有 `host://` 宿主机直跑——无任何隔离，官方文档自己警告 host 模式"对宿主机与网络构成显著安全威胁"（锚：forgejo.org Actions 管理员指南）。**能跑 Rust 门**：host:// 直跑（rustup/cargo 装宿主机）或 docker:// Linux 容器（WSL2 9P 跨盘 IO 慢，大仓构建付出明显代价）。结论：runner 只当"调度/触发壳"，隔离由 ADV 自己的 Job Object + 一次性 worktree 补（D9-002/D9-009） |
| ② | 两层制（pre-commit 快档 + CI 全档）业界标准 | 标准做法=**同一份钩子/门配置文件，两个执行面**：本地快档跑改动子集（pre-commit 只 hook 改动文件），CI 全档跑 `pre-commit run --all-files`（pre-commit.com 官方文档口径）；pre-commit.ci 服务就是把"同一配置在 PR 上全量复跑"产品化。2025 信号：框架加 `-j` 并行；Rust 重写的 drop-in 替代 **prek** 崛起（prek.j178.dev）。ADV 落法：`xtask gate --tier local\|ci`——门注册表唯一，tier 只选子集不改判据；本地跳过的门必须显式列出（绿=SKIP 不算绿 纪律的 CI 版）（D9-019） |
| ③ | 预热池 + ephemeral 工程三角 | 三角=冷启动延迟（镜像拉取+clone+工具链安装）vs 常驻成本（预热实例的内存/陈旧镜像）vs 污染风险（跨 job 状态泄漏/凭据残留/缓存投毒）。业界解=**预热池(minRunners) × 单任务生命周期(--ephemeral) × 缓存外置(sccache 远端)** 组合：ARC 的 runner scale set 用 `minRunners` 保底预热、每 runner 跑一个 job 即销毁（docs.github.com ARC 部署文档；runner 2.299+ 支持 `--ephemeral`）。ADV 单机版：预热 N 个带工具链的 worktree 池，任务后重置工作区，工具链不重装、状态不带走（D9-013/D9-014/D9-015） |
| ④ | 本地 CI 与远程 CI 一致性 | 五件套：单一判据源（同一门注册表被 local/ci 两个 profile 消费）+ 版本钉死（rust-toolchain.toml/Cargo.lock/action SHA/工具版本参数化）+ 统一输出契约（门结果 JSON schema 两端一致，exit code 语义一致）+ 同镜像复跑（act/Forgejo runner docker:// 用与远程相同的容器镜像，缩 OS 漂移）+ 反 SKIP 纪律（运行记录必须含真实 artifact 路径与时间戳，防"绿=没跑"）。容器化同镜像是唯一能压平 libc/路径/大小写漂移的手段，Windows 门在本地只能诚实降级为"宿主机档"并标注档位差（D9-001/D9-002/D9-019） |
| ⑤ | Windows 服务化 runner 管理 | 谱系 NSSM（最后构建 2017，弃维护）→ WinSW（MIT，最后正式发布 v2.12.0 2023-02，维护"复杂"但仍被 rclone/Jenkins 文档推荐）→ 新一代 Shawl（Rust，同源技术栈）与 Servy（2025 活跃）。方案：WinSW XML（`<onfailure>` 重启 + `<log mode="roll-by-size">` 滚动日志）为稳妥首选，原生 `sc.exe` failure 恢复策略为无依赖兜底；Linux 对照 systemd `Restart=on-failure` + journald 轮转（D9-024~027） |

## 1. 本地复现远端流水线

### D9-001 act（nektos）——本地跑 GitHub Actions
- **定位**：读 `.github/workflows`、用 Docker 容器逐 job 执行的本地复现器；"本地 CI 复现"事实标准。
- **可抄机制**：① actrc 配置文件持久化 `-P` 平台映射（`ubuntu-latest=catthehacker/ubuntu:act-latest`），把"runs-on 标签→本地镜像"做成声明式映射表；② `-j` 选 job、`-n` dry-run、secrets 文件注入——门可以先 dry-run 校验拓扑再真跑；③ 生态兼容边界明确：官方只支持 Ubuntu 系容器（Better Stack 2025-04 指南口径），`windows-latest=-self-hosted` 是社区绕法（直接宿主机跑，issue #1984），Windows 步骤没有容器等价物。
- **档位**：有界——只用于复现 Linux 档的 GH Actions 门；Windows 门复现不指望它。
- **第一步动作**：在 ADV 仓加 `actrc` + 一个 `xtask ci --local` 包装，先只映射 ubuntu-latest 门做同镜像复跑（一致性④的容器化手段）。
- **许可证/成熟度**：MIT / 维护中（发版慢、长期招维护者，GitHub 仓为准）。
- **链接**：https://github.com/nektos/act · https://github.com/nektos/act/issues/1984

### D9-002 Forgejo Runner（原 act_runner 线）——主仓过渡的目标 runner
- **定位**：wave-0 05 路线里 Forgejo 主仓的执行器；2024 起从 gitea/act_runner 分叉独立发布（Codeberg e.V. 维护）。
- **可抄机制**：① **标签 scheme**：`labels` 配置把 `runs-on` 映射到执行后端——`docker://镜像`（容器隔离）、`host://`（宿主机直跑，无隔离）、`lxc://`（Linux LXC）；Windows 宿主机上 docker:// 需经 Docker Desktop/WSL2 只跑 Linux 容器，windows 门实际只有 host://（锚：forgejo.org Actions 管理员指南 + runner 安装文档）；② 注册/声明式配置：`forgejo-runner register --instance --token --labels` 生成 config.yml，`runner.capacity` 控制并行度，container 段可控 privileged/volumes/dind——runner 的"标签=能力声明"模型可整抄成 ADV runner 的 capability 协商；③ 隔离谱系自带安全分级：官方明确警告 host 模式威胁宿主机与网络，社区主流缓解是"runner 进 VM/LXC，而不是直接裸跑"（2025-02 博客：Ansible+LXC 隔离部署）。
- **档位**：有界——吸收"标签=后端声明 + capacity 并行 + 注册令牌生命周期"三机制；Windows 隔离缺口用 ADV 自有沙箱补，不裸用 host:// 跑不可信输入。
- **第一步动作**：在 ADV 本地装一个 Forgejo runner（host:// 标签），把 `xtask gate --tier ci` 挂成 `.forgejo/workflows` 的一个 job，验证 Rust 门全绿耗时基线。
- **许可证/成熟度**：MIT / 维护中（独立发布线：2024 v3→v5、2025 v6+，锚 releases 页）。
- **链接**：https://code.forgejo.org/forgejo/runner · https://forgejo.org/docs/latest/admin/actions/

### D9-003 Gitea act_runner（上游对照）
- **定位**：Forgejo Runner 的上游来源；Gitea 生态同步存在。
- **可抄机制**：与 D9-002 同源；上游文档额外固化了"runner 注册→标签→config.yml→服务化"的标准流程叙述，可当文档模板。
- **档位**：watch——功能被 Forgejo 分叉超越，主仓路线下无独立价值；盯其 host:// 语义变化即可。
- **第一步动作**：无（仅在 Forgejo Runner 升级冲突时回看上游 diff）。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://gitea.com/gitea/act_runner

### D9-004 Woodpecker CI——server+agent 双进程本地 CI
- **定位**：Drone 后继（Apache-2.0），自托管轻量 CI；对照对象里"agent 可以直接跑在开发机"的最小完整架构。
- **可抄机制**：① backend 抽象：同一 pipeline 定义可换 docker/local/kubernetes 后端执行，agent 用 `WOODPECKER_BACKEND=local` 让步骤直接以 shell 跑在 agent 宿主机（woodpecker-ci.org 文档）；② local backend 的能力缺口有据可查：service 容器未实现（issue #3095，2023-12 起挂账）——"本地后端=便宜但功能子集"的实证；③ agent 与 server 经队列解耦，agent 可增删——ADV 的"门执行器与触发器分离"可照抄此拓扑。
- **档位**：watch——整系统不吸收（与 Forgejo Actions 路线重叠）；吸收"backend 抽象 + agent 池拓扑"两个设计。
- **第一步动作**：不部署；把 `pipeline→backend→agent` 三段式写进 ADV ci-ops 设计文档作为拓扑参照。
- **许可证/成熟度**：Apache-2.0 / 维护中（v2→v3 线，2024–2025 持续发版，锚 GitHub releases）。
- **链接**：https://woodpecker-ci.org · https://github.com/woodpecker-ci/woodpecker/issues/3095

### D9-005 Concourse（对照）
- **定位**：老牌自托管 CI（worker+Garden 容器原语）；用于校准"重隔离 CI"的复杂度上限，本地开发机路线的反对证据。
- **可抄机制**：① 一切皆资源（resource check/run）的纯函数化流水线模型——input/output 显式声明是判据走真路径的思想同源物；② worker 与 ATC 分离、worker 无状态可随弃——ephemeral 思想的最早系统化实现之一。
- **档位**：reference——架构思想入档，工程不引入（部署重、Windows 支持依赖外部 worker VM，与本地优先相悖）。
- **第一步动作**：无；在 ci-ops 文档里留一段"为什么不是 Concourse"的复杂度论证。
- **许可证/成熟度**：Apache-2.0 / 维护中但节奏放缓（7.x 线，社区维持，以仓库 release 页为准）。
- **链接**：https://concourse-ci.org · https://github.com/concourse/concourse

## 2. 任务隔离（Windows 现状）

### D9-006 Docker Desktop——Windows 容器底座与授权
- **定位**：Windows 上容器隔离的事实底座（WSL2 后端）；act/Forgejo runner docker:// 后端都压在它上面。
- **可抄机制**：① 授权边界记牢：**免费条件=雇员<250 且年收入<$10M**，任一越线需付费（Pro/Team 档约 $9–15/用户/月，2025 报道口径；锚：Docker Subscription Service Agreement）——ADV 作为本地优先产品需在文档里写清"容器隔离档的依赖授权"，不能默认用户有 Docker Desktop；② `docker context`/named pipe（npipe）接入模型：任何兼容 Docker API 的引擎可互换（这是 Podman 可替换性的接口基础）；③ WSL2 集成带来的跨文件系统 IO 代价：仓库在 NTFS、容器在 ext4，9P 协议层是 Rust 大仓构建的慢点（微软 WSL 文档承认 9P 开销）——隔离档与宿主机档的速度差要作为已知档位差记录。
- **档位**：有界——作为"容器隔离档"的可选底座使用；ADV 不能硬依赖（授权 + 用户未必装）。
- **第一步动作**：ci-ops 配置里定义 `isolation: host|job-object|container` 三档，container 档标注"需要 Docker 兼容引擎"依赖声明。
- **许可证/成熟度**：专有（Desktop）/引擎组件 MPL-2.0、Apache-2.0 / 维护中。
- **链接**：https://www.docker.com/legal/docker-subscription-service-agreement/

### D9-007 Podman machine / Podman Desktop——无授权费的替代底座
- **定位**：Windows 上 Docker Desktop 的零授权费替代：`podman machine` 起 Linux VM（WSL2/Hyper-V），daemonless+rootless，暴露 Docker 兼容 API（2025 年多篇文章口径：XDA/Linux Journal/DataCamp）。
- **可抄机制**：① `podman machine start` 后可设 `DOCKER_HOST` 指向其 Docker 兼容 socket——act/Forgejo runner 不改代码即可换引擎（接口即解耦）；② rootless 默认：容器内 uid 映射，逃逸面比 privileged 容器小——ADV 容器档默认 rootless 的依据；③ 无 daemon：命令即进程，崩溃面小，适合本地长跑。
- **档位**：有界——容器档的第二引擎（授权敏感用户的主引擎）；不承诺与 Docker Desktop 行为逐位一致。
- **第一步动作**：在一台 Windows 机上验证 act `-P` + podman machine 组合能否跑通 ADV 的 ubuntu 档门，失败点落档。
- **许可证/成熟度**：Podman GPLv2、Podman Desktop Apache-2.0 / 维护中。
- **链接**：https://podman.io · https://podman-desktop.io

### D9-008 Rancher Desktop（对照）
- **定位**：第三块 Windows 容器底座（WSL2，可选 k3s），开源免费。
- **可抄机制**：与 D9-006/007 同接口竞争；它的"容器运行时(containerd/moby)可切换"选项本身说明：**底座应藏在 Docker API 兼容层后面**，ADV 门执行器不感知具体引擎。
- **档位**：reference——不引入，只作"引擎可替换"论据。
- **第一步动作**：无。
- **许可证/成熟度**：Apache-2.0（仓库 LICENSE 口径）/ 维护中。
- **链接**：https://rancherdesktop.io

### D9-009 Windows Job Object——ADV 进程隔离原语（与 R3/wave-0 08 交界）
- **定位**：Windows 内核的进程组机制；本地 CI 无容器时的主隔离层。
- **可抄机制**：① 进程生命周期原子化：`CreateJobObject` + `AssignProcessToJobObject`，`JOB_OBJECT_LIMIT_KILL_ON_JOB_ON_CLOSE` 保证句柄关闭即全组击杀——runner 崩溃不留孤儿进程树（cargo/rustc 子进程收干净）；② 限额三件套：`JOB_OBJECT_LIMIT_PROCESS_MEMORY`（内存上限防编译器吃满）、CPU rate control（`JOB_OBJECT_CPU_RATE_CONTROL` 限核比防门占用全机）、UI restrictions（禁剪贴板/桌面切换）；③ 子进程自动入组（不可逃出 job，除非显式 breakaway）——门进程树整体成界。对比容器：无文件系统边界，所以**必须**与一次性 worktree（D9-010）组合才构成完整沙箱。
- **档位**：吸收——ADV sandbox 的 host 档核心原语；接口设计成"job+worktree+环境变量白名单"三件套。
- **第一步动作**：adv-sandbox 提供 `spawn_gated(cmd, limits, workspace)` API，门执行器只经它启动任务； wave-0 08 的沙箱设计以本条为 CI 视角输入。
- **许可证/成熟度**：OS 内建（Win32）/ 稳定。
- **链接**：https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects

### D9-010 干净工作区策略——clone 深度 / worktree / fsmonitor
- **定位**：每个任务一个干净 checkout 的成本模型与最优解。
- **可抄机制**：① 成本阶梯：全量 clone O(历史+tip) > `--depth 1 --single-branch` 浅克隆 O(tip) > `git worktree add`（共享同一 .git 对象库，几乎只付检出的钱，git-scm.com/docs/git-worktree）——runner 池的"新任务新工作区"应默认 worktree，浅克隆仅用于一次性环境；② Windows 状态查询加速：`core.fsmonitor`（内置 FSMonitor 守护进程）+ `core.untrackedCache`，把"每次门都问 git status"的成本从全树扫描降到增量（git 配置文档）；③ 清理纪律：worktree 用完 `git worktree remove`（留残目录会积累磁盘与 Defender 扫描负担）。
- **档位**：吸收——runner 池的工作区生命周期：预热 worktree 池 + 任务后 `reset --hard`+`clean -fdx` 或直接销毁重建。
- **第一步动作**：ci-ops 实现 `workspace acquire/release` 两端点，acquire 返回隔离 worktree，release 决定 reset or drop。
- **许可证/成熟度**：git GPLv2 / 维护中（机制随 git 主线）。
- **链接**：https://git-scm.com/docs/git-worktree · https://git-scm.com/docs/git-config#Documentation/git-config.txt-corefsmonitor

### D9-011 Windows Defender 实时扫描——本地 CI 的隐形税
- **定位**：checkout/编译的写风暴逐文件过实时扫描，是 Windows 本地 CI 与远端 Linux 最大的系统性速度差来源之一。
- **可抄机制**：① 微软官方明确"开发场景建议配置排除项"（进程/文件夹/扩展名三类；锚：Microsoft Defender 文档 Configure exclusions）；② 排除的三层落位：仓库根目录（checkout 写风暴）、rustup 工具链与 cargo registry 目录（读多写多）、进程排除（cargo/rustc/adv 自身）；③ 反面纪律：排除=信任声明，只能加在"可信代码"的档位——不可信输入档（PR 沙箱）不得复用这些排除，否则扫描边界与信任边界错位。
- **档位**：吸收——ADV 安装器/文档输出一份"建议排除清单"脚本（显式征得用户确认再写入，符合配置三件套纪律：备份+等价性校验+回滚）。
- **第一步动作**：xtask 加 `adv doctor --defender`：检测实时扫描是否覆盖仓库，给出建议排除清单（不自动改）。
- **许可证/成熟度**：OS 内建 / 随 Windows 服务。
- **链接**：https://learn.microsoft.com/en-us/defender-endpoint/configure-exclusions-microsoft-defender-antivirus

### D9-012 Nix 构建沙箱（思想）
- **定位**：不引入 Nix 本身（无 Windows 原生支持）；吸收"声明式输入=构建可见世界"的模型。
- **可抄机制**：① sandbox 构建只看到 `/nix/store` 中声明的输入，其余路径不可见（Nix 手册 sandbox 配置）——ADV 门执行器的"环境变量白名单+路径白名单"同思想：门声明的输入集决定它能摸到什么；② 输出按内容寻址——门结果与产物哈希绑定，是"判据走真路径"的可校验形态；③ 副作用隔离：构建函数承诺无网络/无随机性，失败可复现——CI 门的 hermetic 档判据。
- **档位**：有界——只吸收输入声明与内容寻址两个思想，不引入 Nix 工具链。
- **第一步动作**：门注册表 schema 增加 `inputs: [paths, env, tools]` 声明字段，执行器据此构造白名单。
- **许可证/成熟度**：Nix LGPL-2.1 / 维护中。
- **链接**：https://nixos.org/manual/nix/stable/

## 3. Runner 工程（池化/缓存/信任边界）

### D9-013 GitHub ARC runner scale sets——预热池的参数化范本
- **定位**：GitHub 官方 k8s 自托管 runner 自动化框架；"预热池"工程的标准叙事。
- **可抄机制**：① `minRunners` 保底预热：helm 参数直接决定"常备多少热 runner 立即接活"（docs.github.com ARC 部署文档；issue #3704 明确 warm runners 语义），`maxRunners` 封顶——池尺寸=两个数字，可调参可实验；② scale set 与 job 队列经 listener 解耦，按需扩缩——ADV runner 池的"监听器/执行器分离"同构；③ 已知坑位记录在案：节点未就绪时 pod 提前删除导致 job 悬挂（issue，2024-04）——预热池实现必须定义"热≠已分配"的状态机。
- **档位**：吸收——机制参数化直接进 ADV ci-ops 的池配置（`warm_pool.min/max`），状态机照抄"热/分配中/回收中"。
- **第一步动作**：本地 runner 池原型实现 warm pool（job-object 隔离 + worktree 池），量化冷启动差（记锚：同一门预热 vs 冷起的秒差，本机复测为准）。
- **许可证/成熟度**：Apache-2.0 / 维护中（GitHub 官方）。
- **链接**：https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners-with-actions-runner-controller/deploying-runner-scale-sets-with-actions-runner-controller

### D9-014 ephemeral 单任务模式——防污染的生命周期
- **定位**：runner 一次注册跑一个 job 后自动注销销毁；自托管 runner 的安全基线模式。
- **可抄机制**：① GitHub runner `--ephemeral` 标志（runner 2.299+，2022 起）：job 结束即注销，凭据与状态不跨 job 存活——ADV 的对应物是"门进程组随 job 退出即整体击杀"（与 D9-009 kill-on-close 互补）；② GitLab 文档有同名 ephemeral runner 概念（docs.gitlab.com，一次性注册即弃），Forgejo runner 的容器后端天然 per-job 容器——三家共识：**跨 job 复用的是基础设施，不是状态**；③ 与预热池的组合：预热的是"环境"（工具链/镜像/空 worktree），销毁的是"状态"（工作区/进程/凭据）——两者不矛盾，是一枚硬币的两面。
- **档位**：吸收——ADV 门的默认生命周期=单任务；预热池只是加速"环境就绪"，绝不复用工作区状态。
- **第一步动作**：把"job 后状态清理清单"（worktree/进程组/临时凭据/env）写成 release 流程的硬步骤。
- **许可证/成熟度**：机制（各家实现不同）/ 维护中。
- **链接**：https://docs.github.com/en/actions/hosting-your-own-runners（ephemeral 章节）· https://docs.gitlab.com/runner/

### D9-015 sccache——缓存分层（本地盘 vs 远端）
- **定位**：Rust/C/C++ 编译缓存服务器化的事实标准；runner 工程里"状态外置"的核心件。
- **可抄机制**：① `RUSTC_WRAPPER=sccache` 透明接管编译，缓存键=编译输入哈希（内容寻址，同 D9-012 思想）；② 后端谱系：本地盘 / S3 / GCS / Azure / Redis / Memcached / GitHub Actions cache 服务（`SCCACHE_GHA_*`）——决策规则清晰化：**持久 runner（预热池）→ 本地盘+定期修剪；ephemeral runner → 必须远端**（否则缓存随 runner 销毁）；③ 命中率是可测量的（sccache --show-stats），门分层里"缓存档"与"全量档"分开报数，防缓存掩盖真实编译错误（编译错误不在缓存命中路径上——但链接/测试结果是）。
- **档位**：吸收——ci-ops 的缓存档默认本地盘（本地优先），远端后端作为 CI 全档可选。
- **第一步动作**：xtask gate 的 compile 层接 `RUSTC_WRAPPER`，基线记"冷/热"两组秒数（锚：本机+指定仓）。
- **许可证/成熟度**：Apache-2.0 / 维护中（2024–2025 持续发版，锚 GitHub releases）。
- **链接**：https://github.com/mozilla/sccache

### D9-016 预烘焙镜像——预热池的极限压缩
- **定位**：把工具链+系统依赖烤进镜像/VM 快照（GitHub runner-images 官方就是 Packer 烤的）；预热池的前置工程。
- **可抄机制**：① 环境分两半：**慢变层**（工具链/rustc/系统包）烤进镜像，**快变层**（源码/依赖锁文件）每任务现取——层切分决定预热效率；② 版本锚进镜像 tag：`adv-runner:rust-1.xx-date`（镜像名含工具链版本+日期，禁用 latest）——环境可复现=镜像可指认；③ 商业快 runner（Depot/Actuated/Namespace/Blacksmith）把"快照恢复微 VM"做成产品，证明该方向上限高，但单机本地优先场景用 worktree 池+常驻工具链已够。
- **档位**：reference——记录分层模型；ADV 第一版用"常驻工具链+worktree 池"，不引入镜像烘焙。
- **第一步动作**：无（池原型跑通后再评估是否值得镜像化）。
- **许可证/成熟度**：Packer BUSL-1.0（2023 起）/ runner-images MIT / 维护中。
- **链接**：https://github.com/actions/runner-images

### D9-017 fork PR 信任边界——action_required / pull_request_target
- **定位**：PR 触发的不可信代码是 runner 工程的第一安全问题；旧仓已用 action_required 审批，这里机制化。
- **可抄机制**：① 分级信任模型：GitHub 默认对**首次贡献者**的 fork PR 触发的 workflow 置 `action_required`，维护者批准才跑——"人工闸门只挂在不可信档"的最小实现；② `pull_request` 与 `pull_request_target` 的本质差异：前者用 PR 合并提交+只读 token+无 secrets，后者用基线仓的 workflow+可写 token+secrets——后者 checkout PR 代码即 PWN request（Legit Security 2021 披露；Orca：把 fork PR 当不可信输入，永不进特权 workflow）；③ 环境降权：PR 档无 secrets、网络受限、job-object+一次性 worktree——与 D9-009/D9-014 组成完整防线。
- **档位**：吸收——ADV 的信任三档（自己的提交=全档可信；维护者 PR=门全档；外部 PR=隔离档+人工批准，绝无凭据注入）。
- **第一步动作**：xtask gate 增加 `--trust` 参数，隔离档强制 adv-sandbox 三件套且拒绝读取任何凭据 env。
- **许可证/成熟度**：平台机制 / GitHub 维护中。
- **链接**：https://www.legitsecurity.com/blog/github-privilege-escalation-vulnerability · https://orca.security（PWN request 研究页）

### D9-018 self-hosted runner × 公共仓红线
- **定位**：GitHub 官方立场：不建议自托管 runner 服务公共仓（fork 可在 runner 机上跑任意危险代码，GitHub community discussion #26722）；2026 仍有现实案例：Sysdig 2026-01 报告自托管 runner 被武器化为持久后门。
- **可抄机制**：① 不可信代码的执行面必须是**一次性**的（对齐 D9-014 ephemeral）；② 缓解栈（StepSecurity/Equinor 指南口径）：ephemeral + 网络隔离/出口白名单 + 审批闸门 + 最低版本及时升级（GitHub 2026-09-29 起强制 runner 最低版本 ≥2.329.0，github.blog/changelog 口径）——四层各自挡一类攻击，缺一层就塌；③ ADV 视角转译：本地平台迟早会遇到"分析不可信代码"的任务（这正是 D 域的产品语义），**扫描不可信代码的进程本身就是不可信上下文**——解析器 fuzz 档与门执行档必须共享同一套隔离三件套。
- **档位**：吸收——信任边界模型进 adv-sandbox 需求；"runner 版本强制最低"进 ci-ops 的自更新策略。
- **第一步动作**：写 ADV 威胁模型一节：列出"门执行器可能接触的输入源"并逐源定信任档。
- **许可证/成熟度**：指南/机制 / 持续演进。
- **链接**：https://github.com/orgs/community/discussions/26722 · https://www.sysdig.com（2026-01 报告）· https://www.stepsecurity.io

## 4. 触发与门分层

### D9-019 pre-commit framework——两层门业界标准 + prek 信号
- **定位**：本地钩子管理的跨语言事实标准；"本地快档/CI 全档"两层制的标准载体。
- **可抄机制**：① **同一份 `.pre-commit-config.yaml` 两个执行面**：本地只对改动文件跑（快档），CI 跑 `pre-commit run --all-files`（全档）——一致性由"配置文件唯一"保证，不需要两份判据；② hook 的 `stages` 字段（pre-commit-commit/manual/…）把"哪道门跑在哪个阶段"声明在配置里而非散落脚本——ADV 门注册表的 `tiers` 字段同构；③ 2025 信号：框架加 `-j` 并行执行；Rust 重写的 drop-in 替代 **prek**（prek.j178.dev，单二进制、复用同配置）出现——侧面证明"配置唯一+执行器可换"的架构是对的；④ pre-commit.ci 服务=把全档跑在 PR 上产品化，社区 2025–2026 仍在活跃使用（未见维护模式公告，检索 2026-10 复核）。
- **档位**：吸收——ADV 落 `xtask gate --tier local|ci`：门注册表唯一，tier=子集选择器；本地层默认跳过的门显式列名单（绿=SKIP 不算绿）。
- **第一步动作**：定义门注册表 schema（id/stages/tier/inputs/judgement），先迁 fmt+clippy+deny 三门。
- **许可证/成熟度**：MIT / 维护中。
- **链接**：https://pre-commit.com · https://prek.j178.dev

### D9-020 watchexec / entr——文件触发（CI 角）
- **定位**：文件变更→命令执行的通用触发器；C5 已覆盖 notify 原语，此处只取 CI 触发编排角色。
- **可抄机制**：① 防抖参数化（watchexec `--debounce`）——"保存即跑快档"与"风暴合并"的旋钮；② entr 的 stdin 管道模型（`ls *.rs | entr -c make`）——极简协议：文件清单进、命令出，适合脚本组合；③ Rust 生态共识：cargo-watch 已弃维护，官方推荐 watchexec/bacon——ADV 自己的通知层接 watchexec 语义即可，不必再造防抖。
- **档位**：reference——不引入（C5 自研 notify 已覆盖），只对齐防抖语义与 CLI 约定。
- **第一步动作**：无（C5 输出对齐 `--debounce` 语义即可）。
- **许可证/成熟度**：watchexec MIT/Apache-2.0、entr BSD 类 / watchexec 维护中、entr 维护中。
- **链接**：https://github.com/watchexec/watchexec · https://eradman.com/entrproject/

### D9-021 bacon——连续本地检查（快档 UI 参照）
- **定位**：Rust 生态的持续检查器（check/clippy/test 循环+终端摘要）；"本地快档"的交互形态参照。
- **可抄机制**：① job 概念把"检查档位"参数化（bacon 的 job 定义可增删覆盖）——快档也是声明式配置不是散脚本；② 增量+聚焦输出：只报新失败，滚动保留最近结论——本地门结果的展示契约可抄；③ 与两层制的关系：bacon 是"local tier 的常驻形态"，CI 全档另有载体——两载体共享同一底层工具链（cargo）保证判据一致。
- **档位**：reference——个人开发循环可用，ADV 产品内不集成；吸收其"常驻+摘要+只报新失败"交互。
- **第一步动作**：无。
- **许可证/成熟度**：MIT/Apache-2.0 / 维护中（3.x 线，2025 持续发版）。
- **链接**：https://github.com/Canop/bacon

### D9-022 cargo-nextest——全档测试执行器
- **定位**：Rust 测试 runner（进程级并行、每测试一进程、失败重试策略）；CI 全档的门执行组件。
- **可抄机制**：① 每测试一进程：测试间隔离天然成立（进程边界=最小隔离单元），崩溃不连坐；② 机器可读输出（JUnit/JSON）——门结果的统一出口，直接喂 ADV 报告层；③ 重试与 flaky 标记分开统计——"绿"必须区分"一次过"与"重试过"（判据诚实性）。
- **档位**：reference——D7 域主选；本域只固定"门执行器输出必须是机器可读"的接口约定。
- **第一步动作**：ci-ops 门结果 schema 增加 `retries/flaky` 字段。
- **许可证/成熟度**：MIT/Apache-2.0 / 维护中。
- **链接**：https://nexte.st

### D9-023 本地 CI dashboard（结论条目）
- **定位**：回答"要不要自建本地 CI 面板"。
- **可抄机制**：业界形态三档：Web UI（Woodpecker/Forgejo 自带流水线视图——需要 server，本地优先场景偏重）、TUI 常驻摘要（bacon）、CLI 单次汇总（nextest 状态行）。共识缺位：没有"本地 CI dashboard"独立品类——说明该需求被 TUI/CLI 消化。
- **档位**：reference——不自建；ADV 的门结果统一进 TUI 摘要（快档常驻）+ 落盘 JSON（全档留档），两层各有一个文本出口即可。
- **第一步动作**：门结果 schema 定稿（含 verdict/duration/artifact 路径/时间戳），TUI 只读该 schema。
- **许可证/成熟度**：结论条目（无单一许可证）。
- **链接**：https://woodpecker-ci.org · https://github.com/Canop/bacon

## 5. 服务化（runner 常驻管理）

### D9-024 WinSW——Windows 服务包装（现状最优稳）
- **定位**：把任意可执行包装成 Windows 服务的 .NET 工具；runner 常驻的标准包装件（rclone/Jenkins 文档推荐口径）。
- **可抄机制**：① XML/YAML 声明服务：`<onfailure action="restart" delay="...">` 分级重启策略（服务化 runner 的自动重启诉求）；② 内置日志轮转：`<log mode="roll-by-size">`（stdout/stderr 落盘+滚动）——runner 常驻的日志纪律不用外挂 logrotate；③ 延迟自启动+依赖排序（`<depend>`），开机自愈链完整。
- **档位**：有界——作为 Forgejo runner Windows 服务化的默认包装；注意其发版停滞（最后正式发布 v2.12.0，2023-02；社区描述维护"复杂"，锚 GitHub releases 与 r/sysadmin 2025 讨论），锁定版本使用、不追新。
- **第一步动作**：写 `forgejo-runner-service.xml` 模板（onfailure+roll-by-size+depend），并入 ADV ci-ops 安装文档。
- **许可证/成熟度**：MIT / 维护停滞但稳定（v2.12.0 2023-02 后无正式发版）。
- **链接**：https://github.com/winsw/winsw

### D9-025 Shawl / Servy——新一代服务包装（2025 信号）
- **定位**：NSSM/WinSW 停滞后出现的继任者：Shawl（Rust 实现，与 ADV 技术栈同源）、Servy（C#，2025 活跃，带 GUI，dev.to 对比文口径）。
- **可抄机制**：① Shawl 的命令行式包装（无 XML）+ Windows 服务生命周期事件转译（CTRL 处理/优雅退出）——ADV 若自研服务化，直接用 Rust 写同款，消灭外部依赖；② Servy 的"服务即被管进程"监控面（实时状态+日志浏览）——本地 dashboard 结论条目（D9-023）的补充参照。
- **档位**：watch——先 WinSW 稳态落地，Shawl 作为"第二版自研服务化"的代码参照；Servy 观望。
- **第一步动作**：把 Shawl 仓库列入 ci-ops 文档引用（服务化自研的骨架参考）。
- **许可证/成熟度**：Shawl MIT / 维护中；Servy 开源（MIT 口径）/ 新项目（2025 起，成熟度待验）。
- **链接**：https://github.com/mtkennerly/shawl

### D9-026 NSSM（弃维护对照）
- **定位**：史上最常用的 Windows 服务包装；反面教材与兼容性参照。
- **可抄机制**：仅存参照价值：CLI 极简配置（`nssm install <svc> <cmd>`）与 AppStdout/AppStderr+AppRotateFiles 日志轮转接口——"包装器最小接口面"的定义仍成立。
- **档位**：不吸收——最后构建 2017（v2.24-101 线，锚 nssm.cc/镜像仓），新项目不引入。
- **第一步动作**：无；ci-ops 文档记一行排除理由。
- **许可证/成熟度**：自定义免费许可 / 弃维护。
- **链接**：https://nssm.cc

### D9-027 systemd 单元（Linux 对照模板）
- **定位**：Linux 侧 runner 服务化的原生载体；Windows 方案的对照面（wave-0 阶段化路线里 Linux 服务器迟早要管 runner）。
- **可抄机制**：① `Restart=on-failure` + `RestartSec=` + `StartLimitBurst`——重启策略的表达力优于多数包装器，值得在 WinSW 配置里对齐同语义；② journald 日志：stdout 天然收账+轮转+保留策略（`journalctl -u forgejo-runner`），对照 Windows 需 WinSW 显式配轮转——两端日志语义在 ADV 报告层应统一（同一 JSON 字段）；③ 单元硬化选项（ProtectSystem/PrivateTmp 等）是 Linux 侧的 job-object 对应物——隔离档位两端各记一份。
- **档位**：有界——Linux 阶段直接用 systemd（不自研）；Windows 阶段 WinSW；两端服务配置字段对齐成一张映射表。
- **第一步动作**：写 systemd unit + WinSW XML 对照表（字段级），进 ci-ops 文档。
- **许可证/成熟度**：systemd LGPL-2.1+ / 维护中。
- **链接**：https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html

## 2025–2026 前沿信号

1. **Forgejo Runner 独立演进加速**：从 gitea/act_runner 分叉后独立发布（2024 v3→v5、2025 v6+ 线，锚 code.forgejo.org/forgejo/runner/releases），rootless 容器后端与 LXC 后端（Linux）逐步成形——wave-0 选 Forgejo 主仓的 runner 生态正在兑现。
2. **GitHub ARC `minRunners` 预热池成为推荐姿势**：scale set 模式 + warm runners + ephemeral 生命周期组合已是官方文档主推（docs.github.com；issue #3704）——"预热池 × 单任务生命周期 × 缓存外置"三角被主流化。
3. **自托管 runner 安全升温**：2026-01 Sysdig 报告 runner 被武器化为持久后门；GitHub 2026-09-29 起强制 self-hosted runner 最低版本（≥2.329.0，github.blog/changelog 口径）——runner 自更新与信任边界从"最佳实践"升格为"硬要求"。
4. **pre-commit 生态换血**：框架 2025 加 `-j` 并行；Rust 重写的 drop-in 替代 prek 崛起（prek.j178.dev）——"配置唯一+执行器可换"架构被二次验证，ADV 门注册表按此形状设计。
5. **Windows 服务包装代际更替**：NSSM（2017）与 WinSW（2023-02 后无正式发版）双滞，Shawl（Rust）/Servy（2025）补位——自研服务化窗口打开，且 Rust 同源方案存在。
6. **商业快 runner 路线图**：Depot/Actuated/Namespace/Blacksmith 用"微 VM 快照恢复"把预热压缩到秒级——单机本地优先用 worktree 池+常驻工具链即可拿到大头，快照恢复留作后续档。
7. **Docker Desktop 授权阈值未变**（<250 人且 <$10M 免费，2025 复核），Podman Desktop 成熟度继续上升——容器隔离档的"引擎可替换"设计仍是正确押注。

## Top-3（按对 ADV 的杠杆排序）

1. **信任三档 + 隔离三件套（D9-009/014/017/018 组合）**：Windows 上任何 runner（Forgejo host:// 也好，本地门也好）都没有现成容器隔离，ADV 的差异化底盘就是"Job Object kill-on-close × 一次性 worktree × 凭据白名单"，并把信任分级（自有提交/维护者 PR/外部 PR）做成门执行器的一等参数——这是"本地 CI 平台敢碰不可信代码"的前提，也是与 05-selfhost 路线的接缝。
2. **两层门制（D9-019）**：业界标准已收敛为"唯一门配置 + local/ci 两个执行面"，ADV 落成 `xtask gate --tier local|ci` 单一判据源；tier 只选子集、判据只此一份、SKIP 显式留痕——"判据走真路径"纪律的 CI 版由此获得机器可校验的载体。
3. **预热池参数化（D9-013/014/015）**：`warm_pool(min,max)` × 单任务生命周期 × sccache 本地盘缓存三件组合，用三个可调参数表达冷启动/常驻成本/污染风险三角；ARC 的状态机（热/分配中/回收中）照抄，第一步先量化本机冷热差作为基线锚。
