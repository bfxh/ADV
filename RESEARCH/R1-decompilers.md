# R1 · 反汇编器/反编译器全家 + 反编译研究（深挖）

> 域：`R1-decompilers` ｜ 日期：2026-10-03 ｜ 方法：WebSearch/WebFetch 优先 2025–2026 一手发布与论文，机制级拆解；许可证/成熟度均带锚（版本号/日期/出处）。
> 档位口径：【吸收】=机制或接口可进 ADV（含可直连的宽松许可件）；【有界】=仅思想可抄，或许可/依赖受限；【不吸收】=明确排除；【watch】=实验/未成熟，跟踪；【reference】=背景/历史/对照。
> 集成分面：`adv-bin`（二进制面）/`adv-parse`（解析层）/`adv-rules`（规则层）/`adv-ast-rust`（Rust AST 层）/`perf-core`/`watch`/`reference`。
> **口径校正**：任务清单中的 **Lemur** 经检索并非反编译/二进制工作（ICLR'24 的 Lemur 是"LLM 辅助程序验证"，另有同名的语言模型 Lemur-70B）；本域以 **SLaDe（CGO 2024）** 与 **Nova（ICLR 2025）** 补位（§4.6）。
> 共 **50 个对象**（机制级 ≥30）。

## §0 执行摘要

- **深档语义底座**：唯一"许可友好（Apache-2.0）+ 语义覆盖完整 + 已有 Rust 消费口"的路径是 **Ghidra P-code/SLEIGH**（Rust FFI：`jingle_sleigh`/`pcode-rs`，2025–2026 在发布）。其余深档选项要么专有（IDA/BN）、要么 GPL 家族（rev.ng/Remill）、要么已归档（McSema 2022-08）。
- **架构范式**：把"归一 AST → 多层语义 IR"抄 **Binary Ninja BNIL** 的"分层 + 每层稳定契约 + 层间多对多映射 + 按需物化"（专有，只抄设计）。
- **相似性/聚类**：抄 **Ghidra BSim** 的"反编译产物→函数特征向量→向量索引"（Apache-2.0，可读码）；结构匹配兜底抄 **BinDiff** 的 call graph + CFG 同构流程（专有：只抄算法）。
- **LLM 的边界**（锚：DecompileBench, ACL 2025 Findings）：LLM 方法在"可读性"上已超商业工具，但"功能正确性"低 **52.2%** ⇒ 在 ADV 里 LLM 只进**命名/解释/摘要层**，不得作为正确性判据；要评估 LLM 反编译输出，用"重编译+重执行结果一致"（re-executability，LLM4Decompile 口径）而不用文本相似。
- **Rust 生态缺口**（本次检索口径）：没有成熟的"反编译级"通用 Rust 库；Rust 能用的只有前段（解码：iced-x86/yaxpeax-x86）与后段（消费 p-code：jingle_sleigh/pcode-rs），**中间段（lifting→SSA→结构化→类型）需自建，或以子进程方式消费 Ghidra/rizin**。

## §1 A 类：通用反编译平台与框架（对象 1–14）

### 1. Ghidra（NSA 开源逆向平台；v11.4，2025）
- **定位**：全栈逆向平台：反汇编 + 反编译 + 仿真 + 相似性（BSim）+ 脚本（Java/Python）。
- **可抄机制**：
  1. **P-code 值模型**：一切值来源统一为 varnode =（space, offset, size）三元组（寄存器/内存/常量/临时/指针都归一到同一命名空间）——这是"语义 IR 的最小值模型"，可直接映射进 ADV 的 IR 契约。
  2. **数据驱动 ISA**：SLEIGH（见对象 33/§6.4）把"指令编码→语义"声明化，编译为 `.sla` 表；同一规范多路产出（解码/语义/显示）。
  3. **Action/Rule 改写管线 + Heritage SSA**：反编译是"一连串具名 Action（常量传播/死代码/跳转表恢复…）+ 可组合 Rule（窥孔改写）"在同一 IR 上反复跑；SSA 构造（Heritage）与类型推断（typegrp）是独立阶段。
- **档位**：**吸收**（机制+可 FFI；代码 Apache-2.0 也可合规引用）。
- **第一步**：把 §6.2 的"值模型 + 层契约"一页纸写进设计稿；用 jingle_sleigh 在本机 x86-64 上跑通"一个函数 → p-code 文本"实验。
- **许可/成熟度**：Apache-2.0；维护中（锚：11.4/11.4.2，2025；11.4.2 起内部反编译文档已公开，SleighBuilder 等类可见）。
- **链接**：https://github.com/NationalSecurityAgency/ghidra ｜ https://ghidra-sre.org

### 2. Ghidra BSim（函数相似性框架）
- **定位**：Ghidra 内置的"函数级相似性"检索（跨二进制/跨版本找同源函数）。
- **可抄机制**：
  1. **特征向量 = 反编译产物的统计投影**：从 p-code 派生指令类别计数、调用/跳转数、常量、字符串引用等数值特征（文档口径，具体权重以源码为准）——即"不比对源码文本，比对语义中间层的统计指纹"。
  2. **向量索引 + 近邻检索**：LSH 式索引支撑大规模库内检索（Ghidra Docs/BSim 所述）。
  3. **可解释匹配**：命中返回特征差异明细，可人工复核——与 ADV"结论带证据"同构。
- **档位**：**吸收**（adv-bin 相似性最小版即可抄这套：特征 → 向量 → 近邻）。
- **第一步**：读 Ghidra 仓库 BSim 模块源码，抄"特征清单"定义，在 adv-bin P0 里先做"函数级指令直方图 + 调用图度"相似性。
- **许可/成熟度**：Apache-2.0（Ghidra 仓内）；维护中（锚：11.4 周期）。
- **链接**：https://github.com/NationalSecurityAgency/ghidra（GhidraDocs/BSim）

### 3. IDA Pro / Hex-Rays（专有；9.2，2025-09-08 发布）
- **定位**：商业逆向标准件；Hex-Rays 反编译器 + microcode IR + 大型插件生态。
- **可抄机制**：
  1. **Microcode 的"成熟度等级"设计**：同一 IR 上按 maturity（MMAT_* 级别）逐步提升优化/结构化程度，分析插件可按等级挂接——"IR 不是一次成型，而是可观测的逐级精炼"。
  2. **9.2 首次开放 Microcode Viewer**：把反编译流水线各阶段的中间态做成可控视图——"流水线可观测性"作为产品特性（对我们：每阶段产物可落盘、可 diff）。
  3. **插件生态分层**：IDAPython/Domain API/开源 SDK（9.2 起 C++/Python SDK 统一进 ida-sdk 开源仓）——脚本面与核心面解耦。
- **档位**：**有界**（专有：只抄思想，不抄代码/不依赖）。
- **第一步**：在 ADV 分析管线里加"每阶段产物落盘 + diff"（我们自己的可观测性），不必依赖 IDA。
- **许可/成熟度**：专有（商业）；维护中（锚：9.2，2025-09-08；Qt6、zstd IDB、ida-sdk 开源）。
- **链接**：https://hex-rays.com/ida-pro

### 4. Binary Ninja（专有；5.0 "Gallifrey" 2025-04-23）
- **定位**：现代逆向平台；**BNIL 多层 IR** 是其与 IDA/Ghidra 最大差异点。
- **可抄机制**：
  1. **分层 IR 契约**：Lifted IL（原始提升）→ **LLIL**（去 NOP、flag 折进条件指令）→ **MLIL**（寄存器/内存变量化、类型关联、常量传播）→ **HLIL**（结构化控制流、死代码、switch 恢复、AST）；**LLIL/MLIL 各有 SSA 形态**。层间映射是**多对多**（一个 HLIL 表达式对应多条 MLIL/LLIL）——"上层的每个节点都能下钻到低层证据"。
  2. **按需物化**：各层在查询时惰性构建（插件只在需要时取层），不是一次性全算——低成本接多消费者。
  3. **Workflows（activity 编排）**：分析 pass 被抽象成可插拔 activity 的数据流编排（2022 起为一级概念）——等价于"可编排 pass 管线"。
