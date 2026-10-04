# X11 · Windows 平台工程总账

> 检索锚：2026-10-04，WebSearch/WebFetch（crates.io API、learn.microsoft.com、GitHub 检索摘要）。
> 域定位：把 windows-rs 生态与 Win32 平台 API 的工程面盘点成 ADV 可执行的总规范；与 R3（调试/ETW）、C3（链接/启动）、C5（USN/MFT/原子写）、E7（Segment Heap）、08 域（sandbox/AppContainer）、X10（WER）互为边界，本文只做聚合与补缺。
> 条目格式：定位 → 可抄机制 → 档位【吸收|有界|不吸收|watch|reference】→ 第一步动作 → 许可证/成熟度【实验|维护中|弃维护】→ 链接。共 27 条（机制级 20 条）。

---

## 0. 重点回答（五问结论，细节见条目）

### ① windows crate 特性门策略（编译体积控制）

- windows crate 按 **Win32 命名空间**做 Cargo feature 门（`Win32_Foundation`、`Win32_Storage_FileSystem`、`Win32_System_JobObjects`…，400+ 个门）；不开启的命名空间不参与编译。生成源头是 win32metadata（Windows SDK 元数据）→ 生成器产出，不是手写绑定（X11-01）。
- ADV 配置范例（按需特性，只开ADV 真正调用的命名空间）：

```toml
[dependencies]
windows = { version = "0.62", features = [
  "Win32_Foundation",                 # HANDLE/BOOL/WIN32_ERROR，几乎必开
  "Win32_Storage_FileSystem",         # CreateFileW/FindFirstStreamW/长路径
  "Win32_System_JobObjects",          # 进程池账本（见②）
  "Win32_System_Threading",           # SetPriorityClass/OpenProcessToken
  "Win32_Security",                   # Token/ACL/AppContainer 基础
  "Win32_System_Registry",            # 仅当不换 windows-registry crate
] }
windows-registry = "0.x"              # 注册表单独走拆分 crate，少开一个大特性
```

- 判据口径：**默认不开任何 `Win32_*` 特性，CI 里加"缺特性"编译检查**——缺哪个门编译器报"item not found"时再补（windows-rs 官方推荐工作法）；禁止图省事开 `all`。热路径低层调用（USN/MFT 批量枚举）可用 `windows-sys`（raw 面、无 COM 包装，编译更轻）替代（X11-02）。
- 体积/编译时间数字未实测（本域未跑基准，R1 口径：先量后改），先按上述门控落地，C3 域基准出来后再对账。

### ② Job Object 完整能力清单 → ADV 进程池映射

| Job Object 能力 | 结构/接口 | ADV 用法 |
|---|---|---|
| 单进程内存上限 | `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.ProcessMemoryLimit`（flag `JOB_OBJECT_LIMIT_PROCESS_MEMORY`） | 每个扫描 worker 的内存预算（防单插件/解析器 OOM 拖垮主机） |
| 全作业内存上限 | 同上 `JobMemoryLimit`（`JOB_OBJECT_LIMIT_JOB_MEMORY`） | 整个进程池的内存顶 |
| 作业 CPU 时间 | `PerJobUserTimeLimit`（`JOB_OBJECT_LIMIT_JOB_TIME`）+ 通知 `JOB_OBJECT_MSG_END_OF_JOB_TIME(=1)` | 扫描任务超时（时间型），配合通知不用轮询 |
| CPU 占比硬顶 | `JOBOBJECT_CPU_RATE_CONTROL_INFORMATION`（`SetInformationJobObject/JobObjectCpuRateControlInformation`；单位=百分比×100，hard cap 档要求 ≥1000 即 ≥10%） | 扫描器不抢用户 CPU 的粗粒度版（见④） |
| IO 速率上限 | `JOBOBJECT_IO_RATE_CONTROL_INFORMATION`（`SetIoRateControlInformationJobObject`，MaxIops/MaxBandwidth/ReservationIops；较新 Win10 起可用，以 learn 页 availability 为准） | 磁盘密集扫描限速（USN/MFT/批量哈希） |
| IO 记账 | `QueryInformationJobObject(JobObjectBasicAndIoAccountingInformation)` → `IO_COUNTERS`（Read/Write/Other 的 OperationCount+TransferCount） | **ADV 性能账的账本源**：每任务/每池的读写字节与次数（X11-09） |
| 基础记账 | 同一查询：用户/内核时间、PageFaultCount、Total/Active/Terminated 进程数 | 进程池健康度与泄露检测 |
| 退出通知 | 完成端口 `AssociateCompletionPort`；`JOB_OBJECT_MSG_EXIT_PROCESS(=7)`、`NEW_PROCESS(=6)`、`ACTIVE_PROCESS_ZERO(=4)`、`PROCESS_MEMORY_LIMIT(=9)`、`JOB_MEMORY_LIMIT(=10)` | 插件进程退出/被杀的异步感知，免轮询 |
| 优先级 | `JOB_OBJECT_LIMIT_PRIORITY_CLASS`（整个作业的 class） | 作业级 BELOW_NORMAL/IDLE（见④） |
| 清理 | `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` + `TerminateJobObject` | 句柄关闭即全池灭绝——CLI 短生命周期必备，防孤儿进程 |
| UI 限制 | `JOB_OBJECT_UILIMIT_*`（`JOB_OBJECT_BASIC_LIMIT_INFORMATION`/UI restrictions） | sandbox 域可复用（08 域联动） |

