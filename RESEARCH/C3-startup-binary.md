# C3：CLI 启动延迟与二进制体积/链接（深挖）

> 域主：C3 · 日期：2026-10-03 · 分支 adv-rewrite · 方法：WebSearch/WebFetch（2025–2026 资料优先）+ 机制级拆解
> 口径声明：本域所有"量级 ms"数字分两类——【锚】= 有来源+口径；【估】= 社区常见口径、无单一权威页，**必须在 ADV 本机复测后才可作为门禁判据**。冷/热启动区分：冷=页面缓存无该二进制/首次运行，热=重复调用（hyperfine 口径，C1 已覆盖，本域不重复超线程数据）。
> wave-0 待验证项消解：**BOLT 对 Windows PE 不可行**（见 C3-009）；**rust-lld on MSVC 已可手动启用**（见 C3-001）。

---

## 0. 结论速览（Top-3）

1. **C3-001 rust-lld on Windows MSVC**：一行 `.cargo/config.toml` 即可启用，随 toolchain 自带、无需 VS；Rust 1.90（2025-09）已把 LLD 设为 `x86_64-unknown-linux-gnu` 默认，MSVC 侧跟进方向明确（rust#71520 open）。收益在构建迭代/CI 时长与去 VS 依赖，非运行期启动。
2. **C3-008 PGO（Windows llvm-tools 链路）**：rustc 一等支持的启动/吞吐优化路径；C1/wave-0 锚 +10~15%（rust 官方发布自身即 PGO 构建）。ADV 用"真实仓扫描/检索"作训练工作负载即可，Windows 实操路径本文已拆。
3. **C3-003/005/006 release profile 三件套 + C3-012 cargo-bloat 账本**：`lto="thin"`（或发布版 `fat`）+ `panic="abort"` + `strip="symbols"`，是体积/冷启动收益最大、风险最低的组合；但必须先 cargo-bloat 记账后动刀，且与测试/符号分发冲突点已列（§5）。

反面结论（省投入）：**BOLT 不吸收**（PE 无支持）；**mold 不适用于 Windows**（ELF-only）；**Wild 上游 ELF-only**（Windows 只在 fork reld 有动议，watch）。

---

## 1. Windows Rust CLI 启动完整成本账（重点回答①）

阶段分解（从 shell 敲回车到首字节输出）。每阶段给机制、量级与锚：

| # | 阶段 | 机制 | 量级【类型】 | 主要影响因子 |
|---|------|------|------------|------------|
| P0 | 进程创建 | `CreateProcessW`：打开镜像文件→建节区/初始线程/令牌/句柄继承/环境块复制 | 1–10 ms【估，2026-10-03 检索未见单一权威基准页；判据=本机 QPC 环测 + WPR 拆相】 | AV 过滤驱动、环境块大小、继承句柄数 |
| P1 | 加载器 | ntdll loader：导入表解析（逐 DLL LoadLibrary 语义）、基址重定位、TLS 回调、节映射（缺页） | 与导入 DLL 数量+重定位量线性；静态链接少导入则短【估】 | CRT 动/静态（/MD 引 vcruntime140+ucrtbase）、二进制大小（冷启动页数） |
| P2 | CRT/运行时初始化 | MSVC CRT startup → Rust `lang_start`（std 初始化、主线程栈、参数收集）；Rust 无 C 式全局构造风暴，静态构造量小 | <1 ms 级【估】 | 全局 `OnceLock`/`LazyLock` 是否被启动路径触碰（见 C3-022） |
| P3 | main→首输出 | 应用层：args 解析、配置加载、索引/缓存打开、正则编译等 | ADV 完全可控；典型反模式可吃掉几十 ms【应用层实测】 | 懒初始化纪律（C3-022）、冷缓存（C3-026） |
| P4 | 环境一次性成本 | Defender 实时扫描新镜像、SmartScreen（MOTW）、代码签名校验 | 首跑可达数十~数百 ms【估，社区口径】 | 签名与否、目录排除、是否首次（C3-026） |

要点：
- **热启动的权重结构**：P0+P1+P2 与"应用写了什么代码"几乎无关，是 CLI 的地板价；ADV 能优化的只有 P3（懒初始化）与"减小 P1 的体积因子"（strip/LTO 少页）。
- **进程创建（P0）在 Windows 比 Linux fork+exec 重**（无 COW 语义、令牌/句柄工作多），因此"多进程流水线"在 Windows 的每进程税更高——批处理档用进程池摊销（C3-018/019）。
- 启动 trace 工具层见 C3-020（WPR/ETW 把 P0–P3 拆成带时间戳的相）；Linux 对照层见 C3-021。
- C1（`C1-cli-acceleration.md`）已有 rg/fd/fzf 的 hyperfine 实测口径，本域启动预算直接继承其测法，不另起炉灶。

---

## 2. 对象卡片（26 个；机制级 ≥15）

> 每卡：定位 → 可抄机制 → 档位【吸收|有界|不吸收|watch|reference】→ 第一步动作 → 许可证/成熟度 → 链接。
> 档位定义沿用 PROGRAM.md：吸收=进 BLUEPRINT 施工队列；有界=限条件下用；watch=跟踪不入库；reference=仅供对照；不吸收=机制不适用并说明理由。

