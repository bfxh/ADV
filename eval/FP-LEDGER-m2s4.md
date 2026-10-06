# FP 会计账 — M2 片A1（2026-10-05）

> 片A1 交付（docs/PLAN-deep-track.md §5 第一行）：**深轨边车空驱动跑通**——
> 新成员 crate `crates/adv-ast-rust`（`adv-ast-rust-driver`）以库方式驱动 rustc，在
> `after_analysis` 取 `tcx.mir_keys` 输出该 crate 的 MIR 函数名清单；组件在位断言两处落
> （build.rs fail-closed + `cargo run -p xtask -- mir`）。**未引入 nightly**。
> 复现：`cargo test -p adv-ast-rust`（2 条，其中 1 条真起子进程跑夹具）。

## 判据（本片实测）

| 项 | 命令 | 结果 |
|----|------|------|
| 驱动输出 | `PATH=$(rustc --print sysroot)/bin target/debug/adv-ast-rust-driver.exe crates/adv-ast-rust/tests/fixtures/m1_probe.rs` | 4 行：`flow`/`helper`/`source`/`unused_unit`，exit=0 |
| 无输入 fail-closed | 同上不带参数 | 非零退出 + stderr 含「用法」（钉成测试 `driver_fails_closed_without_input`） |
| 组件断言 | `cargo run -p xtask -- mir` | 绿 |
| 全量 | `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` | 35 档 42 条全过 / 干净 / 0 |
| 门 | `cargo run -p xtask -- gate` | gate + lockstep + suppress 绿 |
| CI（msvc 档） | `gh run list/view`：670fd70→37306492822、cef520d→37307126960、7f61c73→37307651762 | 三轮均 **success**（fmt/clippy/nextest/coverage/gates/deny 全过） |

**提交后复跑（干净树，7f61c73）**：`cargo run -p xtask -- mutants --base 6201ea4`（**不带** `--update`）
→ `mutants: 绿`，即棘轮从"已入库基线"这一状态成立，而不是靠 `--update` 的自证；
`gate`/`lockstep`/`suppress` 绿、`cargo test --workspace` 35 档全过、`git status` 干净。

**msvc 侧的间接坐实**：本机没有 msvc 的 rustc-dev（C 盘 99% 满，不装），`.lib` 命名是**假设**；
但 build.rs 在判据不匹配时直接 panic ⇒ CI 的 msvc 档编过 = 那把尺在 msvc 上也认得出工件。
这是间接证据（不是 `ls` 观测），所以 driver_probe 的测试注释与这里都按"间接坐实"记账，
不写成"已验证 msvc 布局"。

夹具四条里 `flow` 是唯一一条**跨语句直接数据流**（`raw.trim().to_string()` → `cleaned.len()`），
片A2 的 GenKill 断言将复用它；`unused_unit` 是空函数体（检验"有 MIR 但无语句"也在清单里）。

## 工具链前置：实测把计划稿的两个数字否证了

1. **体积差一个量级**。计划 §1 写"约 200MB"，实测装 `rustc-dev` + `llvm-tools` 后
   C 盘 4.84GB→3.3GB（≈1.5GB）：`rustc-dev` 单份 `rustc_driver-<hash>.dll` **325MB，且在
   `bin/` 与 `lib/rustlib/<host>/lib/` 各存一份**；`llvm-tools` 是 `lib/rustlib/<host>/bin/`
   下 rust-lld 208MB + opt 196MB + llc 196MB + llvm-* 一把 ≈**700MB**。
2. **llvm-tools 对本片无用**：`bin/` 下无独立 LLVM dll ⇒ `rustc_driver.dll` 已内联 LLVM，
   MIR 分析不碰 LLVM API ⇒ 已 `rustup component remove`（回滚 = 一条 add 命令）。
   留它不是"以防万一"的问题：本机 C 盘 99% 满（剩 4.0GB），而盘满在这台机器上写过 NUL 空洞
   （见 ZCode bot-state 事故），多占 700MB 是故障面。

## API 实况修正（1.99.0，读 rustc-src 而非记忆）

- 计划 §2 写 `rustc_driver::RunCompiler`；1.99 实况**无该结构体**，入口是
  `rustc_driver::run_compiler(&args, &mut dyn Callbacks + Send)`（`Callbacks` trait 定义在
  `rustc_driver_impl/src/lib.rs`）。
- `Callbacks` 是**经典四段形态**（`config` / `after_crate_root_parsing` / `after_expansion` /
  `after_analysis` 返回 `Compilation`），不是后来那套 `query_callbacks` ⇒ 计划假设的
  "after_analysis 取 MIR"原样成立。
- 取面：`tcx.mir_keys(())`（返回可 `for &def_id in` 的清单）+ `tcx.def_path_str(def)`。

## 跨工具链的两个坑（本机 gnu 过 ≠ CI msvc 过）

1. **链接搜索路径**：gnu 侧导入库在 `lib/rustlib/<host>/lib/librustc_driver-*.dll.a`，
   msvc 侧在 `.../lib/bin/rustc_driver.lib` ⇒ build.rs 两个目录都 `rustc-link-search`。
2. **运行期 DLL**：`rustc_driver-*.dll` 在工具链 `bin/`，`cargo run`/测试的 PATH 不含它 ⇒
   build.rs 经 `ADV_RUSTC_BIN_DIR` 把目录交给调用方补（测试与将来的 CLI 边车调用各自补 PATH），
   不在 crate 里改全局环境变量。

## god 基线重录盘点（`cargo run -p xtask -- god --write`，两次提交各录一次，快照可比对）

**第一步（670fd70，驱动 crate 落地）**：224 → 246 项 = 新面 22 / 涨 4 / 降 0 / 掉 0。

涨的 4 条全部是"给门自己加接线"的成本，非算法复杂度：

| 键 | 前→后 | 原因 |
|----|-------|------|
| `file:xtask/src/god.rs` | 294→295 | MEMBER_DIRS 加一行登记新 crate |
| `file:xtask/src/lib.rs` | 8→9 | `pub mod mir;` |
| `file:xtask/src/main.rs` | 70→74 | 分派臂 + 用法串换行 |
| `fn:xtask/src/main.rs::main` | 45→48 | 同上（一行 `Some("mir") => report(...)`） |

