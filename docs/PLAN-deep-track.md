# PLAN: adv-ast-rust 深轨边车（M2 片4 设计稿，2026-10-04）

> 目的：Rust 代码的**真正数据流污点**——MIR 级 IFDS/摘要分析，补 tree-sitter 快轨
> 的语义盲区（宏展开后的事实、所有权/借用即传播路径、闭包捕获）。
> 验收总纲（蓝图 M2 行）：与旧 Rust 引擎对拍（行为等价金样）；旧引擎就在本树
> `rust/` 目录（已从 workspace exclude，只读作对拍基准）。
> **← 2026-10-06 实测否证：`rust/` 的污点引擎只扫 Python，Rust 侧无旧数据流引擎可对。
> 对拍对象改为快/深两轨自身，见 §4「片B 前提否证」与 §5 的片B/片C 改口径行。**

## 1. 工具链方案（先定，防止重蹈 rust-toolchain.toml 覆辙）

**选定：RUSTC_BOOTSTRAP=1 + rustc-dev 组件，不引入 nightly。**

- 理由：rust-toolchain.toml 已钉 stable 1.99.0（用户拍板）；per-目录 toolchain 文件
  只在 CWD 命中时生效，`cargo build -p adv-ast-rust`（从根触发）会落回根文件 ⇒
  要么破坏"单一事实源"要么改 CI 的构建编排。RUSTC_BOOTSTRAP=1 是 clippy/miri
  自用的成熟 hack：stable 工具链 + `rustup component add rustc-dev llvm-tools`，
  源码顶部 `#![feature(rustc_private)]` 即可链接 rustc_private。
- 配置落点：`.cargo/config.toml` 加 `[env] RUSTC_BOOTSTRAP = "1"`——**仅作用本仓
  构建**，不污染全局；xtask 加断言（rustc-dev 组件缺失时给可读报错）。
- 前置动作（一次性）：`rustup component add rustc-dev llvm-tools`（约 200MB，
  gnu/msvc 两侧都要，CI 缓存成本记入验收）。
  - **片A1 实测修正（2026-10-05）：体积差一个量级，且 llvm-tools 不要**。
    本机 1.99.0-gnu 装两件实测 C 盘 4.84GB→3.3GB（≈1.5GB，非 200MB）：
    `rustc-dev` 单件含 `rustc_driver-<hash>.dll` **325MB ×2**（`bin/` 与
    `lib/rustlib/<host>/lib/` 各一份）+ rustc-src 41MB + host rmeta（libcore 65MB 等）；
    `llvm-tools` 是 `lib/rustlib/<host>/bin/` 下 rust-lld 208MB / opt 196MB / llc 196MB /
    llvm-* 一把 ≈**700MB**，且 `bin/` 无独立 LLVM dll ⇒ `rustc_driver.dll` 已内联 LLVM，
    MIR 分析用不到它 ⇒ 已 `rustup component remove llvm-tools`（回滚 = 再加回，一条命令）。
    本机 C 盘 99% 满（剩 4.0GB），多占 700MB 不是省钱问题而是故障面问题（盘满写过 NUL 空洞）。
  - **组件声明落点修正**：只写 `rustup component add` 不够——CI 由 `rust-toolchain.toml`
    触发的自动补装走 minimal，只认该文件的 `components` 清单（教训锚②，见那个文件里的注释）。
    故 `rustc-dev` 已进 `rust-toolchain.toml`，`llvm-tools` 明确不列。代价：每轮 CI 多装 325MB。

## 2. 驱动形态

- adv-ast-rust 是**二进制 crate**（adv-ast-rust-driver）：经 `rustc_driver::RunCompiler`
  以库方式驱动 rustc，Callbacks 里在 after_analysis 阶段取 MIR。
  **片A1 实测修正（2026-10-05）**：1.99 无 `RunCompiler` 结构体，入口是
  `rustc_driver::run_compiler(&args, &mut dyn Callbacks + Send)`；`Callbacks` trait 定义在
  `rustc_driver_impl/src/lib.rs`，形态正是这里假设的经典四段（含 `after_analysis`）⇒ 本行设计成立，
  只是名字要改。