### C3-001 rust-lld on Windows MSVC（链接器）
- **定位**：LLVM 链接器 rust-lld（MSVC 目标走 lld-link flavor），随 rustup toolchain 分发（`rustup which rust-lld` 或 toolchain `lib/rustlib/x86_64-pc-windows-msvc/bin/`），不依赖 Visual Studio。
- **可抄机制**：1) `.cargo/config.toml`：`[target.x86_64-pc-windows-msvc] linker = "rust-lld"`（必要时补 `rustflags = ["-C", "linker-flavor=lld-link"]`）；2) 用作 CI 临时工具链：省去 VS Build Tools 安装与授权噪声；3) 1.90 起 Linux-gnu 已默认 LLD（releases.rs/docs/1.90.0），MSVC 默认化由 rust-lang/rust#71520 跟踪（截至 2026-10-03 检索仍 open）——上游方向与手动启用一致，先手动即与未来默认对齐。
- **档位**：**吸收**（构建期收益：链接时间、CI 瘦身；对运行期启动无直接差）。
- **第一步动作**：ADV release 构建 A/B：link.exe vs rust-lld 各测 3 次链接耗时；并验证 lld-link 产 PDB 在 WinDbg/`cargo run` 断点下可用（PDB 兼容性是主要风险，判据不绿则只用于 CI 构建档）。
- **许可证/成熟度**：随 Rust（MIT/Apache-2.0）；LLD 上游成熟，MSVC COFF 后端被 Chromium/Clang 自身长期使用；rust 侧默认化未落地=手动启用属"维护中但非默认"。
- **链接**：https://github.com/rust-lang/rust/issues/71520 · https://releases.rs/docs/1.90.0/ · https://doc.rust-lang.org/rustc/tools/lld.html

### C3-002 MSVC link.exe 链接档（FASTLINK / INCREMENTAL / OPT:REF,ICF / PDB 生成）
- **定位**：Windows 默认链接器；其调试信息与优化开关决定"链接耗时↔可调试性↔体积"的档位。
- **可抄机制**：1) `/DEBUG:FASTLINK`：PDB 只引用 obj/lib 内调试信息而非拷贝→链接快，但 PDB 不可分发、多数剖析工具读不了；**VS2019 16.8+ 官方口径已不再推荐**（链接器基础改进后全量 /DEBUG 足够快，GDK 文档明示）——即"FASTLINK 时代已过"；2) `/INCREMENTAL` 仅对 dev 迭代有意义，发布档必关；3) `/OPT:REF`（去未引用 COMDAT）+ `/OPT:ICF`（相同代码段折叠）减体积；rustc 在 MSVC release 是否已默认传入需在 ADV 用 `cargo build -v` 核实，未默认则 `rustflags=["-C","link-args=/OPT:REF,/OPT:ICF"]` 复测。
- **档位**：**有界**（dev 档迭代提速；发布档按核实结果决定）。
- **第一步动作**：`cargo build -vv` 记录 rustc 实际传给 link.exe 的参数集，归档为 ADV 链接档基线。
- **许可证/成熟度**：Microsoft 专有，随 VS；维护中。
- **链接**：https://devblogs.microsoft.com/cppblog/improved-linker-fundamentals-in-visual-studio-2019/ · https://learn.microsoft.com/en-us/gaming/gdk/docs/tools/tools-console/visualstudio/compiler-switch-recommendations · https://devblogs.microsoft.com/cppblog/faster-c-build-cycle-in-vs-15/

### C3-003 LTO 全档（off / thin-local / thin / fat）
- **定位**：跨编译单元内联+死码消除；`fat`（=`lto=true`）全量、`thin` 跨 crate 增量式、`thin-local` 仅本 crate。Cargo Book 官方口径：thin 耗时显著低于 fat 而"性能收益与 fat 相近"。
- **可抄机制**：1) 默认 release 档 `lto="thin"`（构建/收益平衡）；2) 另立 `profile.release-lto inherits="release"` 档用 `lto="fat"` + `codegen-units=1` 供发布裁剪；3) 体积锚：Ubuntu Discourse 关于 LTO 默认化的讨论给 ~15% 体积缩减口径（2026-10-03 检索转述）；bitdrift 2025-10 案例："fat 比 thin 更省体积（thin 更快但效果弱）"。对启动：体积小→冷启动缺页少，热启动收益微弱——**别用启动数字当 LTO 的主判据，用体积账**。
- **档位**：**吸收**。
- **第一步动作**：ADV 当前 release 配置下三档各出一份 `cargo build --release` 产物体积+链接时长账（口径：同机器、同依赖锁文件）。
- **许可证/成熟度**：rustc 内建；维护中。
- **链接**：https://doc.rust-lang.org/cargo/reference/profiles.html · https://blog.bitdrift.io （2025-10 案例）· https://doc.rust-lang.org/rust/codegen-options/index.html

