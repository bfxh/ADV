# 08 · 沙箱与插件/规则隔离（外部调研）

- 日期：2026-10-03 ｜ 方法：WebSearch/WebFetch 外部调研（本会话内检索，未逐条源码复核）
- 服务对象：unified-rx 重写蓝图（D:\KF\ADV），Rust 主体，MCP 服务器持用户权限运行，未来有规则/插件生态
- 平台：Windows 开发主机为主，Linux 档覆盖 CI/发布
- 证据纪律：结论带来源与日期；检索摘要与原文可能有出入的条目标「待验证」；不把单点基准升格为普遍结论

---

## 0. 三个重点回答（先给结论）

### ① 规则/插件沙箱选型：wasmtime vs wasmi vs JS 引擎

**判据（五轴）**

| 轴 | wasmtime（JIT/Cranelift） | wasmi（解释器） | rquickjs（QuickJS 绑定） | boa（纯 Rust JS） | deno_core（V8） |
|---|---|---|---|---|---|
| 启动开销 | 编译+实例化；用 pooling allocator + CoW 摊到 µs 级（官方口径，具体数字待实测） | 无编译，直接解释，启动最省 | 极小（QuickJS 级） | 中 | 最重（V8 初始化 + 二进制体积） |
| 执行吞吐 | 最高（JIT） | 低于 JIT（量级视负载，见 wasmi-labs 基准，待实测） | 中 | 三者最慢（0.19 自报 ~94% ES 合规，非性能向） | 最高 |
| 内存 | 偏大（JIT 代码 + 池预占） | 小 | 小 | 中 | 大 |
| Windows/移植 | 好；但 JIT 需要可执行内存映射，且 Cranelift 出过平台特定误编译（2026-04 aarch64 沙箱逃逸，见 §7） | 最好（no_std、无 JIT 约束） | 好（C 依赖 QuickJS，需构建链） | 好（纯 Rust） | 好但重 |
| 生态/供应链 | Bytecode Alliance，安全披露流程成文 | 活跃（wasmi-labs），Substrate 系在用 | 快 JS 生态可复用 | 纯 Rust 但社区较小 | Deno 系 |

**分层结论（条件化，按负载选型而不是一刀切）：**
1. **插件 = WIT/Component Model ABI + wasmtime**：规则/插件是第三方不可信代码，要跑在 guest 内存隔离 + fuel/epoch 抢占 + 显式 host 函数白名单里；插件寿命长、可复用实例池，JIT 成本摊得掉。
2. **规则热路径（每次扫描都执行的简单谓词）= wasmi 或树内声明式 DSL**：无编译延迟、内存小；如果规则能表达成纯数据（TOML/JSON 谓词），连沙箱都不用——先设计 DSL 表达力上限，把需要数据流的规则留在 Rust 引擎里（见 §5 Semgrep 教训）。
3. **JS 前端 = rquickjs（可选）**：若要接 Semgrep 类用户的 JS 习惯/npm 生态，用 rquickjs 作为「编译到 WIT ABI」的前端；boa 只在「拒绝一切 C 依赖」时选；deno_core 在本项目的体积/攻击面预算下不吸收。
4. **不要混用两种运行时长期并存**：先定 ABI（WIT），运行时可替换（wasmtime/wasmi 都消费同一 wasm 组件），JS 只进前端不进 ABI。

### ② Windows 进程级隔离最小可行方案

- **MVP（一档）**：broker/worker 进程池——`预启动 N 个 worker + 匿名管道/stdio RPC`。每个 worker：**full AppContainer**（用 rappct，含 profile 创建、capability 声明）+ **Job Object**（kill-on-close、内存/CPU 上限）+ **受限 token**（低完整性、剥 privilege）+ **alternate desktop**（防 UI/窗口消息面）。文件访问按「broker 收到请求 → 临时 ACL 授 worker 读 → 用完撤销」或干脆把文件内容经管道传入。
- **坑清单（AppContainer 实际可编程性）**：
  - LPAC（Less Privileged AppContainer）设置 token 属性需要 SeTcbPrivilege 级别的权限——MVP 用 full AppContainer，LPAC 留作硬化二档；
  - 管理员账户下进程不一定落在预期沙箱档（Windows Internals 记载的 gotcha，待在 Win11 24H2+ 实测）；
  - AppContainer 隔离文件/注册表/网络，**不隔离设备**（摄像头/麦克风）——扫描器无 GUI/媒体需求，可接受；
  - worker 内每个要用的 Win32 API/文件/Socket 都需要显式 capability，最小集要逐个试（Microsoft Learn「Launch an AppContainer」2025-09 版为权威步骤）；
  - COM/命名管道等对象的 DACL 要加 ALL APPLICATION PACKAGES / LPAC SID，否则 broker↔worker 通信打不通。
