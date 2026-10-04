# E7 — OS/内核/隔离实验调研

- 日期：2026-10-03（检索口径同日；标注"检索"的数字来自当日 web 检索摘要，其余为公开文档口径，均以链接处最新版本为准）
- 域边界：本域管「进程外隔离与最小特权」的机制供体；自托管 CI 的工程细节归 05 域，WASM/WIT ABI 归 08 域，本文只在交叉处引用。
- 方法：机制级条目 ≥8（实际 16），其余扫视（10）；每个条目给档位与第一步动作；许可证/成熟度必标（扫视行以机制+档位+链接为主）。

---

## 1. 机制级对象

### 1.1 Redox OS
- **定位**：Rust 微内核通用 OS（2015 起），"一切资源皆 URL/scheme" 的用户态服务化设计。
- **可抄机制**：① scheme 命名空间——资源访问统一为 `scheme:` URL，由用户态 scheme 守护进程仲裁，进程能"看见"哪些 scheme 由其启动配置决定（命名空间即权限边界）；② 内核极小（官方文档口径约 2 万行 Rust，只管进程/内存/IPC/核心 scheme），驱动在用户态——adv-sandbox broker 的"内核极小、能力在外围服务"同构；③ 2025-09 起转向"在 QEMU 里跑 Linux 驱动"的兼容策略（Phoronix 口径），说明微内核生态缺驱动时的务实路径。
- **档位**：reference
- **第一步动作**：把「scheme→URL→守护进程仲裁」映射成 adv-sandbox 的虚拟路径表设计评审输入（1 页对照笔记）。
- **许可证/成熟度**：MIT / 实验（持续开发；最新大版本 0.9 于 2024 末；2025：X11/GTK3 于 2025-06、x86 多线程默认开于 2025-10 报道、内核加 io_uring 风格 API 与 NUMA）
- **链接**：https://redox-os.org · https://gitlab.redox-os.org/redox-os

### 1.2 Theseus OS
- **定位**：语言侵入式（language-intrusive）研究 OS：把 OS 组件写成同一语言内的库，用类型系统承担传统上靠硬件特权的隔离与演化。
- **可抄机制**：① "safe supervisor"——在同一地址空间的 safe 代码内部划分特权域，靠类型/可见性而非页表隔权；② cell-free design——组件间共享状态不经过 unsafe 全局，撤组件不留悬空依赖（"无遗忘"演化）；③ 组件可原地换入换出（堆与控制流持久）。
- **档位**：reference
- **第一步动作**：只取思想一条：adv-sandbox 插件边界尽量用「类型不可达」而非「运行时拒绝」表达（与 08 域 WIT 类型面的交叉检查项）。
- **许可证/成熟度**：MIT / 实验（研究项目，作者博士论文 2021–2022 完成，近年低频维护）
- **链接**：https://www.theseus-os.com · https://github.com/theseus-os/Theseus

### 1.3 Hermit
- **定位**：库形态 Rust unikernel：应用直接链接"内核即库"，跑在 KVM/HVF 等 VM 里，单地址空间、无系统调用边界。
- **可抄机制**：① "内核即依赖项"——目标二进制只包含用到内核功能，攻击面随裁剪收缩；② 明确放弃多进程/多地址空间换取单空间的零拷贝与确定性；③ 以 VM 为外层边界（隔离责任上移给 hypervisor）。
- **档位**：reference
- **第一步动作**：作为 adv-sandbox 自托管底座"单任务微型执行体"的备选形态记录（是否值得引入由 05 域 CI 需求决定）。
- **许可证/成熟度**：MIT/Apache-2.0 / 维护中（RWTH 系转社区维护）
- **链接**：https://github.com/hermit-rs/hermit

### 1.4 Rust for Linux
- **定位**：把 Rust 引入 Linux 主线的工程：Rust 抽象层 + 少量 Rust 驱动。
- **进展锚**：6.15（2025-05）合入 Nova core（Red Hat 主导的 NVIDIA GPU 驱动脚手架）与 driver-core/DRM Rust 抽象，Rust 的"实验性"标签被移除（检索口径）；2026 年内核周期（Phoronix 按 7.x 系列报道，2026-03/06）Rust DRM 工作继续以 Nova 为主。
- **可抄机制**：① 抽象层模式——unsafe 只出现在薄抽象内，驱动侧纯 safe（对 adv-sandbox 内部 unsafe 治理是同构样板）；② 内核**内部无稳定 ABI**：绑定随内核版本演进，这是用内核模块做长期产品的结构性成本。
- **档位**：watch
- **第一步动作**：不立项内核模块；把「内核侧观测走 eBPF（见 1.10）」写进 adv-sandbox 观测设计约束；每半年复查一次 R4L 抽象层成熟度。
- **许可证/成熟度**：GPL-2.0（内核侧代码）/ 维护中（主线子系统化）
- **链接**：https://rust-for-linux.com · https://github.com/Rust-for-Linux/linux

