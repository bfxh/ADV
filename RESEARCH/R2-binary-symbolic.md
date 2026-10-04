# R2 · 二进制分析与符号执行（深挖）

> 域：`R2` ｜ 日期：2026-10-03 ｜ 方法：WebSearch 优先 2025–2026 一手发布/论文，机制级拆解；许可证与成熟度均尽量带锚（版本号/日期/出处），查不到的一律标「待核」，不冒充已验。
> 档位口径：【吸收】= 机制可进 ADV（含宽松许可件）；【有界】= 仅思想可抄，或许可/依赖/成本受限；【不吸收】= 明确排除；【watch】= 实验/未成熟，跟踪；【reference】= 背景/对照/历史。
> 集成分面：`adv-bin`（二进制面）/`adv-parse`/`adv-rules`/`perf-core`/`test-replay`（ADV 测试体系，P7）/`watch`/`reference`。
> 覆盖 41 个对象（机制级 deep 26 个）；与 R1 交集（angr 反编译面、BAP、Ghidra P-code、ESIL）此处只补"分析工具链/执行引擎"角度，不重复 R1 已有结论。
> **数字均为检索口径**（二手汇总，未逐条复现），已在文中标注来源与日期；标「待核」的条目为存疑待验。

## §0 执行摘要

- **符号执行的工业边界（总纲）**：可靠回归（replay）与**有界**探索，不是全程序主力扫描器。锚：KLEE（OSDI'08）在 89 个 coreutils 上各跑 1 小时得 ~84% 平均行覆盖、10 个 crash（论文口径），但约束求解占其运行时间 ~92%、大解析器出现 10^10+ 路径（检索口径）——"小核心 + 明确 harness"是甜点区，"全程序无界"是坑。
- **lifting 三路线取舍**：McSema 已归档（2022-08）⇒ 全程序 LLVM 提升路线退潮；Remill（指令语义库，LLVM 上重建）仍在维护但 IR 冗长（1 条 AMD64 ADD ≈ 132 行 LLVM IR，检索口径）；VEX（angr 血统）仍是"多架构统一 IR"的成熟参照。**adv-bin 起步建议：自写 decode/轻 IR（US-1 档）+ 借鉴 ESIL 低成本仿真，不背 LLVM 全套。**
- **求解器 Rust 生态**：默认档 `z3.rs`（成熟、MIT、多打包方式；但 `!Send+!Sync` ⇒ 每线程独立 Context，多用进程池）；QF_BV 热路径 `Bitwuzla` 性能最强（CAV'24 口径 ~140× vs Z3 于 4,126 条 MBQI 实例），但 Rust 绑定版本失配（0.8 vs C API 0.9.1，检索口径）需自绑；`cvc5-rs` 官方维护（v0.3.2），量词/字符串强。纯 Rust 求解器在 QF_BV 无竞争力（检索口径）。
- **混合 fuzzing × 符号执行**：值得进 P7/test-replay 的是**调度思想**而非整套工具：Driller 的"互补探索/compartment"、QSYM 的 optimistic solving + basic block pruning、SymCC 的"编译期插桩免解释开销"、Fuzzolic 的 Fuzzy-SAT 近似求解、veritesting/state merging 的静态合并。
- **adv-bin 分档**：L0 纯静态反汇编+模式扫描（低成本，马上可做）→ L1 轻 IR 数据流（低成本-中，无 SMT）→ L2 有界符号执行（高成本，SMT 只能进单函数/单路径上限）→ L3 全程序符号执行【不吸收】。详见 §6.⑤。

## §1 符号执行引擎与测试生成（对象 1–16）

### 1. angr（BSD-2-Clause；维护中）
- **定位**：二进制符号执行/分析框架（自 R1：其反编译面 VEX→AIL 已在 R1 记录；此处补"执行引擎"面）。
- **可抄机制**：
  1. **引擎分层链**：`CLE`（加载器，内存 backer）→ `PyVEX`（lifting 到 VEX IR）→ `SimOS`/`SimProcedures`（环境建模，~250 个 libc/内核/Windows 函数桩）→ `Claripy`（符号表达式/求解）→ `SimulationManager`（状态编排）。`SimState` 步进时按序尝试引擎：`SimEngineFailure → SimEngineSyscall → SimEngineHook → SimEngineUnicorn → SimEngineVEX`（检索口径：Quarkslab 报告）——"多引擎按优先级接管"是可直抄的扩展点设计。
  2. **状态容器（stash）+ 探索策略**：`active/deadended/pruned/unconstrained/unsat` 分桶；内置策略：DFS、LoopSeer（循环展开上限 bound=10）、LengthLimiter（基本块数上限）、Veritesting（静态合并菱形分支）、Spiller（空闲状态换页到磁盘）。**"策略是插件，路径爆炸靠组合策略压制"** 比"一个万能探索器"务实。
  3. **状态合并接口**：`state.merge(others, merge_conditions, common_ancestor)`（n 个状态配 n+1 个合并条件）；静态分析场景可用 `widen()`——"合并 = 把多状态压成一个更泛状态"，是 adv-bin 有界探索的核心操作。
- **档位**：【吸收】（BSD-2；可作为子进程/外部引擎消费，架构做自研对照系）。
- **第一步动作**：用 angr 的 `SimulationManager` + LoopSeer(bound=10) 在本机 x86-64 小样本上跑"1 函数 3 路径"实验，记录状态数/solver 时间分布，作为 adv-bin L2 档的基线锚。
- **许可/成熟度**：BSD-2-Clause；维护中（锚：docs v9.2.123；2025–2026 仍在发布）。
- **链接**：https://github.com/angr/angr ｜ https://docs.angr.io

### 2. KLEE（UIUC/NCSA 宽松许可；维护中）
- **定位**：LLVM IR 上的符号执行/自动测试生成鼻祖（OSDI'08），至今是"符号执行能力边界"的基准坐标。
- **可抄机制**：
  1. **测试生成经济学**：每条路径产出可复现实例（concrete input + 覆盖），"求解器查询 → 输入 → 回归语料"闭环——这正是 ADV test-replay 想要的形态。
  2. **约束缓存/反例缓存**（KLEE 的核心工程优化之一：相同查询不重解）——求解器调用是 92% 时间所在（论文口径），缓存即收益。
  3. **边界自省**：不支持浮点/线程/变长内存对象/汇编（论文口径的已知缺口）⇒ 抄它的"明确声明不支持的语义清单"作为我们 L2 档的能力边界表。
