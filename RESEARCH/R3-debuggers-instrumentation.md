# R3 — 调试器与动态插桩（深度调研）

- 域：R3（纲领见 PROGRAM.md）；服务两条产品线：**adv-bin**（逆向/二进制专线）与 **test-replay**（测试/录制回放，需动态执行）。
- 日期：2026-10-03；资料口径：优先 2025–2026，二手/单源结论均已标注「待复核」。
- 方法：WebSearch/WebFetch 定向检索；机制描述尽量锚到官方文档/仓库/论文；数字带锚（版本、日期、来源）。
- 本文与 R2（二进制/符号执行）只在其「动态交界处」补记（PANDA/Unicorn/Qiling/angr-Unicorn 后端），不重复 R2 已覆盖的静态能力。
- 每对象格式：**定位 → 可抄机制 → 档位【吸收|有界|不吸收|watch|reference】→ 第一步动作 → 许可证/成熟度 → 链接**。

---

## 0. 总览表（28 个对象）

| id | 对象 | 类 | 档位 | integration | 许可 | 成熟度 |
|---|---|---|---|---|---|---|
| R3-001 | GDB（含 RSP/MI/Python） | 调试器 | 吸收(协议/脚本) | adv-bin | GPL-3.0 | 维护中 |
| R3-002 | LLDB / lldb-dap | 调试器+协议 | 吸收 | adv-bin, test-replay | Apache-2.0(LLVM 例外) | 维护中 |
| R3-003 | x64dbg | 调试器 | 有界 | adv-bin | GPL-3.0 | 维护中 |
| R3-004 | WinDbg TTD | 录制回放 | 吸收(思想)+有界(工具编排) | test-replay | 专有 | 维护中 |
| R3-005 | rr | 录制回放 | 吸收 | test-replay | BSD-2 | 维护中 |
| R3-006 | Undo LiveRecorder | 录制回放 | reference(思想) | watch | 专有 | 维护中 |
| R3-007 | PANDA | 全系统仿真+回放 | 有界 | adv-sandbox | GPL-2.0 | 维护中 |
| R3-008 | Frida（Interceptor/Stalker） | DBI/注入 | 吸收(思想)+有界(集成) | adv-bin | LGPL-2.1/wxWindows | 维护中 |
| R3-009 | DynamoRIO | DBI | 有界 | adv-bin | BSD 风格 | 维护中 |
| R3-010 | Intel PIN | DBI | reference | watch | 专有 | 维护中(收缩信号待复核) |
| R3-011 | QBDI | DBI | watch | watch | Apache-2.0 | 维护中 |
| R3-012 | Microsoft Detours | hooking | 吸收 | adv-bin | MIT | 维护中 |
| R3-013 | MinHook / minhook-detours | hooking | 吸收 | adv-bin | BSD-2 | 维护中 |
| R3-014 | VEH + 硬件断点 hooking 机制 | hooking 机制 | 有界 | adv-bin | 机制(实现另计) | 实验(生态) |
| R3-015 | eBPF Linux（aya/bpftrace/bcc） | 内核观测 | 吸收(Linux 侧) | perf-core, watch | GPL-2.0/Apache-2.0/MIT | 维护中 |
| R3-016 | eBPF for Windows | 内核观测 | watch | watch | MIT | 实验 |
| R3-017 | DTrace（含 Windows 端口） | 内核观测 | watch | watch | CDDL-1.0 | 维护中 |
| R3-018 | ETW + ferrisetw | OS 事件追踪 | 吸收 | perf-core, adv-bin | OS API / MIT+Apache-2.0 | 维护中 |
| R3-019 | Linux perf events | OS 计数/采样 | 有界 | watch | GPL-2.0 | 维护中 |
| R3-020 | Intel PT（libipt/WinIPT/ipt.sys） | 硬件追踪 | 有界(观测)/watch(集成) | watch | BSD-3/MIT | 维护中(WinIPT 弃维护) |
| R3-021 | Unicorn | 用户态仿真 | 有界 | adv-sandbox | GPL-2.0 | 维护中 |
| R3-022 | Qiling | OS 感知仿真 | 有界 | adv-sandbox | GPL-2.0 | 维护中 |
| R3-023 | strace（syscall 跟踪范式） | 系统调用观测 | 有界(范式) | test-replay | LGPL-2.1+ | 维护中 |
| R3-024 | WPR / WPA / wpaexporter | Windows 性能记录 | 吸收(管线) | perf-core | 专有(免费组件) | 维护中 |
| R3-025 | gimli / addr2line | DWARF 消费 | 吸收 | adv-bin | MIT/Apache-2.0 | 维护中 |
| R3-026 | probe-rs | 嵌入式调试 | watch | watch | MIT/Apache-2.0 | 维护中 |
| R3-027 | DAP（调试适配器协议）+ MCP 桥 | 自动化协议 | 吸收 | test-replay, adv-bin | MIT(规范) | 维护中 |
| R3-028 | DbgEng / WinDbg 自动化 | Windows 调试引擎 | 有界 | adv-bin | 专有 | 维护中 |

---

## 1. 重点回答

### ① TTD / 录制回放机制拆解（对 test-replay 与 bug 复现的价值、Windows 可达性）

**两段式机制（官方口径）**：录制端把进程执行固化为 trace（`.run`），停止录制时生成索引 `.idx`（检索称约为 trace 的 2 倍大小）以加速随机访问；重放端在 WinDbg 中「回放」并允许**双向导航**：`g-`/`p-`/`t-` 反向执行、`!tt [position]` 跳到任意时间点（支持百分比或地址:时间形式）。重放并非重新执行原进程——WinDbg 依据 trace **恢复任意时间点的寄存器与内存状态**（Microsoft Learn, 检索于 2026-10）。

**录制通道与工程约束**：
- 三种录制方式：WinDbg UI、命令行 `TTD.exe`、以及测试自动化场景；`TTD.exe` 支持 `-attach` / `-launch` / `-monitor`（监控当前及未来实例）与 `-children`（连带子进程）——**这是 CI/自动化接入点**（aka.ms/ttd/download）。
- 约束（官方口径，2025 更新）：需管理员权限；仅用户态；**attach 后不可 detach**（系统关键进程需重启）；trace 体积可分钟级到 GB 且无上限；trace 含内存内容（PII 风险）→ 采集与留存需脱敏策略。
- Windows Server 2016/2019/2022/2025 支持 TTD.exe（检索口径，待复核版本矩阵）。
- 开销：官方称 5x–20x 或更高，依应用与录制选项（未经本地实测，仅官方口径）。
- 重放是**指令模拟**：2025 Google Cloud Mandiant 博客记录了 TTD 模拟与真实 CPU 的语义差异（如 `pop r16`、`push segment` 在 Intel/AMD 上行为不一致），并指出恶意软件可检测 TTD 环境 —— 对「录制结果的可信度」与「对抗场景」都要计入。

**对 ADV 的价值**：
1. **bug 复现**：把一次崩溃/flaky 会话固化成 `.run` 资产，任意回退定位根因（官方典型场景：UAF 回溯「谁 free 了它」）。test-replay 可把「录制一次 → 反复断言」当作回归资产。
2. **可分享性**：只分享 `.run`（不带 `.idx`）即可在另一台机器重放——便于「AI 会话 + 人在环」协作定位。
3. **可达性判断**：Windows 客户端/服务器可达，但**需提权**且**格式专有**（分析主要绑定 WinDbg）。⇒ 策略：**只抄思想（确定性事件日志 + 快照/切片回退），只做外部工具编排**（TTD.exe 录、WinDbg/`.run` 消费、结果做断言），**不解析 .run/.idx 格式**（专有 + 无文档保证）。
4. rr（Linux, BSD-2）与 Undo（专有）给出同族思想的**可复用最小集**：只记录非确定性事件（syscall 返回、信号时序、RDTSC/CPUID、共享内存读），其余按「CPU 天然确定」假定重放 —— 见 ②/⑤ 与 rr/Undo 条目。