- **档位**：**吸收**（设计范式；专有，代码/API 不可搬）。
- **第一步**：按 BNIL 四层给 adv-parse/adv-ast-rust 定义"层契约表"（每层保证什么、不允许什么），并定义层间映射表的存储格式。
- **许可/成熟度**：专有（EULA）；维护中（锚：5.0，2025-04-23，约 300 项关闭 issue；Sidekick 5.0 2025-07-28）。
- **链接**：https://binary.ninja ｜ https://docs.binary.ninja/dev/bnil-overview.html

### 5. radare2（LGPL-3.0）
- **定位**：CLI 优先的逆向框架；"seek + 命令会话"模型 + 海量小命令。
- **可抄机制**：
  1. **组合命令语言**：单字母命令 + 修饰符（`~` 过滤、`|` 管道、`;` 串联、`@@` 迭代）——"一切分析动作都是一条可组合命令"，天然适配 CLI/代理调用。
  2. **会话式 seek 模型**：分析结果挂在"当前偏移"上下文（flags/comments/xrefs），批处理与交互同构。
- **档位**：**有界**（LGPL：动态链接/独立进程调用可；静态吸收受限）。
- **第一步**：adv-bin 若出 CLI，抄"命令修饰符"设计（例如 `adv-bin disasm <addr> | grep` 之类组合），别自造 DSL。
- **许可/成熟度**：LGPL-3.0；维护中（锚：持续发布，2025–2026 有版本）。
- **链接**：https://github.com/radareorg/radare2

### 6. rizin（LGPL-3.0；r2 的社区分叉）
- **定位**：radare2 分叉后的"更工程化"版本；**ESIL** 是其可仿真 IR。
- **可抄机制**：
  1. **ESIL：字符串化可求值 IR**：每条指令语义编码成字符串（如 `eax,ebx,+=` 形态），由独立 ESIL VM 解释——**低成本获得"可仿真"能力**（相比完整 lifting 到 SSA，工程量小一个量级）；代价是分析能力上限低。
  2. **rz-* 工具拆分**：rz-bin/rz-asm/rz-diff/rz-find/rz-hash 等小工具面——"每个二进制操作一个独立可组合命令"，与 ADV 的 CLI/MCP 面同构。
- **档位**：**吸收**（ESIL 思想 + 工具面拆分；代码 LGPL 仅动态链接/独立进程口径）。
- **第一步**：抄 rz-* 的工具划分做 adv-bin 子命令清单（bininfo/strings/entropy/disasm/symbols/diff）。
- **许可/成熟度**：LGPL-3.0；维护中（锚：2025–2026 持续发布）。
- **链接**：https://github.com/rizinorg/rizin

### 7. rev.ng / revamb（GPL-2.0 整体；部分文件 MIT）
- **定位**：LLVM 路线的完整反编译器（QEMU 前端提升 → QEMU IR → **LLVM IR** → 全量复用 LLVM 生态）。
- **可抄机制**：
  1. **"提升到 LLVM IR 后吃生态"**：lifting 一次，其后所有分析（优化/类型/切片）用现成 LLVM pass + 工具链；反编译输出是"语法有效 C"（DEF CON 33，2025-08 演示口径）。
  2. **全程序类型恢复**：无符号下跨函数/全程序推导复杂类型（演示口径：含链表类结构）——"类型恢复是全局问题，不是逐函数问题"。
  3. **前置提升层模式**：QEMU TCG 的角色说明——"先提升到简单指令 IR，再做语义优化"，与我们 P0（纯解码）→ P2（语义 IR）之间可插一个中间档。
- **档位**：**有界**（GPL-2.0：只抄架构思想；不可静态吸收代码）。
- **第一步**：在深档设计里保留"中间 IR 档"位置（简单 op 序列 → 优化后的语义 IR 可分级落地）。
- **许可/成熟度**：GPL-2.0（整体，因含 QEMU）+ 部分文件 MIT；维护中/商业混合（锚：DEF CON 33 2025-08 演示、仓库 revng 与 revng-c 合并中、GUI 闭测）。
- **链接**：https://github.com/revng/revng ｜ https://github.com/revng/revamb

### 8. angr（BSD-2-Clause；反编译组件是其分析之一）
- **定位**：Python 二进制分析框架（符号执行 + CFG + 反编译），学术界事实标准之一。
- **可抄机制**：
  1. **端到端全 Python 反编译链**：VEX lifting → SSA（含 φ）→ 区域结构化 → **AIL**（angr 的类 C AST）→ 简化/输出；链上每步是可单独调用的 analysis。
  2. **VEX 的"dirty helpers"**：flag 计算推迟到真正需要时（put/get + 惰性求值），避免 per-指令全量 flag 语义——经典性能折衷。
  3. **共享库识别（FLIRT 式）**：先识别库函数再跳过（大规模二进制先做"减法"）。
- **档位**：**吸收**（参考实现读码友好、许可宽松；Python 性能另说）。
- **第一步**：拉 angr 的 decompiler+ssa analysis 当"参考实现"对照我们自设的层契约（读码不接依赖）。
- **许可/成熟度**：BSD-2-Clause（angr/pyvex；底层 VEX 为 GPL-2.0，见对象 31）；维护中（锚：2025–2026 持续发布）。
- **链接**：https://github.com/angr/angr

### 9. BAP + Core Theory（MIT）
- **定位**：OCaml 二进制分析平台；核心贡献是 **Core Theory**——把二进制语义统一成一个可组合理论（Kleene 代数 + 测试 + 内存），所有分析插件在该语义底座上写。
- **可抄机制**：
  1. **"分析共享语义底座"**：语义定义一次，各分析（符号执行/切片/验证）以该理论为契约，避免每工具自造语义。
  2. **插件即独立包**（OCaml 包生态式扩展）。
- **档位**：**reference**（思想对标；OCaml 生态与 ADV 不接轨）。
- **第一步**：在设计评审时用 Core Theory 当"语义契约"反例清单：我们的 IR 契约是否覆盖"内存模型/测试/组合"三要素。
- **许可/成熟度**：MIT；低活跃/维护中（未有 2025–2026 大版本信号，本次检索口径）。
- **链接**：https://github.com/BinaryAnalysisPlatform/bap

### 10. RetDec（MIT；受限维护）
- **定位**：Avast/Gen 的 LLVM 中台反编译器：bin2llvmir（提升）→ llvmir2hll（出 C）。
- **可抄机制**：
  1. **两段式 + LLVM 中台**：提升与输出解耦，中间产物（LLVM IR）可作为独立分析入口。
  2. **检测器先行**：加壳/编译器检测（含 MS-cabinet SFX 等）作为流水线前置——**先识别"处理不了的东西"再谈分析**（对我们：加壳/异形文件要显式降级，而不是硬跑）。
- **档位**：**reference**（可读源码参考；不强依赖）。
- **第一步**：抄"前置检测表"清单（加壳/编译器/异常节表）进 adv-bin 的降级策略。
- **许可/成熟度**：MIT；**受限维护**（自述 limited maintenance，基本维护继续、开发有限；2025-05 仍有提交；官方最新 release 5.0，约 435 open issues——本检索口径）。
- **链接**：https://github.com/avast/retdec

### 11. Reko（C#；GPL-2.0-only）
- **定位**：C# 写的反编译器；突出点是**从 IR 起就带精确类型（exact type）**。
- **可抄机制**：
  1. **类型化 IR 先行**：IR 节点带精确类型 + 重写器（rewriter）按类型规则改写——类型不是后处理，而是 IR 的一等公民。
  2. **ScannerV2 与 stage 化管线**：扫描/反汇编/IL 生成/重写/输出各级可替换（如 delay slot 处理、架构插件多）。
- **档位**：**reference**（C#/.NET 技术栈不接；思想可对照）。
- **第一步**：对照其"类型先行 vs 后置推断"取舍，给 ADV IR 的类型字段定契约（必填/可选/推导来源）。
- **许可/成熟度**：GPL-2.0-only；维护中（锚：0.12.1 2025-07-26、0.12.2 2025-12-18、0.12.3 2026-06-17；"Looking for contributors"）。
- **链接**：https://github.com/uxmal/reko