### C3-004 codegen-units
- **定位**：release 默认 16（并行编译）；`=1` 把全 crate 交给单优化窗口，与 LTO 叠加进一步缩体积、边际改善运行期，编译时间上升。
- **可抄机制**：仅放进 release-lto 档与 fat LTO 同用；dev 档不动。
- **档位**：**有界**（编译时长换体积，CI 档要核时长）。
- **第一步动作**：release-lto 档内 A/B `codegen-units=1 vs 16` 的体积/编译时长。
- **许可证/成熟度**：rustc 内建；维护中。
- **链接**：https://doc.rust-lang.org/cargo/reference/profiles.html

### C3-005 panic=abort
- **定位**：去展开（landing pad/异常表）与 unwinder 依赖→体积降（min-sized-rust 首推档之一）；CLI 无需 unwind 语义。
- **可抄机制**：1) `[profile.release] panic="abort"`；2) Cargo 对 test/bench profile 自动忽略 panic 设置（保持 unwind）——测试不受伤；3) **PE 侧注意点**：x64 Windows 栈回溯依赖 `.pdata/.xdata`，panic=abort 后崩溃转储回溯质量是否受影响须实测（配合崩溃分析链，见 R3 域）——此项入判据再定档。
- **档位**：**吸收**（先在 ADV 上量体积差与回溯质量）。
- **第一步动作**：开 abort 前后各记一份体积 + 人为触发 panic 验证回溯可读性。
- **许可证/成熟度**：rustc 内建；维护中。
- **链接**：https://github.com/johnthagen/min-sized-rust · https://doc.rust-lang.org/cargo/reference/profiles.html

### C3-006 strip 档与 PDB/符号分发
- **定位**：`profile.strip = "none"|"debuginfo"|"symbols"`（Cargo 1.59+ 稳定）；PE 侧 debuginfo 天然在独立 PDB 里，`strip="symbols"` 删的是 exe 内 COFF 符号表——体积直接降，且不损 PDB 调试。
- **可抄机制**：1) release 档 `strip="symbols"` + PDB 归档进符号目录（或随 release zip 附带）；2) `debuginfo="0"` release 档避免 PDB 生成开销；3) 与 C3-012 冲突：cargo-bloat 需要符号，**先记账后 strip**（或用无 strip 的旁路 profile 记账）。
- **档位**：**吸收**。
- **第一步动作**：确认 ADV release profile 现值，补 `strip="symbols"`，记录体积差。
- **许可证/成熟度**：Cargo 内建；维护中。
- **链接**：https://doc.rust-lang.org/cargo/reference/profiles.html

### C3-007 opt-level 3 / s / z
- **定位**：3=速度（release 默认）；s=按尺寸优化；z=最小（关 loop 向量化——Android BUILD.gn 同口径，见检索摘要）。
- **可抄机制**：CLI 冷启动视角 z/s 缩代码足迹→冷缺页少；但 ADV 主体是扫描/检索吞吐（热点路径吃 speed）——**主体保持 3**，s/z 只考虑给"启动敏感且非热点"的独立小二进制（若有）。
- **档位**：**有界**。
- **第一步动作**：无动作；列入"若拆小子工具"时的选项。
- **许可证/成熟度**：rustc 内建；维护中。
- **链接**：https://github.com/johnthagen/min-sized-rust · https://doc.rust-lang.org/cargo/reference/profiles.html

### C3-008 PGO（Windows MSVC 链路，实操）
- **定位**：rustc 一等支持（`-Cprofile-generate/-Cprofile-use`），运行期 +10~15%（C1/wave-0 锚；rust 官方发行版自身即 PGO 构建）。PGO 也常与 BOLT 组合（LLVM Discourse 2024-07 有官方建议帖），Windows 侧 PGO 即为 BOLT 缺位的替代主力。
- **可抄机制**（Windows 实操顺序）：1) `rustup component add llvm-tools-preview`；2) `set RUSTFLAGS=-Cprofile-generate=D:\pgo` 后 `cargo build --release`；3) 跑代表性工作负载（ADV：对固定样例仓执行一轮 scan+retrieve+MCP 调用），注意 `%LLVM_PROFILE_FILE%` 控制 `.profraw` 落点；4) `llvm-profdata merge -o merged.profdata D:\pgo`（llvm-profdata 在 toolchain 的 `lib/rustlib/x86_64-pc-windows-msvc/bin/`，PATH 是头号坑）；5) `set RUSTFLAGS=-Cprofile-use=...merged.profdata` 重建。陷阱：训练负载必须覆盖真实热点（只跑 `--help` 会得到负优化）；profile-use 与 LTO 需同一次 RUSTFLAGS 组合。
- **档位**：**吸收**（perf-core；发布管线一次性成本，产物带 profile 复训周期）。
- **第一步动作**：写 `scripts/pgo-build.ps1`（或 python）固化 5 步；先在 bench 子命令上验证 +x% 再推广。
- **许可证/成熟度**：rustc/LLVM 内建；维护中。
- **链接**：https://doc.rust-lang.org/rustc/profile-guided-optimization.html · https://discourse.llvm.org/ (BOLT+PGO 建议, 2024-07)