新面 22 条 = 本片代码（driver 2 文件 + build.rs + 2 测试文件 + `xtask/src/mir.rs` 及其函数/类型轴）。

**第二步（cef520d，变异门逼出的判据抽取）**：246 → 258 项 = 新面 13 / 涨 0 / **降 4** / 掉 1。

| 变化 | 明细 |
|------|------|
| 降 | `file:…build.rs` 82→56、`fn:…build.rs::main` 33→22（判据搬走）；`file:xtask/src/mir.rs` 60→44、`fn:…mir.rs::run` 27→9（同上） |
| 掉 | `fn:crates/adv-ast-rust/build.rs::find_driver_artifact`——移入 `src/driver_probe.rs` 后以新键计量，不是"消失的债" |
| 新面 13 | `driver_probe.rs`（4 函数）+ `src/lib.rs` + `tests/driver_probe.rs`（6 函数） |

**第三步（本账提交，只改注释）**：`file:crates/adv-ast-rust/tests/driver_probe.rs` 101→102
（把"msvc 待坐实"改写成"间接坐实"多两行）——棘轮连注释长度都管，如实登记，不悄悄 `--write`。


## 变异面（base 6201ea4，即片3 基线的原生口径，`--update` 重录）

`target/mutants-out/mutants.out/outcomes.json` 实况（顶层键，非 `summary` 嵌套）：

| 量 | 片3 记账 | 本片实测 |
|----|----------|----------|
| 变异总数 | 75 | **96** |
| 捕获 CaughtMutant | 48 | **56** |
| 未捕获 MissedMutant 条目 | 16 | **22** |
| 未 viable Unviable / Success | — | 17 / 1 |
| 收敛后的基线键数 | 8 | **10** |

与旧基线逐键 diff：**只增 2、不减、无杂项漂入**——

| 新增未捕获键 | 性质 |
|--------------|------|
| `xtask/src/mir.rs::run` | IO 壳：解析 sysroot/host 后调判据。"总在位"与"总不在位"两个变异都无法被单测区分（要区分得 mock 外部 `rustc`，那就不再是真路径） |
| `xtask/src/mir.rs::rustc_print` | IO 壳：同上，且它喂的是 rustc 真实输出格式 |

同族的**判定逻辑**（`is_driver_artifact`/`artifact_dirs`/`find_driver_artifact`/`missing_message`）
**没有出现在未捕获清单里**——即 cef520d 补的 5 条金丝雀把它们全杀掉了；
这正是"IO 壳才进债账、判定逻辑必须受断言"那条尺的量化形态。
存量 8 键（taint.rs 四条 + xtask 自身四条）原样保留，未被这次重录洗掉。

> 口径提醒：`--update` 是**整表替换**，不是并集。所以重录必须用与旧基线相同的 `--base`
> （这里 6201ea4 = M2 片1），否则差异面收窄会把存量债键静默删掉。本片按此执行并逐键 diff 留档。


## CI 成本首次计数

`rustc-dev` 进 `rust-toolchain.toml` 的 `components` ⇒ **每轮 CI 多装 325MB**（本机 gnu 实测单份
dll 体积，msvc 同量级）。这是计划 §6"组件缓存策略待测"的首个数：本轮先原样付，若 CI 时长涨幅
>2 分钟再走 §6 的周期档方案（深轨只在周期档编）。

## 片A2（5f5c7b0）：MIR 污点到达——计划稿又被否证四处

`rustc_mir_dataflow/src/framework/mod.rs:3` 原文（读 rustc-src，不是记忆）：
*"There used to be a `GenKillAnalysis` alternative trait for gen-kill analyses that would
pre-compute the transfer function for each block. It was intended as an optimization, but it
ended up not being any faster than `Analysis`."* ⇒ 计划 §3 的"借用 GenKill 框架"在 1.99 不成立。

| 计划稿假设 | 1.99 实况 | 影响 |
|------------|----------|------|
| 实现 `GenKillAnalysis` | 该 trait 已删，只剩 `Analysis`（`Domain: Clone + JoinSemiLattice`，不是旧名 `DataflowStruct`） | 照写编译不过 |
| `Framework::new(...)` 起分析 | 扩展方法 `analysis.iterate_to_fixpoint(tcx, body, pass_name)`（框架 doc 与 `rustc_peek.rs` 两处一致） | 入口换法 |
| 源/汇在 `Rvalue::Call` 与 terminator 两处都有 | **`Rvalue` 已无 `Call` 变体**，调用一律 `TerminatorKind::Call` | 转移面比计划更小；实测 `_2 = var::<&str>(…) -> [return: bb1, unwind continue]` 就是终结点 |
| —（计划未提） | `Analysis` 还要求 `const NAME: &'static str`（区分同一分析的多轮结果）；框架项从 **crate 根**再导出，`framework` 模块私有（按 `::framework::` 写报 E0603） | 两项补齐 |

### 规则同源与"名字从哪来"

四要素一律经 `adv_rules::rule::load_rules` 从 `rules/**.yaml` 读，深轨**不自带规则表**。
新规则 `rules/rust/taint-command.yaml` 里每个 callee 名都取自本片第一个动作的取证输出
（驱动加 `--dump-calls` 档，实测 `def_path_str`）：

```
run  std::env::var                                  _2   const
run  std::result::Result::<T, E>::unwrap_or_default _1   _2
run  std::process::Command::new                     _7   _3
run  std::vec::Vec::<T, A>::len                     _0   _10
```

泛型段会留在路径里（`Result::<T, E>::unwrap_or_default`），所以 `callee_matches` 必须带
`::`/`.` 后缀档，规则里才能只写用户可见的 `unwrap_or_default`、`len`。

### 判据（三条合成夹具，全部起子进程跑真驱动）