### 1.5 Firecracker
- **定位**：AWS 的 microVM VMM：KVM 之上极小设备模型（virtio-net/block/vsock、串口），REST API 走 unix socket，生产配套 jailer。
- **可抄机制**：① jailer 分层收紧：cgroup+namespace 隔离壳 → chroot 私有目录 → 降权 uid/gid → seccomp，才启动 VMM 进程（"启动器本身也要被沙箱化"）；② 设备模型白名单化——不实现任何非必需设备，guest 只能见到明确列出的 virtio 面；③ 控制/数据面分离：API 只在本地 unix socket，配置一经启动即锁定。
- **对宿主的依赖**（两端点名）：Firecracker 对宿主的依赖是 Linux + KVM；**Windows 宿主不支持**（2026-10-03 检索确认）。guest 侧官方支持 Linux 与 Windows（Windows guest 支持为 2023 年起加入，细节以 release notes 为准）。
- **档位**：有界（ci-ops：仅 Linux runner 档）
- **第一步动作**：在 05 域自托管 CI 设计中落一条「Linux 强隔离 runner = Firecracker + jailer」候选路径；Windows 宿主不走此路径（见 §3①）。
- **许可证/成熟度**：Apache-2.0 / 维护中（AWS 生产系统在用）
- **链接**：https://github.com/firecracker-microvm/firecracker

### 1.6 libkrun
- **定位**：库形态轻量 VMM（containers 项目）：把 microVM 嵌进普通进程，无需完整 VMM 守护进程；Podman Machine 在 Apple Silicon 上的后端之一。
- **现状锚**：官方后端为 Linux KVM 与 macOS Hypervisor.framework（2025-06 Red Hat 开发者文章口径：krunkit+libkrun 改善 macOS AI 推理）；**上游无官方 Windows 后端**，Windows 只有第三方 WHPX FFI 绑定（a3s-libkrun-sys，2026 检索）。
- **可抄机制**：① "VM 即进程内库"——把强隔离做成可链接能力而非独立系统；② 依赖宿主 hypervisor 抽象（KVM/HVF），同一 API 多平台后端。
- **档位**：watch（升级条件：上游出现可用的 Windows/WHPX 后端并稳定发版）
- **第一步动作**：跟踪其 release notes；在 05 域记录为 macOS 档 runner 候选。
- **许可证/成熟度**：MIT / 维护中（CNCF containers 项目群）
- **链接**：https://github.com/containers/libkrun

### 1.7 Cloud Hypervisor
- **定位**：Linux 基金会托管的 Rust VMM，社区对标 QEMU 的精简替代；guest 支持 Linux 与 Windows。
- **现状锚**：约每 6–8 周发版（项目惯例口径）；2025 年版本起 virtio-fs/blk 稳定；v52 加入 AMD SEV-SNP 机密 VM 支持（2026 报道口径）；第三方对比（2026-02 safeguard.sh）给出启动约 180–240ms、VMM 开销约 8MB 的量级数字。
- **可抄机制**：① Rust VMM 生态的组件化（virtio-device、vm-memory、vmm-sys-util 等独立 crate）——"VMM 拆成可审计组件库"；② 设备热插拔与机密计算（SEV-SNP）选项。
- **档位**：watch（ci-ops 备胎：若 Firecracker 设备模型不够用时的 Rust 可改选项）
- **第一步动作**：仅记录；不为 ADV 引入第二个 VMM 候选，待 05 域需求明确再比。
- **许可证/成熟度**：Apache-2.0 / 维护中
- **链接**：https://github.com/cloud-hypervisor/cloud-hypervisor