### 12. Snowman（C++；GPL-2.0）
- **定位**：LLVM 中台的老牌开源反编译器（源码即"模式重写 + C 输出"）。
- **可抄机制**：
  1. **模式重写出 C**：在 LLVM IR 上做"看起来像编译器反向"的模式匹配重写，再打印 C——"输出层是独立模式库"。
- **档位**：**watch**（低活跃）。
- **第一步**：无需动作；仅在其模式库清单里核对"我们 P2 输出层"缺哪些常见模式。
- **许可/成熟度**：GPL-2.0；低活跃（2025–2026 无版本信号，本次检索口径；0.1.0 长期未更新）。
- **链接**：https://github.com/yegord/snowman

### 13. Miasm（Python；GPL-2.0）
- **定位**：CEA 的 Python 逆向框架（自有 IR + 符号执行 Sandbox），论文常用工具。
- **可抄机制**：
  1. **Python 内嵌 IR + 符号执行**：IR 表达式可被 Python 直接操纵/求解，反混淆脚本化成本低（"研究脚本友好"的 IR 设计）。
- **档位**：**reference**（GPL；仅方法论）。
- **第一步**：对照 Miasm 的"脚本 API 面"清单，检查 adv-bin 若暴露分析 API 时的最小操纵集。
- **许可/成熟度**：GPL-2.0；低活跃/维护中（本次检索未逐项核版本）。
- **链接**：https://github.com/cea-sec/miasm

### 14. Triton（Apache-2.0）
- **定位**：动态二进制分析框架（DBA）；符号执行 AST + 回调 + 新式 Triton IR。
- **可抄机制**：
  1. **符号表达式 AST 即 IR**：所有运算先建 AST（带类型），可做简化/求解/切片——"AST 与求解器各自可换"。
  2. **回调式插桩 API**：以指令级回调暴露分析点（对 ADV：规则钩子面的设计参考）。
- **档位**：**有界**（思想 + 接口设计参考；C++/Python 引入成本）。
- **第一步**：抄其"回调注册"接口分层（指令级/内存级/过程级），用于 adv-bin 若做动态/仿真档。
- **许可/成熟度**：Apache-2.0；维护中（锚：2025–2026 持续发布）。
- **链接**：https://github.com/JonathanSalwan/Triton

## §2 B 类：托管语言/字节码层（对象 15–23）

### 15. ILSpy（C#；MIT）
- **定位**：.NET 反编译的事实标准（开源）；ILSpy 引擎即"IL → C#"参照实现。
- **可抄机制**：
  1. **多级 AST 变换链**：IL → **ILAst**（可切 SSA 形态做优化）→ C# AST → 代码打印；每级变换可单测（"变换链 + 快照测试"工程法）。
  2. **元数据驱动的命名/类型**：程序集元数据（类型/签名/PDB）是命名与信号的主来源——**有元数据就用元数据，没元数据才推断**（判别顺序可抄进 ADV 任何"命名"功能）。
- **档位**：**吸收**（adv-parse 的"变换链 + 快照测试"工程法）。
- **第一步**：把"变换链每级可单测 + 快照"写进 adv-parse 的测试规范（对照 ILSpy 测试工程）。
- **许可/成熟度**：MIT；维护中（锚：持续发布 2025–2026）。
- **链接**：https://github.com/icsharpcode/ILSpy

### 16. dnSpyEx（GPL-3.0；dnSpy 原仓 2020-12 归档后由社区分叉）
- **定位**：.NET 反编译 + 调试 + 编辑并继续 的一体化工具。
- **可抄机制**：
  1. **"分析-调试-编辑"同源**：反编译视图直接接调试器与热重写——"读-改-验"闭环（对我们：二进制面若要人机闭环，这是形态参考）。
- **档位**：**watch**（仅形态参考）。
- **第一步**：无（仅记录形态）。
- **许可/成熟度**：GPL-3.0；维护中（锚：dnSpyEx 持续发布；原 dnSpy 已归档）。
- **链接**：https://github.com/dnSpyEx/dnSpy

### 17. dotPeek（JetBrains；专有免费）
- **定位**：免费 .NET 反编译器（IDE 集成）。
- **可抄机制**：
  1. **项目化输出**：反编译结果直接还原为可编译项目结构（"输出物是工程而不是文本"）。
- **档位**：**watch**（思想：ADV 报告若要"可复现工程"可参考）。
- **第一步**：无。
- **许可/成熟度**：专有（免费）；维护中（随 JetBrains 工具链）。
- **链接**：https://www.jetbrains.com/decompiler/

### 18. JADX（Java；Apache-2.0）
- **定位**：DEX/APK → Java 的反编译器（事实标准）。
- **可抄机制**：
  1. **多 pass 结构化**：region maker 把 CFG 划区还原控制流 + 类型推断 pass（"区域化 + 约束传播"是 Java 侧成熟路线）。
  2. **反混淆参数化**：`--deobf-min/max/whitelist/映射文件（JOBF 等 10+ 格式）`——**命名恢复被做成"可配置、可导入外部映射"的独立子系统**（对 ADV 命名层：映射/忽略/最小长度都应是参数而非硬编码）。
  3. **插件 API**：jadx-api 稳定扩展点。
- **档位**：**吸收**（机制 2 直接可抄进 ADV 任一带命名的分析）。
- **第一步**：参照 JADX 参数面，给 ADV 命名恢复定参数表（最小长度/白名单/外部映射/持久化映射文件）。
- **许可/成熟度**：Apache-2.0；维护中（锚：v1.5.3 2025-09-08、v1.5.6 2026-07-10）。
- **链接**：https://github.com/skylot/jadx

### 19. CFR（Java；MIT）
- **定位**：单 jar、零依赖 Java 反编译器；以"对 javac 产出的精确还原"著称。
- **可抄机制**：
  1. **面向"生成器模式"的还原策略**：把 lambda/switch-on-string/内部类等编译产物模式逐一对号入座（"输出质量 = 模式表覆盖率"）。
- **档位**：**reference**（策略对标；代码不接）。
- **第一步**：在 adv-rules 里核对"编译器产物模式表"是否需要一列（用于解释层）。
- **许可/成熟度**：MIT；维护中（持续更新）。
- **链接**：https://github.com/leibnitz27/cfr

### 20. Fernflower（Java；Apache-2.0；随 IntelliJ 维护）
- **定位**：JetBrains 系 Java 反编译器（IntelliJ 内置）。
- **可抄机制**：
  1. **IDE 规模下的稳健性优先**：宁可保守少还原、不崩不改语义（"大样本下不翻车"比"最漂亮"重要）——工程取舍可抄。
- **档位**：**reference**。
- **第一步**：无（记录取舍原则）。
- **许可/成熟度**：Apache-2.0；维护中（随 IntelliJ；独立仓为镜像）；另注：**Procyon**（Apache-2.0）已弃维护（无 2025–2026 版本信号）——不在新路线中采用。
- **链接**：https://github.com/JetBrains/fernflower

### 21. Hopper（macOS/Linux；专有）
- **定位**：商业反汇编/反编译（macOS 生态）。
- **可抄机制**：
  1. **人机联动**：伪码/调用图/调试器联动的一体化视图（形态参考）。
- **档位**：**watch**（专有：只抄思想）。
- **第一步**：无。
- **许可/成熟度**：专有；维护中。
- **链接**：https://www.hopperapp.com/

### 22. wasm2c（WABT；Apache-2.0）
- **定位**：WebAssembly → 可运行 C（等价翻译，非"美化式反编译"）。
- **可抄机制**：
  1. **"翻译保语义"优先**：输出即等价程序（SECURECOMM'24 实证研究口径：correctness 100%）——评测"正确性"可用"重编译+重执行"闭环（与 LLM4Decompile 的 re-executability 同口径）。
  2. **类型段驱动**：模块类型段直接给出函数签名/表/内存类型，无需推断（"有声明就吃声明"）。