| 夹具 | 意图 | 实测 |
|------|------|------|
| `taint_direct.rs` | 源点→声明过的 propagator→汇点实参 | 报 1 条，`location = bb2.0`，rule=RS-TAINT-COMMAND，engine=mir |
| `taint_sanitized.rs` | 源点→`len`（sanitizer）→`to_string`（此时输入已无污点） | **零发现**——这条是假阳性尺：把 sanitizer 当传播子就会报 |
| `taint_branch.rs` | 源点在 `if` 的 then 块、汇点在合流之后 | 报 1 条，`location = bb7.2`——抓"只看单基本块"的实现 |

保守口径如实进账：**未声明的调用杀目的局部**，`Aggregate/Discriminant/Downcast/Len/
ThreadLocalRef` 等 Rvalue 形态本片不追 ⇒ 深轨在这些形态上是 **FN（漏报）面**，
片B 对拍时逐条落账，不写成"已覆盖"。

### 适用边界（实测，防止"深轨已能扫真仓"的误读）

计划 §2 写的片A 限制是"自包含文件可分析"。本片实测两个真实文件：

| 输入 | 结果 |
|------|------|
| `crates/adv-rules/src/taint.rs` | exit=101，`error[E0433]: too many leading 'super' keywords within 'crate'`，**finding 0 行** |
| `xtask/src/mutants.rs` | exit=101，`error[E0432]: unresolved import 'anyhow'`，finding 0 行 |

两点都要记：① 单文件驱动不带依赖解析 ⇒ 深轨现在**扫不了本仓真实代码**（片B 的
cargo 集成才是解）；② 失败是**响的**——非零退出且不出 finding，不是静默返回空清单。
若哪天它改成"报错也打印空结果"，就是假绿入口，届时须按这条否证。

### 快轨侧影响（实测，不是推断）

`rule_language()` 把 `languages: [rust]` 映射成 `Language::Rust` 供 matcher 过滤
⇒ 新规则对 rust 文件即进入快轨规则集。加规则后 `cargo test --workspace` 36 档全过，
含片1 的报告层金标准 ⇒ **未见快轨输出变化**。此判据只到"金标准没被推翻"，
不等于快轨的 rust 污点实现已可用（那是另一片的事）。

### god 基线（片A2 登记）

258 → 304 项 = 新面 46 / 涨 4 / 降 1 / 掉 0。涨的 4 条全在驱动 bin（`--dump-calls`、
`--taint` 两档接进 `main.rs`：文件 72→237、`main` 32→38、`MirProbe` 字段 1→5、lib 5→6）；
降的 1 条是 `MirProbe::after_analysis` 12→8（逻辑搬进 `harvest`）。

---

# 片A3（a9e8212 / 9425f11，2026-10-05）：两轨共用一个 Finding + `adv scan --engine`

> 交付：`Finding` 增 `engine` 字段（A 案由用户拍板）；adv-cli 出 `--engine ast|mir|both` 三档，
> 深轨以子进程边车接入并把输出**构造回同一个 `Finding`**；`both` 档 stderr 出三方账。
> 复现：`cargo test -p adv-cli`（4 条，全走真进程）。

## 金标准重录（有意识动冻结值，逐行核对）

机制用的是门自己给的通道：`ADV_UPDATE_GOLDEN=1 cargo test -p adv-rules`（见 golden.rs 头注释）。
改前留了 `target/scan.jsonl.pre-engine` 快照，核对是**脚本比字段**而不是肉眼看 diff：

| 主张 | 判据 | 结果 |
|------|------|------|
| 行数没变 | `len(pre) == len(post)` | 5 → 5 ✓ |
| 除 `engine` 外零漂移 | 逐行去掉 engine 键后与旧行全等 | 有差异行数 = **0** ✓ |
| 新增键只有 `engine` | 键集差 | `['engine']` ✓ |
| 取值一致 | 集合 | 全 `tree-sitter` ✓ |

改 `Finding` 前先确认了构造点只有两处（`matcher.rs::new_finding`、`taint.rs::new_finding`），
并把散落两处的 `format!("{:?}", sev).to_lowercase()` 收成 `matcher::severity_name()` 一处。

## 三方账的实测起点（不是"深轨更强"的证据）

`adv scan crates/adv-ast-rust/tests/fixtures --engine both`：stdout 4 条、
stderr「三方账：两边都报 **0** / 仅快轨 **0** / 仅深轨 **4**」。
快轨在这批 rust 夹具上 0 条——所以这个 4 只说明"深轨第一次通过统一行契约出了账"，
**不构成两轨强弱对比**；对拍要等片B 用真实仓 + 同一规则集跑两边。

加规则对快轨的影响也测了：`rule_language()` 会把 `languages: [rust]` 的规则纳入快轨规则集，
加 `RS-TAINT-COMMAND` 后 `cargo test --workspace` 仍全过（含金标准）⇒ 未见快轨输出变化。

## 变异门：A2 剩的两条存活面为什么"补了断言还活着"

`a9e8212` 干净树复测（base 6201ea4）：194 变异 / 141 捕获 / **9 条未捕获条目（收敛 5 键）** / 44 unviable，
`mutants: 绿`，且**未用 `--update`**——逐键核对 5 个键全部已在基线内（`keys ⊆ base` 判 True）。

但这里有一条**必须写下来的否证**，不能把"绿"读成"债还了"：

- A2 报的两条 `taint_reach.rs::tainted_operand`、`::Reach::all_sanitizers`，我先误判为"断言不够强"，
  加了正反同文件对照仍存活。手动把变异打进源码跑本包测试才看清：**能杀它们的断言在 adv-cli 包里**，
  而 cargo-mutants 默认只跑变异所在包的测试 ⇒ 跨包覆盖对这个门不算数。
  修法是搬判据（`product_rules_do_not_source_the_int_chain` 搬进 adv-ast-rust、
  另加夹具⑤ `tests/dual/` 钉"净化工优先于传播子"），不是去配排除项。
  两条变异在重构后各自被**重新手动验证**一次判红。