- **档位**：【有界】（LLVM 世界；C/C++ 源码级；对 ADV 是思想+基准，代码不直连）。
- **第一步动作**：把 KLEE 论文的"每路径一个可复现输入"落成 test-replay 的用例格式（path-id + 输入 + 覆盖块集合）。
- **许可/成熟度**：UIUC/NCSA（宽松）；维护中（锚：KLEE Workshop #4，2024-04；2025 仍有 LLM 集成研究引用其生态）。
- **链接**：https://klee-se.org ｜ https://github.com/klee/klee

### 3. S2E（LGPL-2.1，待核；维护放缓）
- **定位**：全系统（VM 级）**选择性符号执行**平台（x86/x86-64/ARM）。
- **可抄机制**：
  1. **选择性符号执行**：默认全部具体执行，只在选定代码区/触发条件处开启符号化——"符号执行是开关，不是全局模式"。这是"符号执行在工业上可用"的关键工程口径。
  2. **执行一致性模型 + 懒惰具体化（lazy concretization）**：控制"何时把符号值落成具体值"，避免过早丢失路径信息。
  3. **战绩锚**：DARPA CGC 2016 决赛 7 套系统中有 2 套基于 S2E（检索口径）。
- **档位**：【有界】（VM 级重型；只吸收"选择性开启 + 一致性模型"两个概念）。
- **第一步动作**：给 adv-bin L2 设计"符号化开关"：按函数白名单/触发点开启，默认全具体——写进设计稿。
- **许可/成熟度**：LGPL-2.1（待核）；维护放缓/社区维护。
- **链接**：https://github.com/S2E/s2e

### 4. Mayhem（专有；商业维护中）
- **定位**：ForAllSecure 商业二进制分析系统；CGC 2016 冠军；符号执行工业化的最强存在证明。
- **可抄机制**：
  1. **veritesting 出身地**（MergePoint 血统）：静态与动态符号执行交替——见对象 33。
  2. **状态落盘换内存**：把状态转移磁盘以压内存压力（SoK 调查报告口径）——"状态空间 > 内存"时的工程解。
  3. **CRS 闭环**：发现→验证→利用→加固的全自动管线（S&P'18 论文）。
- **档位**：【不吸收】（专有代码）；机制层面归入 veritesting/状态管理条目。
- **第一步动作**：无（只读论文，标注为对照系）。
- **许可/成熟度**：专有（商业）；维护中（商业产品）。
- **链接**：https://forallsecure.com ｜ Mayhem S&P'18：https://ieeexplore.ieee.org/document/8418602

### 5. Manticore（Apache-2.0，待核；弃维护）
- **定位**：Trail of Bits 的符号执行框架（二进制 + EVM 智能合约）。
- **可抄机制**：多平台（二进制/EVM）共用执行抽象；API 化"找输入"接口（`find` 条件 → 产出输入）。
- **档位**：【不吸收】（已归档停更 ⇒ PROGRAM 口径"弃维护本身就是结论"）。
- **第一步动作**：无；如在别处见到 Manticore 用法，直接以 angr/自研替代。
- **许可/成熟度**：弃维护（已归档；检索口径确认，具体归档日期待核）。
- **链接**：https://github.com/trailofbits/manticore

### 6. Triton（Apache-2.0，待核；维护中）
- **定位**：Quarkslab 的动态符号执行（DSE）库（C++ + Python 绑定），单路径 concolic 引擎。
- **可抄机制**：
  1. **三态执行模型**：Σ =（具体态 Σ_C，符号态 Σ_S，污点态 Σ_T）——具体驱动、符号随行；单路径免路径爆炸，比全符号探索快（适用于线性代码/反混淆/VM handler 恢复）。
  2. **AST + 优化 pass**：符号表达式为 SSA AST，求解前先跑简化 pass（再交 Z3）——**"AST 级化简 → SMT"两级管线**可直抄进 adv-bin L1/L2 之间。
  3. **与 DBI 解耦**：自 v0.3 起独立库，可嫁接不同插桩后端（如 QBDI PoC）——插桩源与符号引擎分离。
- **档位**：【有界】（C++ 大件；机制层抄三态模型 + AST 化简管线）。
- **第一步动作**：在 adv-bin L1 档先实现"表达式 AST + 常量折叠/代数简化"两级，再决定是否上 SMT。
- **许可/成熟度**：Apache-2.0（待核）；维护中（Quarkslab）。
- **链接**：https://github.com/JonathanSalwan/Triton

### 7. miasm（GPL-2.0；维护中，节奏慢）
- **定位**：Python 逆向框架：IR lifting + 符号执行（含表达式简化）+ 仿真 + 二进制改写（IDA 脚本集成）。
- **可抄机制**：
  1. **表达式简化器**：把混淆算术表达式化简（反混淆核心手法）——纯语法级、无需 SMT。
  2. **轻 IR + 仿真 + 符号执行同栈**：加载→lift→符号执行→简化 一条管线，架构紧凑，适合做"低成本仿真档"的参考实现。
- **档位**：【有界】（GPL ⇒ 代码不可内联；思想抄：表达式简化管线）。
- **第一步动作**：读 miasm 简化器 pass 列表，选 3 类（常量折叠/幂等消除/位运算恒等）进 adv-bin L1。
- **许可/成熟度**：GPL-2.0；维护中（节奏慢）。
- **链接**：https://github.com/cea-sec/miasm

### 8. Qiling（GPL-2.0，待核；维护中）
- **定位**：基于 Unicorn 的**OS 级仿真框架**（跨平台/跨架构：x86/ARM/MIPS/Sparc/M68K；Linux/BSD/macOS/Windows 子系统）。
- **可抄机制**：
  1. **OS 层建模面**：syscall、虚拟文件系统、注册表、网络——"仿真不止 CPU，还要有 OS 语义"；rootfs 提供库加载。
  2. **反调试免疫**：无真实 debugger 痕迹（如 `ptrace(TRACEME)` 自动绕过）——动态分析恶意样本的工程优势。
- **档位**：【有界】（GPL；作为独立子进程的动态档引擎候选）。
- **第一步动作**：评估"Qiling 作为 adv-bin 动态档外挂子进程"的可行性（PE/ELF 各 1 个 hello-world 级实验）。
- **许可/成熟度**：GPL-2.0（待核）；维护中（社区）。
- **链接**：https://github.com/qilingframework/qiling