- 输入形态（分两步走）：
  1. 片A：合成 crate 驱动——参数收 .rs 文件列表，拼 `--crate-type=lib --edition=2024`
     调 rustc（自包含文件可分析；真实仓库按 crate 驱动，片B）。
  2. 片B：cargo 集成——RUSTC_WORKSPACE_WRAPPER 环境变量方式挂进目标仓构建
     （对拍 rust/ 时用）。
     **片B1 前置实测（2026-10-06）**：直通档在 1.99 上可行，依据三条实况——
     ① `run_compiler` 会**丢掉 args[0]**（`rustc_driver_impl/src/lib.rs:183`，注释原话
     "Throw away the first argument, the name of the binary"），而 cargo 调包装器的形态正是
     `driver.exe <rustc 路径> <rustc 参数…>` ⇒ 那个 rustc 路径天然当占位，其余参数原样转发即可；
     ② `Callbacks` trait 只有 `config`/`after_crate_root_parsing`/`after_expansion`/
     `after_analysis` 四段，**没有退出钩子**，成功路径靠 `run_compiler` 返回后 main 正常结束、
     失败由 rustc 自身的诊断出口定退出码 ⇒ 只要 `after_analysis` 返回
     `Compilation::Continue`（现在恒返回 `Stop`，`crates/adv-ast-rust/src/main.rs:76-83`），
     工件与退出码就是 rustc 本来的行为，cargo 的 `-vV` 探测同理走 `handle_options` 原路；
     ③ findings 要带**文件路径**才有用：现在 `Hit` 只有行/列（`taint_reach.rs:68-77`），
     文件名是 CLI 侧按"一次一个文件"假设塞进去的（`crates/adv-cli/src/mir.rs:117`），
     整 crate 一档这个假设就断了 ⇒ 取径实测为
     `tcx.sess.source_map().span_to_filename(span).prefer_local_unconditionally()
     .to_string_lossy()`（`rustc_span/src/lib.rs:561` 与 `:611`，1.99 的 `RealFileName`
     已是 struct、`FileNameDisplayPreference` 私有，不能自己构造 pref）。
     ⇒ 本片不自带规则表：规则目录与输出目录一律走环境变量（`ADV_MIR_RULES`/`ADV_MIR_OUT`），
     缺任一个即非零退出（静默产空 = 假绿入口）。
- 输出：MIR 事实 → JSON → 主引擎（adv-taint）消费；**边车不进主进程**
  （nightly/不稳定 API 崩溃隔离在子进程——RESEARCH 01 边车纪律的实现形态）。

## 3. 分析内容（片A 最小闭环）

- MIR 数据流：借用 rustc_mir::dataflow 框架的 GenKill 分析实现「污点值到达」：
  **片A2 实测否证（2026-10-05，5f5c7b0）**：1.99 已删 `GenKillAnalysis`（框架源码原话"它并不比
  `Analysis` 快"），只剩 `Analysis` trait；框架项从 crate 根再导出（`framework` 模块私有）；
  起分析用 `analysis.iterate_to_fixpoint(tcx, body, pass_name)` 而非 `Framework::new(...)`；
  另需实现 `const NAME`。**`Rvalue` 已无 `Call` 变体** ⇒ 下一条"传播"里的 `Rvalue::Use` 仍在，
  但调用点全部只出现在 `TerminatorKind::Call` 一处。
  - 源点：`extern fn` 调用返回值（规则集与 adv-rules 的 taint spec 同源——
    从 YAML 读，单一事实源）。
  - 传播：move/copy/引用 deref；`Rvalue::Use`/`UnaryOp`/`BinaryOp` 直传。
  - 汇点：调用图匹配 spec 的 sinks（callee 路径按 DefPathInfo 拼）。
    **片A2 实测补充**：实际取的是 `tcx.def_path_str(FnDef)`，泛型段会留在名字里
    （`std::result::Result::<T, E>::unwrap_or_default`）⇒ 规则匹配须带 `::`/`.` 后缀档。