### 1.8 gVisor（Sentry/Gofer）
- **定位**：Google 的用户态应用内核：容器进程的 Linux 系统调用全部由用户态 Sentry 接管并重新实现语义，宿主内核只见极小的受控调用面。
- **机制拆解**：
  1. **拦截层（平台）**：默认 Systrap（2023-04 发布、2023-09 起取代 ptrace）——用 seccomp 把 syscall 陷进 stub 进程，Sentry 经共享内存读取/续跑；备选 KVM 平台（硬件加速）。
  2. **语义重写**：Sentry 用 Go 重新实现 Linux ABI（官方口径覆盖 200+ 系统调用），**不转发**——worker 侧看到的"内核"是 Sentry。
  3. **宿主面收敛**：Sentry 自身再被 seccomp 白名单约束，只放行一小批固定宿主 syscall（数十量级，随平台模式变化）——双层白名单。
  4. **文件面代理**：Gofer 每容器一个宿主进程，走 9P 提供文件服务并做校验（Sentry 可能被攻破后仍不能直接摸到宿主 FS）；DirectFS（2023-06）改为经 Gofer 校验后的直接 fd，减少往返。
- **与 adv-sandbox broker/worker 互证**：broker↔Gofer（资源代理+校验）、worker↔Sentry（ABI 面重写）、broker 自身收窄宿主调用面↔Sentry 的 seccomp 白名单——同构成立，方向一致。
- **对 adv-sandbox 的纠偏**（见 §3②）。
- **档位**：吸收（机制思想进 adv-sandbox；运行时本身仅 Linux 可用，不作为 ADV 依赖引入）
- **第一步动作**：把「worker 可见 ABI 面 = broker 重定义面，而非宿主 ABI 子集」写进 adv-sandbox broker 接口设计文档。
- **许可证/成熟度**：Apache-2.0 / 维护中（Google，GKE/Knative 场景长期在用）
- **链接**：https://gvisor.dev/docs · https://github.com/google/gvisor

### 1.9 Kata Containers（+ CoCo）
- **定位**：每容器（pod）一个 VM：宿主侧 shim-v2 + guest 内 kata-agent，virtio-fs/VSOCK 通信；机密计算分支 CoCo。
- **现状锚**：3.x 深入 3.2x（2026 检索：CoCo helm 已配 Kata 3.28.0）；CoCo v0.14.0（2025-05-23，基于 Kata 3.17.0）在 Intel TDX / AMD SEV-SNP / IBM SE 三平台通过测试；**2025-09 披露 CVE-2025-58354**：恶意宿主可绕过关键安全校验——VM 级隔离的边界实现也要跟补丁审计。
- **可抄机制**：① agent-in-guest 模式（guest 内最小代理对接宿主运行时，与 broker/worker 分层同构）；② CoCo 把"宿主不可信"列为威胁模型的工程化（attestation 流程）。
- **档位**：watch（ci-ops：K8s 生态强绑定，ADV 自托管场景暂无 K8s 面板）
- **第一步动作**：无；仅把 CVE-2025-58354 记为「隔离层补丁节奏」的反例锚。
- **许可证/成熟度**：Apache-2.0 / 维护中（OpenInfra 基金会）
- **链接**：https://github.com/kata-containers/kata-containers · https://github.com/confidential-containers

### 1.10 eBPF LSM（BPF LSM）+ aya
- **定位**：内核侧安全钩子的"无模块"路线：`BPF_PROG_TYPE_LSM` 程序挂到 LSM 安全钩子上做允许/拒绝判定（内核 5.7+），aya 提供 Rust 原生加载与编写工具链。
- **可抄机制**：① 观测与策略内核侧化但**用户态加载**：不随内核版本编译（CO-RE 重定位），升级成本远低于模块；② LSM 钩子覆盖 open/mmap/bpf/ptrace 等安全敏感点，可做"观察+阻断"双模式；③ aya 侧：`#[lsm]` 宏已文档化（aya 书/社区资料口径），aya-ebpf-bindings 2026-06 仍在 crates.io 发版。
- **档位**：吸收（adv-sandbox Linux 档的观测/策略执行底座候选）
- **第一步动作**：在 adv-sandbox 设计里给 Linux 档定两条钩子探针（file_open、bpf）做 PoC，验证 aya 加载链路（只观测不阻断先跑通）。
- **许可证/成熟度**：Apache-2.0/MIT（aya）；LSM 钩子为内核特性 / 维护中
- **链接**：https://github.com/aya-rs/aya · https://aya-rs.dev