### 9. Unicorn（GPL-2.0；维护中）
- **定位**：QEMU 派生的**纯 CPU+内存**仿真框架（Black Hat 2015 发布）；Qiling 等上位工具的底座。
- **可抄机制**：
  1. **最小仿真核**：剥掉设备/BIOS 子系统，体积/内存约 1/10 于 QEMU（检索口径），攻击面小。
  2. **多语言绑定 + 细粒度 hook（地址/基本块/指令/内存访问）**；线程安全，可同时仿真多个不同架构 CPU。
- **档位**：【有界】（GPL；若 adv-bin 要动态档，优先"子进程 + 独立工具"边界而非内联链接）。
- **第一步动作**：明确 adv-bin 动态档的**进程边界契约**（输入：二进制+起点；输出：trace/内存快照），引擎可选 Unicorn/Qiling 任一。
- **许可/成熟度**：GPL-2.0；维护中（但有"fork 维护负担/架构跟进慢"的社区口径）。
- **链接**：https://github.com/unicorn-engine/unicorn

### 10. PANDA（GPL-2.0；维护中）
- **定位**：QEMU 上的全系统**记录/回放**平台（MIT Lincoln Lab）；插件式动态分析。
- **可抄机制**：
  1. **record/replay 作为分析基座**：先录一段执行，再反复重放跑不同分析插件——"一次录制，多次分析"；回放确定性强，天然适合 test-replay 的"复现"需求。性能口径：record 1.85×、replay 3.57× 于 QEMU 基线（PANDA 论文口径）。
  2. **插件即分析**：CFG/污点在 LLVM IR 上做（继承 S2E 血统）。
- **档位**：【有界】（GPL 重型；抄"record/replay 范式"进 test-replay）。
- **第一步动作**：在 test-replay 里定义"捕获一次 → 确定性重放 N 次"的最小格式（哪怕先不含全系统仿真）。
- **许可/成熟度**：GPL-2.0；维护中（patch 跟随 QEMU 上游是长期负担，社区口径）。
- **链接**：https://github.com/panda-re/panda

### 11. SAGE（微软内部/专有；只抄思想）
- **定位**：首个白盒 fuzzer（MSR，~2007 起）：record path → 约束生成（TruScan）→ Z3 求解 → 新输入 → 循环；**基于代（generation）的搜索**。
- **可抄机制**：
  1. **generational search**：从 1 个种子出发，每代只翻转路径条件的一小部分，产出"覆盖新路径"的输入——输入生成的工业化模板。
  2. **规模锚**：500+ 机器年、3.4B+ 约束（"史上最大 SMT 用量"）、Windows/Office 日常使用、约 1/3 的 Win7 WEX 安全补丁来自它（MSR 公开口径）——证明"符号执行的收益单位 = 深路径 bug"，贵但值。
- **档位**：【有界】（无公开代码；抄 generational search 思想进 test-replay）。
- **第一步动作**：为 ADV 的种子语料定义"路径覆盖增量"指标（新块数/新分支），对应 SAGE 的 generation 收益度量。
- **许可/成熟度**：专有（MSR 内部产品）；思想常青（2007–今）。
- **链接**：https://www.microsoft.com/en-us/research/project/sage/

### 12. Driller（BSD 类，待核；shellphish 维护性一般）
- **定位**：AFL + angr 的混合 fuzzing 代表（NDSS'16）：fuzzer 卡住时调 concolic 求解，产出新输入回灌。
- **可抄机制**：
  1. **互补循环**：fuzz 的代价低、符号执行的代价高 ⇒ 只在"覆盖停滞/难点分支"处调用符号执行（compartment 检测）。
  2. **接口化**：`driller(target, testcase, afl_bitmap)` ——符号执行消费 fuzz 位图，产出测试用例；"位图 = 反馈契约"。
- **档位**：【吸收】（思想直接可进 test-replay 调度：fuzz 优先、符号兜底）。
- **第一步动作**：在 P7/test-replay 定义"停滞判据"（N 分钟无新块 ⇒ 触发符号执行候选清单）。
- **许可/成熟度**：BSD 类（shellphish/driller，待核）；维护性一般（研究原型质地）。
- **链接**：https://github.com/shellphish/driller

### 13. QSYM（Apache-2.0，待核；弃维护）
- **定位**：USENIX Security'18，"为混合 fuzzing 定制的实用 concolic 引擎"：轻量、不维护全状态、不 fork。
- **可抄机制**：
  1. **optimistic solving（乐观求解）**：只解约束的一小部分（如最后一条），产出"可能到达新分支"的输入，先跑再说——把求解器压力切成小片；配合重试。
  2. **basic block pruning（块剪枝）**：对重复执行的块做指数退避，减少重复求解。
  3. **口径锚**：同任务下比 angr 快 10–100×（论文口径）；位图/轨迹来自 Pin。
- **档位**：【吸收】（机制级工程手法，与具体工具解耦）。
- **第一步动作**：写一页"求解预算策略"：每次调用限 N 毫秒/只解 1 条约束/超时弃疗，作为 adv-bin L2 的默认参数。
- **许可/成熟度**：Apache-2.0（待核）；弃维护（原型，2018 后基本停更）。
- **链接**：https://github.com/sslab-gatech/qsym

### 14. SymCC / SymQEMU（GPL-3.0，待核；维护中）
- **定位**：USENIX Security'20，"Don't Interpret, Compile!"：符号执行**编译进目标**（编译器 IR 插桩）而非解释执行；SymQEMU 是二进制版（SymCC 运行时复用 QSYM）。
- **可抄机制**：
  1. **插桩即符号化**：把符号跟踪嵌入编译环节，消除解释器/观察器开销（tcpdump 例：0.3s vs QSYM 27.1s，论文口径）；查询数比 QSYM 少 ~34%（检索口径）。
  2. **边界诚实**：需重编译整个应用+库（实践门槛），libc 模型少 ⇒ 这就是后续 Fuzzolic/SymQEMU 存在的理由。