- **档位**：**吸收**（adv-bin 若碰 wasm 的首选机制参考）。
- **第一步**：把"重编译+重执行"作为 adv-bin 任何"改写/翻译"功能的验收判据模板。
- **许可/成熟度**：Apache-2.0；维护中（锚：持续提交）。
- **链接**：https://github.com/WebAssembly/wabt

### 23. wasm-decompile（Binaryen；Apache-2.0）
- **定位**：wasm → 类 C 可读伪码（面向人读）。
- **可抄机制**：
  1. **结构化控制流"白拿"**：wasm 天然是 block/loop/end 结构化字节码，**不需要 goto 结构化算法**，按嵌套直接重建 if/loop（对照：本地代码的结构化是难题，wasm 不是）。
  2. **"贴近底层但可读"的度**：输出保留与字节码的近似对应（不追求完美的 C）——省掉大量无谓还原。
- **档位**：**吸收**（学"输入格式自带结构时不要过度加工"）。
- **第一步**：adv-parse 若处理 wasm，先按嵌套块直译层（L1），不做激进规范化。
- **许可/成熟度**：Apache-2.0；维护中（锚：2025-11 仍有代码改进提交）。
- **链接**：https://github.com/WebAssembly/binaryen

### 24. NotDec（wasm 反编译研究；实验）
- **定位**：Wasm 反编译 + **过程间类型恢复**的研究原型。
- **可抄机制**：
  1. **过程间类型全域传播**：声明口径——结构体成员访问恢复 85.33%（对照 Ghidra 9.24%）——"类型恢复做全局，收益最大"。
- **档位**：**watch**（研究代码；许可待核，接入前必查 LICENSE）。
- **第一步**：无（跟踪；其"类型恢复评测表"可用作我们 P2 目标线参考）。
- **许可/成熟度**：研究原型（许可本次未核）；实验（2024–2025 arXiv 口径）。
- **链接**：https://github.com/NotDec/NotDec

## §3 C 类：反汇编库与 lifting（对象 25–34）

### 25. capstone（BSD-3-Clause；6.0.0-Alpha5，2025-08-05）
- **定位**：多架构反汇编框架（C）；"统一 C API + 多 arch 模块"的事实标准。
- **可抄机制**：
  1. **Auto-Sync 机制**：各 arch 模块定期同步到对应 LLVM 后端版本（6.0 起至 LLVM 16–18；LoongArch 新入）——"解码正确性外包给上游编译器后端"的维护模式。
  2. **统一 detail 模式**：一条指令的显式操作数/隐式寄存器/分组可结构化取出（"一个 API 同时喂人读文本和机读结构"）。
- **档位**：**有界**（C 依赖 + Rust 绑定滞后；6.0 仍是 alpha；**安全注意**：CVE-2025-67873 skipdata 路径堆越界，影响 ≤Alpha5，2025-12-17 披露 CVSS 4.8——若用其 skipdata 回调必须核补丁 cbef767）。
- **第一步**：x86 场景不用 capstone（用 iced）；多 arch 场景走 capstone 时**禁自定义 skipdata，或锁补丁版本**。
- **许可/成熟度**：BSD-3-Clause；维护中（6.0 系列 alpha 阶段）。
- **链接**：https://github.com/capstone-engine/capstone

### 26. iced-x86（Rust；MIT）
- **定位**：纯 Rust x86/x64 解码/编码/格式化库（无 C 依赖、no_std 可用）。
- **可抄机制**：
  1. **吞吐与格式化一体**：外部基准（icedland/disas-bench，2021，i5-6600K/Rust 1.52）解码 256.69 MB/s 为榜首、解码+格式化 149.21 MB/s 领先扩大——**"格式化当一等公民"**（输出渲染不是性能后腿）。
  2. **指令信息面**：flow control/used regs/used memory 等结构化元数据 + **BlockEncoder**（改指令后重编码回字节）——"分析-改写-回写"闭环的最小件。
- **档位**：**吸收**（adv-bin P0 的 x86 解码首选；注意：基准数字 2021 年口径，选型时用自家语料复测）。
- **第一步**：用 iced 写 P0 原型：PE/ELF 函数扫描 → 反汇编 → 指令直方图。
- **许可/成熟度**：MIT；维护中（持续发布）。
- **链接**：https://github.com/icedland/iced

### 27. yaxpeax-x86（Rust；0BSD，接入前按仓内 LICENSE 复核）
- **定位**：超宽松许可的纯 Rust x86 解码器族（另有 ARM 等兄弟件）。
- **可抄机制**：
  1. **极小足迹 + 次席吞吐**：基准（同上 2021 口径）解码 182.08 MB/s ≈ iced 的 71%；二进制约 20 KB 量级（Zydis 约 10 KB 量级）——**嵌入式/体积敏感场景的兜底**。
  2. **自述"user beware"清单**：无 AVX-512、AVX-256 存疑、部分 ring-0 指令存疑——**把覆盖缺口显式写进 README** 的工程诚实（值得抄进 ADV 任何解码器的能力声明）。
- **档位**：**吸收**（备用解码后端；许可最松）。
- **第一步**：在 adv-bin 解码后端抽象上预留第二实现位（iced 主 / yaxpeax 备）。
- **许可/成熟度**：0BSD（按 crates.io/仓库口径；接入前逐字复核）；维护中（持续发布）。
- **链接**：https://github.com/playford/yaxpeax-x86

### 28. Zydis（C；MIT）
- **定位**：x86 解码/编码/格式化库（x64dbg 等大量工具在用；Rust 有 FFI 绑定 crate）。
- **可抄机制**：
  1. **极小体积 + 准确性优先**：约 10 KB（-O3 口径，第三方对比披露）；第三方差分流述称 XED/Zydis 准确性优于 Capstone（个人工程观察，非论文口径）——**"不追最快，追最准"的定位样本**。
  2. **表驱动 + 独立 encoder**：解码表由指令数据库生成，encoder 与 decoder 同源。
- **档位**：**有界**（C/FFI；仅当需要"准确性优先 + 极小体积"时选）。
- **第一步**：无（记录选型判据：即"体积/准确性 vs 纯 Rust"的取舍表）。
- **许可/成熟度**：MIT；维护中。
- **链接**：https://github.com/zyantific/zydis

### 29. LLVM MC / TableGen（Apache-2.0 with LLVM exception）
- **定位**：LLVM 的反汇编/汇编/编码基础设施；**TableGen 数据驱动**生成指令描述。
- **可抄机制**：
  1. **同源描述多路产出**：一份 `.td` 指令描述 → 生成 disassembler/asm parser/encoder/codegen 表——与 SLEIGH 同构的"数据驱动 ISA"，但生态与生成器不同（对照件）。
  2. **MCInst/MCStreamer 抽象**：解码层与渲染层分离（"机器指令对象"与"文本流"解耦）——我们反汇编输出层可照此分层。
- **档位**：**吸收**（思想与生成器架构；代码引用走 Apache-2.0 合规）。
- **第一步**：为 adv-bin 定义 "MCInst 等价物"（结构化指令对象）与"渲染器"两层，禁止渲染逻辑混入分析。
- **许可/成熟度**：Apache-2.0 w/ LLVM exception；维护中。
- **链接**：https://llvm.org/docs/TableGen/

### 30. binutils / libopcodes / objdump（GPL-3.0）
- **定位**：GNU 工具链反汇编端（参照物）。
- **可抄机制**：
  1. **"反汇编器即库"**：libopcodes 让 objdump 与调试器共享解码（对照：我们 P0 也应"一个解码内核，多前端"）。
- **档位**：**reference**（GPL-3.0：代码不吸收，仅对照）。
- **第一步**：无。
- **许可/成熟度**：GPL-3.0；维护中。
- **链接**：https://www.gnu.org/software/binutils/