### C3-009 BOLT（post-link 二进制优化器）——wave-0"待验证"消解
- **定位**：LLVM 旗下 post-link 优化器；官方 README 定位 ELF（后加 PIE/so），剖析链路（perf2bolt）依赖 Linux `perf`。
- **结论（2026-10-03 检索）**：**无任何 PE/COFF 支持落地、RFC 或进行中补丁的公开信号**；且 ELF 侧已有质量案例（llvm#60045 BOLT 优化 clang/lld 后段错误，2023-01；llvm#124601 BOLT 用于 Postgres 17.2 吞吐/延迟回退 ~20%，2025-01）——即便在 Linux 上也非免费午餐。
- **可抄机制**：无可抄（Windows PE 不可用）；间接机制=**用 PGO（C3-008）补位**，BOLT 的"真实执行路径驱动布局"思想可由 PGO 的基本块/边频次近似承接。
- **档位**：**不吸收**（Windows PE）；Linux ELF 侧标记 watch（若 ADV 未来出 Linux 发行档，评估随附成本）。
- **第一步动作**：无（把 wave-0 条目从"待验证"改判为"不吸收：PE 无支持"）。
- **许可证/成熟度**：LLVM Apache-2.0（维护中，Meta 主导）；PE 侧=实验缺失。
- **链接**：https://github.com/llvm/llvm-project/tree/main/bolt · https://github.com/llvm/llvm-project/issues/124601 · https://github.com/llvm/llvm-project/issues/60045

### C3-010 mold
- **定位**：Rui Ueyama 的高速 ELF 链接器；**ELF-only**，无 Windows 支持。
- **可抄机制**：无直接机制可抄（Windows 不可用）；其"并行布局+快速符号解析"的工程手法只在读代码层面有参考价值。
- **档位**：**reference**（Linux CI 侧若嫌 lld 慢可再比较，ADV 当前无此需求）。
- **第一步动作**：无。
- **许可证/成熟度**：AGPL-3.0（作者 2023 年调整许可证；以仓库 LICENSE 为准）——**红线：源码不可混入 ADV**，仅可作外部工具。
- **链接**：https://github.com/rui314/mold

### C3-011 Wild 链接器（与 reld fork）
- **定位**：Rust 写的高速 ELF 链接器（David Lattimore，2025–2026 高频迭代），上游 **ELF/Linux-only**；Hysen Labs 对比文明确警告"需要增量链接/Mach-O/Windows 支持就别采用"。
- **可抄机制**：1) 上游无 Windows；2) **前沿信号**：2026-08 出现 fork **reld**，宣称以 Windows/Linux/macOS 同级为目标 + 增量链接架构（单点信号，成熟度未证）；3) Chromium 已在部分 Android 构建集成 Wild（ELF 场景背书）。
- **档位**：**watch**（上游 ELF-only 且 Windows 未排期；reld 达到可用再升格）。
- **第一步动作**：季度复查 reld 仓库进度（链接器换血是 CI 档收益，不是启动收益）。
- **许可证/成熟度**：MIT（上游）；reld=实验性。
- **链接**：https://github.com/wild-linker/wild · https://github.com/zackees/reld

### C3-012 cargo-bloat（体积账）
- **定位**：对最终产物做 crate/函数级体积归账；是"先量后改"的体积侧判据工具。
- **可抄机制**：1) `cargo bloat --release --crates`（crate 级占比）；2) `cargo bloat --release --filter adv_*`（自家函数）；3) 依赖符号信息→**在 strip 之前跑**，或用无 strip 旁路 profile；CI 里可做体积回归门（阈值+锚）。
- **档位**：**吸收**（ci-ops 门 + perf-core 账本）。
- **第一步动作**：ADV release（strip 前）跑一份基线账落盘，作为 §4 预算书的体积侧锚。
- **许可证/成熟度**：MIT/Apache-2.0；维护中（rafalh/cargo-bloat）。
- **链接**：https://github.com/rafalh/cargo-bloat

### C3-013 cargo-asm / cargo-show-asm（汇编账）
- **定位**：查看函数最终汇编，验证 opt-level/target-cpu 是否真生效（向量化、内联）。
- **可抄机制**：原 cargo-asm（gnzlbg）多年低维护；改用维护中的 **cargo-show-asm**（`cargo asm` 子命令，支持 Intel 语法/过滤）。用途偏体积/热点核查，启动域属辅助。
- **档位**：**watch**（启动预算不直接需要；热点域用）。
- **第一步动作**：无（热点优化域接入）。
- **许可证/成熟度**：MIT；cargo-show-asm 维护中，cargo-asm 弱维护。
- **链接**：https://github.com/pacak/cargo-show-asm · https://github.com/gnzlbg/cargo-asm

### C3-014 -Zbuild-std / std-aware cargo
- **定位**：nightly 重编 std（可裁 feature、std 侧也吃 panic=abort/LTO），体积进一步下探；min-sized-rust 的进阶档。
- **可抄机制**：仅 nightly + 编译时长/工具链复杂度显著上升；对"本地优先平台"的主 CLI，收益/成本比差。
- **档位**：**watch**（除非未来做超小子工具）。
- **第一步动作**：无。
- **许可证/成熟度**：rustc nightly；实验性（std-aware cargo 长期未稳定）。
- **链接**：https://github.com/johnthagen/min-sized-rust