- **档位**：【有界】（重编译不现实；抄"插桩在编译/IR 层"的思想 ⇒ 对 Rust 生态即"在 AST/HIR 层插桩"）。
- **第一步动作**：在 test-replay 记录"符号插桩点契约"（源/IR 层插桩接口草案）。
- **许可/成熟度**：GPL-3.0（待核）；维护中（Eurecom）。
- **链接**：https://github.com/eurecom-s3/symcc

### 15. Fuzzolic（开源，许可待核；半维护）
- **定位**：二进制级 concolic（SymQEMU 类 tracer）+ **Fuzzy-SAT** 近似求解（Computers & Security 2021）。
- **可抄机制**：
  1. **近似求解换覆盖**：求解不求精确满足，只求"可能触发新路径"——覆盖率优先的求解哲学（在 4 个基准的 3 个上覆盖高于 SymCC，论文口径）。
  2. **教训锚**：SymCC 查询更高效但覆盖输在"库模型缺口" ⇒ **环境/库建模质量 > 引擎速度**，对 ADV 的 L2 设计是首要约束。
- **档位**：【有界】（抄"近似求解"思想；代码不直用）。
- **第一步动作**：adv-bin L2 设计里把"外部调用建模清单"列为一等公民（先做 syscall/常见 libc 子集）。
- **许可/成熟度**：待核；半维护。
- **链接**：https://github.com/season-lab/fuzzolic

### 16. SymSan（开源，许可待核；新，watch）
- **定位**：新的动态数据流分析路线（sanitizer 式符号化）。
- **可抄机制**：FuzzBench 口径下多数目标优于 SymCC/SymQEMU/Fuzzolic；CVE-2018-13785：29 秒 vs SymCC 14.58 小时（检索口径，待复核）。
- **档位**：【watch】（新项目，成熟度未定，链接待核）。
- **第一步动作**：跟踪 6–12 个月，若进 FuzzBench 稳态再评估。
- **许可/成熟度**：待核；实验期。
- **链接**：待核（检索口径：SymSan）

## §2 lifting 与 IR（对象 17–23）

### 17. McSema（许可待核；**弃维护**）
- **定位**：二进制 → LLVM IR 全程序提升器（lifting-bits）。
- **可抄机制**：分层"frontend（CFG 恢复，依赖 IDA Pro）→ 语义提升"的接口划分；教训：**重度依赖专有前端 + 跟随 LLVM 版本 = 维护陷阱**（锚：2022-08 归档；构建于 LLVM 9，检索口径）。
- **档位**：【不吸收】（弃维护是结论）。
- **第一步动作**：无；若见旧文献推荐 McSema，改看 Remill（指令级）或放弃 LLVM 全程序路线。
- **许可/成熟度**：待核；弃维护（2022-08 归档）。
- **链接**：https://github.com/lifting-bits/mcsema

### 18. Remill（Apache-2.0，待核；维护中）
- **定位**：**指令语义库**（"LLVM 之于 McSema 相当于 Clang"）：单指令 → LLVM IR 片段（x86/amd64/AArch64/SPARC32/64）。
- **可抄机制**：
  1. **语义与 CFG 恢复解耦**：Remill 只做指令语义，程序级控制流交上层——"语义库"边界清晰，可单独复用。
  2. **IR 冗长是设计选择**：故意 over-expressive（1 条 ADD ≈ 132 行 LLVM IR，检索口径），把内存/控制流延迟到 intrinsics ⇒ 后续工具可优化；对我们启示是：**lifting 要明确"给谁看"**——给人看/给 SMT 看/给模式匹配看，三者的 IR 设计不同。
- **档位**：【有界】（机制与边界划分可抄；实现不搬）。
- **第一步动作**：在 adv-bin IR 设计文档里写"语义层 vs 结构层分离"一节（见 GTIRB）。
- **许可/成熟度**：Apache-2.0（待核）；维护中（锚：2024-03/2025 仍有活动，检索口径）。
- **链接**：https://github.com/lifting-bits/remill

### 19. VEX IR / PyVEX（Valgrind GPL-2.0 血统；PyVEX BSD-2）
- **定位**：Valgrind 的中间表示；angr/PyVEX 将其独立化，成为"多架构统一 IR"的事实参照（x86/ARM/MIPS/PPC/s390x）。
- **可抄机制**：
  1. **寄存器平铺 + flag 显式化**：所有值进统一寄存器命名空间（含临时/flag），flag 用 ITE 表达式展开——语义保真、代价是节点多。
  2. **"IR 即库"**：PyVEX 独立成包，任何工具可只取 lifting 层——**分层可单独消费**的工程范例。
- **档位**：【有界】（Valgrind 血统许可重；思想抄：值模型统一 + flag 显式）。
- **第一步动作**：若自写 IR，对齐"值=临时/寄存器/内存/常量"四类的最小值模型（R1 已记录 Ghidra varnode 同型设计）。
- **许可/成熟度**：Valgrind GPL-2.0 / PyVEX BSD-2；维护中（angr 生态内）。
- **链接**：https://github.com/angr/pyvex ｜ https://valgrind.org

### 20. BAP / Core Theory（MIT）
- **定位**：OCaml 二进制分析平台：BIL（指令语言，ML 风格、显式副作用）→ BIR（结构化）；插件式（loader/target/disassembler/CFG/analysis）。
- **可抄机制**：
  1. **显式副作用的指令语义**（EFLAGS/栈都建模）——语义清单化，可形式化推理。
  2. **Core Theory**：所有插件共享的统一语义底座 = "分析间契约"（R1 已记；此处补 2025 信号：**IsaBIL（ECOOP'25）**把 BIL 语义机化进 Isabelle/HOL 验二进制 ⇒ 证明"语义 IR 可形式化"这条路仍活）。
- **档位**：【reference】（OCaml 生态不直连；抄"语义契约"思想）。
- **第一步动作**：读 BIL 语义定义（1 页），校验我们自写 IR 的副作用完整性清单。
- **许可/成熟度**：MIT；维护中（社区，节奏慢但 2025 有学术应用）。
- **链接**：https://github.com/BinaryAnalysisPlatform/bap

### 21. GTIRB（Apache-2.0，待核；**维护放缓**）
- **定位**：GrammaTech 的**结构化二进制 IR**（v2.3.3；最后大推 2024-08，检索口径）：模块/IPCFG/AuxData/全元素 UUID；Protobuf 序列化；C++/Python/Common Lisp API。
- **可抄机制**：
  1. **结构 IR 与语义 IR 解耦**：GTIRB 故意**不表示指令语义**（只存原始字节 + 符号/控制流），语义交给任意 IL（BIL/Vex/P-code）——"一张结构图 + 可插拔语义层"。
  2. **UUID + AuxData 表**：每个元素有稳定 UUID（改写后仍可引用）+ 任意分析结果以可移植格式挂载——**分析结果与结构解耦**，多工具可增量填充同一份 IR。