### ② Windows 用户态插桩/注入的工程路径与合规边界

**技术路径分层（由易被检测到难，工程成本递增）**：

| 层 | 机制 | 关键约束 | 备注 |
|---|---|---|---|
| IAT/EAT patch | 改导入/导出表指针 | 需解析 PE；对静态绑定/直接系统调用无效 | 最易绕过，仅作能力演示 |
| Inline hook（Detours/MinHook） | 复制被改写指令到 trampoline，前 5 字节写 JMP（x64）；需指令长度解码与 PC 相对指令重定位 | 需线程挂起/安全性保障（Detours 有事务化 API 与 `DetourUpdateThread`）；MinHook 仅 x86/x64，**无 ARM64** | Detours=MIT、MinHook=BSD-2；Rust 绑定 `detours`、`minhook 0.7.1`（crates.io 口径，检索于 2026-10） |
| HWBP + VEH | `Get/SetThreadContext` 写 DR0–DR3 + DR7 控制位（执行型断点），VEH 接管异常；**不改内存代码** | **仅 4 个槽位**、线程级、需在目标线程上下文生效（新建线程需重新布设） | VEH 优先级高于 SEH；对完整性扫描/静态检查更友好；Rust 生态有 `veh-hooking-rs`（近月仍发版，单源待复核） |
| 进程外（调试器路径） | `DebugActiveProcess` + 调试事件循环，或 DbgEng 客户端；断点用 `WriteProcessMemory` 写 INT3 | 附加即被 `IsDebuggerPresent` 类检测看到（对抗场景另论） | x64dbg 的 TitanEngine 即此族的开源参照 |

**注入路径（与上面正交）**：`CreateRemoteThread`+`LoadLibrary`（经典、最易被拦）、`QueueUserAPC`（需目标线程 alertable）、`SetWindowsHookEx`（仅 GUI 线程/全局钩子在部分场景受限）、调试附加。对 ADV 的**自有进程/测试靶机**，优先「调试器附加（进程边界清晰、可审计）」而非隐蔽注入。

**合规边界（本地优先平台的工程红线，建议写进 ADV 能力说明）**：
- 只对**用户自有进程或显式授权的测试目标**启用插桩；默认拒绝 attach 非本产品/非白名单进程。
- **只做观测与测试断点**，不实现 EDR 规避：ScyllaHide/TitanHide 类（隐藏调试器/HWBP 痕迹、内核态 hook）**不吸收**——它们既触碰合规灰区，也与我们「可审计」的目标相反（ScyllaHide 作为 x64dbg 插件生态里最常被引用的「反-反调试」代表，仅 reference）。
- 记录 side effect：hook 安装/卸载、目标进程与调用方身份、时间戳，可审计、可回滚。

### ③ eBPF/ETW 在 Windows 的现状

- **aya 在 Windows 不可用**：aya（纯 Rust eBPF 库）与 Linux 内核 verifier/loader 绑定；社区实践是在 `Cargo.toml` 用 `cfg(target_os = "linux")` 门控（项目文档/依赖注释口径，检索于 2026-10）；2026-01-31 FOSDEM「What's new in Rust for eBPF」（M. Rostecki）列出的进展（集成测试、ring buffer 日志、`sk_storage` 等 map、TCX）**全部在 Linux 侧**。⇒ 若 ADV 要在 Windows 做内核/系统观测，**不能指望 aya**。
- **eBPF for Windows 是另一套生态**：Microsoft 主导的 ebpf-for-windows 复用 Clang eBPF 后端与 libbpf 思路，但 **verifier 用 PREVAIL、JIT 用 uBPF**（Linux 内核 GPL verifier 不可复用）；三条加载路径：`bpf2c` 原生程序生成 `.sys`（官方称最安全、首选）／uBPF JIT（eBPFSvc）／解释器（仅 debug）；**验收在用户态 secure environment** 完成而非内核内。仓库状态为 WIP（检索到的最近推送 ≈2025-05，单源待复核）。⇒ **watch**：不作为近期能力，关注其 API/驱动签名门槛。
- **DTrace 有 Windows 端口**：微软 2019 起发布 Windows 版 DTrace；检索口径称 **Windows Server 2025 已内置**（二手源，待复核）。DX 语言/sdt/fbt 探针思想与 Linux eBPF 同族，但生态远小于 ETW。⇒ watch。
- **ETW 是 Windows 上事实标准且可直接用**：机制面在 ④ 与 ETW 条目展开；要点：manifest 声明式 schema / TraceLogging 自描述事件；`StartTrace` 建会话 → `EnableTraceEx2` 开 provider → `OpenTrace/ProcessTrace` 实时消费（回调在独立线程）；字段解码走 **TDH**（`TdhGetEventInformation`/`TdhFormatProperty`，schema 有缓存）。Rust 消费端 **ferrisetw 1.2.0**（MIT/Apache-2.0，最近提交 2025-08-20，基于 windows-rs；自称 KrabsETW 的 Rust 化）可用，C++ 侧 KrabsETW 为参照。⇒ **吸收**（消费/管线层），无需自研驱动。

### ④ 给 adv-bin 与 test-replay 的「最小可用动态能力清单」

**做（近期，按优先级）**：
1. **DAP 会话驱动层（两者共用）**：以 `lldb-dap`（Apache-2.0 with LLVM exception）为第一适配器，包一层 MCP/DAP 客户端 → 断点、步进、变量/表达式求值、`stopped` 事件流可脚本化（机制见 ⑤）。这是「AI 会话驱动调试」的地基。
2. **Windows 进程观测三件套（adv-bin，仅自有/授权进程）**：ETW 消费（ferrisetw）+ ETL 离线管线（`wpr.exe` 录 → `wpaexporter` 按 `.wpaProfile` 导出 CSV，支持批量）+ HWBP/VEH 与 inline hook 的**自有实现或薄封装**（Detours/MinHook 都 MIT/BSD 级，可静态集成；ARM64 需求出现时看 minhook-detours）。
3. **地址→源码/符号服务（adv-bin）**：gimli + addr2line（zero-copy、lazy 解析、`find_frames` 内联栈）——与 R1/R2 的 DWARF 消费互补，纯 Rust、MIT/Apache-2.0。
4. **test-replay 最小「非确定性记录」内核（吸收 rr/Undo 思想，非代码移植）**：定义并实现事件日志最小集 —— 系统调用返回、时钟（含 RDTSC 语义层）、随机源、异步信号/线程调度点；记录边界=用户/内核边界。先做「记录→确定性重放断言」的本地闭环（Linux 上可直接对照 rr 行为）。
5. **Windows 回放资产编排（test-replay）**：把 `TTD.exe -launch/-attach` 纳入 CI 脚本，产物 `.run` 归档 + 用 WinDbg/脚本化查询做断言；**不解析格式**。

**只 watch / reference（明确不投入）**：
- eBPF for Windows（WIP + 需驱动签名/WDK 链）；DTrace-Windows；Intel PT 深度集成（WinIPT 最后推送 2023-04-27 = 弃维护信号；`ipt.sys` 自 Win10 1809 起为 in-box 驱动，但消费工具链薄，仅保留「WinAFL PT 模式」作为灵感）；Intel PIN（专有）；Undo（专有，只抄思想）；QBDI（移动优先）；PANDA（GPL-2 + 全系统仿真重，交 adv-sandbox 有界评估）；WPA MCP/ETW MCP 类新生态（实验期）。
- **不吸收**：EDR 规避技术（TitanHide/ScyllaHide 类）、`.run/.idx` 格式私有解析、自研内核态 hook 驱动。