### 31. VEX / pyvex（VEX GPL-2.0；pyvex BSD-2-Clause）
- **定位**：Valgrind 的块级 IR；经 pyvex 成为 angr 的 lifting 层。
- **可抄机制**：
  1. **IRSB 超级块 + 表达式树**：以"基本块"为单位提升，表达式树延迟求值，天然适配"按块分析"（对照：我们 P2 若按函数分析，需要块级中间层）。
  2. **dirty helpers 折衷**（同对象 8 机制 2）。
- **档位**：**reference**（VEX 本体 GPL：只抄思想；pyvex 层 BSD 可读码）。
- **第一步**：无（已由 angr 条目覆盖）。
- **许可/成熟度**：见上；维护中（随 angr/Valgrind）。
- **链接**：https://github.com/angr/pyvex

### 32. Remill / McSema（Apache-2.0；状态分化）
- **定位**：Trail of Bits 的 LLVM lifting 双子：Remill=指令级 lifting 库；McSema=整二进制翻译器（用 IDA 做 CFG 恢复，另有免费 bin_descend）。
- **可抄机制**：
  1. **"每指令一段 C++ 语义函数"**：指令语义写成语义函数库（lift 成 LLVM 调用），而非表驱动——"语义即代码"路线的样本（与 SLEIGH 表驱动对照）。
  2. **"提升一次，吃全 LLVM 生态"**（同 rev.ng 路线）——KLEE 等源码级工具可直接跑二进制。
- **档位**：**watch/reference**（**McSema 已于 2022-08 归档**；Remill 最后活动约 2024-03——不要押注）。
- **第一步**：把"是否依赖已归档项目"写进技术选型门（本条目即案例）。
- **许可/成熟度**：Apache-2.0；McSema 弃维护（归档 2022-08），Remill 低活跃（约 2024-03 止，本检索口径）。
- **链接**：https://github.com/lifting-bits/remill ｜ https://github.com/lifting-bits/mcsema

### 33. libLISA（Rust；研究/实验；许可待核）
- **定位**：把 **CPU 当 oracle** 自动学习指令语义的研究基础设施（OOPSLA'24；HARRIS'25 报告）。
- **可抄机制**：
  1. **语义真值来自执行，不来自手册**：fuzz + 程序综合枚举指令空间；口径：5 款 CPU 各约 12 万 encoding，约 88% 合成出语义；单次全量 3–4 个月；约 90% encoding 跨 CPU 一致（差异集中在扩展与 undefined behavior）——"语义库是长周期基础设施"的量级锚。
  2. **SMTLib 导出 + Rust crate（liblisa）**：语义数据可被下游工具消费（"数据与引擎解耦"）。
- **档位**：**watch**（方法论借鉴：我们的 ISA 语义若自建，验证必须走"真执行对照"；直接接入需先核许可与数据量级）。
- **第一步**：记录"语义数据的产生成本"锚（对象级判据），避免把"自建 ISA 语义库"排进短期目标。
- **许可/成熟度**：研究基础设施（许可本次未核）；实验（持续研究）。
- **链接**：https://github.com/libLISA/liblisa ｜ https://liblisa.nl/

### 34. Rust p-code 三件套：pcode-rs / jingle_sleigh / rsleigh（Apache-2.0 系）
- **定位**：Rust 侧消费 Ghidra SLEIGH/p-code 的三条路径（2025–2026 从 0 到 1）。
- **可抄机制**：
  1. **jingle_sleigh**：Ghidra SLEIGH 的 Rust FFI 层（Apache-2.0；35 个 release；把 sleigh 以 submodule 带入并随 crates.io 分发）——**零重写接入 SLEIGH 规范**的现成杠杆。
  2. **pcode-rs**：SLEIGH 反汇编 + IR 转换库（灵感来自 angr 的 pycode；0.1.0 2025-04-25 / 0.2.0 2026-01-31）——解码+lift 的可试件。
  3. **rsleigh**：纯 Rust 的 .slaspec 解析 + 解码器/p-code 生成代码（Ghida 兼容反汇编）；0.4.1 口径（Apache-2.0）。
- **档位**：**吸收**（adv-bin 深档的"最小接入实验"首选路径；成熟度低，必须以小实验先行）。
- **第一步**：`cargo add jingle_sleigh` 跑通"x86-64 一段字节 → p-code 文本"冒烟测试，记录精度/覆盖率缺口清单。
- **许可/成熟度**：jingle_sleigh/rsleigh Apache-2.0（检索确认）；pcode-rs 许可本次未逐字核（按同族暂记 Apache-2.0，接入前复核）；均**实验—早期**（2025–2026 新发布）。
- **链接**：https://lib.rs/crates/jingle_sleigh ｜ https://github.com/williballenthin/pcode ｜ https://lib.rs/crates/rsleigh

## §4 D 类：反编译研究（对象 35–46）

### 35. LLM4Decompile（EMNLP 2024；模型+数据）
- **定位**：首个大规模开源"x86 汇编 → C"LLM 系列（1.3B–33B；End 直出 / Ref 精修 Ghidra 输出）。
- **可抄机制**：
  1. **re-executability 度量**：判据不是"编译过"而是"重编译后行为与原二进一致"——ADV 评估任何"改写/翻译/修复"产物都该用这个口径。
  2. **Ref 模式**：LLM 不替代反编译器，只**精修已有反编译输出**（工程上比端到端现实得多）。
- **档位**：**吸收**（评测口径 + Ref 形态；模型本身为 reference）。
- **第一步**：把 re-executability 写进 ADV 的"输出可用性"验收模板（对象 22 同步）。
- **许可/成熟度**：模型/代码开放（见仓库）；维护中（锚：v2 系列，6.7B-v2 重执行率约 52.7%、Ref-33B 约 63.6%，检索口径）。
- **链接**：https://github.com/albertan017/LLM4Decompile

### 36. Decompile-Bench（NeurIPS 2025；数据/评测）
- **定位**：百万级二进-源码函数对（口径：约 2,000,000 对训练 + 7 万对评测，由 100M 对/450GB 浓缩）。
- **可抄机制**：
  1. **"训练集规模 = 能力下界"**：LLM 反编译质量与数据量级强相关——若 ADV 走 LLM 路线，先解决数据（编译/收集），而非先调模型。
- **档位**：**reference**（数据来源参考；本地优先平台即使不用云 LLM，也可自建小型编译语料用于回归评测）。
- **第一步**：无（记录口径）；若做评估，抄其"从真实项目+HumanEval/MBPP 派生"的构造法。
- **许可/成熟度**：研究数据集（见仓库）；实验—维护中（arXiv 2505.12668）。
- **链接**：https://arxiv.org/abs/2505.12668

### 37. DecompileBench（ACL 2025 Findings；评测框架）
- **定位**：面向真实逆向工作流的反编译器评测（约 23,400 个真实函数、130 个真实程序；运行时验证 + LLM-as-Judge）。
- **可抄机制**：
  1. **双轴评测**：功能正确性（运行时语义）与"拟人可读性"分轴——**可读性高 ≠ 正确性高**（结论口径：LLM 方法可读性超商业工具，但功能正确性低 52.2%）。
  2. **LLM-as-Judge 的用法边界**：只用于主观维度（可读性/像不像人写的），不用于正确性。
- **档位**：**吸收**（作为 ADV 报告/解释层的评测边界锚）。
- **第一步**：把"正确性/可读性分轴"写进 ADV 的 LLM 输出评测表（LLM 结论不得出现在正确性轴）。
- **许可/成熟度**：研究框架（开源）；实验—维护中（锚：ACL 2025 Findings）。
- **链接**：https://aclanthology.org/2025.findings-acl.1194/

### 38. DeGPT（NDSS 2024）
- **定位**：用 LLM 优化反编译器（Ghidra 等）输出的研究（可读性/简化导向）。
- **可抄机制**：
  1. **"精修器"独立成层**：原反编译器输出 → LLM 精修（变量名/结构/注释），原分析结论不动——与我们"LLM 只进解释层"的设计一致。
- **档位**：**reference**（机制已被 35/37 覆盖；作为时间线锚）。
- **第一步**：无。
- **许可/成熟度**：研究（NDSS 2024）；实验。
- **链接**：https://www.ndss-symposium.org/ndss2024/