### 1.11 seL4（+ Microkit / LionsOS）
- **定位**：形式化验证的能力微内核：能力（capability）是内核管理的权能令牌，一切系统资源经能力寻址。
- **机制拆解（能力模型数据结构）**：
  1. **CSpace**：每进程一张 CNode 槽表；每个槽 = {内核对象引用/类型，rights 位集，badge/guard}——能力是"内核对象 + 权限 + 辨识标签"的三元组，与地址空间解耦。
  2. **派生树**：untyped 内存可 retype 出子对象；子能力权限 ⊆ 父能力；revoke 沿派生树级联撤回——权限有出处、可撤销。
  3. **IPC 传递即授权**：能力只能经 endpoint 消息显式传递，无环境权威。
  4. **Microkit**（seL4 基金会官方框架，文档 docs.sel4.systems/projects/microkit）：静态定义保护域与通道，Rust 一等支持；LionsOS（2025-01 发表）构建其上；2025 年还有 SBIR 约 190 万美元项目选 seL4+Microkit 做 x86_64 hypervisor（检索口径）——能力模型工程化在提速。
- **对 ADV 能力白名单表的映射**：见 §3③。
- **档位**：吸收（数据结构/语义思想；不引入 seL4 本体）
- **第一步动作**：把 ADV 白名单表 schema 按三元组+派生树改版（§3③ 的结构），在 08 域 WIT 接口评审时过一遍。
- **许可证/成熟度**：GPL-2.0（内核）；用户态工具多为 BSD 系（以各仓库为准）/ 维护中（seL4 基金会）
- **链接**：https://sel4.systems · https://docs.sel4.systems/projects/microkit

### 1.12 Fuchsia / Zircon 命名空间
- **定位**：Google 的非 Linux 内核 OS：句柄（handle）+ rights + 每进程命名空间。
- **可抄机制**：① 权限挂在 handle 上（`ZX_RIGHT_*` 位集），名字与权限分离——拿到"路径名"不代表有权限；② 每进程命名空间 = 私有的「名字→句柄」映射表，进程只见被显式放进命名空间的对象；③ 组件框架以 offer/expose/use 显式路由能力（能力路由可审计）。
- **档位**：reference
- **第一步动作**：同 §3③：确认 ADV 白名单表采用「worker 视角虚拟名 → broker 侧真实句柄+rights」两层结构，worker 不持有真实路径。
- **许可证/成熟度**：BSD/MIT 系混合开源 / 维护中（Google；消费级设备落地有限）
- **链接**：https://fuchsia.dev

### 1.13 Capsicum（FreeBSD 能力模式）
- **定位**：把"环境权威"从进程中拿掉的内核机制：capability mode + 每 fd 权限收窄。
- **可抄机制**：① `cap_enter()` 后进程失去环境权威：不能按绝对路径或相对 cwd 打开文件（AT_FDCWD 类操作被拒），只能用既有 fd——"先有句柄，后有操作"；② `cap_rights_limit()` 给每个 fd 挂 CAP_* 权限位集（read/write/seek/fattr 等数十个粒度）；③ 已被 Chromium 沙箱、OpenSSH 采用（工程可信度锚）。
- **档位**：吸收（fd 级 rights 位集 → ADV 白名单表的每句柄权限列）
- **第一步动作**：同 §3③：白名单表每条目带 rights 位集；broker 默认拒绝"按名打开"，只接受句柄操作。
- **许可证/成熟度**：BSD-2-Clause（FreeBSD base）/ 维护中
- **链接**：https://docs.freebsd.org/en/books/handbook/security/#capsicum

### 1.14 systemd 沙箱指令集
- **定位**：Linux 档"进程级策略面"的事实全集：单文件 unit 内声明式组合内核隔离原语。
- **机制拆解（分组清单，ADV Linux 档 schema 的维度来源）**：
  - 名字空间：PrivateTmp / PrivateDevices / PrivateNetwork / PrivateUsers / PrivateIPC、RestrictNamespaces
  - 权限：User=/DynamicUser=、NoNewPrivileges、CapabilityBoundingSet、AmbientCapabilities、RestrictSUIDSGID
  - 文件系统：ProtectSystem(=strict)、ProtectHome、ReadOnlyPaths、InaccessiblePaths、TemporaryFileSystem、RestrictFileSystems（v249+，基于 Landlock）、ProtectProc(invisible)、ProcSubset(pid)
  - 系统调用：SystemCallFilter（allowlist + @system-service 等预置集）、SystemCallArchitectures、SystemCallLog
  - 网络：RestrictAddressFamilies、IPAddressDeny/Allow、SocketBindDeny/Allow
  - 设备/杂项：DeviceAllow、MemoryDenyWriteExecute、LockPersonality、ProtectKernel{Tunables,Modules,Logs}、ProtectClock、ProtectHostname、KeyringMode