### ⑤ 调试会话自动化（DAP/脚本化）对「AI 会话驱动调试」的可行性

**协议机制（可抄的骨架）**：DAP = JSON-RPC over stdio（也有 socket）的「编辑器-调试器」解耦协议：`initialize` 能力协商 → `launch`/`attach` → `setBreakpoints`（source/function/data 断点、条件/命中数）、`configurationDone` → 事件流 `stopped`/`continued`/`threads` → 请求 `stackTrace`/`scopes`/`variables`/`evaluate` → `disconnect`。适配器侧：`lldb-dap`（LLVM 官方，原 lldb-vscode）、CodeLLDB（VS Code 前端，可当 DAP server 被脚本驱动）、debugpy/Delve/netcoredbg 等。

**2025–2026 信号（AI 驱动已成生态）**：
- **MCP-DAP 桥**：`mcp-dap-server`（Go 系，13 个工具：launch/attach、断点、执行控制、状态检查、表达式求值，支持 source/binary/core dump/attach 模式）；`dap-mcp-server`（npm，Python/Go/C++/Node 适配，刻意**粗粒度工具**（如一次性 `debug_inspect`）以降低 agent 往返轮次，附 Claude Code 的 `SKILL.md`）；另有 12 工具版 DAP-MCP 提供 `full`/`readonly` 安全模式与细粒度权限（allowSpawn/allowAttach/allowModify/allowExecute）——**权限分级是必须抄的安全模式**。
- **IDE 内置 agentic debugging**：JetBrains IntelliJ IDEA 2026.1.3+ 暴露共享 MCP server（工具前缀 `xdebug_`），2026.2+ 提供 Router 模式（单一 `execute_tool` + `ij-debugger` 技能，降低 token 占用）——「工具面按需展开」与我们的渐进披露思路同构。
- **已知边界 bug（自动化需容错）**：LLVM 21.1.8 的 `lldb-dap` 在调试 Rust inline `#[test]` 时，`stopped` 事件线程 ID 可能不在初始 `threads` 响应里（nvim-dap #1577 口径）→ 客户端需「事件驱动 + 重试/重查」而非依赖一次快照。
- **Windows 注意**：`lldb-dap` 在 Windows 支持不佳（社区口径），Windows 侧优先 CodeLLDB 或 DbgEng 路线；`rust-lldb` 包装器不是 DAP adapter（2025-02 澄清，nvim-dap wiki 口径）。

**可行性结论**：可行且**已具工程路径**。LLDB(含 Rust 支持) + DAP 稳定 + MCP 桥生态 2025–2026 成型；ADV 可自建「DAP 客户端 + 粗粒度工具面 + readonly 默认 + 显式授权 attach」的三层：DAP 会话（能力）→ 工具面（token 预算化）→ 策略门（谁能 attach 什么）。风险端：调试权限=进程内存读写的完全能力，必须默认只读、白名单、全审计。

---

## 2. 对象逐条

### A. 调试器

#### R3-001 GDB（含 GDB RSP / MI / Python API）
- 定位：GPL 系通用调试器；其**接口面**是与第三方工具集成的长期事实标准。
- 可抄机制：① **远程串行协议（RSP）**：调试器与 stub 解耦（`g`/`m`/`Z0` 断点包、`vCont` 续行），任何新后端（虚拟机、仿真器、录放器）只要实现 RSP 即可复用现成前端——这正是 `gdbstub`（Rust）生态的支点；② **脚本/扩展面**：Python API 是「调试会自动化的外挂面」，GDB 16.1（2025 年线）新增 `gdb.missing_objfile` 处理器注册（core file 缺库场景）、`gdb.tui_enabled` 事件、`gdb.record.clear`，并把 **Intel PT 的 PTW payload 以 `RecordAuxiliary` 暴露给 Python**（可自定义 ptwrite 过滤）——即「硬件追踪数据 → 脚本消费」的通路；③ **JIT 接口**：为运行时生成代码注册符号/源码（`JIT_READER`），Rust/JS/V8 系工具依赖此面。GDB 侧 2025 还在推 Python JIT API（RFC v4/v5，可在 Python 中构造 `gdb.Objfile`/`Block`/`Symbol`）与 core file API（`gdb.Corefile`，2025-09 补丁线）。
- 档位：**吸收（协议与脚本面思想）**；GPL-3.0 代码不可进 ADV 二进制，但 RSP/DAP 语义与「脚本面」设计可学。
- 第一步动作：对照 RSP 包语义给 adv-bin 的「动态执行后端」定义最小接口（断点/内存/寄存器/续行），确保未来能挂 gdb 或自建前端。
- 许可证/成熟度：GPL-3.0-or-later / 维护中（16.1 2025）。
- 链接：https://sourceware.org/gdb/ ；https://sourceware.org/gdb/download/onlinedocs/

#### R3-002 LLDB / lldb-dap（Rust 支持现状）
- 定位：LLVM 系调试器；**Rust 调试的第一公民**（原生识别 Rust，`rust-lldb` 只是包装脚本，带 Rust pretty-printer）。
- 可抄机制：① `lldb-dap`（原 lldb-vscode）把 LLDB 能力完整映射为 DAP（JSON-RPC/stdin），是「无 GUI 调试会话」标准入口；② SB API 分层（Target/Process/Thread/Frame）适合做程序化驱动；③ pretty-printer/格式器加载链（`.lldbinit` + `command script import`）——dap 场景下把 Rust 数据格式器放 `.lldbinit` 即生效（StackOverflow 口径）。
- 档位：**吸收**（ADV 可直接集成 lldb 库或驱动 lldb-dap；Apache-2.0 with LLVM exception 与闭源兼容）。
- 第一步动作：在 test-replay/adv-bin 里做 `lldb-dap` 最小客户端（spawn → initialize → launch → setBreakpoints → stopped → stackTrace/variables），并测 Rust `#[test]` 场景的线程列表边界 bug（见 ⑤）。
- 许可证/成熟度：Apache-2.0 with LLVM exception / 维护中（2025 活跃：Zed 内置调试 2025-06 走 GDB/LLDB；MCP 桥井喷）。
- 链接：https://lldb.llvm.org/ ；https://lldb.llvm.org/use/vsconcepts.html

#### R3-003 x64dbg（Windows 用户态调试器实现）
- 定位：Windows 用户态开源调试器；**前端/后端分层**与插件 SDK 是工程样板。
- 可抄机制：① 三层架构：GUI（`src/gui`）— 调试引擎 DBG（`src/dbg`，把 OS 调试事件翻译成 UI 模型）— 底层引擎 TitanEngine（抽象 Windows Debugging API；`InitDebug`/`AttachDebugger`/`StepInto`/`StepOver`/`StopDebug`，含 Wow64 变通）——调试循环跑在专用 `hDebugLoopThread`；② 插件回调面：`cbStep`/`cbUserBreakpoint`/`cbMemoryBreakpoint`/`cbHardwareBreakpoint` 事件回调 + C 风格导出接口（DBG `_exports.cpp`），`.dp32/.dp64` 插件；③ 断点类型学：INT3/长 INT3/UD2、内存断点、硬件断点（QWORD 支持在社区 fork 补强）；④ 生态参照 ScyllaHide（反-反调试插件）——**只作为威胁模型输入，不作能力吸收**。GleeBug 为另一开源调试引擎参照。
- 档位：**有界**（架构与事件模型可抄；代码 GPL-3.0，仅思想；TitanEngine 部分许可不清，不碰）。
- 第一步动作：把「调试事件 → 内部模型 → 上层消费」三段分层写进 adv-bin 的调试模块设计（先不做 GUI）。
- 许可证/成熟度：x64dbg GPL-3.0 / 维护中。
- 链接：https://github.com/x64dbg/x64dbg