- **档位**：【吸收】（数据契约定制直抄；adv-parse/adv-bin 的 IR 存储格式）。
- **第一步动作**：adv-bin 的 IR 存储草案按 GTIRB 三件套写：结构表（块/边）+ UUID 引用 + AuxData 挂载点；先不定义语义。
- **许可/成熟度**：Apache-2.0（待核）；维护放缓（2024-08 后活动少）。
- **链接**：https://github.com/GrammaTech/gtirb

### 22. ESIL（rizin；LGPL-3.0，已在 R1）
- **定位**：radare2/rizin 的**字符串化可求值 IR**（低成本仿真）。
- **可抄机制**：IR 既可选文本又是可执行脚本（~200 算子），实现成本远低于 VEX/LLVM——"够用的语义"典范；分析工具链角度：ESIL 让"仿真"成为命令而非框架。
- **档位**：【有界】（LGPL；抄 IR 设计理念）。
- **第一步动作**：adv-bin L1 档的轻 IR 以"算子数 ≤ 300、可文本化、可求值"为验收线。
- **许可/成熟度**：LGPL-3.0；维护中（rizin 活跃）。
- **链接**：https://rizin.re

### 23. ddisasm（许可待核；维护中，watch）
- **定位**：GTIRB 生态的**Datalog 式反汇编器**（GTIRB 配套：ddisasm + gtirb-pprinter）。
- **可抄机制**：把反汇编写成 Datalog 推理规则集（符号化交叉引用/数据代码区分），"反汇编 = 逻辑程序求不动点"——与 R1/01 域（Souffle）交叉，此处只记"GTIRB 工具链完整度"。
- **档位**：【watch】（adoption 依附 GTIRB）。
- **第一步动作**：跟踪；若 GTIRB 格式进 adv-bin 再评 ddisasm。
- **许可/成熟度**：待核；维护中。
- **链接**：https://github.com/GrammaTech/ddisasm

## §3 求解器与 Rust 绑定（对象 24–28）

### 24. Z3 + z3.rs（MIT）
- **定位**：SMT 求解器事实标准（MSR）；Rust 绑定 `prove-rs/z3.rs`。
- **可抄机制 / 工程参数**：
  1. **成熟度锚**：z3.rs 0.20.0/0.19.x（2025–2026 持续发版）；deps.dev 维护分 10/10（检索口径）；打包四选：system/bundled(静态 CMake)/vcpkg/gh-release 预编译库。
  2. **并发约束**：`Context`/`Solver` 为 `!Send + !Sync` ⇒ 每线程独立 Context；策略：进程池/多 Context，不要跨线程共享。
  3. **性能定位**：QF_BV 上比 Bitwuzla 慢约 5×（累计口径：glaurung 报告，引 CAV'23）；但综合（非线性算术等）SMT-COMP'26 仍有 Z3 变体夺冠 ⇒ **按查询类别选引擎，不搞单押**。
- **档位**：【吸收】（默认求解器；MIT 无碍）。
- **第一步动作**：adv-bin L2 原型接 z3.rs（bundled 特性），跑 10 条典型约束记录耗时分布。
- **许可/成熟度**：MIT；维护中。
- **链接**：https://github.com/prove-rs/z3.rs ｜ https://github.com/Z3Prover/z3

### 25. cvc5 + cvc5-rs（cvc5: BSD-3-Clause；绑定许可待核）
- **定位**：SMT-COMP 综合强手（2025 增量 track 第一，40.05 分，检索口径）；官方 Rust 绑定 cvc5-rs v0.3.2（cvc5-sys v0.3.1）。
- **可抄机制**：**量词/字符串理论**强于 Z3（适用：字符串约束、数组）；SMT-COMP'26 综合/并行多数组赢、QF_BV 让位 Bitwuzla（检索口径）。
- **档位**：【有界】（可用但绑定较新；作 Z3 的备选第二引擎）。
- **第一步动作**：写"引擎选择矩阵"一页：理论类别 → 首选/备选（BV→Bitwuzla；字符串/量词→cvc5；默认→Z3）。
- **许可/成熟度**：cvc5 BSD-3-Clause；维护中（官方绑定随主仓）。
- **链接**：https://github.com/cvc5/cvc5 ｜ https://crates.io/crates/cvc5-rs

### 26. Bitwuzla（MIT）
- **定位**：Boolector 后继（CAV'24），QF_BV/QF_ABV/FP 专精求解器。
- **可抄机制 / 性能锚**：Bitwuzla(A) 抽象技术：smtlib 集上解出 ~5× 于基线；MBQI 类 4,126 实例 37.8s vs Z3 5,279.6s（≈140×，CAV'24 论文口径）；SMT-COMP 2026 QF_BV/FP 组冠军（检索口径）。
- **档位**：【有界】（Rust 侧 `bitwuzla-sys` 0.8 与 C API 0.9.1 版本失配，需自绑 C API；先进 perf-core watch）。
- **第一步动作**：给 adv-bin L2 的 BV 密集查询准备"Bitwuzla 子进程 + SMT-LIB 文本协议"降级路径（不绑定也能用）。
- **许可/成熟度**：MIT；维护中。
- **链接**：https://github.com/bitwuzla/bitwuzla

### 27. Boolector（MIT；已被 Bitwuzla 取代）
- **定位**：前代 BV 专用求解器（Bitwuzla 前身同作者组）。
- **可抄机制**：无（历史）；**结论：新项目直接 Bitwuzla**。
- **档位**：【reference】。
- **第一步动作**：无。
- **许可/成熟度**：MIT；转向维护（authors 迁 Bitwuzla）。
- **链接**：https://github.com/Boolector/boolector

### 28. Yices（Yices 2；许可待核；维护中）
- **定位**：SRI 的 SMT 求解器：线性算术/位向量；SMT-COMP 常客。
- **可抄机制**：**多引擎组合（MCSAT + 位爆破 + simplex）**的求解器架构——"同一前端按理论分派引擎"；对 adv-bin：**按查询类型分派求解器**的设计先例。
- **档位**：【reference】（Rust 绑定不成熟）。
- **第一步动作**：无（架构设计读一页）。
- **许可/成熟度**：待核（GPL 类+商业例外）；维护中。
- **链接**：https://yices.csl.sri.com/