- 同一轮里，基线的 5 个存量键（`xtask/src/mir.rs::run`/`::rustc_print`、
  `mutants.rs::git_diff_patch`/`::missed_key`/`::run`）**本轮一个 missed 都没产**——
  它们的变异全被判 **Unviable**（这两个文件 17 条）。
- **更正（同日查因，撤回上一版的定性）**：当时我把"全 unviable"写成"分类变化"，
  读了 `log/xtask__src__mir.rs_line_13_col_5.log` 后真相是**环境假象**：
  `error: linking with 'x86_64-w64-mingw32-gcc' failed` → `ld.exe: final link failed: No space left on device`
  （cargo-mutants 在 `C:\Users\...\Temp\cargo-mutants-ADV-*.tmp` 建产物，本机 C 盘仅剩 4.1GB）。
  ⇒ 上一轮的 `mutants: 绿` 里含**被盘满掩盖的漏测**（这 5 个键的变异根本没跑到测试阶段），
  不是"债失去可测性"那种理论情况，而是实打实的 false green。
  处置：复跑时把 TMP 指到有空间的盘；并（下面这条）把"基线键本轮既无 caught 又无 missed"
  变成门会判红的判据——**这道新门正是为了抓住这一类而加，而它的第一案就是我自己这次的漏测**。

⇒ 由此暴露的门的洞（下一片建议补，先记此处）：missed 棘轮只看"新增未捕获键"，
对"基线键本轮既没 caught 也没 missed（全 unviable）"完全无感——债可以从账上静默蒸发。
补法应是把它判红/判警告：基线里的键若本轮既不出现在 caught 也不出现在 missed，就报
"该键失去可验证性"，而不是默认绿。

## CI

`a9e8212` → run 37312377908 **success**；`9425f11` → run 37314107000 **success**。
msvc 档能编并跑通深轨相关测试，此前已由 `5f5c7b0`/`8adf1c6` 两轮绿确立。

## 本片重放记录

`cargo test --workspace` 37 档 53 条全过 · `cargo clippy --workspace --all-targets -- -D warnings` 干净 ·
`cargo fmt --all --check` 0 · `xtask gate`（god+lockstep+suppress）绿 · `xtask mir` 绿 ·
`xtask mutants --base 6201ea4` 绿。god 基线 342 → 346（涨 1、新面 5、掉 1）。

## 下一步

1. **补门的洞**（本片正在做）：变异门增加判据——基线里的键若本轮既不出现在 caught
   也不出现在 missed，报"该键本轮不可验证"并判红，不得默认绿。
   两种成因都要被它抓住：理论上的"变异全判 unviable"，以及本次实测到的
   **环境性假 unviable（盘满导致链接失败，变异没跑到测试阶段）**——后者更危险，
   因为它让门在缺测的情况下报绿。
2. **片B**：cargo 集成（RUSTC_WORKSPACE_WRAPPER）+ 对拍 `rust/` 旧引擎出真三方账。
   深轨当前只能吃自包含文件（见"适用边界"），片B 才是价值证明的那一步。

---

# 片A4（5dd4211 / e4c3a53，2026-10-05）：门补强——"基线键可验证性"判据

> 交付：变异门第二道判据 + 用 cargo-mutants 真实产物锚定的键提取测试 + `--update` 移除点名。
> 复现：`cargo test -p xtask`（9 条金丝雀）；`cargo run -p xtask -- mutants --base 6201ea4`。

## 判据

基线里的键本轮必须有 ≥1 个变异跑到判定环节（`CaughtMutant` / `MissedMutant` / `TimeoutMutant`）。
全判 Unviable、或压根不在本轮差异面上 ⇒ 判红"基线键本轮无可判定变异"。
旧判据只看新增 missed，所以"债变得不可测"这件事在门眼里等于没问题。

## 三条实测（顺序就是发现顺序）

| 主张 | 判据 | 结果 |
|------|------|------|
| 上一轮的 `mutants: 绿` 含假象 | 读 `log/xtask__src__mir.rs_line_13_col_5.log` | `ld.exe: final link failed: No space left on device`（C 盘剩 4.1GB，cargo-mutants 在 C 盘 Temp 建产物）⇒ 17 条变异没进测试阶段 |
| 把 TMP 挪到 D 盘（剩 38GB）就能区分环境性与真 unviable | 同面复跑对比 | unviable **44 → 30**、caught **141 → 147**、missed 条目 **9 → 26**；基线 5 个"消失"的键里 4 个恢复出判定结果 ⇒ 盘满定性成立 |
| 新判据不是纸上的 | 复跑当场报红 | 抓到 `xtask/src/mutants.rs::missed_key`——它在片A4 被我改名成 `outcome_key`，本轮无任何可判定变异 |
| 片A3 搬判据真有效 | 同轮 outcomes 分类 | `tainted_operand`、`all_sanitizers` 均由 Missed 转 **CaughtMutant**（不只我手动验过） |

## 键提取的语料锚定（防"门作者自证"）

`xtask/tests/data/outcomes-sample.json` = 真产物切片（102 条，Missed/Caught/Unviable 三类，
保留外层 `{"outcomes":[…]}` 形态与 `file::function`、`<impl T for X>::method` 这些真实键形态）；
`.expected.txt` 由**同一产物的 summary 字段**导出 MISSED / VERIFIABLE 两份期望键清单。
测试含语料退化守卫（期望 missed ≥10 且可验证数 > missed 数）——否则"空语料"能让它假绿。

## 基线 10 → 9（有意识重录，移除被点名）

`--update` 输出即证据：`注意：本次重录从基线移除了 1 个键：xtask/src/mutants.rs::missed_key`。
合法性核过：`rg missed_key xtask/src/` **零命中** ⇒ 该键指向的函数已改名，退账不是洗债；
新增键 **0**（我新写的 `keys_by_summary`/`outcome_key` 被上面那套语料测试杀掉了）。
重录后逐键比对：本轮 missed 键集与基线**完全相等**（双向无漂移）。

## 变异面走势（同一条 `--base 6201ea4` 口径）

同一口径（`--base 6201ea4`）各轮实测，逐轮标明当时 HEAD：