#### R3-004 WinDbg + Time Travel Debugging（TTD）
- 定位：Windows 官方录制回放（思想源：Nirvana）；对 **test-replay 极有价值**。
- 可抄机制：① 两段式录制/重放 + 索引加速（`.run`/`.idx`，idx≈2×trace 大小，检索口径）；② 时间线双向导航与位置寻址（`g-`/`p-`/`t-`、`!tt` 百分比/绝对定位）；③ 数据模型可查询（LINQ 式跨 trace 查询模块加载/异常——「把 trace 当库来查」是 test-replay 的理想交互）；④ 工程参数：5x–20x+ 开销（官方口径）、需管理员、仅用户态、attach 不可 detach、trace 可到 GB 且无上限、含 PII 内存内容；⑤ 重放=指令模拟：2025 Mandiant 博客记录模拟语义与真机差异（`pop r16`/`push segment` 等）与可检测性——**录制资产的「保真度」要实测标定**。
- 档位：**吸收（思想：事件日志+快照回退+可查询时间线）/ 有界（工具编排：TTD.exe 自动录制 + 结果断言）**；格式私有 ⇒ 不解析。
- 第一步动作：在 CI 靶机脚本里跑 `TTD.exe -launch/-attach [-children]` 录 `.run`，配「崩溃/断言的归档 + WinDbg 批处理询问」闭环，验证提权与体积上限的工程边界。
- 许可证/成熟度：专有（免费工具） / 维护中（Learn 文档 2025 仍更新；Server 2016–2025 支持口径待复核）。
- 链接：https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/time-travel-debugging-overview

#### R3-005 rr（Mozilla，记录回放）
- 定位：Linux 用户态确定性录制回放 + GDB 前端；**本域「最可抄的机制源」**。
- 可抄机制：① **记录边界=用户/内核接口**：只记非确定性输入（syscall 结果、异步事件时序、RDTSC（陷阱+模拟+记录）、cpuid（固定核）、vdso 改写、RTM/HLE 关闭等）——「CPU 天然确定，只记噪声」的哲学；② **perf_event 硬件计数器**（`PERF_COUNT_SW_CONTEXT_SWITCHES` + 引退条件分支 RCB 计数中断）+ **「中断迟到」补偿**：因计数器溢出中断晚触发数十条指令，重放时提前触发+临时断点精确定位；③ **syscall 缓冲/拦截库**（`syscall_buffer.c`/`preload.c`）：包装常用 syscall，输出重定向到 per-thread scratch 缓冲、直写 trace，减少上下文切换；文件块克隆（CoW 大块读）压缩记录；④ 单线程调度（ptrace 抢占式）消除多核竞态。论文：O'Callahan et al., *Engineering Record And Replay For Deployability*, arXiv:1705.05937。
- 档位：**吸收**（思想与结构直接用于 test-replay 的确定性内核设计；BSD-2 代码也可对照）。
- 第一步动作：写一页「ADV 录制边界定义」：列出本产品接受的非确定性事件清单（时钟/随机/IO/线程调度）与其记录格式，先在 Linux 原型上对照 rr 行为验证。
- 许可证/成熟度：BSD-2-Clause（以仓库为准） / 维护中。
- 链接：https://github.com/rr-debugger/rr ；https://arxiv.org/abs/1705.05937

#### R3-006 Undo LiveRecorder（专有思想源）
- 定位：商用「软件飞行记录仪」（Linux x86/x64，C/C++/Go/Java）；**思想富矿，代码不碰**。
- 可抄机制：① 仅记录非确定性（软件二进制翻译+JIT 插桩，无源码改动；事件日志用 **diff 式表示**压缩）；② 倒带实现=**快照+切片（slice）+nanny 进程**：向前跳快照、向后从最近切片重放，v6.8 起 **Parallel Search** 多核并行倒带（官方称最高 4x 加速）；③ 工程化选项：`--thread-fuzzing`（随机化线程调度，把偶发竞态变 100% 复现）、`--disable-aslr`、`--save-on error/exit-signal`（只留失败样本，省开销；官方口径 2x–5x 慢）。
- 档位：**reference（只抄思想）**；专有 ⇒ 不集成、不逆向。
- 第一步动作：把「thread-fuzzing 诱发竞态 + 失败样本留存」列为 test-replay 的实验特性清单（先文档、后原型）。
- 许可证/成熟度：专有 / 维护中（商业产品）。
- 链接：https://undo.io/

### B. 全系统动态仿真（与 R2 交界）

#### R3-007 PANDA（QEMU 全系统记录回放 + 插件）
- 定位：QEMU 之上的全系统分析平台；**录制回放 + 插件（osi/hooks2）**是动态面代表。
- 可抄机制：① 全系统 record/replay（块级重放 + 插件在回放中可重跑分析——「一次录、多遍分析」）；② pypanda（Python）/ **panda-rs 0.14（Rust 绑定，明确含 libpanda 驱动与「Functions for record and replay」）**——Rust 侧已有可用绑定的信号；③ 与 angr 等协作的分析编排（avatar2 类）——R2 已提，此处只记动态面。
- 档位：**有界**（GPL-2.0 + 全系统重量级：定位为 adv-sandbox 的可选后端，不做主力）。
- 第一步动作：评估 panda-rs 0.14 能否在 Windows 宿主/CI 构建与运行（QEMU 依赖、录制体积），产出一页可行性记录。
- 许可证/成熟度：GPL-2.0 / 维护中。
- 链接：https://github.com/panda-re/panda ；https://docs.rs/panda-re/0.14.0/panda/

### C. 动态二进制插桩（DBI）

#### R3-008 Frida（Stalker / Interceptor）
- 定位：跨平台动态插桩工具包（frida-gum 引擎）；**两大组件机制清晰、可直接对照**。
- 可抄机制：① **Interceptor**：函数级 hook = 重定向到 trampoline（attach 观察 onEnter/onLeave / replace 完全替换），2025–2026 演进出「可组合选项结构体（scratch 寄存器选择、在线/离线场景、重定位策略、替换数据、监听器数据）」并**在创建期获取 unwind 支持**（异常/回溯可穿过 trampoline）；② **Stalker**：基本块级**动态重编译**——为每个块写「插桩副本」（**不改原代码**，天然抗自校验），Relocator 修正重定位后的 PC 相对指令；控制流通过改写块尾分支回流引擎；优化含块哈希缓存（循环不重插）、**landing pad + `GumExecFrame` 侧栈**处理 CALL/RET、分支 **backpatching** 与 `Stalker.trustThreshold` 确定化；③ 性能工程：`Stalker.exclude()` 排除 libc 等（社区口径约 10x 提速）、follow 线程 10x–100x 慢（社区口径）、热路径禁 `console.log`（callout 累积后 dump）——**「排除+批量导出」是插桩成本控制的标准手法**；④ 2025-03 的 16.7 引入**线程/模块观察 API**（`attachThreadObserver`/模块 onAdded → 新线程/新库出现的瞬间装 Stalker/Interceptor）；2026-05 的 17.10 让 PC 翻译常开、ARM64 backend 不再要求近邻 slab。
- 档位：**吸收（机制）**；**有界（集成）**——LGPL/wxWindows 许可 + 注入场景合规（见 ② 红线：只对授权进程）。
- 第一步动作：以 Stalker 的「插桩副本+块缓存」为蓝本，写 adv-bin 的最小基本块跟踪原型（先 Linux/自有进程，coverage 用 `{compile:true}` 每块一次的语义）。
- 许可证/成熟度：LGPL-2.1 / wxWindows（gum 部分；以仓库 LICENSE 为准） / 维护中（16.7 2025-03、17.10 2026-05）。
- 链接：https://frida.re/docs/stalker/ ；https://frida.re/news/2025/03/13/frida-16-7-0-released/