## §4 二进制相似/差分（对象 29–32；工具链角度，与 R1 只补不重）

### 29. BinDiff（专有；Google 2023 起免费分发）
- **定位**：二进制差分事实标准（Dullien 2005 起）。
- **可抄机制**：
  1. **两阶段匹配**：①预过滤：反汇编→调用图→每函数 CFG 特征向量（顶点/边数、出度等）→ 唯一特征 1:1 固定点；②**传播阶段**：沿调用图邻居扩展映射（已匹配顶点的邻居互配）。
  2. **MCS 式图匹配**：按非公开启发式做近似最大公共子图；核心是"特征 + 图传播"而非纯哈希。
- **档位**：【有界】（专有：抄算法流程；R1 已录 BSim，本域补差分算法侧）。
- **第一步动作**：adv-bin 相似性 MVP 实现"固定点 + 调用图传播"两阶段（先忽略 MCS 细节）。
- **许可/成熟度**：专有（2023 起免费使用，闭源）；维护中（Google 维护）。
- **链接**：https://github.com/google/bindiff

### 30. Diaphora（AGPL-3.0）
- **定位**：开源差分工具（IDA 插件扩展；亦接 Ghidra/BN 后端），工业常用。
- **可抄机制**：
  1. **贪心最大权匹配**：接近 Preis(1999) 的 ½-近似（**无传播阶段**——与 BinDiff 的已知差距，锚：Quarkslab 差分论文口径）。
  2. **高信息量属性启发**：CFG/调用图/圈复杂度/操作码类型/指令块数/栈帧大小/AST-伪代码差比（Hex-Rays 产物）——属性清单可直接抄。
- **档位**：【有界】（AGPL ⇒ 只抄属性清单与匹配算法；代码不内联）。
- **第一步动作**：抄其"属性清单"作为 adv-bin 相似性特征候选表，逐项标注可得性（没有 IDA 的项降级）。
- **许可/成熟度**：AGPL-3.0；维护中。
- **链接**：https://github.com/joxeankoret/diaphora

### 31. Trex（许可待核；研究原型，watch）
- **定位**：用**微轨迹（micro-traces）预训练的层级 Transformer**做函数相似（跨架构 x86/x64/ARM/MIPS）。
- **可抄机制**：动态微轨迹预训练 → 静态特征匹配；"执行语义学相似性"（对比静态语法相似）；数据集口径：10 库（binutils/coreutils/GMP/ImageMagick/SQLite 等）跨 4 架构。
- **档位**：【watch】（DL 相似性，部署成本/可解释性未定）。
- **第一步动作**：跟踪；若 adv-bin 相似性需跨优化级鲁棒，再评 embedding 路线。
- **许可/成熟度**：待核；研究原型（2021– 论文系）。
- **链接**：待核（检索口径：Trex, Pei et al.）

### 32. DeepBinDiff 等 embedding 路线（研究；reference）
- **定位**：跨优化级二进制相似的代表（程序级代码 embedding）。
- **可抄机制**：**注意口径**：GNN 类在函数相似任务总体占优，但模糊哈希（Catalog1/FunctionSimSearch）在多编译变量同变时失效（检索口径，TU Wien 2025 论文综述）——相似性系统的评测必须带"编译变量控制组"。
- **档位**：【reference】。
- **第一步动作**：adv-bin 相似性评测先定"编译变量矩阵"（O0/O2、strip、不同编译器）。
- **许可/成熟度**：研究；链接待核。
- **链接**：待核

## §5 工程边界与方法学论文（对象 33–41）

### 33. Veritesting（CMU；专利 US20150339217A1）
- **定位**：状态合并的经典手法（Avgerinos/Rebert/Cha/Brumley，CACM'16）：在**合并点**把菱形控制流静态合并（SSE 段），与动态符号执行交替。
- **可抄机制**：
  1. **DSE/SSE 交替**：动态到合并点后切静态探索一段，再回动态——"哪段便宜用哪段"。
  2. **战绩与实现**：MergePoint 在 Linux 发行版测得 10,000+ bug（论文口径）；angr 内置 Veritesting 策略（对象 1）；注意**有专利**（US20150339217A1）⇒ 抄思想，落地设计走"深度/长度上限 + 折中合并"的独立实现，避免照搬权利要求细节。
- **档位**：【有界】（专利+实现细节；思想抄进 adv-bin 探索策略）。
- **第一步动作**：L2 档默认"先 DFS 到合并点，再按深度上限合并"策略草案。
- **许可/成熟度**：论文+专利（非开源工具）；思想广泛实现（angr/Mayhem）。
- **链接**：https://cacm.acm.org/research/enhancing-symbolic-execution-with-veritesting/

### 34. SoK: (State of) The Art of War（S&P'16）
- **定位**：进攻性二进制分析的系统化综述（angr 团队）：符号执行、崩溃复现、自动利用。
- **可抄机制**：**框架式对比表**：引擎（KLEE/Mayhem/S2E/angr）在状态表示/路径选择/内存模型/环境建模/剪枝 五轴上的取舍——adv-bin 的技术选型表可直接沿用其五轴。
- **档位**：【reference】（选型方法论）。
- **第一步动作**：用五轴把 adv-bin L2 候选（自研/angr/Triton）横评一页。
- **许可/成熟度**：论文（2016）。
- **链接**：https://ieeexplore.ieee.org/document/7546500

### 35. Baldoni et al. 符号执行综述（ACM CSUR 2018）
- **定位**：符号执行技术的系统综述（路径爆炸/状态合并/求解策略分类学）。
- **可抄机制**：路径爆炸缓解技术的分类树（搜索启发/剪枝/合并/摘要）——查规划漏项用。
- **档位**：【reference】。
- **第一步动作**：无。
- **许可/成熟度**：论文（2018）。
- **链接**：https://dl.acm.org/doi/10.1145/3182657