### 39. ReSym（CCS 2024，Distinguished Paper）
- **定位**：LLM + 程序分析**混合**恢复变量名与数据结构符号（VarDecoder + FieldDecoder 两个微调模型）。
- **可抄机制**：
  1. **训练数据投影**：用 DWARF 符号把反编译变量/表达式投影回源码变量——"有调试信息的样本即可自动造标注"。
  2. **Prolog 推理聚合 + 相似度投票压幻觉**：多次 LLM 查询的结果做逻辑交叉校验、冲突投票——**工程化的幻觉抑制组件**（可抄进 ADV 的 LLM 结论合并层）。
  3. **簇合并**：反编译器把数组/结构拆成多个单变量时，用相似度合并回簇（口径：85.9% F1 识别被拆散的数组/内联结构）。
- **档位**：**吸收**（机制 2/3；模型为 reference）。
- **第一步**：给 ADV 命名层设计"多查询交叉校验 + 投票"组件接口（先不做 LLM，用确定性分析模拟）。
- **许可/成熟度**：研究开源（GitHub + Zenodo 数据集/权重）；实验—维护中（锚：CCS 2024；口径：变量名 56.4%、类型 64.6%、结构布局精度 72.9%）。
- **链接**：https://github.com/lt-asset/resym

### 40. Nova / SLaDe（ICLR 2025 / CGO 2024；小模型与汇编 LLM）
- **定位**：Nova=层级注意力+对比学习的汇编/代码生成模型；SLaDe=可移植小模型（数百 MB 级）反编译优化后汇编。
- **可抄机制**：
  1. **"小模型可移植"路线**：反编译辅助不一定上大模型（SLaDe 面向资源受限部署）——本地优先平台的现实选项。
  2. **对比学习的用法**：以汇编↔源码对齐做表示学习（Nova）——若自建命名/检索模型可借鉴。
- **档位**：**reference/watch**。
- **第一步**：无（跟踪）。
- **许可/成熟度**：研究（ICLR 2025 / CGO 2024）；实验—维护中。
- **链接**：https://scholar.google.com/scholar?q=Nova+Generative+Language+Models+for+Assembly+Code ｜ https://scholar.google.com/scholar?q=SLaDe+Portable+Small+Language+Model+Decompiler

### 41. 变量/类型恢复三作：DIRTY（USENIX Sec 2022）/ varBERT（S&P 2024）/ DIRE（FSE 2022）
- **定位**：反编译变量名与类型恢复的经典学习路线（BART/Transformer 系）。
- **可抄机制**：
  1. **"命名/类型恢复 = 独立可评测任务"**：三作提供了任务定义、数据集与指标（Acc@k 等），使"命名质量"可量化——ADV 若做命名层，直接沿用这套任务口径。
- **档位**：**reference**（ReSym 已聚合其机制）。
- **第一步**：无。
- **许可/成熟度**：研究；实验（后续已被 ReSym 等超越——口径见对象 39）。
- **链接**：https://scholar.google.com/scholar?q=DIRTY+decompiler+variable+names+types ｜ https://scholar.google.com/scholar?q=varBERT+variable+name+recovery ｜ https://scholar.google.com/scholar?q=DIRE+variable+name+recovery+binary

### 42. BinDiff（Google；专有免费）
- **定位**：二进制差分事实标准（前 Zynamics SABRE；2011 起属 Google；经 BinExport 也支持 Ghidra）。
- **可抄机制**：
  1. **结构匹配流水线**：call graph 匹配 → 函数内 CFG 匹配（基本块按"指令数/边数/调用/字符串引用"等特征 + 图同构）→ 启发式回填——**"图+统计特征"的经典组合**（算法思想可抄，代码不可）。
  2. **补丁差分产品化**：1-day 场景（patch diffing）作为一等用例——adv-bin 若做差分，用例面照抄。
- **档位**：**有界**（专有：只抄算法；**首选不进依赖**）。
- **第一步**：抄其"匹配特征清单"，做我们最小差分器（函数级特征匹配 + 人工确认）。
- **许可/成熟度**：专有（免费ware）；维护中（6.x；BinExport 桥接 Ghidra）；实证个例口径：Smokeloader 案例中 BinDiff 对"精确补丁点"优于 Diaphora（单案例，非普遍结论）。
- **链接**：https://github.com/google/bindiff

### 43. Diaphora（AGPL-3.0；IDA/Ghidra 插件）
- **定位**：最活跃的开源差分工具（v3.4，2026 口径；IDA 6.8–8.4+ 支持）。
- **可抄机制**：
  1. **SQLite 中间表示**：差分结果落 SQLite，启发式以 SQL 表达——**"分析结果 = 可查询数据库"**（对我们检索面是直接同构）。
  2. **可插拔启发式 + 分数聚合**：数十条基于图论/汇编/字节/函数特征的启发式，按"最佳匹配"聚合——**多启发式投票架构**（pseudo-code diff 也支持）。
  3. **"可能被修复的漏洞"提示**：补丁差分里标注可疑修复点——对 ADV 的应用场景（本地补丁审计）高价值。
- **档位**：**有界**（**AGPL-3.0：任何情况下不静态吸收代码**；机制 1/2/3 可抄）。
- **第一步**：抄"启发式清单 + 聚合分数"结构设计进 adv-bin 差分器（机制级，不涉及代码）。
- **许可/成熟度**：AGPL-3.0（≤1.2.4 为 GPL-3；2.0 起 AGPL；另有商业授权）；维护中（锚：2026 年 3.4）。
- **链接**：https://github.com/joxeankoret/diaphora

### 44. jTrans（ISSTA 2022；研究代码）
- **定位**：跳转感知（jump-aware）token 化的 BERT 式二进制相似性模型（原始口径：ArchLinux 包预训练 + 对比学习）。
- **可抄机制**：
  1. **跳转目标进 token 流**：把跳转的绝对目标编码进 token 化（提升跨优化级别稳健性）——"控制流信息进词表"的最小设计。
  2. **2025 评测判据（McGill TSE 预印本，检索口径）**：vanilla BERT 在四个下游任务上可比甚至优于 jTrans/Trex/StateFormer/PalmTree 等定制 Transformer；收益主要来自**微调策略（对比学习）而非架构**；且 jTrans/Trex/BinCola 的分数并列率（tie rate）偏高（约 5%–20%+）——**选型别迷信自定义架构**。
- **档位**：**watch**（做检索/相似性选型时的对照判据）。
- **第一步**：adv-bin 相似性选型评审时，把"先试 BERT/特征基线"写进判据（对上条）。
- **许可/成熟度**：研究代码（许可未核）；实验—维护中。
- **链接**：https://arxiv.org/abs/2205.12713

### 45. Trex（2023；研究代码）
- **定位**：用 micro-trace 学习执行语义的二进制相似性模型。
- **可抄机制**：
  1. **执行轨迹作为语义信号**：静态难以拉平的编译差异，用轻量执行轨迹补齐（对照：ADV 本地优先，动态档成本需评估）。
  2. **2025 评测口径同对象 44**（custom transformer 优势证据不足；Trex 与 StateFormer 高度相似）。
- **档位**：**watch**。
- **第一步**：无。
- **许可/成熟度**：研究；实验。
- **链接**：https://scholar.google.com/scholar?q=Trex+Learning+Execution+Semantics+from+Micro-Traces+Binary+Similarity

### 46. BinKit（SoftSec-KAIST；数据集/框架）
- **定位**：二进制相似性评测的标准训练/评测集构造框架（跨编译器/优化级别，常用 16 设置）。
- **可抄机制**：
  1. **受控变量构造**：同一源码 × 多编译器 × 多优化级别 × 多架构 → 让"相似性算法的假阳性率"可被控制测量——**评测集要控变量，而不是攒样本**（2025 多篇评测都以 BinKit 为基：GBsim（Entropy 2025）R@1 0.831 > jTrans 0.787；InsnAlign（arXiv 2025）把 jTrans 平均 R@1 从 0.4011 提到 0.4282——均为文献口径）。