#### R3-009 DynamoRIO
- 定位：Google 维护的开源 DBI 框架（源自 MIT 项目）；**Windows 侧最实用的开源 DBI**。
- 可抄机制：① JIT trace + code cache 的执行模型 + **client API**（进程内分析客户端与引擎分离）；② **drcov 覆盖率格式与 AFL++/WinAFL 的 DBI 后端**——「覆盖率 → 反馈式 fuzz」的标准接口（R3 与 R4 的接点）；③ 引擎可整仓改造（Dr. Memory 系工具证明其可承载完整工具链）。
- 档位：**有界**（BSD 风格许可可用，但引擎级集成成本高；优先使用其覆盖率/插桩服务而非嵌引擎）。
- 第一步动作：在 Windows CI 上跑通 drcov 采集 → 覆盖率可视化闭环（验证对目标二进制的实际覆盖质量）。
- 许可证/成熟度：BSD 风格（仓库 LICENSE.txt） / 维护中（v11.3.0 2025 活跃——二手源，待复核）。
- 链接：https://github.com/DynamoRIO/dynamorio

#### R3-010 Intel PIN
- 定位：Intel 专有（免费许可）DBI；学术界 DTA（libdft/LIFT/TRITON 等）的底座。
- 可抄机制：① JIT trace 代码缓存 + C++ 插桩 API 的**教学级完整度**（ManualExamples 是最全的 DBI 教程）；② 「指令级抽象 + 回调分类（IPOINT_BEFORE/AFTER）」的 API 设计——被后来者广泛参照；③ 生态事实：DTA/符号执行工具多以它为原型。
- 档位：**reference**（专有 + 生态惯性；不引入）。
- 第一步动作：无（读其 ManualExamples 目录作为 API 设计参考即可）。
- 许可证/成熟度：专有（免费下载） / 维护中但收缩信号（"closing update cycle"为二手源，待复核）。
- 链接：https://www.intel.com/content/www/us/en/developer/articles/tool/pin-a-dynamic-binary-instrumentation-tool.html

#### R3-011 QBDI（QuarkslaB DBI）
- 定位：Quarkslab 的 LLVM 化 DBI，移动（ARM/AArch64）优先；设计与 Frida 可组合。
- 可抄机制：① 模块化分层（引擎/补丁/执行上下文分离），支持「只截取部分执行」；② 与 Frida 集成的先例证明「DBI 引擎可作为另一个插桩框架的插件」；③ 透明性实验记录（COBAI 等评测中通过 VM/debugger 检测项——取自 2021 论文口径，非最新）。
- 档位：**watch**（Apache-2.0 友好，但移动优先与我们的 Windows 主线错位；留作 Android/ARM 场景储备）。
- 第一步动作：无（登记，需移动端场景时再评估）。
- 许可证/成熟度：Apache-2.0 / 维护中。
- 链接：https://github.com/QBDI/QBDI

### D. Windows hooking / 注入

#### R3-012 Microsoft Detours
- 定位：微软官方 hooking 库（MIT，2018 起开源）；**production 级参照实现**。
- 可抄机制：① **trampoline 重定位**：将目标函数头指令复制到 trampoline（修正 RIP 相对寻址），再改写入口跳转；`DetourTransaction` 事务化保证多线程下原子安装/卸载（`DetourUpdateThread` 处理线程栈上返回地址）；② API 抽象：`DetourAttach/DetourDetach` + 前/后回调注入；③ ARM64 生态在社区 fork（m417z/minhook-detours）出现——说明官方 ARM64 覆盖曾是缺口。
- 档位：**吸收**（MIT 是唯一可静态集成的老牌方案之一；Windows 侧 hooking 的默认底座）。
- 第一步动作：用 Detours 写「自有测试进程 API 调用记录器」最小样例（attach 到 CreateFileW 等，记录参数字符串 → ETW/日志），作为 adv-bin 的用户态观测起点。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://github.com/microsoft/Detours

#### R3-013 MinHook / minhook-detours
- 定位：轻量 inline hook（单文件级依赖、无外部依赖）；活跃 fork 补 ARM64。
- 可抄机制：① 极简 trampoline 实现（备份原指令 + JMP，Freeze/Unfreeze 队列式线程管理）；② Rust 绑定成熟度信号：`minhook 0.7.1`（crates.io ≈28.8k 下载口径）、`detours` 绑定（检索时「3 天前」更新）；③ `m417z/minhook-detours`：**MinHook 风格 API + SlimDetours 实现 + ARM64**（SlimDetours 修了 Detours 的死锁问题）——「API 亲和 + 更稳实现」的档位选择样板。
- 档位：**吸收**（x86/x64 场景默认；ARM64 出现时切 minhook-detours）。
- 第一步动作：与 R3-012 同样例 A/B：同一目标函数分别用 Detours 与 MinHook 实现，量记录开销与线程安全性差异（单机单窗口，不作普适结论）。
- 许可证/成熟度：MinHook BSD-2 / 维护中（低频）。
- 链接：https://github.com/TsudaKageyu/minhook ；https://github.com/m417z/minhook-detours

#### R3-014 VEH + 硬件断点（HWBP）hooking 机制
- 定位：**不改内存代码**的线程级 hook：设置调试寄存器断点，VEH 接管异常。
- 可抄机制：① `GetThreadContext`/`SetThreadContext` 写 DR0–DR3 地址 + DR7 控制位（执行型断点），异常经 VEH（**优先级高于 SEH**）分发；② 无代码修改 ⇒ 不触发基于完整性扫描的检测，且天然避免「指令重定位」问题；③ 硬约束：**仅 4 个槽位**、每线程生效（目标进程新建线程需补设）、异常处理频次即开销上限。Rust 生态：`veh-hooking-rs`（近月发版）、`neohook`（`detours_veh_install`，仍受 4 槽限制）。
- 档位：**有界**（作为「轻量观测/断点」的补充通道；不适合大规模、非频繁的插桩面）。
- 第一步动作：实现「自有进程 + 单线程 + 1 个执行断点」的 VEH 观测 PoC，记录与 inline hook 的引入成本差异。
- 许可证/成熟度：机制（实现以所选 crate/自研为准；生态多为实验期） / 实验。
- 链接：https://learn.microsoft.com/en-us/windows/win32/debug/vectored-exception-handling ；https://crates.io/crates/veh-hooking-rs

### E. OS 级追踪与观测

#### R3-015 eBPF Linux：aya / bpftrace / bcc / libbpf
- 定位：Linux 内核可编程观测事实标准；Rust 侧对接面是 aya。
- 可抄机制：① 观测点体系：kprobe/uprobe（用户态函数探针，经 uprobes + perf_event_open 挂载）、tracepoint、USDT——「函数入口/出口 + 结构化参数」的通用能力；② BTF + CO-RE：类型信息驱动的内核结构可移植访问（aya 侧推进「BTF 兼容的 map 定义」、完整 Rust 类型 BTF）；③ 数据通道演进：perf buffer → **ring buffer**（aya 把日志从 perf buffer 迁到 ring buffer 是 2025-2026 明确信号）；④ `bpf()` 系统调用 + verifier 的安全模型（无 verifier 不成 eBPF——这是它「可安全暴露给脚本」的根因）。
- 档位：**吸收（Linux 侧观测能力，服务 perf-core）**；Windows 侧仅作思想参照。
- 第一步动作：若 perf-core 有 Linux 目标：选一个「uprobe 探针 → ring buffer → 解析」样例（aya 官方 book 口径的样板）做通链验证。
- 许可证/成熟度：内核 GPL-2.0 / bpftrace Apache-2.0 / aya MIT+Apache-2.0 / 维护中（FOSDEM 2026-01 仍在活跃汇报）。
- 链接：https://github.com/aya-rs/aya ；https://github.com/bpftrace/bpftrace