- **模板来源**：Chromium sandbox 设计文档（broker/target、Job Object、restricted token、alternate desktop 四件套）；fastrender 的 windows_sandbox.md 是一份可直接抄架构的现成笔记；Trail of Bits AppJailLauncher-rs 提供了 Rust 侧把不可信程序关进 AppContainer 的工程化起点。
- **Linux 档**：Landlock（无需 root、自限，内核 5.13+）+ seccomp（extrasafe 或 seccompiler），双见于 §3。
- **跨平台抽象**：统一「策略 JSON + worker 协议（请求=文件+策略，响应=JSON 结果）」，Windows/Linux 各自实现后端；MCP 服务器主进程永不直接解析最深不可信输入（见 §6）。

### ③ 扫描器攻击面收敛清单

**必须进沙箱/隔离的输入路径：**
| 输入路径 | 隔离档位 | 兜底 |
|---|---|---|
| 不可信源码文件解析（tree-sitter/词法/正则） | 档1（wasm 内不可行→进程内+panic 边界+资源上限） | fuzz 长跑（CI corpus + 结构化变异） |
| 第三方规则/插件（wasm/JS/DSL） | 档2（wasm 沙箱：fuel/epoch + host 白名单） | guest ABI 差分测试 |
| 解包/解压缩（zip bomb、深路径、Windows 保留名） | 档1+限制器（条目数/解压比/总字节上限） | fuzz 压缩格式；已有路径逃逸探测复用 |
| 触发外部命令的规则动作（运行检测脚本等） | 档3（worker 进程池：AppContainer / Landlock+seccomp） | 命令白名单 + dry-run |
| MCP 工具自身参数（路径/命令） | 档0（进程内 fail-closed 校验，沿用现有沙箱纪律） | input fuzz（已有）+ 攻击面巡航 |
| 凭据/密钥驻留（扫描中读到的 secret、API token） | 不隔离，卫生件处理（§5） | 掩码输出 + zeroize |

**不需要沙箱**：纯内存 IR 计算、结果聚合、报告生成（只消费已验证的内部结构）。

**档位语义**：档0=进程内校验；档1=进程内+资源限制+fuzz 兜底；档2=引擎内沙箱（wasm）；档3=OS 级进程隔离；档4=VM/microVM（仅发布环境可选，本项目暂不吸收 Firecracker 本体）。

---

## 1. Wasm 沙箱层

### 1.1 wasmtime
- **一行定位**：Bytecode Alliance 的参考 Wasm 运行时，Cranelift JIT + WASI + Component Model，插件沙箱的事实标准。
- **值得抄的机制**：
  1. **pooling allocator + CoW 实例化**：内存/表/栈预池化复用，实例化快且可并行——worker 池与插件池都适用；注意它历史上有过池间内存泄漏缺陷（RUSTSEC-2022-0076；检索结果称 2026-04 公告中再现池实例内存泄漏 CVE-2026-34988，待验证），启用时须锁定上限配置并跟公告。
  2. **fuel / epoch interruption**：对不可信 guest 的 CPU 抢占双通道，规则插件超时杀是硬需求。
  3. **成文的安全政策与披露流程**（docs.wasmtime.dev/security.html + Bytecode Alliance 公告文）：升级节奏、公告订阅、影响面声明格式都值得照抄成我们自己的「沙箱公告」制度。
- **档位【吸收】**：插件/扩展 ABI 的主引擎；安全记录显示它仍有沙箱逃逸 CVE（2026-04 aarch64 Cranelift 误编译 CVE-2026-34971，CVSS 9.0 口径），但披露-修复流程健康——引用为「跟版本节奏 + host 侧纵深」的理由而非「零缺陷」。
- **第一步动作**：在重写骨架里加 `wasmtime` demo：WIT 定义一个 `rule-check` 接口，fuel=有限值跑一个恶意死循环插件验证可抢占；锁 pooling allocator 配置进默认档。
- **许可证**：Apache-2.0 WITH LLVM-exception。
- **链接**：https://docs.wasmtime.dev/security.html ｜ https://bytecodealliance.org/articles/wasmtime-security-advisories ｜ https://docs.wasmtime.dev/api/wasmtime/struct.PoolingAllocationConfig.html