| HEAD | 总变异 | 捕获 | 未捕获条目 | unviable | 门结论 / 基线键数 |
|------|--------|------|------------|----------|-------------------|
| 片3 首录（cdc36b7 后） | 75 | 48 | 16 | — | 首录 8 键 |
| cef520d（A1 补断言后） | 96 | 56 | 22 | 17 | 重录 → 10 键 |
| 5f5c7b0（A2） | 146 | 88 | 33 | 25 | 红：新增 5 键候选（后全部补断言杀掉，未入账） |
| a9e8212（A3 前段） | 194 | 141 | 9 | 44 | **绿——但是假绿**：5 个基线键的变异全判 unviable（盘满） |
| 5dd4211（A4 门补强，TMP 挪 D 盘） | 203 | 147 | 26 | 30 | 红 3 条：新代码 2 键未捕获 + 改名旧键不可验证 |
| e4c3a53（A4 语料测试后） | **203** | **153** | 20 | **30** | 绿；重录 10 → **9 键**（移除 1、新增 0） |

## 遗留（下次再做，别忘）

`xtask mutants` 目前不检查 TMP 所在盘的余量，盘满会以 unviable 的形式伪装成"没问题"。
新判据已经能把后果抓住（报红而不是报绿），但报错文案对使用者不够直白：
值得在跑之前加一次盘量预检并把"unviable 异常增多"写进提示。这是体验改进，不是判据缺口。

## 本片重放记录

`cargo test --workspace` 37 档 55 条全过 · clippy `--all-targets -D warnings` 干净 · fmt 0 ·
`xtask gate`（god+lockstep+suppress）绿 · `xtask mir` 绿 · god 基线 346 → 352（涨 3、新面 2）。

---

# 片A5（5b1d862 / 93b5f4f / 127c186，2026-10-06）：门补强——TMP 盘量预检 + unviable 日志认盘满

> 交付：片A4「遗留」那条做完——变异门跑之前查一次 scratch 盘量，跑之后从 unviable 条目
> 的日志里认盘满签名。复现：`cargo test -p xtask`（15 条金丝雀）、
> `ADV_MUTANTS_MIN_FREE_GIB=99999 cargo run -p xtask -- mutants --base 6201ea4`（必红且秒退）。

## 判据

| 判据 | 位置 | 语义 |
|------|------|------|
| 盘量预检 | `xtask/src/mutants.rs::precheck_scratch` | 实测 TMP 所在盘余量 < 下限 ⇒ 红 + 一行照抄修法；**查不到余量只播"未知"，不判红**（不拿"查不到"当"没空间"） |
| 盘满正判据 | `xtask/src/mutants.rs::unviable_disk_failures` | Unviable 条目的日志含 `No space left on device`/`final link failed`/`os error 112` 等签名 ⇒ 点名判红（这才是判据，预检只是提前拦） |
| 重录拒绝对缺测放行 | 同 `run` 的 `--update` 分支 | 本轮存在盘满缺测 ⇒ **不写基线**并返回红。否则一次盘满就能把 9 条债"重录"成 4 条 |

`DISK_FAILURE_SIGNATURES` 里的原话有账：`eval/FP-LEDGER-m2s4.md` 片A4 节记录的
`ld.exe: final link failed: No space left on device`（2026-10-05 实测）。

## 五条实测（顺序就是发现顺序）

| 主张 | 判据 | 结果 |
|------|------|------|
| 预检查的是真盘不是猜的 | 同一时刻三把尺对读 | `df -k /c` 42,853,012 KiB、`powershell (Get-PSDrive C).Free` 43,881,484,288 字节、门的读数 42.7GB —— **×1024 后逐字节相等**，两把尺互相验 |
| MSYS 的 `df` 不能作 Windows 路径的唯一查询 | `df -kP "C:\Users\...\Temp"` | 输出把参数吞了、报的是 `/tmp` 那个挂载的行 ⇒ Windows 主通道定成 PowerShell，`df` 只作兜底，并把"带盘符必须走 powershell 通道"钉进断言 |
| 盘量下限不是恒定红线 | 本会话内 C 盘余量 | 1.9GB（门判红）→ 约 40 分钟后 42.7GB（放行）。中间是另一件事清了 `%TEMP%\unified-rx-pytest`，不是我改的 ⇒ 8GB 这个默认值只表示"低于此当场会缺测"，写进注释并允许 `ADV_MUTANTS_MIN_FREE_GIB` 覆盖 |
| 语料不能叫 `mutants.out` | 第一次门复跑 | cargo-mutants `copy_tree.rs:109` 无条件跳过该目录名 ⇒ scratch 里没语料、**未变异树自测就失败**、门在"一条变异都没跑"时中止（判红是对的，但白跑一轮） |
| `.log` 语料等于没提交 | `git ls-files` + `git check-ignore -v` | 本仓 `.gitignore:6` 是 `*.log` ⇒ 5b1d862 里那两个语料文件**根本没进提交**（只有 README 和 outcomes.json），CI 的 `cargo test --workspace` 会因缺文件红。改为 `.txt` 摊平存、测试期拼 `<tmp>/mutants.out/log/<产物名>` |

金丝雀另抓到一条真 bug：`bytes[0].to_ascii_uppercase().to_string()` 给的是 ASCII 码
`"67"` 而不是盘符 `"C"`（`u8::to_string` 打数字），改 `char::from(bytes[0])` 后
`canary_scratch_space_query_actually_answers_on_this_machine` 才过。

## 门自己报红一次，然后被补断言杀掉

`--update` 未跑，棘轮如实报 6 个新增未捕获键：`precheck_scratch`、`free_bytes_at`、
`free_bytes_via_df`、`free_bytes_via_powershell`、`parse_u64_lines`、`drive_letter`。
处理方式按片A3 的教训（判据要与被变异代码同包）：把**解析与拼参数**从"起子进程"里拆出来
成纯函数（`parse_df_avail` / `parse_u64_lines` / `powershell_args` / `df_args` /
`short_message` / `free_bytes_probe`），子进程壳只留"起命令 + 交解析"；语料用同日两把尺的
逐字真输出（`xtask/tests/data/df-kp-{two,single}.txt`、`powershell-free.txt`），
断言含"同一块盘两把尺读数必须相等"与"畸形输入一律 None"。