第一步动作：进程池启动时 `CreateJobObjectW` → `SetInformationJobObject`（Extended limits + KILL_ON_JOB_CLOSE）→ 每个 spawned worker `AssignProcessToJobObject`；账本按任务结束查询 IO_COUNTERS 落日志。

### ③ 路径处理总规范（可直接进 spec 的聚合）

1. **长路径**：MAX_PATH=260 是默认；解除需双门槛——注册表 `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled=1`（或 GPO）+ 应用清单 `longPathAware=true`（Win10 1607+）。`\\?\` 前缀绕过两者但要求**绝对、已规范、不含 `/`、不含 `.`/`..`**；`std::fs::canonicalize` 在 Windows 返回 verbatim（`\\?\C:\...`）路径，跨进程传给 git/cargo 等第三方会踩坑（对方未必 long-path-aware），展示/传参前用 dunce 风格"能剥则剥"。ADV 规则：内部比较用原始串；出进程边界时剥 verbatim；错误消息里剥。
2. **大小写与归一化**：NTFS 大小写不敏感基于 `$Up-Case` 表（NT 自己的映射，不是 NLS 大写、也不是 locale 大写——避免土耳其 i 类问题）；Win32 文件系统**不做 Unicode 归一化**——NFC 与 NFD 是两个不同文件（与 APFS 行为不同）。ADV 比较规范：**不 NFKC、不做 locale 大写；代码点级 + NT upcase 表折叠后比较**（可内置 upcase 表或依赖 windows crate）。WSL 启用的每目录大小写敏感（Win10 1803+，`fsutil file setCaseSensitiveInfo`）会让"同目录两个仅大小写不同的文件"合法存在——扫描结果不得按路径去重。
3. **Reparse point 三兄弟**：`IO_REPARSE_TAG_SYMLINK(0xA000000C)`（可相对、建链默认要管理员或开发者模式）、`IO_REPARSE_TAG_MOUNT_POINT(0xA0000003)`（junction：目标必为绝对路径、**普通用户就能建**、可指向本机任意卷目录）、另有 APPEXECLINK / OneDrive 占位（WCI 系）等。扫描规则：枚举见 `FILE_ATTRIBUTE_REPARSE_POINT` → **默认不下钻**（junction 循环 + 路径逃逸双风险）；需要本体时以 `FILE_FLAG_OPEN_REPARSE_POINT` 打开；tag 用 `IsReparseTagMicrosoft`/具体 tag 值分类记录而不是猜。白名单机制（C5 已有）与 tag 白名单合并成一张表。
4. **ADS（备用数据流）**：NTFS/ReFS 支持、FAT32/exFAT（Windows 实现）不支持；`FindFirstStreamW/FindNextStreamW` 枚举 `::$DATA` 流；`Zone.Identifier` 是 MOTW 来源。扫描面规则：哈希主流时**一并枚举并哈希 ADS**（NIST 有全流哈希法可抄），恶意负载藏 ADS 是经典手法，且多数工具默认漏检——这是 ADV 的覆盖优势点，也是输出文件（报告缓存）本身要留意的面。
5. **8.3 短名**：`C:\PROGRA~1\...` 合法可达；`NtfsDisable8dot3NameCreation` 可关、`fsutil 8dot3name strip` 可清，但用户机器上默认存在率高（未实测本域不下断言，按"必须兼容"设计）。前缀/路径匹配类检测（白名单、忽略规则）需先展开短名再匹配，否则被 `PROGRA~1` 绕过。
6. **共享冲突**：Windows 无 POSIX 式"随意打开"，`ERROR_SHARING_VIOLATION(32)` 是扫描常态。规范：扫描读一律 `FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE` 最大化共享；仍冲突则带退避重试（有限次数）；进程内 `SetThreadErrorMode(SEM_FAILCRITICALERRORS|SEM_NOOPENFILEERRORBOX)`（线程级，Win7+）防坏介质弹窗卡死扫描线程。

### ④ 后台运行优先级策略（扫描器不抢用户 CPU）

三层叠加（Windows 做法）：

1. **CPU 优先级**：`SetPriorityClass`（或 Job 的 `JOB_OBJECT_LIMIT_PRIORITY_CLASS`）→ `BELOW_NORMAL_PRIORITY_CLASS` 起步、深度后台档 `IDLE_PRIORITY_CLASS`。进程内按线程再细分（UI/调度线程 NORMAL，扫描线程 BELOW_NORMAL）。
2. **一键后台模式**：`SetPriorityClass(PROCESS_MODE_BACKGROUND_BEGIN)`（线程级 `SetThreadPriority(THREAD_MODE_BACKGROUND_BEGIN)`）——同时把 CPU 降到 idle、把该线程所有 IO 降到 VeryLow，是最省事的"不扰民"开关；`*_END` 恢复。
3. **细粒度 IO 优先级**：`SetFileInformationByHandle(FileIoPriorityHintInfo, FILE_IO_PRIORITY_HINT_INFO)`，`IoPriorityHintVeryLow(0)/Low(1)/Normal(2)/High(3)`；注意它是 **hint**：分页 IO 等不保证生效，High 需要特权；卷/存储栈需支持。可靠性排序：background mode > job 限速（`MaxIops`）> IO hint。

ADV 策略建议：默认 `BELOW_NORMAL` + Job IO rate control 限速；"静默模式"（用户交互时）切 `PROCESS_MODE_BACKGROUND_BEGIN`；hint 只用在批量哈希/USN 大扫这类明确低优先路径。

### ⑤ elevated 判定与降级路径

- 判定：`OpenProcessToken` → `GetTokenInformation(TokenElevation)` → `TOKEN_ELEVATION.TokenIsElevated`；需要区分成因时用 `TokenElevationType`：`Full`（拆分令牌已提升）/`Limited`（UAC 受限令牌）/`Default`（UAC 关闭或内置管理员）。`IsUserAnAdmin` 已弃用，不采用。
- 降级清单（检测非提升后自动切档，不弹 UAC）：

| 能力 | 要管理员？ | 降级路径 |
|---|---|---|
| 用户态文件扫描/哈希/AST | 否 | 全功能 |
| Job Object/优先级/IO 限速 | 否 | 全功能 |
| 创建 AppContainer 沙箱（AC） | 否 | 全功能；LPAC 相关见 08 域 |
| 建 junction | 否 | 可用；建 symlink 需管理员或开发者模式 → 记档跳过 |
| USN/MFT 卷句柄（`\\.\C:` 读） | 是 | 降级为目录遍历（`FindFirstFileEx` 面），覆盖率标注降档 |
| Defender 排除目录 | 是 | 跳过 + 在运维报告里列出"需手动执行的一条命令"（见 X11-21） |
| HKLM 注册表 / 服务安装 | 是 | 降级为 HKCU + 单文件常驻（run key 用户态），或提示 elevation 重启 |

判定函数放 `cli` 启动首屏，`verdict` 进运行档案（C5 的 elevated 档已有先例，对齐其口径）。

---

## 1. 条目账

### A. windows-rs 生态（7 条）

### X11-01 windows crate（总门面）
- 定位：微软官方"Rust for Windows"，覆盖全部 Win32/COM/WinRT API；生成器从 win32metadata 产出，命名空间即 Cargo 特性门。
- 可抄机制：① 命名空间特性门（见重点回答①）② `windows::core` 的 Result/Error 统一 HRESULT ③ 缺门时报错驱动增量加特性。
- 档位：**吸收**。第一步动作：按上面的 Cargo.toml 范例收敛 ADV 依赖特性集。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（0.62.2，2025-10-06 发布；crates.io API 检索于 2026-10-04；总下载 3.48 亿，rustc ≥1.82）。
- 链接：https://crates.io/crates/windows · https://github.com/microsoft/windows-rs

### X11-02 windows-sys（raw 绑定）
- 定位：无安全包装的原始绑定（`extern "system"` 直呼），编译更轻，适合热路径。
- 可抄机制：① USN/MFT/NT 级批量枚举走 sys 面 ② 与 windows crate 可共存（类型命名空间分开）。
- 档位：**有界**（仅 perf-core 热路径；一般代码用 windows crate 的安全包装）。
- 第一步动作：C5 的 USN 枚举若仍在手写 FFI，评估切 sys 面。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（0.61.2，2025-10，检索摘要锚）。
- 链接：https://crates.io/crates/windows-sys

### X11-03 windows-core / windows-result / windows-strings（拆分基础层）
- 定位：windows crate 的地基拆分件：trait/接口基建、HRESULT Result、PCWSTR/HSTRING 字符串类型。
- 可抄机制：① 错误模型对照（windows-result 的 `Error` 携带 HRESULT 与 code）② `HSTRING`/`PCWSTR` 转换纪律（UTF-16 边界）。
- 档位：**reference**。第一步动作：不直接依赖；仅在写 windows crate 扩展封装时认识其类型。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（随主 crate 同节奏发版）。
- 链接：https://crates.io/crates/windows-core

### X11-04 windows-registry（注册表独立 crate）
- 定位：注册表读写的小而专 crate，免去在 windows crate 里开 Registry 特性全家桶。
- 可抄机制：① key 句柄 RAII（关闭自动）② 类型化取值（字符串/DWORD/multi-sz）。
- 档位：**吸收**（ADV 配置读取、defender 状态探测、环境审计都要注册表）。
- 第一步动作：替换现有手写 advlink/FFI 注册表调用（若有）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（2025-06 有发布活动，issue #3635 锚）。
- 链接：https://crates.io/crates/windows-registry · https://github.com/microsoft/windows-rs/issues/3635

### X11-05 windows-services（服务 crate，较新）
- 定位：Windows 服务工程（安装/控制/宿主）的官方 crate，2025-05 起加扩展控制码、hosting/testing、fallback 支持（issue 锚 2025-05-21）。
- 可抄机制：① 服务入口与 SCM 交互的 Rust 化模板 ② 扩展控制码做"暂停扫描/恢复扫描"通道。
- 档位：**watch**（ADV 当前无服务形态；若做常驻守护再启用）。
- 第一步动作：不引入；在常驻方案设计文档里挂链接。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（微软，新 crate，API 面仍在扩）。
- 链接：https://github.com/microsoft/windows-rs （windows-services 改进 issue，2025-05）

### X11-06 WIL（C++ Windows Implementation Library）——思想对照
- 定位：微软官方 C++ RAII/错误处理库：资源包装、`RETURN_IF_FAILED` 系宏、错误码携带、wil::unique_handle。是 Win32 C++ 工程的"工程面标配"。
- 可抄机制（→Rust 对照）：① `unique_handle` ↔ Rust Drop 守卫（ADV 自建 `HandleGuard`，成本≈0）② 错误宏链 ↔ `?` + `From<WIN32_ERROR>` 统一错误 ③ "调用点不裸写 FFI"的封装纪律 ↔ ADV 的 advlink 薄层。Rust 侧无 WIL 等价物（windows crate 已覆盖大半），抄思想不抄代码。
- 档位：**reference**。
- 第一步动作：advlink 编码规范里补一节"守卫与错误码纪律"，引 WIL 文档为对照。
- 许可证/成熟度：MIT（仓库声明；本次 GitHub 页抓取超时未直接核对，以仓库为准）/ 维护中（微软）。
- 链接：https://github.com/microsoft/wil

### X11-07 rappct（Rust AppContainer/LPAC 工具箱）
- 定位：把 AppContainer profile 创建、capability 管理、AC/LPAC 安全进程启动、token 内省、ACL 打包成 Rust API（08 域已提，此处补 API 细节）。
- 可抄机制：① `CreateAppContainerProfile` 系封装 + capability SID（S-1-15-3-…）授予 ② LPAC（少能力集）与 AC 的启动差异面 ③ token 内省接口直接服务 ⑤ 的 elevated 判定场景。
- 档位：**有界**（08 域 sandbox 主力候补；先小面接入做降级兼容验证）。
- 第一步动作：与 08 域合议：rappct vs 手写 FFI 二选一，跑一个 LPAC 最小启动样例。
- 许可证/成熟度：MIT / 维护中但年轻（v0.13.3，2025-10-23；下载 28,518 其中近期 21,613——crates.io API 2026-10-04；单人项目，锁版本使用）。
- 链接：https://docs.rs/rappct · https://github.com/cpjet64/rappct

### B. Job / 调度 / 优先级（4 条）

### X11-08 Job Objects 完整能力面
- 定位：把一组进程作为可命名/可保护/可限速单元管理——ADV 进程池的总开关（能力映射见重点回答②）。
- 可抄机制：① Extended limits（进程/作业内存）② CPU rate control（万分数 hard cap）③ IO rate control（MaxIops/MaxBandwidth）④ KILL_ON_JOB_CLOSE 灭绝语义 ⑤ 完成端口异步通知（EXIT_PROCESS=7 等）。
- 档位：**吸收**。第一步动作：perf-core 落地 job 包装 + 每 worker AssignProcessToJobObject。
- 许可证/成熟度：系统 API（Win32，随 OS）/ 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects

### X11-09 进程/作业 IO 记账（ADV 性能账账本源）
- 定位：`QueryInformationJobObject(JobObjectBasicAndIoAccountingInformation)` → `JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION`（基础：用户/内核时间、页错误、进程计数 + `IO_COUNTERS` 四组读写计数/字节）；单进程对照 `GetProcessIoCounters`。
- 可抄机制：① 每任务结束读一次 IO_COUNTERS 进账本（免插桩、内核计数）② 与 D8 性能域的采样口径对齐（窗口均值，非单点）。
- 档位：**吸收**。第一步动作：账本 schema 加 read/write ops+bytes 四字段。
- 许可证/成熟度：系统 API / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject

### X11-10 优先级 class 与后台模式
- 定位：`SetPriorityClass`（BELOW_NORMAL/IDLE）+ `PROCESS_MODE_BACKGROUND_BEGIN`（CPU+IO 一键降底，线程级 `THREAD_MODE_BACKGROUND_BEGIN`）。
- 可抄机制：见重点回答④。注意：background mode 的 IO 降底依赖存储栈支持；对 SSD/NVMe 上效果仍以实测为准（未实测，不预设幅度）。
- 档位：**吸收**。第一步动作：CLI 启动时按 `--quiet/--interactive` 档设 class。
- 许可证/成熟度：系统 API / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass

### X11-11 I/O priority hint（细粒度）
- 定位：`SetFileInformationByHandle(FileIoPriorityHintInfo)` 设句柄级 IO 优先级 hint（VeryLow=0/Low=1/Normal=2/High=3）。
- 可抄机制：① 批量哈希/全盘扫把句柄标 VeryLow ② 明知是 hint 不当硬保证（分页 IO/元数据不生效；High 需特权）。
- 档位：**有界**（作为 background mode 之下的补充，不做主策略）。
- 第一步动作：USN 大扫路径加 hint 字段，账本记录生效与否（可查 `GetFileInformationByHandleEx` 回读）。
- 许可证/成熟度：系统 API（Vista+）/ 稳定维护（语义保守）。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle

### C. 路径与文件系统语义（6 条）

### X11-12 长路径与 \\?\ verbatim
- 定位：MAX_PATH=260 默认限；解除=注册表 LongPathsEnabled + 清单 longPathAware 双门槛（Win10 1607+）；`\\?\` 前缀独立绕过但有规范限制。
- 可抄机制：见重点回答③第 1 条（含 canonicalize/dunce 坑与跨进程边界规则）。
- 档位：**吸收**。第一步动作：ADV 二进制嵌 longPathAware 清单；路径工具函数集中处理 verbatim。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation

### X11-13 大小写与 Unicode 归一化（路径比较规范）
- 定位：NTFS 用自有 `$Up-Case` 表做大小写折叠；不做 NFC/NFD 归一化；WSL 每目录大小写敏感（1803+）可造出仅大小写不同的两个文件。
- 可抄机制：见重点回答③第 2 条（R4/C5 的零散口径在此聚合为唯一规范）。
- 档位：**吸收**。第一步动作：路径比较函数只留一个实现，规范注释引本条。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file （命名/比较语义）· fsutil file setCaseSensitiveInfo 文档

### X11-14 Reparse point：symlink / junction / 其他 tag
- 定位：`IO_REPARSE_TAG_SYMLINK(0xA000000C)` vs `IO_REPARSE_TAG_MOUNT_POINT(0xA0000003)`（junction）vs APPEXECLINK/WCI 占位；差异=相对性、建链权限、目标形态。
- 可抄机制：见重点回答③第 3 条（防循环/防逃逸/FILE_FLAG_OPEN_REPARSE_POINT）。
- 档位：**吸收**。第一步动作：扫描器枚举层加 tag 记录字段（现在只看属性位）。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points

### X11-15 ADS 备用数据流（扫描覆盖面）
- 定位：NTFS/ReFS 多流；`Zone.Identifier`=MOTW；`FindFirstStreamW/FindNextStreamW` 枚举 `::$DATA`；FAT32/exFAT（Windows 实现）无此面。
- 可抄机制：① 主流+ADS 一并枚举哈希（NIST 全流法）② 输出/缓存文件自身写 ADS 做来源标注（可选）③ 恶意样本藏 ADS=ADV 的覆盖卖点。
- 档位：**吸收**。第一步动作：文件清单 schema 加 stream 枚举开关（默认开，卷类型判别后跳过）。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirststreamw · NIST 全流哈希论文（tsapps.nist.gov pub_id 50914）

### X11-16 8.3 短名
- 定位：`PROGRA~1` 类短名默认仍存在（卷级 `NtfsDisable8dot3NameCreation` 可关、`fsutil 8dot3name strip` 可清，但不可假设已清）。
- 可抄机制：① 路径前缀/白名单匹配前做短名展开（`GetLongPathNameW` 或枚举属性携带的 alternate name）② 检测规则里把短名视为绕过向量记录。
- 档位：**有界**（兼容处理为主，不主动依赖短名）。
- 第一步动作：忽略规则引擎加一条"匹配前规范化（长名+大小写折叠）"测试用例。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file

### X11-17 共享冲突与错误模式（Windows 特有坑）
- 定位：独占打开（杀软/索引器/编辑器）→ `ERROR_SHARING_VIOLATION(32)`；坏介质默认弹窗卡线程。
- 可抄机制：见重点回答③第 6 条（共享位最大化 + 退避重试 + SetThreadErrorMode 线程级防弹窗）。
- 档位：**吸收**。第一步动作：advlink 打开包装统一共享位与重试参数（次数/退避可配）。
- 许可证/成熟度：系统机制 / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-setthreaderrormode

### D. 安全 API（5 条）

### X11-18 Token elevated 判定
- 定位：`GetTokenInformation(TokenElevation)` → `TokenIsElevated`；`TokenElevationType`（Full/Limited/Default）区分 UAC 成因；`IsUserAnAdmin` 已弃用不用。
- 可抄机制：见重点回答⑤。启动首屏判定一次缓存，不重复系统调用。
- 档位：**吸收**（C5 已用 elevated 档，此处固化为标准判定）。
- 第一步动作：cli 域落 `is_elevated()` 单测（提升/非提升两环境各跑）。
- 许可证/成熟度：系统 API / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation

### X11-19 完整性级别与降权子进程
- 定位：`TokenIntegrityLevel` → SID `S-1-16-*`（4096 低 / 8192 中 / 8448 中+ / 12288 高 / 16384 系统）；完整性模型实现 no-write-up（读策略另有配置面）。
- 可抄机制：① 读自身 IL 判定当前信任档 ② 降权子进程三步：`DuplicateTokenEx` → `SetTokenInformation(TokenIntegrityLevel)` → `CreateProcessAsUser`（解析器跑不可信输入用）。
- 档位：**有界**（与 08 域 sandbox 分工：IL 是轻量档，AppContainer 是强档）。
- 第一步动作：低完整性最小样例跑通并记坑（命名对象/文件写权限都会被 no-write-up 拦）。
- 许可证/成熟度：系统 API / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control

### X11-20 AppContainer API 细节（补 08 域）
- 定位：AC/LPAC 的 API 面：profile 创建、capability SID（`S-1-15-3-…`）授予、LPAC 少能力集、网络隔离开关、`PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` 启动属性。
- 可抄机制：① 普通用户即可创建 AC profile（无管理员依赖——⑤ 降级清单依据）② LPAC 与 AC 差异 = 默认能力集裁剪 ③ 文件/注册表 ACL 授 capability SID 才可见。
- 档位：**有界**（08 域主判；本文只存 API 面索引）。第一步动作：无（引 08 域结论）。
- 许可证/成熟度：系统 API / 稳定维护。
- 链接：https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation

### X11-21 Windows Defender 程序化交互（运维面）
- 定位：排除目录/按需扫描的合法通道：PowerShell `Add-MpPreference -ExclusionPath`、WMI `root\Microsoft\Windows\Defender` 的 `MSFT_MpPreference.Add`、`MpCmdRun.exe -Scan`；无稳定公开 C API。
- 可抄机制：① ADV 运维脚本生成一条可审计的排除命令（防实时防御扫 ADV 自身工作目录拖慢扫描——需用户知情同意，这是系统安全设置）② 写前检测：`Get-MpPreference` 读回验证生效；组策略/Tamper Protection 接管时失败可检测，不静默。
- 档位：**有界**（文档+脚本交付，不做进产品 API；需管理员，进 ⑤ 降级清单）。
- 第一步动作：运维手册加一节（含服务器 SKU Defender 为可选组件的注意点）。
- 许可证/成熟度：系统组件 / 维护中（PowerShell/WMI 面长期稳定）。
- 链接：https://learn.microsoft.com/en-us/defender-endpoint/manage-microsoft-defender-antivirus-in-your-business

### X11-22 无管理员降级清单（聚合条）
- 定位：⑤ 的表即本条：能力 → 是否要管理员 → 降级路径；判定用 X11-18。
- 可抄机制：① 启动时能力探测一次性出"运行档案"（elevated、卷访问、开发者模式）② 降档自动完成并在报告标注覆盖率损失（USN→遍历）。
- 档位：**吸收**。第一步动作：运行档案 schema 定稿（与 C5 对齐字段名）。
- 许可证/成熟度：ADV 自有规范 / 本文档维护。
- 链接：见本文件重点回答⑤。

### E. 遥测与计数器（2 条）

### X11-23 PDH 性能计数器
- 定位：PDH（pdh.dll）`PdhOpenQuery`/`PdhAddEnglishCounter`（英文名跨本地化安全）拉系统级计数器；`Win32_System_Performance` 特性直调。
- 可抄机制：① 系统级基线（磁盘队列、CPU）与 Job 账本互补 ② 计数器路径查询（本地化机器必须用 English 名）。
- 档位：**有界**（ADV 第一账本用 Job IO 计数——内核直供、零解析；PDH 留作系统基线可选源）。
- 第一步动作：不引入；perf-core 的账本设计里注明备选源。
- 许可证/成熟度：系统组件 / 维护中（legacy 定位但随 OS 分发，无弃用公告）。
- 链接：https://learn.microsoft.com/en-us/windows/win32/perfdct/using-the-pdh-functions-asynchronously

### X11-24 windows-version（版本能力门）
- 定位：轻量 crate 报 Windows 版本/构建号，支撑"特性按 OS 版本门控"（如 IO rate control、Dev Drive 检测）。
- 可抄机制：① 启动探测版本号进运行档案 ② 降级分支挂版本谓词而不是试错调用。
- 档位：**吸收**（小、官方、零依赖负担）。第一步动作：随运行档案一起落。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（windows-rs 拆分件）。
- 链接：https://crates.io/crates/windows-version

### F. 2025–2026 动向（3 条）

### X11-25 windows-rs 生成器与 1.0 状态
- 定位：截至 2026-10-04 crates.io 检索：windows 0.62.2（2025-10-06）仍为 0.x，1.0 未发；生成器源头 win32metadata 持续驱动再生成；趋势=继续拆小 crate（registry/services/version/link/window…）。
- 可抄机制：① 锁 minor 版本 + 定期（季度）升级检查 ② 新拆分 crate 优先于开大特性。
- 档位：**watch**。第一步动作：cargo deny/audit 清单里登记 windows 系列的升级策略。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://crates.io/crates/windows · https://github.com/microsoft/win32metadata

### X11-26 Win32 新面（服务/窗口等新拆分 crate）
- 定位：2025 内 windows-services 扩展（2025-05，扩展控制码/hosting/testing）、windows-window 0.0.0 占位出现——官方在把"非 API 调用"（服务宿主、窗口循环）也工程化。
- 可抄机制：① ADV 常驻形态若落地，服务宿主走官方 crate 而不是手写 SCM ② 关注 release notes 节奏（issue 时间锚为证）。
- 档位：**watch**。第一步动作：订阅 windows-rs releases。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（新件实验属性强，锁版本）。
- 链接：https://github.com/microsoft/windows-rs/releases

### X11-27 Microsoft 开源动作（windows 系）
- 定位：与本域直接相关的微软开源面：microsoft/windows-rs（生成器+拆分 crate）、microsoft/win32metadata（元数据源）、microsoft/wil（C++ 对照面），均活跃维护；opensource.microsoft.com 收录索引。
- 可抄机制：① 上游 issue/PR 是最权威的"机制为什么这样"文档（如 windows-registry 发布节奏 issue #3635）② 元数据仓库可反查某 API 的签名/归属命名空间（特性门决策依据）。
- 档位：**reference**。第一步动作：无（检索习惯入库）。
- 许可证/成熟度：MIT 系 / 维护中（微软官方）。
- 链接：https://opensource.microsoft.com · https://github.com/microsoft/win32metadata

---

## 2. Top-3（本域对 ADV 最先动手的三件）

1. **X11-08/09 Job Object 总账**：进程池内存/CPU/IO 限速 + KILL_ON_JOB_CLOSE 清理 + IO_COUNTERS 账本——一次接入同时解决"不扰民、防失控、可计量"三件事，是 perf-core 的地基件。
2. **X11-01 windows crate 特性门收敛**：按命名空间门控 + windows-sys 热路径分层 + 缺门驱动增量，直接写进依赖规范（配置范例见重点回答①），影响后续所有 FFI 引入方式。
3. **X11-12~17 路径总规范聚合**：长路径/大小写/归一化/reparse/ADS/短名/共享冲突七条规则合一份 spec（重点回答③已是成文），扫描正确性（防循环、防绕过、覆盖 ADS）与可移植性都压在这份规范上。

## 3. 与既有域边界
- R3（调试/ETW）、C3（链接/启动）、C5（USN/MFT/原子写/elevated 档）、E7（Segment Heap）、08（sandbox/AppContainer 判定）、X10（WER）：本文不重复其结论，只在其上补 API 面与工程规范；冲突时以各专域为准并以本文为聚合索引。