- **对 ADV 的借鉴**：这份清单本身就是「Linux 档完整策略面」的验收维度表（每条给 Windows 档对应物，见 §3⑤）。
- **档位**：吸收（策略 schema 维度，非引入 systemd 依赖）
- **第一步动作**：把上表转录成 ADV 策略 schema 草案分组（Windows 档每行给对应物），进 08 域策略面评审。
- **许可证/成熟度**：LGPL-2.1+ / 维护中（指令集持续扩张，256/257 于 2024–2025 发布）
- **链接**：https://www.freedesktop.org/software/systemd/man/（sandboxing 指令节）

### 1.15 Bubblewrap
- **定位**：Flatpak 底座的极简沙箱 CLI：一次性进程，构建 mount/pid/net 等 namespace 后 exec 目标，无守护进程。
- **可抄机制**：① 无 daemon、单次生命周期——不留长期特权驻留面；② 无特权 user namespace 优先，setuid 助手仅在 userns 被禁的系统上启用；③ 参数即策略：命令行里显式列出每个 bind/挂载/能力，缺省即拒绝。
- **档位**：reference（最小化原则：无驻留、显式枚举、缺省拒绝）
- **第一步动作**：把「broker/worker 生命周期 = 一次性、无长期特权驻留」写成 adv-sandbox 设计红线条款。
- **许可证/成熟度**：LGPL-2.1+（以仓库为准）/ 维护中
- **链接**：https://github.com/containers/bubblewrap

### 1.16 Unikraft
- **定位**：可组合 unikernel 构建系统：菜单选配内核微组件 + KraftKit CLI，按应用特化裁剪，启动毫秒级（官方口径）。
- **可抄机制**：① 特化构建（应用+内核一起裁剪编译）——与 Hermit 同族但工程化/生态更强；② KraftKit 的"构建即配置"工具链；③ 商业化延伸 Unikraft Cloud（2024 起）说明该路线有持续投入。
- **档位**：watch
- **第一步动作**：无；待 05 域出现"每任务极快冷启动隔离执行体"需求时再评估。
- **许可证/成熟度**：BSD-3-Clause / 维护中（v0.17+（2024 起）持续发版）
- **链接**：https://unikraft.org · https://github.com/unikraft/unikraft

---

## 2. 扫视对象（一句机制 + 档位 + 链接）

| 对象 | 一句机制 | 档位 | 链接 |
|---|---|---|---|
| MirageOS | OCaml unikernel：类型安全语言写"库即内核"，Solo5 做轻量 target，与 Hermit/Unikraft 同族对照 | reference | https://mirage.io |
| Qubes OS | 分区思想：按信任级切分域 VM（sys-net/sys-firewall/sys-vault 等服务域 + 模板域），跨域只经受控通道——对 ADV 自托管底座的"职责分区"模板 | reference | https://www.qubes-os.org |
| Nabla Containers | rump kernel unikernel-per-container；Intel 项目，GitHub 自 2021 年前后基本无更新 | 不吸收（弃维护） | https://nabla-containers.github.io |
| Chromium Mojo | 强类型 IPC：接口即 message pipe，句柄随消息传递即能力传递——ADV broker/worker 协议的接口定义风格参照 | reference | https://chromium.googlesource.com/chromium/src/+/HEAD/mojo/ |
| Windows AppContainer / LPAC | lowbox token + capability SID 门禁命名对象；LPAC 更严（默认拒绝，显式加 capability）；broker 模式即 Chromium/Edge 的Windows 落法——**ADV Windows 档的进程级隔离原生底座** | 吸收 | https://learn.microsoft.com/windows/win32/secauthz/appcontainer-isolation |
| Windows Sandbox | 一次性桌面 VM（每次干净启动），.wsb 声明式配置——Windows 宿主上 VM 级隔离的现实选项（2025 起仍在 Insider 迭代功能） | 有界 | https://learn.microsoft.com/windows/security/application-security/application-isolation/windows-sandbox/ |
| eBPF for Windows | 微软把 eBPF 移植到 Windows（MIT），仍是预览态/功能子集——Windows 档内核观测的远期选项 | watch | https://github.com/microsoft/ebpf-for-windows |
| Landlock | 无特权即可用的 FS 访问控制 LSM（内核 5.13+，ABI 逐版扩展网络等）——Linux 档"无 root 也能收紧 FS"的原语，systemd RestrictFileSystems 底座 | 吸收 | https://landlock.io · https://docs.kernel.org/userspace-api/landlock.html |
| Apple Exclaves | 2025 年披露的 Apple 方向：独立核上跑沙箱化执行体，外设访问经嵌入式 reference monitor 仲裁——"监控器下沉"信号 | watch | 论文/公开演讲口径（arXiv 2025） |
| WASI 0.3 / Component Model | 2025 年进入 0.3 发布周期（native async 等）——adv-sandbox 的 ABI 底座演进，归 08 域跟踪，本域不重复登记 | reference | https://github.com/WebAssembly/WASI |