### C3-015 windows-link / windows-sys（no_std 直连系统调用）
- **定位**：微软 windows-rs 家族 2025-02 首发独立 crate `windows-link`：以 `link` 宏 + raw-dylib 机制直接绑定系统库（kernel32 等），无需 import lib、无需 std——`windows`/`windows-sys` 均已改依赖它。机制=编译期 `extern "system"` 声明 + 链接器合成导入，运行期零解析开销。
- **可抄机制**：1) ADV 的 Windows 专属路径（进程/句柄/文件元数据）用 windows-sys 直连可减少抽象层；2) no_std 化 CLI 是"去 std 精简运行时"的极端路线——对 ADV 平台型 CLI 不划算（std 依赖太深）；3) 该路线真正价值：**为"少 DLL 导入、轻 P1 加载"提供工具面**。
- **档位**：**有界**（按需用 windows-sys 精准调用；不做 no_std 全量改造）。
- **第一步动作**：盘点 ADV 现有 Win32 调用点，评估从包装层直连 windows-sys 的减依赖空间。
- **许可证/成熟度**：MIT/Apache-2.0（Microsoft 维护，维护中）。
- **链接**：https://crates.io/crates/windows-link · https://github.com/microsoft/windows-rs

### C3-016 CRT 动/静态（/MD vs /MT，`crt-static`）
- **定位**：Rust MSVC 目标默认 /MD（动态 CRT：vcruntime140.dll + ucrtbase.dll）；`-C target-feature=+crt-static` 切 /MT 静态。
- **可抄机制**：1) /MT → 单 exe 免 VC redist、P1 导入表更短；但系统 DLL（ucrtbase）几乎总已驻留且可共享映射，**启动差预期微小**（判据待测）；体积升（CRT 静态并入）；2) /MT 的主要收益=部署独立性（绿色单文件），不是启动；3) 与符号/崩溃分析链的交互需复核（R3 域）。
- **档位**：**有界**（若 ADV 决策"单文件绿色分发"则 /MT；否则保持默认 /MD）。
- **第一步动作**：在启动账（P1 阶段）里实测 /MD vs /MT 各一组的 WPR 拆相差，用数据定档。
- **许可证/成熟度**：rustc/MSVC 内建；维护中。
- **链接**：https://doc.rust-lang.org/rust/codegen-options/index.html (crt-static) · https://github.com/johnthagen/min-sized-rust

### C3-017 -C target-cpu=native vs 发布基线
- **定位**：native 打开本机全部指令集（AVX2 等）→热点循环更快；但对启动几乎无关（启动路径无长循环），且**产物不可移植**。
- **可抄机制**：发布档禁止 native；如需提吞吐可显式 `-C target-cpu=x86-64-v2/v3` 分发档（明确受众），或运行期特性探测（std::arch is_x86_feature_detected）。
- **档位**：**不吸收**（native 用于发布）；x86-64-v3 发行档=有界备选。
- **第一步动作**：无（红线入 CI：发布构建不带 native）。
- **许可证/成熟度**：rustc 内建；维护中。
- **链接**：https://doc.rust-lang.org/rust/codegen-options/index.html

### C3-018 std::process::Command / CreateProcess 成本纪律
- **定位**：Windows 上每次子进程 = P0 全程（镜像打开+AV+令牌+环境块），社区口径每次 spawn 1–10ms 级【估，需本机复测】；Rust std 的 `Command` 在此之上还有环境块/句柄继承构造。
- **可抄机制**：1) 批处理路径**逐文件 spawn 是反模式**——进程内并行优先（rayon/线程池），进程边界只留给隔离需求；2) 必须 spawn 时：精简环境变量（env_clear+白名单）、关继承句柄；3) 计数器：CI 记录"每任务 spawn 数"作性能回归指标。
- **档位**：**吸收**（纪律级，进编码规范）。
- **第一步动作**：rg 全仓 `Command::new` 调用点，标注哪些在热路径。
- **许可证/成熟度**：std 内建；维护中。
- **链接**：https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw

### C3-019 进程预启动池（批处理档）
- **定位**：旧仓已验证的模式（背景附录口径）；预启动 N 个常驻 worker，任务经行协议分发，摊销 P0/P1/P2 的每进程税。
- **可抄机制**：1) worker 常驻 + stdin/stdout NDJSON 行协议（或本地命名管道）；2) Windows Job Object 挂全部 worker→父进程死则级联终止（防孤儿）；3) 与 C1 域的并发遍历分工：进程池只包"需隔离/异构运行时"的子任务，同语言负载用线程。
- **档位**：**有界**（仅批处理/CI 档；交互档禁止常驻残留）。
- **第一步动作**：把旧仓进程池实现里的 Job Object+行协议两件套整理进 ADV 设计稿（不迁移代码）。
- **许可证/成熟度**：自研模式；成熟度=旧仓实测（口径留旧仓档案）。
- **链接**：Windows Job Object：https://learn.microsoft.com/windows/win32/procthread/job-objects