#### R3-016 eBPF for Windows
- 定位：微软主导的 Windows eBPF（与 Linux 生态「同 API 不同实现」）。
- 可抄机制：① **许可驱动的架构选择**：内核 GPL verifier 不可复用 → 选 **PREVAIL**（verifier）与 **uBPF**（JIT）；② 三条字节码落地路径：`bpf2c` 生成原生 `.sys`（首选/最安全） / JIT（eBPFSvc） / 解释器（debug）——「验证/编译在用户态安全环境完成，内核只装载结果」的设计；③ 兼容面：复用 Clang eBPF 后端与 libbpf 风格 API，目标是 Linux 工具链上移。
- 档位：**watch**（WIP；引入需驱动签名/WDK 链；最近推送 ≈2025-05 为单源口径）。
- 第一步动作：登记观察项（是否 GA/是否进入 Windows 正式组件），不投入。
- 许可证/成熟度：MIT / 实验（WIP）。
- 链接：https://github.com/microsoft/ebpf-for-windows

#### R3-017 DTrace（含 Windows 端口）
- 定位：Solaris/macOS/FreeBSD 经典动态追踪 + 语言（D）；被微软移植到 Windows。
- 可抄机制：① **动态探针体系**（fbt/pid/sdt）+ **D 语言**在探针上做过滤/聚合——「过滤下沉到内核，用户态只收聚合结果」；② **CTF 缓冲格式 + 聚合/推测（speculation）** 语义；③ predicate 与 in-kernel aggregation 的成本模型（大量事件、极少回传）。
- 档位：**watch**（Windows 端口情况：2019 起发布、检索称 Windows Server 2025 内置——二手源待复核；生态远小于 ETW）。
- 第一步动作：若出现 Windows 侧可用性硬证据，补一页 ETW-vs-DTrace 选型对照；当前不投入。
- 许可证/成熟度：CDDL-1.0（OpenDTrace） / 维护中。
- 链接：https://github.com/opendtrace

#### R3-018 ETW（Windows 事件追踪）+ ferrisetw
- 定位：Windows 内建的统一事件管道（内核与用户态 provider）；**Windows 观测的默认答案**。
- 可抄机制：① **事件模型**：manifest 声明式 schema 或 **TraceLogging 自描述**事件；事件头（时间戳、ProviderId、EventId、PID/TID、层级）+ 参数载荷；② **会话-消费架构**：`StartTrace` 建会话（属性=缓冲、日志文件、实时模式）→ `EnableTraceEx2` 开 provider（可带 keyword/level 过滤）→ `OpenTrace`+`ProcessTrace` 消费（实时回调在独立线程）；日志文件模式（`.etl`）供离线分析；③ **schema 解析链**：TDH（`TdhGetEventInformation`/`TdhFormatProperty`）按 provider 元数据解码字段，schema 缓存是性能关键——ferrisetw 的 `SchemaLocator` 正是此点；④ Rust 落地：**ferrisetw 1.2.0**（MIT/Apache-2.0、windows-rs 绑定、最近提交 2025-08-20、自称 KrabsETW 移植）可直接消费实时会话与日志文件（`TryParse` trait 做字段类型化）。
- 档位：**吸收**（adv-bin 的 Windows 进程/系统观测底座；perf-core 的采集通道）。
- 第一步动作：用 ferrisetw 订阅一个内核 provider（如进程/线程）落 JSONL，验证 schema 解析与吞吐；同时跑通 `wpr`→`wpaexporter` 离线管线（R3-024）。
- 许可证/成熟度：OS API（Windows SDK）/ ferrisetw MIT+Apache-2.0 / 维护中。
- 链接：https://learn.microsoft.com/en-us/windows/win32/etw/event-tracing-portal ；https://github.com/n4r1b/ferrisetw

#### R3-019 Linux perf events
- 定位：Linux 性能计数/采样子系统（`perf_event_open`）；rr 的时序源。
- 可抄机制：① 计数 vs 采样双模式 + group 复用；② 软件事件（上下文切换、任务迁移）与硬件计数器（引退条数/分支，`PERF_COUNT_HW_CONDITIONAL_BRANCHES` 即 rr 的 RCB 源）；③ 调用链采集（fp/dwarf）与 `PERF_SAMPLE_*` 数据面。
- 档位：**有界**（Windows 主线外；作为 Linux 侧 perf-core 的既有能力登记）。
- 第一步动作：无（若 perf-core 做 Linux 采集，直接以 perf_event_open 为准）。
- 许可证/成熟度：GPL-2.0（内核/工具） / 维护中。
- 链接：https://perf.wiki.kernel.org/

#### R3-020 Intel PT（Processor Trace）
- 定位：CPU 硬件分支追踪（包流）；Windows 自 10 1803/1809 起有 in-box 驱动 `ipt.sys`。
- 可抄机制：① 硬件：PSB/TNT/TIP 包流 + ToPA 缓冲，**低运行时扰动**的分支级 trace；② Windows 消费链：`ipt.sys` 以 IOCTL/注册表配置（非 ETW 内部机制；原为 WER/PSS 场景），WinIPT 提供库/CLI 参照；③ 解码：libipt（BSD-3）做包解码；上层回放/切片（GDB 16.1 已能把 PTW payload 暴露给脚本）；④ **fuzz 集成先例**：WinAFL 的 Intel PT 模式；2025-06 有研究用它做 Windows fuzzing 的崩溃去重与根因切片（call-stack 去重 + 指令级流重建 + 反向切片）。
- 档位：**有界（观测潜力）/ watch（集成）**——WinIPT 最后推送 2023-04-27 ⇒ 弃维护信号，自研消费链不划算；保留「低扰动分支 trace → 覆盖率/根因」的思想。
- 第一步动作：登记；若 test-replay 需要「低开销执行轨迹」，先评估 `ipt.sys` 直用的工程成本（驱动 API 文档薄）。
- 许可证/成熟度：libipt BSD-3-Clause / WinIPT MIT（弃维护） / `ipt.sys` 随 OS 维护中。
- 链接：https://github.com/intel/libipt ；https://github.com/ionescu007/winipt

### F. 用户态仿真（与 R2 交界的动态侧）

#### R3-021 Unicorn
- 定位：QEMU TCG 抽出的 CPU 仿真器（多架构、指令级 hook）。
- 可抄机制：① **hook 模型**：`UC_HOOK_CODE`/`UC_HOOK_BLOCK`/内存读写 hook + 指令计数，是「仿真即插桩」的通用面；② API 设计（`uc_open/uc_emu_start` + 回调上下文）已成为同类工具的事实接口；③ 作为 Qiling 等 OS 级框架的底座（分层：CPU 仿真 vs OS 语义）。
- 档位：**有界**（GPL-2.0 注意；adv-sandbox 的仿真后端候选，验证「单指令/基本块单步 + 覆盖率」是否满足需求）。
- 第一步动作：跑通「自建小型二进制 + UC_HOOK_BLOCK 记录块序列」样例，量精度与吞吐。
- 许可证/成熟度：GPL-2.0 / 维护中（Unicorn 2 线）。
- 链接：https://github.com/unicorn-engine/unicorn

#### R3-022 Qiling
- 定位：Unicorn 之上的 OS 感知仿真框架（syscall/loader/rootfs 语义）。
- 可抄机制：① **OS 语义层**：用外部 rootfs（Windows/Linux 等）提供 DLL/ELF 依赖解析，「仿真到能跑真实样本」；② 与 fuzz 的组合：Synacktiv 2025-02 的「黑盒仿真 fuzz（Qiling）」演示了「仿真器 + fuzzer」在固件场景的闭环；③ Python 层 API 便于编排（hooks + 脚本）。
- 档位：**有界**（adv-sandbox 候选；GPL-2.0 + Python 性能上限）。
- 第一步动作：以「一个 Windows PE 小样本能否在 Qiling 跑到 main」为验收，评估 rootfs 可得性与稳定性。
- 许可证/成熟度：GPL-2.0 / 维护中。
- 链接：https://github.com/qilingframework/qiling