### 1.2 wasmi
- **一行定位**：Rust 实现的 Wasm 解释器，v0.32 起换寄存器机引擎，定位嵌入式/插件场景（无 JIT 约束）。
- **值得抄的机制**：
  1. **解释器即回退档**：wasmtime 不能用/不想开 JIT 的环境（加固的 CI、aarch64 Win 早期误编译窗口期）用同一 wasm 组件换解释器跑——双运行时消费同一 ABI 是它最大的价值。
  2. wasmi-labs 基准仓库：公开 execution/compilation 基准方法，可直接抄成我们「选型判据走真路径」的测量脚本。
- **档位【吸收（作回退与轻量档）】**：吞吐低于 JIT（具体倍数随负载，不写死），但启动/内存/移植三轴占优；Substrate 系长期在用是生态锚。
- **第一步动作**：写 wasmi/wasmtime 双后端 trait（`SandboxEngine`），用同一 `.wasm` 插件测启动 µs 与规则延迟差，数字进 CLI-SPEED 同款记录表。
- **许可证**：MIT OR Apache-2.0（待验证 crate 当前 LICENSE 文本）。
- **链接**：https://wasmi-labs.github.io/blog/posts/wasmi-v0.32/ ｜ https://github.com/wasmi-labs/wasmi-benchmarks

### 1.3 WASI Preview 2 / 0.3（生态现状）
- **一行定位**：P2（2024）稳定了 Component Model + WIT + 能力化 I/O（fs/clocks/random/sockets/cli）；WASI 0.3 于 2026-06-11 正式发布，async（future/stream）成为组件模型原语。
- **值得抄的机制**：
  1. **能力即参数**：文件系统访问靠 preopen 句柄授予、无环境权威——这正是我们策略模型（§5）在 wasm 侧的现成实现，不必自造。
  2. **WIT 作为插件 ABI**：类型化、多语言、版本化，是「规则/插件生态」的接口契约层。
- **档位【有界（ABI 层吸收，0.3 特性缓跟）】**：P2 稳定可用；0.3 原生 async 在 wasmtime 37+ 还是 preview 质量且有 0.2 兼容性迁移噪音（社区反馈），插件 ABI 首版锁 P2 语义。
- **第一步动作**：插件接口用 `wasm-tools` 生成绑定；依赖里固定 wasmtime 大版本并写「跟进 WASI 0.3 的升级判据」（async 稳定 + 全平台 CI 绿）。
- **许可证**：规范开放（W3C/WG 流程）。
- **链接**：https://wasi.dev/roadmap ｜ https://bytecodealliance.org/articles/WASI-0.3 ｜ https://component-model.bytecodealliance.org/reference/faq.html

## 2. 插件框架与 JS 规则运行时

### 2.1 extism（Dylibso）
- **一行定位**：基于 wasmtime 的多语言插件框架，PDK 覆盖主流语言，host 函数是唯一出口。
- **值得抄的机制**：
  1. **「host 函数=安全边界」的显式化**：官方口径承认沙箱外一切取决于 host 函数实现——我们照抄：每个 host 函数必须注册进白名单并挂能力检查，不存在「顺手加一个」。
  2. **host 函数一次构建、全插件共享**（Owncast 的 Go 实现模式）：降低每插件开销与实现漂移。
  3. PDK 的「宿主↔插件内存约定」（malloc/offset 协议）：我们要么直接用 extism，要么自建 ABI 时抄这套内存约定。
- **档位【有界】**：引入整框架省时间但带来 Dylibso 系 API 面；若插件生态是我们核心资产，自建 WIT ABI + 只抄其内存/PDK 设计更可控。
- **第一步动作**：读 extism Rust host SDK 源码的 host-function 注册与权限位实现，摘成蓝图里的「host 能力表」一节，再决定引依赖还是自建。
- **许可证**：MIT（待验证各仓库 LICENSE）。
- **链接**：https://extism.org/ ｜ https://thenewstack.io/extending-nginx-agent-with-webassembly/（生态实例）