## god 基线两次重录（逐键披露）

| 提交 | 项数 | 新增面 | 涨 | 降 | 移除 |
|------|------|--------|----|----|------|
| 5b1d862 | 352 → 368 | 16 | 3（mutants.rs 182→397、canary.rs 230→379、`run` 79→98） | 0 | 0 |
| 127c186 | 368 → 374 | 7 | 4（mutants.rs 397→421、canary.rs 379→512、两个盘满测试） | 3（三个壳变薄） | **1**：`free_bytes_at`（改名成 `free_bytes_probe`，`grep -rn free_bytes_at --include=*.rs .` 零命中 ⇒ 合法退账，同片A4 的 `missed_key` 案） |

## CI 与一次我自己的流程漏

`93b5f4f` → run 37420836104 **failure**：`xtask gate` 报 god 棘轮
（`canary.rs` 379→396、盘满测试 43→60）。原因是我在 93b5f4f 改了 `canary.rs` 之后只重放了
`test/clippy/fmt`，**没重放 `xtask gate`**，基线没跟着重录。这不是门的误报，是我漏步——
本片起把"改任何被 god 计量的文件 ⇒ 必跑 gate"当作收尾硬条件写在重放记录里。
`127c186` 起 CI 复绿情况见本节末「重放记录」。

---

# 片A6（2026-10-06）：门会虚计——三方账与深轨失败面的观察面补起来

> 起因不是新代码，是**同一提交连跑两轮变异门对不上账**：总数 283 恒定，
> caught/missed 却是 221/30 → 218/33，3 条从"被杀"翻成"存活"。
> 复现：`TMP=D:/tmp/adv-mut cargo run -p xtask -- mutants --base 6201ea4` 连跑两轮对数字。

## 机制（读产物日志坐实，不是猜）

第二轮带变异跑 `crates/adv-cli/tests/engine_mir.rs` 4 条全过（`test result: ok. 4 passed`）。
为什么过？因为**那批 rust 夹具上快轨 0 发现、深轨 0 失败**，于是三处分支的观察面是空的：

| 变异 | 位置 | 为什么观察不到 |
|------|------|----------------|
| `replace != with ==` | `adv-cli/src/main.rs` 快轨开关 `if engine != Engine::Mir` | 快轨在该语料产 0 条，跑与不跑输出一样 |
| `replace += with *=` | `tally.deep_failed += 1`（配 `:210` 的退出码 2 保护） | 深轨 0 失败 ⇒ 计数恒 0，`0*1 == 0+1-1` |
| `delete !` | `three_way` 的 `filter(\|k\| !both.contains(k))` | `both` 与 `only_fast` 两桶恒空 |

方向要分清：**虚高的 caught 比漏掉的 missed 危险**。它让门看起来比实际强，而"门能不能拦住东西"
正是本仓建门的理由。第一轮那 3 个"捕获"因此是虚计——不是随机性好，是那几条分支根本没被看。

## 修法（沿用片A3/片A5 的口径：判据与被变异代码同包）

不靠"等快轨会扫 Rust"（快轨的 rust 污点实现是另一片的事，账在 `FP-LEDGER-m2s4.md:180-182`），
而是把分支抽成本包纯函数直接喂数据判：

- `fast_runs_for` / `deep_runs_for` / `deep_failure_exit` / `Tally::note_deep_failure` /
  `three_way_buckets`（+ `LedgerKey` 三元键别名）。
- 6 条单测进 `main.rs` 的 `#[cfg(test)] mod tests`：档位互斥且穷尽、深轨失败逐个计一次
  （`+= 1` 变异成 `*= 1` 会让计数恒 0 ⇒ 退出码 2 静默失效，这条正是钉它）、分桶按三元键、
  **同文件不同行不重合**、**列差不拆桶**（口径写在原注释里：快轨是 AST span、深轨是
  MIR terminator span）、不同规则不并桶、空输入三桶全空。
- 空语料那条用例是故意留下的反面记录：老验收就是那个形状，写出来免得后人以为那份语料验过桶逻辑。

## god 基线 374 → 388（逐键披露）

新增 14 面（全部在 `crates/adv-cli/src/main.rs`）、涨 1（该文件 282→453）、
降 1（`three_way` 27→14，打印与分桶拆开）、**移除 0**。

## 本片重放记录

`cargo test --workspace` 67 条全过 · clippy `--all-targets -D warnings` 干净 · fmt 0 ·
`xtask gate`（god+lockstep+suppress）绿 · `xtask mir` 绿。

`xtask mutants --base 6201ea4`（不带 `--update`，HEAD=`aa12154`，TMP 在 D 盘）：

| 项 | 上一片（e4c3a53 同口径） | 本片实测 |
|----|--------------------------|----------|
| 总变异 | 203 | **299** |
| 捕获 | 153 | **237** |
| 未捕获条目 | 20 | 30 |
| unviable | 30 | 31 |
| 门结论 | 绿（基线 9 键） | 绿（基线 14 键，无新增） |

**绿不等于杀掉**——那 3 个 adv-cli 键当时已在基线里，棘轮自然不报新增。所以另算一遍真产物：
本轮 missed 键 11 条，基线 14 条 ⇒

- 基线里有、本轮不再产存活变异（可划账）：`crates/adv-cli/src/main.rs::scan`、`::scan_deep`、
  `::three_way` —— 正是本片补断言的三个目标，**实测被杀**。
- 本轮有、基线里没有（新债）：**0**。
- 基线里本轮不可验证的：**0**。

⇒ 基线随后有意识重录 **14 → 11**（减 3、增 0；`--update` 的移除点名输出留档）。
留在账上的 11 键里，本片新增的是 `xtask/src/mutants.rs::free_bytes_via_df` 与
`::precheck_scratch` 两条薄壳（不可杀的理由写在片A5 节）。