- **档位**：**reference**（若自建评测集，直接借其设置矩阵）。
- **第一步**：把"跨编译器/优化级别矩阵"写进 adv-bin 相似性评测计划。
- **许可/成熟度**：研究框架（见仓库）；实验—维护中。
- **链接**：https://github.com/SoftSec-KAIST/BinKit

## §5 E 类：LLM×RE 工具桥（对象 47–50）

### 47. GhidraMCP（Apache-2.0；2025 起）
- **定位**：把 Ghidra 暴露给 LLM 客户端的 MCP 桥（社区口径：5.4k+ star 级）。
- **可抄机制**：
  1. **工具面小而多**：把逆向动作拆成大量小工具（列函数/重命名/反编译单函数…），每次只回小上下文——**防 context rot 的工具设计**（对 ADV 的 MCP 面直接同构）。
  2. **批处理 + 日志**：支持多二进制批处理与优化日志（"脚本化逆向"形态）。
- **档位**：**吸收**（ADV MCP 工具面设计的直接参照）。
- **第一步**：对照其工具清单，检查 adv-bin 的 MCP 工具粒度（单个工具输出应可预算化）。
- **许可/成熟度**：Apache-2.0（检索口径）；维护中（2025–2026 活跃）。
- **链接**：https://github.com/LaurieWired/GhidraMCP

### 48. ReVa（reverse-engineering-assistant；Ghidra 扩展，2025 重写）
- **定位**：Ghidra + MCP 的"逆向助手"（工具驱动，非大 prompt）。
- **可抄机制**：
  1. **反幻觉三条**：容忍错输入并把可纠正错误重定向回 LLM、输出附带"下一步线索"、小片段+强化链接替代整块上下文——**长任务代理的接口写法**（可抄进 ADV 的任何 LLM/代理面）。
  2. **版本门槛显式化**：重写版要求 Ghidra 12.0+（信号：Ghidra 已进入 12.x 周期）——集成时锁版本。
- **档位**：**吸收**（机制 1）。
- **第一步**：把"错误可重定向 + 输出带下一步线索"写进 ADV MCP 工具的响应规范草案。
- **许可/成熟度**：开源（Ghidra 扩展；PyPI `reverse-engineering-assistant`）；维护中（2025–2026）。
- **链接**：https://github.com/cyberkaida/reverse-engineering-assistant

### 49. ida-pro-mcp（2025）
- **定位**：IDA Pro 的 MCP 接口（把 Hex-Rays 伪码/反汇编暴露给 AI 客户端）。
- **可抄机制**：
  1. **伪码优先暴露**：给 LLM 的默认视图是 Hex-Rays 伪码（而非汇编）——"给 LLM 的粒度 = 伪码函数"（我们若做类似桥，粒度选层同理）。
- **档位**：**reference**（IDAPython 依赖；ADV 无 IDA 场景仅作接口参照）。
- **第一步**：无。
- **许可/成熟度**：开源；维护中（2025–2026；生态另有 ida-codex-mcp 等）。
- **链接**：https://github.com/mrexodia/ida-pro-mcp

### 50. Binary Ninja Sidekick（专有；5.0，2025-07-28）
- **定位**：BN 的 AI 助手产品（Tasks 侧栏、Notebook 知识库、查询语言改进、升级模型）。
- **可抄机制**：
  1. **"分析笔记本"作为持久上下文**：把二进制级认知沉淀为可复用知识库（对照：ADV 的结论/证据落盘天然是这件）。
  2. **平台方把 LLM 当一等公民**（对照我们：LLM 面应是可选层，默认关，符合本地优先）。
- **档位**：**watch**（专有产品形态；不依赖）。
- **第一步**：无。
- **许可/成熟度**：专有；维护中（锚：Sidekick 5.0，2025-07-28）。
- **链接**：https://binary.ninja/sidekick/

## §6 重点回答

### 6.1 多层 IR 设计对比 → "归一 AST → 多层语义 IR"的借鉴
- **Ghidra P-code 是"单层语义 IR + 多形态"**：没有 LLIL/MLIL/HLIL 那种分层；靠"同一 IR 上反复 Action/Rule 改写 + SSA（Heritage）形态切换"达到精炼。好处：改写规则只写一遍；代价：**无法按消费者只给某层**（需要"层"时靠导出/打印。
- **BNIL 是"分层 + 每层契约 + 多对多映射"**：Lifted IL → LLIL → MLIL → HLIL；LLIL/MLIL 各有 SSA 形态；层间多对多（上层节点可下钻到低层证据）；按需物化。
- **我们的借鉴（定契约，不定实现）**：
  1. **建议采用"分层 + 层契约"而非单层多形态**：ADV 已有 AST（L0）——加 L1"语义 IR（SSA/变量化）"、L2"结构化 IR（恢复控制流）"；规则引擎在 L1（数据流类）与 L2（模式类）分别落；报告层回溯 L0 位置须由**层间映射表**保证（下表）。
  2. **值模型直接借 varnode 三元组**（space, offset, size）：统一"寄存器/内存/常量/临时/指针"五类值来源，规则写起来不分类（Ghidra 已验证 20+ 年）。
  3. **映射表是硬需求**：BNIL"多对多"提示——上/下钻必须存映射，否则规则命中的证据无法回链（违反 ADV"结论带证据"红线）。
  4. **可观测性抄 IDA 9.2**：每阶段产物可落盘、可 diff（"流水线可观测"是产品特性，不是调试功能）。
- **适用范围**：以上为架构建议，落地成本主要在 L1（SSA + 值模型）；L2 的结构化算法是难点（见 6.2），可延后。

### 6.2 反编译流水线拆解：各阶段可复用件（尤其 Rust）
| 阶段 | 现成件 | 许可/成熟度 | 结论 |
|---|---|---|---|
| ① 反汇编 | iced-x86（纯 Rust，主）；yaxpeax-x86（0BSD，备）；capstone（多 arch，C）；Zydis（准/小，FFI） | MIT/0BSD/BSD-3/MIT | **Rust 段可直用**（对象 25–28） |
| ② lifting→IR | pcode-rs / jingle_sleigh / rsleigh（SLEIGH→p-code，Rust）；或自建表驱动 | Apache-2.0 系；**早期** | 先冒烟测试（对象 34）；x86 语义数据量级参考 liblisa（对象 33） |
| ③ SSA | **Rust 无现成通用件**；算法参考 Cytron et al. 1991（φ 放置）；参考实现读 angr/pyvex（BSD-2） | — | 自建（算法不受版权保护，实现自研） |
| ④ 控制流结构化 | 算法参考 Cifuentes 1994 两遍法（区间/支配→复合条件）；实现对照 angr structurer / Reko ScannerV2 / jadx RegionMaker（各许可不同） | — | 自建；**先做"有符号/有调试信息"档，无符号档延后** |
| ⑤ 类型/变量恢复 | 思路：ReSym（对象 39）、DIRTY 系（对象 41）；**Rust 无现成件** | — | 自建；有元数据先吃元数据（ILSpy 机制） |
- **判据（可执行）**：P2 深档立项前，必须先交付"③④ 两段的自研最小实现 + 对照集（与 Ghidra/rizin 输出抽样对比）"，否则深档就是无底洞。

### 6.3 LLM 反编译研究现状与工程边界（能回写进哪一层）
- **现状（2025–2026 口径）**：① 端到端直出（LLM4Decompile v2：6.7B-v2 重执行率约 52.7%，Ref-33B 约 63.6%）；② 精修已有输出（DeGPT/Ref 模式）；③ 两段化（SK²Decompile，2025-10 口径：骨架/皮肤分离 + 编译器反馈 RL，HumanEval 约 70% 重执行率）；④ 评测反思（DecompileBench：可读性 ≠ 正确性，功能正确性低 52.2%）。
- **能回写 ADV 的层**：**命名/解释/摘要层（L2 之上的"人类可读层"）**——函数名、变量名、注释、自然语言摘要；且必须：a) 结论挂回 IR 位置（证据链）；b) 与确定性分析结论分轴记录（DecompileBench 双轴）；c) 幻觉抑制用"多查询聚合 + 投票"（ReSym 口径）。
- **不能回写**：正确性判据、规则命中判定、去混淆后的"事实"。**判据必须是确定性的**（LLM 只做"表述"）。
- **适用边界**：以上任何数字（52.7%/63.6%/70%/52.2%）均为对应论文在各自评测集上的口径，不可外推到 ADV 场景；ADV 若要引用，需自建小评测复现。