### 2.2 rquickjs
- **一行定位**：QuickJS 的高质量 Rust 绑定，ES2020，小而全。
- **值得抄的机制**：JS→WIT 编译路径的前端（golemcloud/wasm-rquickjs 给了「把 JS 包成 wasm 组件」的现成样例）；Runtime/Context 双层对象隔离模型。
- **档位【有界（仅前端）】**：C 依赖（QuickJS）增加构建/审计面；QuickJS 的沙箱边界弱于 wasm（无内存隔离强保证，靠引擎自身 GC 隔离 + 我们的资源限制）。
- **第一步动作**：验证 wasm-rquickjs 能否把一条 JS 规则编译成 P2 组件；能则 JS 进前端白名单，不能则 JS 规则降级为「进程内受限 rquickjs + 无任何 host 能力」。
- **许可证**：MIT（rquickjs；QuickJS 本体 MIT，作者 Bellard）。
- **链接**：https://crates.io/crates/rquickjs ｜ https://github.com/golemcloud/wasm-rquickjs

### 2.3 boa
- **一行定位**：纯 Rust 实现的 JS 引擎，0.19 自报 ~94% ECMAScript 合规（引擎自测口径）。
- **值得抄的机制**：纯 Rust=无 C 供应链；其合规度测试方法可复用。
- **档位【不吸收（现状）】**：三者中最慢（dev.to 2025 引擎基准与社区口径一致），合规未满，作为规则执行器性能不够、作为安全边界又弱于 wasm——除非未来「零 C 依赖」成为硬门槛再回看。
- **第一步动作**：无（在选型表里保留记录与复核日期）。
- **许可证**：MIT OR Apache-2.0。
- **链接**：https://github.com/boa-dev/boa ｜ https://dev.to/ahaoboy/js-engine-benchmark-2025-7-8-163b

### 2.4 deno_core
- **一行定位**：Deno 的 Rust 嵌入层（Rusty V8 + ops 机制），性能与合规顶格。
- **值得抄的机制**：**ops 权限模型**（JS 侧调用原生函数需注册且可按 Deno.permission 风格门控）——这是「JS 规则 + 细粒度宿主能力」的成熟参考实现。
- **档位【不吸收】**：V8 体积与构建复杂度对单二进制扫描器不可接受；本项目 JS 需求（若有）由 rquickjs 前端覆盖。
- **第一步动作**：无；把 ops 权限模型思想记入 §5 策略模型。
- **许可证**：MIT。
- **链接**：https://deno.com/blog/open-source ｜ https://stackoverflow.com/questions/79486773/

## 3. OS 级进程隔离

### 3.1 Windows：AppContainer / LPAC / rappct / AppJailLauncher-rs
- **一行定位**：Windows 内核级沙箱原语（Lowbox token + capability SID + DACL），Chrome/Edge 生产在用；rappct 把 API 打包成 Rust 库。
- **值得抄的机制**：
  1. **「capability = 显式声明清单」**：worker 启动时带 capability 列表（文件读/网络 deny 等），天然映射我们的策略 JSON。
  2. Microsoft Learn「Launch an AppContainer」（2025-09 更新）给出的启动步骤序列：profile 创建 → token 派生 → capability 附加 → CreateProcess——照抄成我们 worker 启动器的 checklist。
  3. Trail of Bits 的实践文：把「微软自家都没沙箱化的组件」关进 AppContainer 的完整过程，含最小 capability 集调试法。
- **档位【吸收】**：Windows 档进程隔离的基座；坑见 §0-②（LPAC 权限、设备不隔离、管理员账户行为、DACL 追加）。
- **第一步动作**：用 rappct 写一个最小 worker：AppContainer 进程内尝试读本仓库目录（应拒）与管道回写结果（应通），两条判据都过才算 MVP 门。
- **许可证**：rappct 与 AppJailLauncher-rs 的 crate LICENSE 待验证（均为开源项目）；系统 API 无许可问题。
- **链接**：https://learn.microsoft.com/en-us/windows/win32/secauthsecurity/launching-an-appcontainer（以站内实际标题为准）｜ https://crates.io/crates/rappct ｜ https://blog.trailofbits.com（"Microsoft didn't sandbox Windows Defender" 一文）｜ https://github.com/WildByDesign/AppContainer-Launcher