## 片B1 的两条实测否证（同一条设计改了两处）

1. **`--as-rustc` 这种自定义 flag 走不通**：cargo 的 `RUSTC_WORKSPACE_WRAPPER` 只会递
   `<包装器> <rustc 路径> <rustc 参数…>`，没有插自定义参数的位置。实测第一次调用就是
   目标探测 `rustc.exe -vV` / `rustc.exe - --crate-name ___ --print=file-names`，
   自定义 flag 被当成输入文件 ⇒ `error: multiple input filenames provided`。
   ⇒ 触发条件改成环境变量 `ADV_MIR_WRAPPER=1`（`--as-rustc` 保留给手测与单测），
   且 `-vV` 这类探测没有 `--crate-name`，不能拿它当硬前置。
2. **sysroot 必须注入**：rustc 缺省按**自身 exe 位置**推 sysroot，而驱动 exe 在
   `target/debug`，那儿没有 `lib/rustlib/<host>/lib` ⇒ 找不到 `core`。cargo 不传
   `--sysroot`，而 `run_compiler` 会丢掉 args[0]（`rustc_driver_impl/src/lib.rs:183`），
   所以注入位置是 1（`forwarded_args`）。

---

# 片B1（c76b1f1，2026-10-06）：cargo 包装器直通档——深轨第一次扫真实 crate

> 交付：驱动加直通档（`ADV_MIR_WRAPPER=1` 触发，`--as-rustc` 留给测试），发现按 crate 落
> `<crate>.jsonl` 且**自带文件名**；工件照常产出（`Compilation::Continue`）。
> 复现：`RUSTC_WORKSPACE_WRAPPER="$PWD/target/debug/adv-ast-rust-driver.exe" ADV_MIR_WRAPPER=1
> ADV_MIR_RULES="$PWD/rules" ADV_MIR_OUT=D:/tmp/mir-out cargo build -p adv-cli`。
> 设计与两处被实测改掉的假设写在 `docs/PLAN-deep-track.md` §2。

## 深轨第一次碰真实代码就炸出的真缺陷

`optimized_mir` 对 const 上下文 panic：断言在 `rustc_mir_transform/src/lib.rs:790-797`，
按 `tcx.hir_body_const_context` 分派，`Some(非 ConstFn)` 直接炸。实测炸在 `adv-core` 的
`Const { allow_const_fn_promotion: true }`。改法照 rustc 自己的 `instance_mir`
（`rustc_middle/src/ty/mod.rs:1955-1977`）：新增 `mir_body()`，const 项/static/anon const 走
`mir_for_ctfe`，`const fn` 仍走优化版；三处取 body（`main.rs::call_sites`、
`taint_reach::build_plan`、`taint_reach::analyze`）统一走它。

## 一条 0 发现是**对的**，不是漏报

`adv-cli` 那轮 `adv.jsonl` 是 0 字节。核对后确认成因：`crates/adv-cli/src/mir.rs:59` 用的是
`std::env::var_os`，而规则源点写的是 `std::env::var`（`rules/rust/taint-command.yaml:11`）
⇒ 不是同一条 callee 路径，不该命中。**先量再判**，别把"符合规则的 0"记成"深轨漏报"。

## 一个观察面空洞（记账，归片B2 补）

`mir_body` 这轮只生成 1 条变异且判 Unviable，而它的 `mir_for_ctfe` 分支**没有任何测试覆盖**——
它是我在真实 crate 上撞出来的，新增的三条测试用的夹具（`taint_direct.rs`）没有 const 上下文。
这正是片A6 刚处理过的那一类："分支观察不到 ⇒ 门对它的态度是沉默"。
⇒ 片B2 要在带 const 上下文的真实 crate 上跑一遍直通档，把这条臂变成有实测证据的面
（顺带就是三方账的输入面）。

## 本片重放记录

`cargo fmt --all --check` 0 · `cargo clippy --workspace --all-targets -- -D warnings` 干净 ·
`cargo test --workspace` 70 条全过（含本片新增 3 条：工件+jsonl、缺环境变量判红、三个纯判据）·
`xtask gate` 绿 · `xtask mir` 绿 · god 基线 388 → 406（新增 18 面、涨 12、降 1、移除 0）。

`xtask mutants --base 6201ea4`（不带 `--update`，HEAD=`c76b1f1`，TMP 在 D 盘）：
总 **316** / 捕获 **253** / 未捕获条目 30 / unviable 32，**门绿**。
双向漂移核对：本轮 missed 键 11 条 == 基线 11 条 ⇒ 新债 0、可划账 0、不可验证 0。
本片新代码的 6 个键（`is_wrapper`、`forwarded_args`、`crate_output_name`、`mir_body` 之外的
`span_file`、`finding_json`、`emit_wrapper`）全部进可验证集且**无一留在 missed** ⇒ 都被杀掉。

---

# 片B2（2026-10-06）：深轨吃 crate 形状的输入——`adv scan --deep-via-cargo`

> 交付：`adv scan <crate目录> --engine both --deep-via-cargo` 把边车当
> `RUSTC_WORKSPACE_WRAPPER` 挂进目标 crate 的 cargo 构建，读回按 crate 落的 JSONL。
> 语料是**合成多文件 crate** 夹具 `crates/adv-cli/tests/data/taint-crate/`
> （用户 2026-10-06 选定；另两个选项是"引外部真 crate"和"只证跑通"）。
> 复现：`./target/debug/adv.exe scan crates/adv-cli/tests/data/taint-crate --rules rules
> --engine both --deep-via-cargo`。

## 实测结论