### C3-020 WPR/ETW 启动 trace（Windows tracer 层）
- **定位**：hyperfine 只给总时长；WPR/ETW 把启动拆成带时间戳的相（进程/线程生命周期、镜像加载、DiskIO、CPU 采样）——是 §1 成本账唯一能落到"P0/P1/P2 各占多少"的仪器。
- **可抄机制**：1) `wpr -start CPU -start FileIO -start GeneralProfile` → 跑一次 CLI → `wpr -stop D:\trace.etl`；2) WPA 打开看 Process Lifetime / Image Load / CPU Usage (Sampled)，把 main 时间与 loader 时间分开；3) 常态化：每档 profile 变更后录一次 trace 留档（否证也要留档）。
- **档位**：**吸收**（perf-core 仪器）。
- **第一步动作**：录制 ADV 当前 `adv --version` 与 `adv scan <小仓>` 的基线 trace，回填 §1 表格实测列。
- **许可证/成熟度**：Windows SDK/ADK 随附（免费）；维护中。
- **链接**：https://learn.microsoft.com/windows-hardware/test/wpt/windows-performance-recorder

### C3-021 strace -c / perf sched（Linux 对照层）
- **定位**：Linux 侧启动拆相：`strace -c ./cli` 给 syscall 账（execve/mmap/openat 数量↔依赖数），`perf sched` 给调度延迟。ADV 若出 Linux CI 档，这是同仪器的另一端。
- **可抄机制**：CI 里对 Linux 产物跑 `strace -c` 记录 syscall 计数作依赖膨胀哨兵。
- **档位**：**reference**（Windows 主开发机用不上；CI 可选）。
- **第一步动作**：无。
- **许可证/成熟度**：GPL 工具链（外部使用，不混入源码）；维护中。
- **链接**：https://man7.org/linux/man-pages/man1/strace.1.html

### C3-022 LazyLock 懒初始化纪律（反模式清单）
- **定位**：Rust 无 C 式静态构造风暴，P2 天然轻；真正的启动税来自**启动路径触碰重型惰性全局**（`OnceLock<HashMap>` 建/加载索引、正则全量编译、配置重解析）。std 1.80（2024-07）起有稳定 `LazyLock/LazyCell`。
- **可抄机制**：1) 启动路径（main→首输出）零重活：args 解析轻量化，`--version/--help` 不触碰任何索引/缓存；2) 重型全局全部 `LazyLock` 包裹 + 审计"首次解引用发生在哪条命令路径"；3) 打印尽早：进度头先输出，重活后置（感知延迟下降）。
- **档位**：**吸收**（编码规范级）。
- **第一步动作**：审计 ADV 全部静态全局，逐个标注"哪条命令路径会首次解引用"。
- **许可证/成熟度**：std 内建；维护中。
- **链接**：https://releases.rs/docs/1.80.0/ · https://github.com/BurntSushi/ripgrep（同手法标本）

### C3-023 ripgrep 标本（启动视角）
- **定位**：CLI 启动纪律的社区标杆（C1-001 已对吞吐侧深挖，本域只取启动侧）：低频路径正则按需编译、无启动期全局预建、`--version` 极轻。
- **可抄机制**：见 C3-022 三条；另加"帮助文本与 clap 配置结构体惰性构建"。
- **档位**：**reference**（手法已并入 C3-022 吸收项）。
- **第一步动作**：无。
- **许可证/成熟度**：MIT/Unlicense；维护中。
- **链接**：https://github.com/BurntSushi/ripgrep

### C3-024 zoxide / bat 标本
- **定位**：小而快 CLI 的启动样本。zoxide：单二进制、查询本地 db 的毫秒级响应；bat：stdout 非 tty 时跳过高亮管线（快路径检测先于重活）。2025–2026 的 CLI 工具对比文（haril.dev 2025-03 等）均以"单二进制+毫秒级"为卖点口径，但**未见公开的逐工具启动毫秒权威表**——数字一律走本机 hyperfine（C1 口径），不引二手 ms 值。
- **可抄机制**：1) bat 的"输出目标检测决定是否进重管线"前置；2) zoxide 的"数据文件小、 mmap/单次读"。
- **档位**：**reference**。
- **第一步动作**：无。
- **许可证/成熟度**：MIT/Apache-2.0；均维护中。
- **链接**：https://github.com/ajeetdsouza/zoxide · https://github.com/sharkdp/bat · https://haril.dev（2025-03 CLI 工具文）

### C3-025 cargo-dist（安装器/分发）
- **定位**：axo.dev 的发布打包器：产出 shell/PowerShell 安装器 + GitHub Releases 归账；0.30.x 系列持续维护（2025–2026 仍有项目新采纳，检索锚）。
- **可抄机制**：1) 安装器只做下载/解压/PATH——**不改变首次运行的 P4 成本**（AV/SmartScreen 对下载来的新 exe 照扫，见 C3-026）；2) 若 ADV 决定对外分发，cargo-dist 是"少写 CI 胶水"的参考实现；ADV 本地优先定位下仅参考其 PowerShell 安装器写法。
- **档位**：**reference**。
- **第一步动作**：无（分发决策另立域）。
- **许可证/成熟度**：MIT/Apache-2.0；维护中。
- **链接**：https://github.com/axodotdev/cargo-dist