### G. Rust 侧与 DWARF 消费

#### R3-023 strace（syscall 跟踪范式）
- 定位：Linux syscall 跟踪标准工具；其**机制组合**是 test-replay 的记录器原型。
- 可抄机制：① ptrace 逐 syscall 停（`PTRACE_SYSCALL` + entry/exit 两次停）+ 参数/返回解码；② **seccomp-bpf 加速路径**（`--seccomp-bpf`）：`SECCOMP_RET_TRACE` 只对关心的 syscall 陷入 ⇒ 降开销的「过滤下沉」手法（与 DTrace predicate 同哲学）；③ 输出契约：每行 = 一次系统调用（可解析结构化）。
- 档位：**有界**（范式吸收：syscall 事件 = test-replay 的最小记录单元之一）。
- 第一步动作：定义 syscall 事件记录格式（编号/参数/返回值/时间戳），对照 strace 输出做兼容性检查。
- 许可证/成熟度：LGPL-2.1+ / 维护中。
- 链接：https://github.com/strace/strace

#### R3-024 WPR / WPA / wpaexporter（Windows 性能记录管线）
- 定位：Windows 官方 ETW 采集/分析工具链（Windows Performance Toolkit）。
- 可抄机制：① `wpr.exe`：内置于 Windows 10+，管理员权限，`-start <Profile>`/`-stop <file.etl>`，可连锁 profile（`CPU`/`GeneralProfile`/`Registry` 等），`-filemode` 落盘——**CI 可脚本化**；② `wpaexporter`：`-i trace.etl -profile x.wpaProfile -outputfolder ...` 把 WPA 表格批量导出 CSV（开关**按类别有序**：输入/范围/配置/杂项；`-tle`/`-tti` 为「容忍丢事件/时间倒挂」的非文档开关，同 xperf 口径）——「GUI 里设计视图 → 存 profile → 批量导出」是工程化关键；③ 2025 生态：WPA 预览版内置 **WPA MCP**（Copilot 集成）、社区 `wpa-mcp`（包 `wpr`/`xperf`/`wpaexporter` 成 MCP，提供 validate/export/analyze 三工具 + 预设 profile；单源待复核）、ETW MCP（需 .NET 10 SDK 口径）。
- 档位：**吸收（管线）**：perf-core 的 Windows 采集通道 + adv-bin 的 CPU/IO/挂起类问题取证。
- 第一步动作：写脚本「`wpr -start GeneralProfile -filemode` → 目标负载 → `wpr -stop` → `wpaexporter`（CPU 热路径 profile）→ CSV」，纳入 CI 基线。
- 许可证/成熟度：专有（Windows SDK/ADK 免费组件） / 维护中。
- 链接：https://learn.microsoft.com/en-us/windows-hardware/test/wpt/

#### R3-025 gimli / addr2line（Rust DWARF 消费）
- 定位：纯 Rust 的 DWARF 读/写与地址→源码/内联栈解析；adv-bin 的符号化基础件。
- 可抄机制：① 设计原则：**zero-copy**（引用原始缓冲）、**lazy**（按需解析 CU，`DW_AT_sibling` 跳读）——大二进制解析的性能关键；② 消费 API：`Loader::find_location`/`find_frames`（含内联调用栈），或低层 `Context`；③ 无对象格式绑定（与 `object` crate 组合）⇒ 可消化 PE/ELF/Mach-O 的调试节。gimli 0.31.1（CRATES 口径），addr2line 仓库最近推送 2025-02-20（检索口径）。
- 档位：**吸收**（直接依赖；与 R1/R2 的 DWARF 消费链协同）。
- 第一步动作：给 adv-bin 加「PE/ELF 地址 → 文件:行:函数（含内联帧）」服务，基准测试对标 GNU addr2line/LLVM。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/gimli-rs/gimli ；https://github.com/gimli-rs/addr2line

#### R3-026 probe-rs
- 定位：Rust 嵌入式调试工具包（探针驱动/CMSIS-DAP/J-Link + RTT 日志）。
- 可抄机制：① **目标-探针抽象**（同一 API 覆盖多探针/多芯片）+ RTT（实时传输）作为「无 UART 的真机日志通道」；② 与 gimli/addr2line 同栈（DWARF 侧复用）——Rust 调试栈自洽的示例；③ 对我们的交界：嵌入式不在主线，但其「探针抽象 + 会话管理」可参照。
- 档位：**watch**（无嵌入式目标前不投入）。
- 第一步动作：登记（评估仅当 adv-bin 出现固件/嵌入式场景）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://github.com/probe-rs/probe-rs

### H. 调试会话自动化协议

#### R3-027 DAP（Debug Adapter Protocol）+ MCP 桥
- 定位：编辑器与调试器解耦的 JSON-RPC 协议；2025–2026 成为 **AI 会话驱动调试**的接点。
- 可抄机制：① 请求/事件骨架：`initialize`（能力协商）→ `launch`/`attach` → `setBreakpoints`（含条件/命中计数/函数/数据断点）→ `configurationDone` → 事件 `stopped`/`continued`/`threads` → `stackTrace`/`scopes`/`variables`/`evaluate`/`readMemory` → `disconnect`；② 适配器生态：`lldb-dap`（LLVM 官方，Rust/C/C++/Swift）、CodeLLDB、debugpy、Delve、netcoredbg；③ **MCP 桥设计模式**（2025–2026 成型）：粗粒度一次性工具（`debug_inspect`/`debug_run` 降低往返）、Router/技能按需展开（IntelliJ `xdebug_` 前缀 + router 模式）、安全分级（`readonly`/`full` + allowSpawn/allowAttach/allowModify/allowExecute）——**三层：协议能力 → token 预算化工具面 → 授权策略门**；④ 已知边界：LLVM 21.1.8 `lldb-dap` 的 Rust inline test 线程列表缓存不全（nvim-dap #1577）⇒ 客户端按事件驱动+重试设计；Windows 侧 lldb-dap 支持不佳 ⇒ CodeLLDB 或 DbgEng 路线。
- 档位：**吸收**（test-replay 的会话层与 adv-bin 的「AI 驱动调试」入口）。
- 第一步动作：实现最小 DAP 客户端（spawn `lldb-dap`，attach 到 Rust 测试二进制，命中 → 取栈+变量），再包一层 MCP 粗粒度工具，验证「AI 会话 → 断点 → 状态快照」闭环。
- 许可证/成熟度：规范 MIT（microsoft/debug-adapter-protocol）；适配器各异 / 维护中（生态 2025–2026 井喷：mcp-dap-server、dap-mcp-server、IntelliJ 2026.x agentic debugging 等）。
- 链接：https://microsoft.github.io/debug-adapter-protocol/

#### R3-028 DbgEng / WinDbg 自动化
- 定位：Windows 官方调试引擎（dbgeng.dll 可再分发）+ WinDbg 前端；Windows 深调试的兜底。
- 可抄机制：① COM 自动化面：`IDebugClient`/`IDebugControl`/`IDebugDataSpaces`（附加/控制/内存读写/符号），可无人值守驱动（对照 `-c` 初始命令 + `.script` JS 脚本）；② 与 TTD 的联动（打开 `.run`、数据模型查询）——Windows 回放资产的程序化消费入口；③ 符号链（sympath/srv*）与 minidump 支持的工程成熟度。
- 档位：**有界**（专有 + 文档分散：只做「兜底自动化」实现最小子集，优先 DAP 路线）。
- 第一步动作：封一个「attach PID → 读内存/模块列表 → 分离」的最小 DbgEng 客户端，验证 CI 提权与稳定性。
- 许可证/成熟度：专有（免费可再分发组件） / 维护中。
- 链接：https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/debugger-engine-overview