---

## 3. 重点回答

### ① Firecracker/krun 对「自托管 CI 强隔离 runner」的适用性（Windows 宿主怎么办）
- **Firecracker 对宿主的依赖是 Linux+KVM；Windows 宿主不支持**（2026-10-03 检索确认）。libkrun 上游仅 Linux(KVM)/macOS(HVF)；Windows 只有第三方 WHPX 绑定（a3s-libkrun-sys，非上游、未见生产背书）。
- **替代路径（按推荐序）**：
  1. 强隔离档绑 **Linux runner**（裸机或独立小主机）：Firecracker + jailer，每任务一个 microVM，快照恢复加速冷启动。
  2. Windows 宿主退一层：**Windows Sandbox / Hyper-V 隔离**做 VM 级一次性执行（有界档），与 ADV broker+AppContainer 的进程级隔离分层并存，不互相替代。
  3. WSL2 嵌套 KVM 跑 Firecracker：仅限开发自验；生产 runner 不建议把强隔离架在嵌套虚拟化上（TCB 扩大 + 性能/稳定性折损，此为一般工程判断非实测数字）。
  4. 远程 Linux runner（云）作为最外档；本地只承担普通档。
- **结论**：CI 强隔离档与宿主 OS 解耦——Linux 档用 microVM，Windows 档用 Windows Sandbox/AppContainer 分层；不要追求"Windows 宿主上跑 Firecracker"。

### ② gVisor 拦截机制拆解 → 对 adv-sandbox broker/worker 的互证与纠偏
- **互证**：broker↔Gofer（资源代理+校验）、worker↔Sentry（看到的是重写的"内核"）、双层白名单（worker 面 + broker 自身面）——分层假设一致。
- **纠偏三条**：
  1. gVisor 的核心不是"拦截转发"而是**重写语义**：adv-sandbox 的 broker 应定义 worker 专用的最小 ABI 面（WIT 接口），而非把宿主 syscall 透传后过滤。
  2. 代理自身也要被收紧：Sentry 对宿主内核只见固定 syscall 白名单；ADV 的 broker 进程应把自己的 Win32/Linux 调用面收敛到白名单（Windows 档结合 AppContainer/restricted token）。
  3. **不信任 worker 提交的名字**：gVisor 的 Gofer 假设 Sentry 可能撒谎（路径穿越校验/句柄化）。ADV broker 必须以自己持有的句柄为准，worker 只送虚拟名——不送真实路径。

### ③ seL4/Fuchsia/Capsicum 能力模型 → ADV 能力白名单表的数据结构启示
- 共同点：**权限挂在对象引用（句柄）上而非名字上；权限有派生出处、可撤销；无环境权威**。
- 建议表结构（ADV capability 表）：
  ```
  CapEntry {
    cap_id, virtual_name,        // worker 可见名（worker 永不持真实路径）
    broker_handle,               // broker 侧真实句柄
    rights: bitset,              // read/write/seek/fattr/net-connect/... （Capsicum CAP_* 粒度参照）
    derived_from: Option<cap_id>,// 派生树（seL4 CSpace/retyping 参照）
    revocable: bool, ttl,        // 撤销沿 derived_from 级联（seL4 revoke 语义）
  }
  ```
- 语义红线：子能力权限 ⊆ 父能力；"按名打开"默认拒绝（capability mode 语义）；能力传递必须显式（Fuchsia offer/expose/use 语义）。