### 3.2 Chromium sandbox 模式（broker/target + Job Object + restricted token + alternate desktop）
- **一行定位**：浏览器级进程沙箱的祖师爷设计，文档即蓝图。
- **值得抄的机制**：
  1. **broker 永不处理不可信数据**：只做策略裁决与 IPC 转发——映射到我们：MCP 主进程（broker）只路由，解析/规则在 worker。
  2. **策略即 broker 侧可编程接口**（policy object 决定 target 限制）——我们的策略 JSON 就照这个语义建。
  3. **Job Object 四用途**：kill-on-close、内存/CPU 配额、UI 限制、alternate desktop 绑定——Windows worker 的最低配置。
- **档位【吸收（架构模式）】**：抄思想与结构，不引 Chromium 代码（BSD-3，允许引用但实现按需）。
- **第一步动作**：蓝图里画 broker/worker 时序图（预启动、健康检查、超时回收、策略下发），每一步标注 Windows/Linux 对应原语。
- **许可证**：Chromium BSD-3（参考文档不构成依赖）。
- **链接**：https://chromium.googlesource.com/chromium/src/+/HEAD/docs/design/sandbox.md ｜ https://github.com/fastrender 的 windows_sandbox.md（现成架构笔记）

### 3.3 Linux：Landlock（rust-landlock 及生态）
- **一行定位**：内核 5.13+（2021 合入）的无特权自限 LSM，进程对自己下路径级 fs 访问规则，无需 root。
- **值得抄的机制**：
  1. **自限而非他限**：worker 启动后第一件事锁死自己的可访问路径集——与 AppContainer 语义对齐，跨平台策略 JSON 可直译。
  2. 生态封装：leucite（rust-landlock 包装 + 命令执行限制）、hakoniwa（namespace+rlimit+cgroups+Landlock+seccomp 一体）、cargo-cage（给 Cargo build script 上 Landlock）——抄它们的规则构造与降级策略（老内核警告而非崩溃）。
- **档位【吸收】**：Linux 档（CI/发布）的基座；注意内核 5.13+ 下限（老内核如树莓派 stock kernel 没有），必须写成「探测→降级→如实上报」而非假装沙箱生效（对应我们「绿=SKIP 不算绿」纪律）。
- **第一步动作**：CI 里加 Landlock 门：worker 只许读目标目录，跑一次越读探测断言被拒；探测不可用时 CI 显黄。
- **许可证**：rust-landlock：MIT OR Apache-2.0（待验证）；leucite/hakoniwa 各自 LICENSE 待验证。
- **链接**：https://landlock.io ｜ https://github.com/basalt-rs/leucite ｜ https://lib.rs/crates/hakoniwa

### 3.4 Linux：seccompiler / extrasafe
- **一行定位**：seccompiler=Firecracker 的纯 Rust seccomp-BPF 编译器（JSON 规则→BPF 安装）；extrasafe=更高层 API，seccomp+Landlock 一起给。
- **值得抄的机制**：1) **JSON→过滤器的编译期产物化**（过滤器文件可审计、可 diff、可锁哈希）；2) extrasafe 的「按 syscalls 组声明」API 比裸 BPF 更适合我们这种「白名单即是策略」的需求。
- **档位【有界】**：worker 内双层防御的第二层（Landlock 管路径，seccomp 管 syscall 面）；seccompiler 直引（Apache-2.0），extrasafe 评估其维护活跃度后再定（待验证）。
- **第一步动作**：为 worker 生成一份最小 syscall 白名单 JSON（read/write/exit/pipe 类），锁进仓库并在 CI 校验哈希。
- **许可证**：seccompiler Apache-2.0；extrasafe 待验证。
- **链接**：https://github.com/firecracker-microvm/firecracker/blob/main/docs/seccompiler.md ｜ https://docs.rs/seccompiler