---

## 3. 2025–2026 前沿信号

1. **录制回放「思想商品化」**：TTD 工具链持续更新（TTD.exe 多录制模式；Server 版本矩阵扩展），Undo v6.8 把倒带并行化（官方称最高 4x）——「事件日志 + 快照切片」成为共识结构（rr/Undo/TTD 三家同构）。
2. **AI ↔ 调试器的协议层打通**：2025-2026 出现 MCP-DAP 桥集群（mcp-dap-server 13 工具、dap-mcp-server 粗粒度、DAP-MCP 安全分级版），JetBrains 把调试器直接 MCP 化（2026.1.3+ 共享 server / 2026.2+ router）——「AI 会话驱动调试」从演示走向产品化；token 效率（粗粒度+router）与权限门（readonly/allow*）成为设计要点。
3. **插桩引擎的可组合化**：Frida 16.7（2025-03）线程/模块观察 API + gum-prof；17.10（2026-05）Interceptor 选项结构体化与 unwind 支持、Stalker ARM64 分配自由化——「插桩原语化、策略可组合」方向明确。
4. **Rust 观测栈成熟**：aya 在 Linux 侧持续补齐（ring buffer 日志、BTF、TCX；FOSDEM 2026-01 汇报），但 **Windows 明确缺席**；Windows 侧 Rust 消费端由 ferrisetw（2025-08 仍活跃）+ windows-rs 承担；DWARF 侧 gimli/addr2line 稳（2025 仍在维护）。
5. **DBI 格局移动**：DynamoRIO v11.3.0（2025 活跃，二手源）对比 Intel PIN 收缩信号（二手源待复核）；QBDI（Apache-2.0）主打移动；**Windows 开源 DBI 的实用解=DynamoRIO + 覆盖率格式（drcov）**。
6. **eBPF 双生态成型**：Linux（aya/bpftrace；verifier=内核）+ Windows（ebpf-for-windows；verifier=PREVAIL、JIT=uBPF、bpf2c→.sys；WIP）——「同 API 名、不同内核」，跨平台抽象成本高。
7. **硬件追踪的下沉与工具化**：`ipt.sys` in-box 化（Win10 1809 起）让 PT 在 Windows 可达，但 WinIPT 弃维护（最后推送 2023-04-27）⇒ 消费链自建成本高；PT 的「低扰动分支 trace」价值在 fuzz/根因（WinAFL PT 模式、2025-06 学术线）被反复验证。
8. **透明度/检测对抗成为显学**：TTD 指令模拟与真机差异（2025 Mandiant）、DBI 的 VM/debugger 检测面（2021 论文口径仍被引）——动态能力的「保真度标定」与「合规边界」需产品级声明。

---

## 4. Top-3（对 ADV 的落地排序）

1. **LLDB/lldb-dap + DAP/MCP 会话层（R3-002 + R3-027）→ adv-bin & test-replay**：协议稳定、Rust 支持好、Apache-2.0 可集成、2025–2026 MCP 生态已验证；直接支撑「AI 会话驱动调试」这一产品差异化点。第一步：最小 DAP 客户端 + 粗粒度 MCP 工具面 + readonly 默认权限门。
2. **rr 思想（R3-005）本地化为 test-replay 的确定性记录内核**（吸收 TTD（R3-004）的可查询时间线与 Undo（R3-006）的业务化选项）：只记非确定性事件 + 快照/切片回退 + 失败样本留存；Windows 侧先以 TTD.exe 外部编排过渡。第一步：录制边界定义文档 + Linux 原型对照。
3. **Windows 观测三件套：ETW/ferrisetw（R3-018）+ WPR/wpaexporter 管线（R3-024）+ Detours/MinHook 薄封装（R3-012/013）**，配 VEH/HWBP（R3-014）作补充通道：全部在「自有/授权进程 + 可审计」边界内，构成 adv-bin 的最小动态观测面。第一步：ferrisetw 落 JSONL + wpr→wpaexporter CSV 管线进 CI。

---

## 5. 否证、风险与边界（本批结论的适用条件）

- **数字口径**：TTD 5x–20x 开销、idx≈2×trace 体积为**官方/检索口径**（未本地实测）；Frida「10x–100x」与 exclude「≈10x 提速」为**社区口径**；Undo「2x–5x / 4x 加速」为**厂商口径**；以上均非 ADV 环境实测值，采用前需在目标环境复测（复核日期=本文写稿日 2026-10-03）。
- **单源/二手信号**（待复核）：PIN「更新周期收尾」、DTrace「Windows Server 2025 内置」、DynamoRIO v11.3.0、ebpf-for-windows 最后推送 2025-05、wpa-mcp、Windows Server 的 TTD.exe 版本矩阵。
- **合规红线**：一切 attach/inject 只对自有或显式授权进程；不实现 EDR 规避（TitanHide/ScyllaHide 类只作威胁模型）；不解析专有录制格式（.run/.idx）；调试会话默认只读、白名单、全审计。
- **绿=SKIP 提醒**：本域所有「可达/可用」判断需走真路径验证（如在 Windows CI 真跑 TTD.exe 与 ferrisetw），文档结论不算验收。
- **交界声明**：PANDA/Unicorn/Qiling 的静态符号化能力属 R2；本文只记其动态执行/回放面，避免与 R2 重复计分。

---

## 6. 参考链接（按条目）

- GDB：https://sourceware.org/gdb/
- LLDB：https://lldb.llvm.org/
- x64dbg：https://github.com/x64dbg/x64dbg
- TTD：https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/time-travel-debugging-overview
- rr：https://github.com/rr-debugger/rr ；论文 https://arxiv.org/abs/1705.05937
- Undo：https://undo.io/
- PANDA：https://github.com/panda-re/panda ；panda-rs https://docs.rs/panda-re/0.14.0/panda/
- Frida：https://frida.re/docs/stalker/ ；https://frida.re/news/2025/03/13/frida-16-7-0-released/
- DynamoRIO：https://github.com/DynamoRIO/dynamorio
- Intel PIN：https://www.intel.com/content/www/us/en/developer/articles/tool/pin-a-dynamic-binary-instrumentation-tool.html
- QBDI：https://github.com/QBDI/QBDI
- Detours：https://github.com/microsoft/Detours
- MinHook：https://github.com/TsudaKageyu/minhook ；https://github.com/m417z/minhook-detours
- VEH：https://learn.microsoft.com/en-us/windows/win32/debug/vectored-exception-handling
- aya：https://github.com/aya-rs/aya ；bpftrace https://github.com/bpftrace/bpftrace
- eBPF for Windows：https://github.com/microsoft/ebpf-for-windows
- DTrace：https://github.com/opendtrace
- ETW：https://learn.microsoft.com/en-us/windows/win32/etw/event-tracing-portal ；ferrisetw https://github.com/n4r1b/ferrisetw
- perf：https://perf.wiki.kernel.org/
- Intel PT：https://github.com/intel/libipt ；https://github.com/ionescu007/winipt
- Unicorn：https://github.com/unicorn-engine/unicorn ；Qiling https://github.com/qilingframework/qiling
- strace：https://github.com/strace/strace
- WPT（WPR/WPA）：https://learn.microsoft.com/en-us/windows-hardware/test/wpt/
- gimli：https://github.com/gimli-rs/gimli ；addr2line https://github.com/gimli-rs/addr2line
- probe-rs：https://github.com/probe-rs/probe-rs
- DAP：https://microsoft.github.io/debug-adapter-protocol/
- DbgEng：https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/debugger-engine-overview

（完；复核日期 2026-10-03）