| 主张 | 判据 | 结果 |
|------|------|------|
| 单文件档的局限真的被绕开了 | 上面那条命令的 stdout | **3 条发现，跨两个文件**：`src/lib.rs:10`、`src/lib.rs:15`、`src/inner.rs:5`——非根模块的流扫到了，单文件档做不到（没有模块解析） |
| sanitizer 仍判得住 | 夹具 `// adv-expect: miss` 那条（`len` 之后） | 不在账里 |
| 快轨对 Rust 的能力边界 | `--engine ast` 扫同一份语料 | **0 条**（实测，不是推断）⇒ 三方账的非空桶只有"仅深轨"，`两边都报`/`仅快轨` 两桶靠片A6 的单测撑，不能声称端到端验过 |
| 本仓真实 Rust 代码没有该规则的真流 | 逐处 grep `env::var` 与 `Command::new` 同文件 | 只剩 `adv-cli/src/mir.rs`（用 `var_os`，不该命中）与 `xtask/src/mutants.rs`（值流向比较而非命令实参）⇒ 在本仓代码上出账必然全 0 |
| 片B1 记的覆盖空洞补上了 | 夹具放 `pub const OFFSET: usize = 40 + 2;` | const 上下文体走 `mir_body()` 的 `mir_for_ctfe` 分支，扫描不 panic ⇒ 那条臂从此在门内有覆盖 |
| 空账不能算通过 | `collect_crate_findings` | 一个 `.jsonl` 都没有 ⇒ Err（"不能折算为无发现"）；非 crate 目录（无 `Cargo.toml`）⇒ 退出码 2，有测试钉住 |

踩到的一条环境变量口径坑（如实记）：`ADV_MIR_RULES` **必须传绝对路径**。包装器进程的工作
目录是**被扫 crate 的根**（cargo 在那儿起 rustc），传 `rules` 会解析到不存在的目录，驱动报
`Error: 读规则目录 rules` ⇒ 整条 cargo 构建 exit=101。驱动侧 fail-closed 是对的，错在调用方。

## 本片重放记录

`cargo fmt --all --check` 0 · clippy `--all-targets -D warnings` 干净 ·
`cargo test --workspace` **73** 条全过（含本片新增 3 条：跨模块流 + 金样冻结 + 非 crate 目录判红）·
`xtask gate` 绿 · `xtask mir` 绿 · god 基线 406 → **423**（新增 17 面、涨 5、降 0、**移除 0**；
新面含夹具那两个 `.rs`——god 按仓内 .rs 计量，这是有意的）。
金样：`crates/adv-cli/tests/golden/deep-cargo.jsonl`（3 行，`ADV_UPDATE_GOLDEN=1` 通道重录）。
`xtask mutants --base 6201ea4`（HEAD=`1fc62d3`，不带 `--update`，TMP 在 D 盘）：
总 **331** / 捕获 **265** / 未捕获条目 30 / unviable 35 ⇒ **门绿**。
双向漂移核对：本轮 missed 键 11 == 基线 11 ⇒ 新债 **0**、可划账 **0**、不可验证 **0**；
本片新增的 `scan_crate_via_cargo`、`collect_crate_findings`、`parse_finding`（改动过的）
全部落在可验证集且不在 missed 里 ⇒ 都被杀掉。

---

# 片D（门链分层，2026-10-06）：试验面不计量 + 设计债到期门

> 起因是用户一条设计口径：**对正在被测试的东西别用维护承重面的尺**——测试面上有 bug、
> 可维护度差是预期属性；不能放过的是**设计层**问题，而且它必须**有到期日**（≤10 个任务），
> 否则"设计上的问题"就退化成一条没人再看的注释。定案三格全 A（commit 数计时 /
> 只松 god+变异门 / 先不动 branch protection），写在 `docs/DESIGN-DEBT-GATE.md`。

## 生效点与实测

| 件 | 位置 | 实测 |
|----|------|------|
| god 跳过试验面 | `xtask/src/maturity.rs` + `god::collect_workspace_entries` | god 基线 423 → **409**：移除 21 键**全部**落在 `tests/data/`、`tests/fixtures/` 之下，逐键核过"非试验面被误放掉 = 空" |
| `**` 的语义收窄 | `pattern_matches` | 定为**至少吃一层**：登记写的是"该目录以下"，放松不许比字面更宽（金丝雀 `canary_experimental_pattern_only_covers_what_it_says` 钉住 `*` 不跨段、`datamore/` 不命中） |
| 到期门 | `scripts/debt_gate.py`（D1–D7）+ `tests/test_debt_gate.py` 12 条 | 在册 7 条债全绿；每条判据都有反向用例（缺字段、成因空、限期>10、**SHA 不存在**、超期、id 重复、销账无据） |
| 接进门链 | `local_gate.py` fast 档 + `core.yml` 一步 + PR 模板 | `test_s145_gates.py` 10 条同锁绿（core.yml 出现的脚本必须都在 local_gate） |

## 三条比"实现完成"更该记的

1. **`--write-baseline` 是整表替换，会把垃圾面洗成基线**：python god 门重录实测把表从
   377 涨到 443，新增里含 `RESEARCH/.tmp-p1/` 这类遗留临时件。⇒ 本片不做整表重录，
   只单条更新 `scripts/local_gate.py` 的 `file_lines 230→233`（条目数保持 377），
   并把这件事登记成 **DD-0006**。绕过分没有判据值钱。
2. **伞仓快档门链默认就是红的**：30 步里 15 步失败。我先怀疑是自己引入，用"把本片新增
   两个 py 文件挪开再量"排除——同样报 6 条增长，且 naming/typos 命中里 grep 本片新文件
   为 0、`local_gate.py` 的 C901 在 `main()`（我没碰）。⇒ 归属清楚后登记成 **DD-0007**
   （缺的是"门链按工作线分组"这一层设计），而不是跳过或假装没看见。
3. **D4 那条判据是给我自己写的**：本会话我连着两次凭印象写错 commit 号
   （`f4a39d5`、`36179b6`，真实是 `1fc62d3`）。现在编造 SHA 会直接判红——
   门把作者的这个失败模式接管了。

## 本片重放记录

`debt_gate` / `claim_gate` / `god_gate`（python 侧）全 OK · `pytest tests/test_debt_gate.py
test_s145_gates.py` 22 条过 · fmt 0 · clippy 干净 · `cargo test --workspace` 74 条过 ·
`xtask gate` 绿 · `xtask mir` 绿 · `xtask mutants --base 6201ea4`：（待填）