### 3.5 Linux：Google sandbox2 / bubblewrap
- **一行定位**：sandbox2=Google Sandboxed API 的 C++ 全进程沙箱（namespace+seccomp 策略引擎）；bubblewrap=独立的 namespace+seccomp CLI 工具（libseccomp）。
- **值得抄的机制**：sandbox2 的「policy 即 C++ 对象 + 违规即 SIGSYS 审计」心智模型；bubblewrap 的「外部工具沙箱」模式适合 CI 脚本而非进程内集成。
- **档位【不吸收（本体）/有界（思想）】**：C++ 依赖与外部工具都进不了 Rust 单二进制；CI 的发布环境可用 bubblewrap 作 belt-and-suspenders（可选项，不是依赖）。
- **第一步动作**：无强制项；CI 可选任务里留 bubblewrap 档。
- **许可证**：sandbox2 Apache-2.0；bubblewrap LGPL（版本待验证）。
- **链接**：https://developers.google.com/code-sandboxing ｜ https://github.com/containers/bubblewrap

## 4. 策略模型：capability-based 的工程化

- **一行定位**：把「能力=不可伪造的句柄/清单项」落成三张表：路径白名单、命令白名单、wasm import 白名单。
- **值得抄的机制**：
  1. **WASI 的 preopen 模式**：能力在启动时以句柄形式授予，运行期无「环境权威」可绕——策略 JSON 的每个条目对应一个启动期授予动作，杜绝运行期动态提权。
  2. **deno ops + extism host 函数共同点**：所有跨边界调用显式注册、按能力门控——我们的统一句法：`Capability::{ReadPath, WritePath, RunCommand, NetHost, WasmImport}`，deny-by-default，提权必须显式授权标记（对齐现有 fs 沙箱 `__authorized` 模式）。
  3. **Semgrep DSL 的表达力教训**：模式匹配规则只能表达句法层策略，数据流规则需要 taint/CodeQL 级引擎——所以我们的规则 DSL 定位「句法谓词 + 白名单动作」，数据流分析留在 Rust 引擎内部，不开放给沙箱内脚本。这既是能力模型也是攻击面收敛（规则作者拿不到图遍历宿主）。
- **档位【吸收（自建，薄层）】**：三张表 + deny-by-default + 显式提权标记 + 每条规则声明所需能力集（规则清单里写 `requires: ["read:target"]`，worker 启动前校验）。
- **第一步动作**：起草 `policy.schema.json`（v0：paths read/write、commands、net、wasm imports、资源上限五节），跨 Windows/Linux 语义对齐写进蓝图。
- **许可证**：思想层，无依赖。

## 5. 卫生件：密钥/敏感数据内存处理

- **一行定位**：zeroize（drop 即清零）+ secrecy（类型层防误打日志）+ mlock/VirtualLock（防换出），三层各管一件事。
- **值得抄的机制**：
  1. **secrecy 的 `ExposeSecret` 模式**：SecretBox 包裹 + 显式暴露 trait——API token、扫描到的凭据一律走此类型，`Debug`/序列化默认掩码（我们已有掩码输出纪律，这里补类型层）。
  2. **zeroize 的 no_std 与零依赖**：全库可用，无供应链负担（1.8.x）。
  3. **mlock 档位克制**：mlock/VirtualLock（memguard/secstr/shrouded 一族）只在「长期驻留的密钥」场景开——扫描器大多是短生命周期读取，清零+防日志已覆盖大头；锁页有配额上限且增加运维面。
- **档位【吸收（zeroize+secrecy）/有界（mlock 族）】**。
- **第一步动作**：全局枚举「敏感数据流」（MCP token、规则源里的密钥、扫描命中的 secret），标出各自经过的内存点，给零清零与掩码判据。
- **许可证**：zeroize / secrecy：Apache-2.0 OR MIT（RustSec 维护）。mlock 族：secstr（待验证）、shrouded（2026-03 crates.io 条目，待验证维护度）。
- **链接**：https://docs.rs/zeroize ｜ https://crates.io/crates/secrecy ｜ https://crates.io/crates/shrouded

---

## 6. 攻击面收敛与 fuzz 组合（扫描器自身）

- **原则**：沙箱挡「执行面」，fuzz 挡「解析面」，两者不是替代关系。
- **组合清单**（每行=输入路径 → 主防线 → 兜底）：
  1. 源文件解析器 → 进程内资源上限 + panic 边界 → fuzz 长跑（CI + 语料沉淀，对齐已有 03-fuzz 文档）；
  2. 规则/插件 wasm → 档2 沙箱（fuel/epoch/host 白名单）→ guest ABI 差分 fuzz；
  3. 压缩包/嵌套文件 → 限制器（解压比、条目数、总字节、深度）→ 格式 fuzz；
  4. MCP 参数 → 档0 fail-closed 校验 → 已有 input fuzz + 攻击面巡航复用；
  5. worker 协议 → 版本化帧 + 大小上限 + 超时回收 → 协议模糊。