### 36. Symbolic Execution in Practice（arXiv:2508.06643，2025-08）★2025 信号
- **定位**：2025 综述：漏洞/恶意软件/固件/协议四域的符号执行实践现状。
- **可抄机制**：四域适用性矩阵（各域的 harness 模式、环境建模需求、典型收益）；含 LLM+符号检查混合的引用图。
- **档位**：【reference】（本域边界结论的最新汇总锚）。
- **第一步动作**：抽出"固件/协议"两节作为 adv-bin 场景筛选的输入。
- **许可/成熟度**：论文（2025-08）。
- **链接**：https://arxiv.org/abs/2508.06643

### 37. LLM × KLEE（arXiv:2511.08530 等，2025）★
- **定位**：两类工作：LLM 生成 KLEE 外部函数模型（Aalto 学位论文口径）；LLM 直接"模拟"KLEE 输出（检索口径：GPT-4o 仅 ~20% 准确，待核）。
- **可抄机制**：**反面对照**：符号执行的三件套（精确路径枚举/约束追踪/求解）不是统计模型能替代的 ⇒ ADV 里 LLM 只做"模型/桩函数草稿生成 + 解释"，绝不作为路径/约束判据。（与 R1 §0 的 LLM 边界同构。）
- **档位**：【watch】。
- **第一步动作**：若做 LLM 辅助，限定在"生成 syscall/库桩草稿"，人工复核后入模型库。
- **许可/成熟度**：论文/实验。
- **链接**：https://arxiv.org/abs/2511.08530（待核）

### 38. Veritas（LLM agent × stripped binaries）★
- **定位**：用"静态 grounding（LLVM IR 切片）+ 运行时 grounding"把 LLM 推理锚到二进制证据；口径：90% recall、发现 1 个 Apple 0-day（检索口径，待核）。
- **可抄机制**：**grounding 结构**：LLM 的每个结论必须挂到 IR 切片证据上——与 ADV"结论带证据"直接同构；它自我定位为 fuzzing/符号执行的互补件。
- **档位**：【watch】。
- **第一步动作**：把"证据切片"接口先做进 adv-bin（与是否上 LLM 无关）。
- **许可/成熟度**：研究（2025–2026）；链接待补。
- **链接**：待补

### 39. LLM×符号混合调度（PALM / Gordian / S²F）★
- **定位**：2025–2026 新方向：PALM（路径感知 LLM 测试，2025-06 口径）、Gordian（LLM 插入 ghost code 引导探索，2026-01 口径）、S²F（符号与 fuzz 的成本-收益调度）。
- **可抄机制**：**调度器视角**：把"符号/模糊/LLM"当三种预算源，按边际收益调度——adv-bin 不实现 LLM 探索，但可抄"成本-收益调度表"结构。
- **档位**：【watch】。
- **第一步动作**：跟踪 arXiv；不做实现。
- **许可/成熟度**：论文/实验；链接待补。
- **链接**：待补

### 40. E0 / selective symbolic instrumentation（Phrack，2025–2026 口径）★
- **定位**：fuzzing + 按需符号切片 + LLM 的自强化管线（检索口径）。
- **可抄机制**：**按需切片**：只在卡点处把执行切片交给符号引擎（与 Driller/QSYM 同族思想的 LLM 版）。
- **档位**：【watch】（Phrack 类来源，细节待核）。
- **第一步动作**：跟踪。
- **许可/成熟度**：待核；实验。
- **链接**：https://phrack.org（检索口径）

### 41. avatar2（许可待核；学术原型，watch）
- **定位**：编排多引擎（GDB/QEMU/angr/OpenOCD/PANDA）并在其间**转移状态**（NDSS BAR'18）。
- **可抄机制**：**状态转移契约**：不同工具各自有状态模型，把"状态导出/导入"做成统一抽象——adv-bin 若多引擎并存（静态/仿真/符号三代引擎），需要同型契约。
- **档位**：【watch】。
- **第一步动作**：设计 adv-bin 内部"引擎状态快照"接口草案（内存/寄存器/路径约束三分量）。
- **许可/成熟度**：待核；学术原型。
- **链接**：https://github.com/avatartwo/avatar2

## §6 重点回答

### ① 符号执行在工业扫描器里的适用边界
- **值得的（有界场景）**：单函数/单协议状态机/单解析器边界上的**深路径可达性**与**输入合成**（产出可复现测试用例 = test-replay 资产）；崩溃复现与输入最小化；环境建模完备的小核心（KLEE 甜点：coreutils 级、~84% 行覆盖/1h 每程序，论文口径）。
- **坑（不绝对化，均为口径）**：a) 路径爆炸：大解析器 10^10+ 路径（检索口径），无界探索不可行；b) 求解开销：KLEE 中 ~92% 时间在求解（论文口径）⇒ 预算是首位约束；c) 环境建模缺口：libc/syscall 模型不全直接导致假阴性（Fuzzolic 对 SymCC 的教训，论文口径）；d) 语义不支持面：浮点/线程/变长对象等（KLEE 论文口径）；e) 工程门槛：源码级方案要重编译（SymCC），二进制级要插桩/仿真（QSYM/SymQEMU）。
- **结论（适用范围）**：适用 = "小核心 + 明确 harness + 有界预算 + 与 fuzz 互补调度"；不适用 = "全程序、无界、无环境模型"的通用扫描。工业定位是**探针与用例工厂**，不是主力扫描器。

### ② lifting 路线选型（对 adv-bin）
| 路线 | 代表 | 成本 | 许可 | 建议 |
|---|---|---|---|---|
| VEX 类成熟 IR | PyVEX/angr | 高（全语义+flag 展开） | BSD-2/GPL 混合 | 有界：作对照系/外部引擎，不自建 |
| LLVM 重建 | Remill（McSema 已归档） | 很高（IR 冗长+LLVM 版本绑定） | 待核 | 不吸收实现；抄"语义库 vs CFG 恢复解耦" |
| 自写 decode + 轻 IR | iced-x86/yaxpeax + ESIL 式 | 中（x86 焦点可控） | 自研 | **adv-bin 起步首选**：L0/L1 够用；预算 ~300 算子 |
| 结构 IR 契约 | GTIRB | 低（只是格式） | Apache-2.0 | **吸收**：结构层先行，语义层插拔 |
- **选型结论**：起步自写（x86 优先，L0/L1）；需要多架构/全程序符号执行时以 angr/Triton **子进程**消费而非重写；GTIRB 式"结构-语义分离"作为 IR 存储契约。