### ④ Rust for Linux 对「ADV 内核侧观测」的可行边界
- R4L 已主线化（6.15 Nova core、实验标签移除，2025-05 口径），但**内核内部无稳定 ABI**：模块（无论 C/Rust）绑定随内核版本演进，是长期维护的结构性成本，且内核模块的安装权限本身就是高价值攻击面。
- **边界结论**：ADV 的内核侧观测不走内核模块（任一语言）；走 eBPF——tracepoint + BPF LSM 钩子、aya（Rust）用户态加载（见 1.10），把"随内核演进"的成本留在用户态工具链。R4L 本体标 watch，仅作 unsafe 治理与抽象层设计的参照。

### ⑤ systemd sandbox 指令清单 → ADV 策略 schema（Linux 档完整策略面）
- 采纳 1.14 的六组维度作为 ADV Linux 档 schema 骨架，并要求**每条 Linux 指令给出 Windows 档对应物**（避免单一平台 schema）：
  | Linux（systemd 语义） | Windows 档对应物 |
  |---|---|
  | PrivateTmp/ProtectSystem/InaccessiblePaths | AppContainer capability SID 门禁 + broker 句柄化虚拟路径 |
  | CapabilityBoundingSet/NoNewPrivileges | restricted token + 去除特权组 |
  | SystemCallFilter(allowlist) | （无等价物：以 broker ABI 面裁剪替代，Win32k filter/win32k lockdown 可减面） |
  | RestrictAddressFamilies/IPAddressDeny | WFP/防火墙规则 + broker 网络代理 |
  | DeviceAllow/PrivateDevices | 无设备句柄分发（默认无） |
  | RestrictNamespaces/PrivateUsers | 无（进程级用 Job Object 限进程树/内存/时间） |
- schema 设计约束：策略条目 = {声明字段, 平台映射, 默认值(拒绝), 观测探针}，默认全拒绝、显式开启。

---

## 4. 2025–2026 前沿信号

- **机密计算下沉到轻量 VMM**：Cloud Hypervisor v52 加 AMD SEV-SNP（2026 报道口径）；CoCo v0.14.0（2025-05-23）三 TEE 平台（TDX/SEV-SNP/IBM SE）通过测试——microVM+TEE 的组合在工程化。
- **VM 级隔离也要跟补丁**：Kata CVE-2025-58354（2025-09，恶意宿主绕过关键校验）——隔离层实现本身的审计节奏要进 ADV 的依赖维护流程。
- **libkrun 的 Windows 缺口**：上游只有 Linux/macOS 后端，Windows 靠第三方 WHPX 绑定（2026-10 检索）——Windows 宿主强隔离仍是生态空白，支持 §3① 的分层结论。
- **微内核务实化**：Redox 2025 转"Linux 驱动跑在 QEMU 里"（2025-09 报道）、加 io_uring 风格 API——纯微内核生态缺口的现实解法信号。
- **能力模型工程化提速**：LionsOS（2025-01 发表，构建于 Microkit）、Microkit Rust 一等支持、2025 SBIR 资助 seL4+Microkit x86_64 hypervisor（约 190 万美元，检索口径）。
- **Rust 观测工具链可用**：aya 的 `#[lsm]` 宏文档化、aya-ebpf-bindings 2026-06 仍发版（crates.io 口径）——"内核侧观测+用户态 Rust 加载"链路已可 PoC。
- **Windows 侧在动**：Windows Sandbox 2025 起继续 Insider 功能迭代；eBPF for Windows 仍预览态——Windows 档观测/隔离的原生选项在缓慢演进。
- **监控器下沉**：Apple Exclaves（2025 披露）把 reference monitor 做进外设访问路径——"最小特权的仲裁点尽量小且靠近资源"的方向信号。

## 5. Top-3（按对 ADV 的吸收价值）

1. **gVisor（Sentry/Gofer/双层白名单）**——adv-sandbox broker/worker 的机制互证 + 三条纠偏（重写语义而非拦截转发；broker 自身调用面收敛；不信任 worker 名字只认 broker 句柄）。
2. **systemd 沙箱指令集（六组维度 + Windows 对应物表）**——ADV Linux 档策略 schema 的完整维度清单，直接转录成验收表。
3. **seL4/Capsicum/Fuchsia 的能力数据结构（{句柄, rights 位集, 派生树} + 无环境权威）**——ADV 能力白名单表的 schema 与撤销语义。

次选：Firecracker+jailer 作为 Linux 强隔离 CI runner 的有界路径（Windows 宿主按 §3① 分层处理）。