- 产出 findings 与 adv-taint 快轨同 schema（复用 Finding::to_jsonl），来源字段
  加 `"engine": "mir"` 供对拍分账。**片A2 状态**：finding 已是 JSON 且带 `engine: mir`，
  但**尚未与快轨 `Finding::to_jsonl` 对齐**——对齐归片A3。

## 4. 对拍（验收核心）

- 基准 = `rust/`（旧引擎，20K LOC）上跑同一 YAML 规则集：
  快轨（tree-sitter）与深轨（MIR）的结果做**三方账**：两边都报 / 仅快轨 / 仅深轨。
  仅快轨 = 快轨误报或深轨漏报（宏/控制流盲区），逐条进 FP 会计账；
  仅深轨 = 快轨 FN 清单（这正是深轨的价值证明）。
- 金样：挑 3–5 个含 `unwrap/panic/expect` 的真实文件冻结两端输出。
  - **片B 前提否证（2026-10-06 实测）**：本节的"基准 = `rust/` 旧引擎"**不成立**——
    `rust/` 里的污点引擎只吃 Python：`rust/src/taint.rs:131` 按 `.py` 过滤，整个
    `taint.rs` 里 "rust"/".rs" 零命中；`rust/` 对 `.rs` 的处理只有
    `rust/src/astscan/rust.rs`（403 行，`mask_rust`/`fn_re_search`/`panic_call_finditer`/
    `unsafe_count_line` 这类行级+掩码模式匹配），**没有任何 Rust 侧数据流实现**。
    ⇒ Rust 数据流这一侧不存在可对的旧基准，"深轨 MIR 结果 vs 旧引擎"按字面做不到。
    对拍对象据此改为**两轨自身**（快轨 tree-sitter vs 深轨 MIR）在真实 crate 上出三方账
    （判据已在 `crates/adv-cli/src/main.rs:221-247`，键 = 规则/文件/起始行）；
    "旧引擎 vs 快轨"的账仍然可做，但那是 **Python 侧**的账，与深轨无关，另立一片。
    本节"仅快轨/仅深轨"的语义、以及金样那句，都按两轨对拍继续成立。

## 5. 里程碑切分

| 片 | 内容 | 验收 |
|----|------|------|
| 片A1 | rustc-dev 组件 + bootstrap 配置 + 空驱动跑通（打印一个 crate 的 MIR 函数名） | 对给定 .rs 输出函数名列表；xtask 断言组件在位 |
| 片A2 | GenKill 污点到达分析（源/汇同源 YAML） | 合成夹具：直接流/ sanitizer 零发现/跨基本块流 |
| 片A3 | 子进程隔离 + JSON 对接 adv-taint | adv-cli `--engine mir` 档；快轨/深轨三方账出账 |
| 片B | cargo 集成 + 对拍 rust/ | 行为等价金样 |
| 片B（2026-10-06 改口径） | cargo 集成（`RUSTC_WORKSPACE_WRAPPER`）让深轨吃带依赖的真实 crate | 本仓某真实 crate 上 `adv scan --engine both` 跑通（不再 exit=101/0 finding）+ 两轨三方账出账 + 快/深两端金样冻结。对拍对象改两轨自身，理由见 §4 的「片B 前提否证」。 |
| 片C（新增，原"对拍 rust/"可做的部分） | 旧引擎（Python 侧污点）vs 新快轨（tree-sitter）在同一批 Python 文件上出旧/新账 | 规则集对齐口径写清（旧引擎规则在 `rust/` 内部、新仓在 `rules/*.yaml`，不同源）；这条账属快轨，不作为深轨验收。 |

## 6. 已知风险

- rustc_private API 逐版本变动：已钉 1.99.0 ⇒ 冻结期内 API 稳定；升版 = 边车适配
  （这正是边车隔离纪律的存在理由）。
- rustc-dev 体积与 CI 时长：组件缓存策略待测（actions/cache 对 rustup components
  的命中）；预算超标则 CI 只在周期档跑深轨（RESEARCH 03 Miri/Kani 同款分档）。
- windows-gnu 工具链的 llvm-tools 兼容性：片A1 第一件事就是验证，不通则
  片A1 改用 msvc 侧工具链跑边车（本机 msvc 1.99.0 已修复可用）。