### 6.4 SLEIGH 式"数据驱动指令集描述"怎么用
- **机制本质**：一份声明式规范（tokens 位域 + 构造器模式 → 语义 ops）编译成表，运行时消费 → 解码/语义/显示多路产出；"加指令 = 数据变更，不是代码变更"。
- **三种用法（按成本递增）**：
  1. **直接消费现成规范**（推荐起手）：Ghidra 自带全 arch 的 `.slaspec`；Rust 侧用 jingle_sleigh/pcode-rs 消费（对象 34）——零规范维护成本。
  2. **只为自己需要的 arch/指令子集写规范**（渐进覆盖）：适合"只要少数 arch 的少数指令语义"的场景，避免全量移植。
  3. **自建同构 DSL**（长期）：若要摆脱 Ghidra 依赖，可仿 SLEIGH/TableGen"同源多路产出"思想自建（成本高，不建议短期）。
- **必须配套的风险控制**（锚：InSPECtor，2025）：自动验证发现 SLEIGH 规范 38,920 处差异/125 个唯一 bug，并给 DSL 8 条改进建议（含给 LOAD/STORE 增加对齐信息）——**采用任何"数据驱动语义规范"方案，必须同时建"规范差分测试"**（对真实执行/多工具对照），否则规范错误会静默污染上层全部结论。

### 6.5 adv-bin 集成路线（分档）
- **P0 最小可用（反汇编 + 符号 + 字符串 + 模式扫描）**：
  - 件：object/goblin（格式解析）→ iced-x86（x86 解码，对象 26）→ 字符串提取（ASCII/UTF-16，注意编码与最小长度参数化，抄 JADX 的"参数化"思路）+ 熵/加壳启发（前置检测表抄 RetDec，对象 10）+ 模式扫描（规则集在"指令直方图/字节序列/字符串"三层）。
  - 相似性最小版：函数级特征向量（指令类别直方图 + 调用度 + 字符串引用；BSim 思想，对象 2）→ 向量近邻。
  - 集成面：`adv-bin`（工具）+ `adv-rules`（模式规则）。
- **P1 中档**：多 arch（capstone，注意 CVE/版本）、调用图与跨引用、函数边界启发（符号→序言→递归扫描）、与 secrets 扫描合流（二进制内字符串层）、差分最小版（BinDiff 特征清单，对象 42）。
- **P2 深档（反编译到 IR 后跑规则）**：SLEIGH→p-code（对象 34）→ 自建 SSA（6.2 ③）→ L1 上跑污点/常量传播/危险模式规则 → （可选）L2 结构化 + 命名层（LLM 只进命名/解释，6.3）。
  - **闸门**：P2 立项前须过 6.2 两段自研判据；许可上避 AGPL（Diaphora）与 GPL 静态吸收（rev.ng/Remill/VEX）。
- **全档共性**：所有结论带"层 + 证据位置"；LLM 面默认关（本地优先）；每档发布前对照 rizin/Ghidra 抽样复算（"判据走真路径"）。

## §7 2025–2026 前沿信号（带锚）
1. **BN 5.0 "Gallifrey"**（2025-04-23，约 300 项关闭 issue）+ **Sidekick 5.0**（2025-07-28）：平台方把 LLM 助手做成第一方（Notebook/Tasks/查询语言）。
2. **IDA 9.2**（2025-09-08）：首次开放 **Microcode Viewer**（流水线可观测）；ida-sdk 开源；Qt6；zstd IDB。无内置 AI——AI 走生态（MCP）。
3. **capstone 6.0.0-Alpha5**（2025-08-05）：Auto-Sync 至 LLVM 16–18；**CVE-2025-67873**（skipdata 堆越界，2025-12-17，CVSS 4.8）——解码器输入路径校验是安全面。
4. **rev.ng @ DEF CON 33**（2025-08-09）：全程序类型恢复 + 初步 LLM 命名 + 100% 开源可复现——LLVM 中台路线回暖。
5. **评测反思年**：DecompileBench（ACL 2025，可读性↔正确性分轴）、Decompile-Bench（NeurIPS 2025，2M 对）、SK²Decompile（2025-10，骨架/皮肤 + RL，HumanEval 约 70% re-exec）。
6. **InSPECtor**（2025，arXiv）：SLEIGH 规范自动验证 → 38,920 差异/125 bug/8 条 DSL 建议（含 LOAD/STORE 对齐信息）——"数据驱动 ISA 必须配差分验证"。
7. **Rust p-code 从 0 到 1**：pcode-rs 0.1.0（2025-04-25）/0.2.0（2026-01-31）；jingle_sleigh 35 releases；rsleigh 0.4.1——Rust 消费 SLEIGH 首次"够用可试"。
8. **MCP 桥生态**（2025–2026）：GhidraMCP（Apache-2.0）、ReVa（重写版要求 Ghidra 12.0+）、ida-pro-mcp、ida-codex-mcp——"工具面小而多 + 防 context rot"成为事实设计标准。
9. **二进制相似性反思**（2025）：McGill TSE 预印本（vanilla BERT 可比/优于定制架构；Trex/jTrans tie 率 5–20%+）；GBsim（Entropy 2025）BinKit R@1 0.831 > jTrans 0.787；InsnAlign（arXiv 2025）jTrans 平均 R@1 0.4011→0.4282。
10. **库状态变化**：McSema 归档（2022-08）、Remill 低活跃（约 2024-03 止）；RetDec 受限维护（2025-05 仍有提交）；Diaphora v3.4（2026）；Reko 0.12.3（2026-06-17）；jadx v1.5.6（2026-07-10）。

## §8 Top-3
1. **Ghidra P-code / SLEIGH（+ Rust 消费口 jingle_sleigh/pcode-rs）**——唯一"许可友好（Apache-2.0）+ 语义底座完整 + 有 Rust 接入实验件"的深档路径。抄：varnode 值模型、Action/Rule 改写管线、数据驱动 ISA；配 InSPECtor 式差分验证。集成：adv-bin / adv-rules / adv-parse。
2. **Binary Ninja BNIL 分层 IR + Workflows**——"归一 AST → 多层语义 IR"的架构范式（层契约/多对多映射/按需物化/可编排 pass）。专有：只抄设计。集成：adv-parse / adv-ast-rust。
3. **Ghidra BSim（+ BinDiff 结构匹配算法对照）**——adv-bin 相似性/聚类最小算法骨架：函数特征向量（p-code 派生）→ 向量索引；结构匹配兜底。集成：adv-bin。
- 近邻候选（并列参考）：iced-x86（P0 解码主件）、ReSym（命名层幻觉抑制）、DecompileBench（评测双轴边界）。

## §9 口径与缺口
- **Lemur 澄清**：本域检索未发现名为 Lemur 的反编译/二进制工作；以 SLaDe（CGO 2024）/Nova（ICLR 2025）补位。
- **未核项（显式列出，接入前必查）**：yaxpeax-x86 许可逐字复核；pcode-rs / liblisa / NotDec / jTrans / Trex / BinKit 的 LICENSE；Snowman/Miasm 的 2025–2026 活跃度逐项核对。
- **基准数字时效**：iced/yaxpeax/Zydis 性能对比为 icedland/disas-bench 2021 口径（i5-6600K/Rust 1.52），选型须用自家语料复测；Zydis 体积（约 10 KB）与准确性评述来自第三方对比文（非论文）。
- **单一来源项**：capstone CVE 细节、ReVa 版本门槛、Sidekick 版本日期均为单个检索源口径，未二次复核。
- **本域结论适用范围**：面向 ADV（本地优先、Rust 主体、规则+证据链）的选型与机制借鉴；不构成对上述工具的通用评价。