### C3-026 首跑环境成本（Defender/SmartScreen/页面缓存）
- **定位**：Windows 特有的一次性启动税：实时过滤驱动扫描新镜像、MOTW 触发 SmartScreen、冷页面缓存缺页。社区口径首跑可比热跑慢一个量级【估，无权威页；判据=同机清缓存复测】。
- **可抄机制**：1) 测法隔离：预算书口径必须注明"热启动为主指标，冷启动单列"；2) 缓解：Authenticode 签名（减 AV 启发与 SmartScreen 声誉积累）、安装器落盘时触发预扫描、文档给"排除目录"指引；3) 与 C2 域 shell 集成的关系：PATH 常驻 shell 缓存命令路径，不缓存页面。
- **档位**：**reference**（观测口径，不入门禁）。
- **第一步动作**：在启动账模板里固定"冷/热两列 + 是否首跑"字段。
- **许可证/成熟度**：n/a（环境行为）。
- **链接**：https://learn.microsoft.com/windows/win32/seccauth/authenticode（签名机制）

---

## 3. 2025–2026 前沿信号

- **LLD 默认化浪潮**：Rust 1.90（2025-09）把 rust-lld 设为 `x86_64-unknown-linux-gnu` 默认链接器（releases.rs 锚）；MSVC 侧 rust#71520 持续 open——"rust 自带链接器、去外部依赖"是上游明确方向，Windows 手动启用即提前对齐。
- **Rust 原生链接器生态**：Wild（ELF，2025–2026 高频迭代、被 Chromium 部分采用）→ 2026-08 出现 fork reld 主张 Windows/macOS 同级支持 + 增量链接（单点信号，未证成熟）。Windows 侧半年一复查。
- **BOLT 停在 ELF**：2025–2026 检索无 PE/COFF 支持推进；且 ELF 侧出现回归案例（Postgres 17.2 ~20%，2025-01）——post-link 优化赛道在 Windows 由 PGO 独挑。
- **FASTLINK 退场**：VS2019 16.8+ 官方口径不再推荐 `/DEBUG:FASTLINK`（全量 /DEBUG 已快），VS2022 17.14 推 C++ Dynamic Debugging——Windows 调试信息策略应按"全量 PDB + 分发"设计，不再绕 FASTLINK。
- **windows-rs 拆薄**：`windows-link`（2025-02 首发）+ raw-dylib 让 Win32 绑定 no_std 化、零 import-lib——系统调用直连的工具面已成熟，std 精简路线的技术障碍消失（商业动机仍缺）。
- **体积档成为发布默认话题**：Ubuntu Discourse 讨论对部分包默认开 LTO（~15% 体积口径）；bitdrift（2025-10）公开移动端 fat-LTO 缩体积案例——上游默认收紧是趋势，ADV 提前落 profile 档位即顺势。
- **cargo-dist 仍活跃**（0.30.x，2025–2026 有新采纳）：分发侧生态稳定，但与本域启动预算正交。

---

## 4. ADV 启动预算书（重点回答④）

> 性质：**目标值提案**，非已测事实。判据走真路径：hyperfine（总时长，C1 口径）+ WPR（拆相）双仪器；每个预算项绑定"测法+档位"，未标数字的项 = 待基线回填。

### 4.1 分档目标

| 档 | 场景 | 指标 | 目标（提案） | 判据仪器 |
|----|------|------|------------|---------|
| 交互档 | 命令行单次调用（scan 单仓/retrieve/query） | 热启动→首输出 | ≤30 ms | hyperfine 热循环中位数（C1 口径） |
| 交互档 | 同上 | 冷启动（清缓存/首跑单列） | ≤200 ms | 冷启动脚本 + 注明 AV 状态 |
| 交互档 | `--version` / `--help` | 热启动 | ≤15 ms | hyperfine（零重活纪律的守门员） |
| MCP 常驻档 | MCP 工具面进程 | 启动一次→ready | ≤100 ms，之后常驻复用 | 启动日志时间戳 + WPR 一次拆相 |
| 批处理档 | CI/全仓扫描 | 每进程预算 | **无每进程预算**；指标=吞吐与 spawn 计数（C3-018） | C1 吞吐口径 + spawn 计数器 |

### 4.2 交互档 30 ms 的内部拆分（提案口径，待 WPR 回填）

- P0 进程创建 + P1 加载器 + P2 运行时初始化：目标 ≤15 ms【量级依据：§1 的 1–10ms+<1ms 估算 + 体积控制假设（release-lto 档产物）；**必须以本机 WPR trace 实测为准**】。
- P3 main→首输出：≤15 ms——由懒初始化纪律（C3-022）保证，`--version` 路径零索引触碰。

### 4.3 达成手段清单（映射对象）