### ③ SMT 求解器在 Rust 生态的可用绑定与性能档
| 求解器 | Rust 绑定 | 状态 | 性能档（口径） | 建议 |
|---|---|---|---|---|
| Z3 | z3.rs 0.20/0.19 | 成熟（deps.dev 10/10） | QF_BV ~慢 Bitwuzla 5×（CAV'23 累计口径） | **默认档**（MIT；注意 !Send/!Sync） |
| Bitwuzla | bitwuzla-sys 版本失配（0.8 vs C API 0.9.1） | 需自绑 | QF_BV/QF_ABV 最强（CAV'24 ~140× vs Z3 MBQI 口径） | BV 热路径第二引擎（可用 SMT-LIB 子进程降级） |
| cvc5 | cvc5-rs v0.3.2 官方 | 新但官方 | 量词/字符串强；SMT-COMP'25 增量第一 | 第三引擎/备选 |
| Yices | 无成熟绑定 | — | 线性算术向 | reference |
| 纯 Rust | — | 在 QF_BV 无竞争力（检索口径） | — | 不投入 |

### ④ 混合 fuzzing+符号执行：值得进 P7/test-replay 的思想
1. **互补调度**（Driller）：fuzz 便宜、符号贵 ⇒ 只在覆盖停滞时触发；"位图=反馈契约"接口化。
2. **约束削峰**（QSYM）：optimistic solving（只解末段）+ block pruning（指数退避）⇒ 每次求解限预算。
3. **插桩位置**（SymCC）：符号跟踪放编译/IR 层，不放解释器；对 Rust 目标 = 在 AST/HIR 插桩。
4. **近似求解**（Fuzzolic Fuzzy-SAT）：覆盖率优先，容错求解。
5. **合并/边界压制**（veritesting + LoopSeer bound=10 + LengthLimiter）：策略组合而非单一探索器。
5 条全部是**调度级**思想，可先落进 test-replay 的"探索预算表"，不依赖任何重型引擎。

### ⑤ adv-bin 动/静态分档路线（成本-收益）
| 档 | 内容 | 成本 | 收益 | 依据 |
|---|---|---|---|---|
| L0 | 纯静态反汇编 + 模式扫描（字符串/常量/导入表/YARA 类规则） | 低 | 已知模式命中；无求解 | R1/R5 已铺 |
| L1 | 轻 IR 数据流（def-use/常量传播/轻污点/间接跳转解析） | 中低 | 语义级模式（不止字节）；无 SMT | miasm 简化器/Triton AST pass/GTIRB 结构 |
| L2 | 有界符号执行（单函数/单路径；深度+循环上限；z3.rs；产出输入/约束） | 高 | 深路径可达性 + 测试输入合成 | QSYM 预算策略/veritesting/KLEE 边界 |
| L3 | 全程序无界符号执行 | 极高 | 不可交付（路径爆炸+建模） | 检索口径 10^10+ 路径 ⇒ **不吸收** |
- **动态档**（可选，独立子进程）：Unicorn/Qiling 仿真 + PANDA 式 record/replay 用于复现；GPL 边界靠进程隔离。

## §7 2025–2026 前沿信号（摘）
1. **综述**：*Symbolic Execution in Practice*（arXiv:2508.06643，2025-08）四域实践汇总 —— 本域边界结论的最新锚。
2. **LLM×符号双向**：LLM 造桩/建模（KLEE 生态）+ LLM 直接模拟符号执行被证伪（~20%，检索口径待核）⇒ 分界清晰：LLM 辅助**建模与解释**，不碰**路径与约束**。
3. **调度系新作**：PALM（2025-06 口径）/ Gordian（2026-01 口径）/ S²F —— "成本-收益调度"成为混合执行热词。
4. **求解器**：Bitwuzla(A)（CAV'24）持续在 QF_BV 领先；SMT-COMP 2025/2026：cvc5 综合、Bitwuzla BV、Z3 非线性——**多引擎分派**成为标准姿势。
5. **Rust 侧**：z3.rs 维护活跃（0.20.x）；cvc5-rs 官方化；Bitwuzla 绑定仍失配 ⇒ "z3.rs 默认 + 子进程降级"是当前务实档。
6. **lifting 退潮信号**：McSema 归档（2022-08）、GTIRB 最后大推 2024-08、Remill 活跃但只做指令级 ⇒ "全程序 LLVM 提升"不再是主流路线；BAP 侧 IsaBIL（ECOOP'25）代表"语义 IR 形式化"的持续价值。
7. **新混合路线**：SymSan（FuzzBench 口径优于三家，待复核）；E0（Phrack，按需符号切片+LLM）。
8. **相似性评测学**：GNN 类占优但模糊哈希在"多编译变量同变"时失效（TU Wien 2025 口径）⇒ 评测必须带编译变量控制组。

## §8 Top-3（行动建议）
1. **angr（吸收，adv-bin 对照系 + 外部引擎）**：它是"引擎分解"的完整标本（CLE/VEX/SimOS/SimProcedures/Claripy/SimulationManager+策略）；adv-bin 的分层/扩展点/状态桶设计照它对齐，L2 档可直接子进程消费。BSD-2 无碍。
2. **GTIRB（吸收，adv-parse/adv-bin 的 IR 契约）**：结构 IR（Protobuf+UUID+AuxData）与语义 IR 解耦 —— 先定结构表与挂载点，语义层（自写/ESIL 式/VEX）可后换；这是"现在就对、以后不用返工"的一次性投资。
3. **QSYM+veritesting 的调度包（吸收，test-replay/P7）**：optimistic solving + block pruning（QSYM）与合并点压制（veritesting）+ LoopSeer/LengthLimiter 上限（angr）组成"探索预算表"；先用 z3.rs 默认档落地，不依赖任何重型引擎即可开跑。

## §9 统计与登记
- 对象：**41**（deep 26 / sweep 15）；机制级 ≥ 26。
- 登记：`registry/R2.jsonl`（41 行，UTF-8，一行一 JSON）；字段 id/domain/name/kind/verdict/mechanism/integration/license/link/depth/date。
- 未决/待核清单：SymCC/SymQSYM 许可、SymSan 链接、Trex/DeepBinDiff/Veritas/PALM/Gordian 链接、S2E/Qiling/Remill/GTIRB/McSema 许可确认、"GPT-4o 模拟 KLEE ~20%"数字复核、SymSan CVE 对比数字复核。