- **Windows 特有条目**：设备文件名/保留名（wasmtime 曾有 RUSTSEC-2024-0438 未完全沙箱 Windows 设备文件名的记录，约 2025 修复——警示：路径规范化在 Windows 上必须有自己的单测集，不能指望底仓）。

---

## 7. 2025–2026 前沿信号

1. **WASI 0.3 发布（2026-06-11）**：async（future/stream）进入组件模型；wasmtime 37+ 提供预览；wasmCloud 已端到端跑通但自称 preview 质量。信号：插件 ABI 锁 P2、留 0.3 升级判据。（bytecodealliance.org/articles/WASI-0.3）
2. **wasmtime 2026-04 安全公告**：CVE-2026-34971（Critical 9.0 口径，aarch64 Cranelift 误编译→guest 读写任意宿主内存的沙箱逃逸）、池化实例内存泄漏 CVE-2026-34988、WASI panic DoS CVE-2026-27572（后两者细节待验证）。信号：JIT 误编译类逃逸仍在出现，且 aarch64（含 Windows on ARM）是热点平台——wasmtime 必须锁版本+订阅公告，worker 池提供进程级第二层。（bytecodealliance.org/articles/wasmtime-security-advisories）
3. **Windows 特有缺陷记录**：RUSTSEC-2024-0438（wasmtime 对 Windows 设备文件名沙箱不全，~2025 修复）。信号：Windows 路径语义是沙箱供应链的薄弱点，需自有测试集。
4. **AppContainer 工程化升温**：Microsoft Learn「Launch an AppContainer」2025-09 更新；rappct、Trail of Bits AppJailLauncher-rs 等出现，AppContainer 从「只有浏览器在用」转向「可被普通 Rust 项目消费」。
5. **Chrome 沙箱逃逸研究（2025-07 STAR Labs「Chrome-atic Escape」、Theori broker UAF）**：Job Object/Untrusted integrity 也有绕过研究。信号：档3 不是终态，发布环境保留档4 可选。
6. **eBPF 强化 Wasm host 调用**（2025 学术工作，针对 wasmtime/WasmEdge 把 host-call 检查下沉 eBPF）：思想可借——host 函数的检查与实现分离。
7. **Rust 沙箱指南类内容成潮**（OneUptime 2026-01 seccomp/Landlock 指南；extrasafe；hakoniwa；cargo-cage）：Landlock+seccomp 的 Rust 组合已从「手写 syscall 表」进入「现成 crate」阶段。
8. **JS 引擎格局**：2025 基准显示 QuickJS 系/boa 与 V8 差距仍大；rquickjs+组件模型封装（wasm-rquickjs）出现「JS→wasm 组件」路径。信号：JS 规则若要做，走编译进 wasm 而不是裸嵌引擎。

## 8. Top-3

1. **插件 ABI 定 WIT（P2 语义）+ 双运行时**：wasmtime（pooling+fuel/epoch+host 白名单）为主，wasmi 为回退/轻量档，JS（rquickjs）只作编译到同一 ABI 的前端；host 函数=唯一能力出口，一次构建全插件共享。
2. **Windows 进程隔离 MVP = broker/worker 池四件套**（full AppContainer[rappct] + Job Object kill-on-close + 受限 token + alternate desktop），Linux = Landlock+seccomp 同构实现；统一策略 JSON（deny-by-default + 显式提权标记）；LPAC 与档4(VM) 留硬化二档。
3. **攻击面分档表进蓝图**：解析面靠 fuzz 兜底、执行面靠沙箱、能力面靠三张白名单表；Windows 路径语义（保留名/设备名）设专项单测；敏感数据 zeroize+secrecy 类型化，mlock 仅限长期驻留密钥。

---

*检索时间 2026-10-03；标「待验证」处（wasmi/secrecy/extism 等 LICENSE 文本、CVE-2026-34988/27572 细节、extrasafe/bubblewrap 许可证版本、AppContainer 在管理员账户行为）在进入实现阶段时需逐条源码/官方文档复核。*