1. profile 基建：`lto="thin"`（发布档 fat+`codegen-units=1`）+ `panic="abort"`（回溯质量复核后）+ `strip="symbols"`（C3-003/004/005/006）。
2. 体积账门：cargo-bloat 基线 + CI 体积回归阈值（C3-012）。
3. 链接器：rust-lld A/B 通过后设为 ADV CI 与本地默认（C3-001）。
4. PGO 发布管线：scripts/pgo-build.ps1 固化，真实负载训练（C3-008）。
5. 懒初始化审计：静态全局清单 + `--version` 轻路径守门（C3-022/023）。
6. spawn 纪律 + 批处理进程池（Job Object 级联）（C3-018/019）。
7. 仪器常态化：每次档位变更录 WPR trace 留档（C3-020）。

---

## 5. 体积与启动的取舍规则（重点回答⑤，冲突矩阵）

| 组合 | 冲突 | 裁决规则 |
|------|------|---------|
| strip="symbols" ↔ cargo-bloat/cargo-show-asm 账本 | strip 后符号消失，账本跑不了 | 先账后 strip；账本走无 strip 旁路 profile（C3-006/012） |
| panic="abort" ↔ cargo test | test/bench 需 unwind | Cargo 自动对 test profile 忽略 panic 设置，无需处理；但要复核 PE 崩溃回溯质量（C3-005） |
| opt-level=z/s ↔ 扫描/检索吞吐 | 向量化被关（z） | 主体保持 3；z/s 仅限启动敏感的非热点小二进制（C3-007） |
| lto="fat" ↔ CI/本地链接时长 | fat 显著拉长链接 | 日常档 thin，发布档 fat 单独跑（C3-003） |
| codegen-units=1 ↔ 编译时长 | 并行度归一 | 只进 release-lto 档（C3-004） |
| /MT ↔ 体积与调试链 | CRT 并入体积升，崩溃链路径变化 | 单文件绿色分发才用；否则 /MD 默认（C3-016） |
| rust-lld ↔ PDB 生态兼容 | lld-link PDB 与部分工具链兼容性待验 | A/B 通过（断点/回溯/ETW 符号解析）才设默认（C3-001/002） |
| target-cpu=native ↔ 产物可移植 | 机器特定指令集 | 发布禁 native；吞吐需求走显式 v2/v3 档或运行期探测（C3-017） |
| 体积最小 ↔ 冷启动 | 二者同向（少页=快），但 z 档可能伤热启动 | 冷启动判据看体积档，热路径判据看吞吐档，分别记账（C3-003/007） |

---

## 6. 方法学补充（hyperfine 之外的 tracer 层）

- Windows：WPR/ETW（C3-020）为拆相主仪器；进程创建微观复测用 QPC 环测（父进程循环 CreateProcess+WaitForInputIdle，取中位数/弃首跑）。
- Linux：`strace -c`（syscall 账）、`perf sched`（调度延迟）（C3-021）。
- 纪律：一切 ms 级主张绑"来源+口径+日期"；【估】级数字进预算书前必须升级为【锚】（本机实测）。

---

## 7. 附：检索与锚清单（2026-10-03）

- Cargo Book Profiles（lto/strip/codegen-units/panic 档）：https://doc.rust-lang.org/cargo/reference/profiles.html
- rustc Codegen Options（lto 全档/crt-static/target-cpu）：https://doc.rust-lang.org/rust/codegen-options/index.html
- rustc PGO：https://doc.rust-lang.org/rustc/profile-guided-optimization.html
- Rust 1.90.0 release notes（LLD 默认于 linux-gnu）：https://releases.rs/docs/1.90.0/
- rust-lang/rust#71520（lld default on x64 msvc，open）：https://github.com/rust-lang/rust/issues/71520
- LLVM BOLT README + 议题：https://github.com/llvm/llvm-project/tree/main/bolt · #124601（Postgres 回归）· #60045（clang/lld segfault）
- MSVC 链接器：https://devblogs.microsoft.com/cppblog/improved-linker-fundamentals-in-visual-studio-2019/ · https://learn.microsoft.com/en-us/gaming/gdk/docs/tools/tools-console/visualstudio/compiler-switch-recommendations · https://devblogs.microsoft.com/cppblog/faster-c-build-cycle-in-vs-15/
- min-sized-rust：https://github.com/johnthagen/min-sized-rust
- bitdrift LTO 案例（2025-10）：https://blog.bitdrift.io
- Wild / reld：https://github.com/wild-linker/wild · https://github.com/zackees/reld · Hysen Labs 对比文
- mold：https://github.com/rui314/mold
- windows-link：https://crates.io/crates/windows-link · https://github.com/microsoft/windows-rs
- cargo-dist：https://github.com/axodotdev/cargo-dist
- cargo-bloat：https://github.com/rafalh/cargo-bloat · cargo-show-asm：https://github.com/pacak/cargo-show-asm
- WPR/ETW：https://learn.microsoft.com/windows-hardware/test/wpt/windows-performance-recorder
- CreateProcess 量级【估】：2026-10-03 检索未见单一权威基准页；判据=本机 QPC 环测 + WPR。
