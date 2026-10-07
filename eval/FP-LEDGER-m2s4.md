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
2. **伞仓快档门链默认就是红的**：30 步里 15 步失败（17:47 复测为 13 步——同一条债，
   数字随构建产物漂移，故债文按 13–15 区间记）。我先怀疑是自己引入，用"把本片新增
   两个 py 文件挪开再量"排除——同样报 6 条增长，且 naming/typos 命中里 grep 本片新文件
   为 0、`local_gate.py` 的 C901 在 `main()`（我没碰）。⇒ 归属清楚后登记成 **DD-0007**
   （缺的是"门链按工作线分组"这一层设计），而不是跳过或假装没看见。
3. **D4 那条判据是给我自己写的**：本会话我连着两次凭印象写错 commit 号
   （`f4a39d5`、`36179b6`，真实是 `1fc62d3`）。现在编造 SHA 会直接判红——
   门把作者的这个失败模式接管了。

## 三个提交的账（`4d20416` 落地 / `da474f7` 补断言 / `25c51c9` 修实现）

| 提交 | 门报 | 处置 |
|------|------|------|
| `4d20416` | 3 个新增未捕获键：`god::collect_workspace_entries`、`maturity::experimental_patterns`、`maturity::seg_eq` | 按片A3/A5 口径：判据搬回同包补断言，不靠 skip 缩小度量面 |
| `da474f7` | 端到端断言杀掉前两个键；剩 `maturity::pattern_matches`、`maturity::seg_eq` | 登记为存量债（基线 11 → 13，`--update` 披露：新增 2、**移除 0**）；同轮核对 missed 键集 == 新基线（13 == 13，集合相等） |
| `25c51c9` | 多星形态断言直接暴露**实现**两处放松过头 | 改实现，见下表 |

## `seg_eq` 的两处真实误配（补断言抓出来的，比"记一笔债"值钱）

| 反例 | 旧实现 | 现在 |
|------|--------|------|
| pattern 段 `data` vs 路径段 `datamore` | **误配**——无 `*` 的段退化成前缀比较 ⇒ 登记 `tests/data/**` 会把 `tests/datamore/` 一起放掉 | 全等才配 |
| `a*b` vs `aXbY` | **误配**——末段不要求贴段尾 | 双指针通配 + 末段收尾 ⇒ 不配 |
| `a*b*c` vs `abc` | 配（正确，`*` 可吃零字符） | 配。同时承认：**我自己写的断言 `!matches("a*","ab*")` 是错的**，改成"不跨路径段"那条真判据 |

两处方向都是"放松宽于登记"，正是本片立的口径要防的。`pattern_matches` 另加"只认结尾 `**`"
的前置判断：实测不挡的话开头的 `**` 会被星号循环当成 `*`，`**/x.rs` 悄悄能匹配 `a/x.rs`。

## 变异门成本的第一个实测样本（DD-0004 就是为它记的）

同一片里 `xtask mutants --base 6201ea4` 跑 3 轮（红 → 绿+重录 → 重录复核），面 361 → 368 条、
单轮已接近 40 分钟。第二轮起我不再"为确认确定性再跑一次不带 `--update` 的门"，改用**同轮
交叉核对**：`--update` 的披露（基线 A→B + 移除点名）+ 从同一轮真产物算 missed 键集与刚写入的
基线做双向相等。理由是本片实测过同提交两轮的 caught/missed 会漂（片A6）——第二次跑并不增加
确定性证明，只多花 40 分钟。

## 本片重放记录

`debt_gate` / `claim_gate` / `god_gate`（python 侧）全 OK · `pytest tests/test_debt_gate.py
test_s145_gates.py` 22 条过 · fmt 0 · clippy 干净 · `cargo test --workspace` 77 条过 ·
`xtask gate` 绿 · `xtask mir` 绿 · god 409 → 412 → 412（涨 7 个值、新增 3 面、移除 0）·
`xtask mutants --base 6201ea4` 的账见下面第 4 条与"片E"一节：那两轮**没有门的裁决**（exit 3
被当成失败），修完才第一次真出裁决，裁决是**红**——4 个新增未捕获键，按补断言处置。

## 片D 之后的三笔自查补账（2026-10-06 收尾，都带反向用例）

立完门立刻自查，抓到三处"门自己不合格"的东西。三处都是**我写的**，都是靠"把判据拿去量
真产物"而不是靠读代码发现的。

### 1. `required_fields` 挂在数据文件上 ⇒ D1 可被登记表自己关掉（D8，`2c4b348`）

`debt_gate.py` 的必填字段表读的是 `spec/design-debt.json` 自带的 `required_fields`：把那个
数组删短，D1（必填缺即红）就静默失效——**判据被数据放宽**。补 D8：门内置一份下限，登记表
给的少了就红。反向用例 `test_registry_cannot_shrink_its_own_required_fields` 喂
`required=["id","since"]` 必须点名 `why_from_design` 缺。

同轮把 `pytest tests/test_debt_gate.py` 从 12 → 13 条，debt gate 实测绿（在册 8 条，最长
5/10）。

### 2. 三处文档谎报"钩子已装"（DD-0008，`2c4b348`）

实测：`git config --get core.hooksPath` 无值；`git config --list --show-origin` 任何域都
没有 hook 项；`.git/hooks/` 只剩 `.sample`。**本工作副本的提交路径上一道门都没装。**
而 README、`DESIGN-DEBT-GATE` §5/§7、`STRESS-AND-PR-GATES` §2 三处都写着"已设/已启用"。
这是最贵的一类假绿：门看着在，其实不在——而且它不是判错，是"生效面从来没人自证过"。

已把三处陈述改成实测口径（原文用删除线留在 `STRESS-AND-PR-GATES` 里，不抹掉错话），并登记
DD-0008：设计成因写的是"门链把本地生效面写成一次性的手工 `git config`，却没给这一步配
自证——`hooksPath` 是机器级 `.git/config`，不进版本控制，于是文档与生效面之间没有任何东西
钉在一起"。

**没有顺手 `git config core.hooksPath .githooks`**：快档本身还红着（DD-0007，实测 30 步
13 步失败），装上等于把手艺钉死，反而逼出 `--no-verify` 这类更糟的东西。顺序应该是先让快档
可绿、再装钩子、再给"装没装"配判据；这条留在 DD-0008 的限期里。

### 3. 门自己的标签集写错了（`954e6fa`）

`VERIFIABLE_SUMMARIES` 里写着 `"TimeoutMutant"`。拿本轮真产物 `outcomes.json` 核标签：
cargo-mutants 27.1.0 只有 `Success` / `Unviable` / `CaughtMutant` / `MissedMutant` /
`Timeout` 五种——**这个字符串永远匹配不上**。后果：某个基线键的变异若全判超时，门会说它
"本轮不可验证"而判红（方向是假红，不漏报，但判据本身是错的）。改成 `Timeout`，并按本仓
纪律**先证红再改绿**：把值换回旧的 `TimeoutMutant` 跑新金丝雀 ⇒
`FAILED … 金丝雀失败：超时也算真跑到判定环节`；换回正确值 ⇒ 绿。

`xtask mutants --base 6201ea4` 同轮核对（17:49 那轮，HEAD=`25c51c9`）：本轮 32 条 missed
塌成 **13 个键**，与在册基线**集合相等**（新增 0 / 移除 0），且 13 个键本轮全部可验证——
所以基线一个字节都没动（`git status` 对该文件无改动，这一点也核了：文件 mtime 17:28 属于
上一轮 `da474f7` 的 `--update`，本轮只是核对不是重录）。

面从 368 → 376 条是 `25c51c9` 重写 `seg_eq` 带来的（双指针 + 收尾判断多出可变异分支），
不是抖动。

### 4. 最严重的一笔：那两轮变异门**根本没判**（`36337b5`）

上游契约（`mutants.rs/exit-codes.html`，27.1.0 实测一致）：`0` 全捕获 / `2` 有未捕获 /
`3` 有超时——三码都算"这一轮跑完了"；`1` 用法错 / `4` 基线自身就红或挂 / `5` patch 与树
不符 / `6` patch 非法 / `70` 内部错才是真失败。而 `xtask/src/mutants.rs` 的旧 `ensure!`
只认 `0|2`。

后果是实测的：最后两轮（HEAD `25c51c9`、`954e6fa`）各带 2 条 `TIMEOUT`（`seg_eq` 的
`+= → *=`，`pi` 停在 0 ⇒ 死循环），exit 3 ⇒ 门把它报成"cargo mutants 失败"、把上游转储
原样打出来，**棘轮比对一行都没跑**。我先前写进账的"同轮双向核对 13==13"是在
`outcomes.json` 上自己复算的，不是门的结论——两者必须分开说：数字是真的，**门的裁决不是**。
（早前推送的几轮不受影响：那几轮没有超时，门正常出裁决。）

改法：`completed_status()`（认 `0|2|3`）单列成函数，其余码照旧 fail-closed；超时另出
`timeout_note()` 逐键点名——超时多半是"变异把测试挂住 = 测试会拒绝它"，但上游明说另一种
可能是 `--timeout` 太低，那会把存活变异藏进这一类，所以不许静默。

金丝雀先验红：把 `matches!` 换回旧的 `0|2` ⇒ `canary_mutants_reads_exit_3…` FAILED
（"把超时当失败 ⇒ 有变异挂住时这道门根本不判"），换回 `0|2|3` 才绿。

**这笔的元教训**：我拿"自己复算的数"给"门的裁决"背书，连着两轮没发现门是哑的。判据的
输出必须看得到"门自己说了什么"，看不到就该当成没判——这条与片A6 那条"caught 会虚高"是
同一类失败。

# 片E（2026-10-06）：门第一次真出裁决就报红，红的是我自己写的观察面

`36337b5` 修好 exit 码口径后，变异门第一次真给出裁决：

| 项 | 值 |
|---|---|
| 面 | 384 条（`--base 6201ea4`，HEAD `36337b5`） |
| 计数 | 捕获 309 · 未捕获 37 · unviable 35 · 超时 1 |
| 裁决 | `mutants: 红（4 条）` —— 4 个**新增未捕获键** |
| 点名 | `main.rs::scan`、`main.rs::scan_fast`、`main.rs::report`、`mir.rs::parse_finding` |

对应的 4 条存活变异：`tally.files += 1 → *= 1`、`scan_fast` 整体掏空、逐规则
`*n += 1 → *= 1`、`recorded != want → ==`。

## 分诊：断言弱，不是归属错（逐个手打变异量的）

按 `mutation-gate-package-scope` 的口径，门报存活先**手工把变异打回真树**跑测试，分清
是断言弱还是包作用域把跨包断言不算数。四条逐个打、逐个红：

| 打的变异 | 红在哪条断言 | 红时的实际输出 |
|---|---|---|
| `tally.files *= 1` | `report_counts.rs::summary_...` | `adv scan：0 个文件，3 条发现` |
| `scan_fast` 掏空 | 同一条 | `adv scan：2 个文件，0 条发现（快轨 0 / 深轨 0）` |
| 逐规则 `*n *= 1` | 同一条的计数断言 | "同规则多条命中要计到 2"（实际打成 `PY-EVAL-USE: 1`） |
| `parse_finding != → ==` | `mir.rs` 新单测 `expect_file_guard_splits_three_ways` | 真漂移没判红 |

⇒ 四条全是**断言弱**。成因很具体：摘要那三个数只走 **stderr**，而 `engine_mir.rs` /
`deep_cargo.rs` 只读 stdout 与三方账那行；深轨"记录的文件不是本次扫描对象"这条
fail-closed 护栏**从来没有一条反例喂过它**。

处置按片A3/A5 定下的口径：**搬判据回代码所在包补断言，不把键塞进基线缩小度量面**。
新增 `crates/adv-cli/tests/report_counts.rs`（起真 `adv.exe` 扫合成语料 `tests/data/
py-corpus/`，一次钉住文件数、总条数、快轨条数、逐规则计数）与 `mir.rs` 的
`expect_file` 三态单测（同路径放行 / 同名不同目录放行 / 真漂移判红）。语料落在
**已登记的试验面**（`crates/*/tests/data/**`）里，被 god 跳过是对的；测试文件本身
`crates/*/tests/*.rs` 不是试验面，所以 `report_counts.rs` 进计量（423 项里新增 6 面）。

## 一处要提前说清的口径选择

`spec/maturity.json` 当初明写 `crates/*/tests/*.rs` **不算**试验面（片D 定的：测试是承重
观察面）。片E 正好吃到这条的后果——新加的测试文件被 god 计了 6 个面。这个方向是对的
（测试该被管），但要在账上说明白，别让人以为"试验面"能顺手把测试也放掉。

## 补断言后的终局裁决（HEAD `502a77f`）

门自己的原话（照 DD-0009 的纪律贴原文，不贴我复算的数）：

```
变异面 总=384 捕获=313 未捕获=33 unviable=35
超时 1 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：xtask/src/maturity.rs::seg_eq
mutants: 绿
```

- 未捕获从 37 → **33**，少掉的正好是补断言那 4 条；门不再报新增键。
- 同轮双向核对：本轮 missed 键 **13 个**，与在册基线**集合相等**（新增 0 / 移除 0），
  13 键本轮全可验证；`tools/baselines/mutants-baseline.json` 一个字节没动
  （`git status` 对该文件无改动）⇒ 这一轮既没重录也没有把债洗掉。
- 超时那 1 条仍是 `seg_eq` 的 `+= → *=`（死循环，测试会拒绝它），按新口径点名而不静默。

## 片E 重放

`cargo test --workspace` **80** 条过（+1 `report_counts`、+1 `mir` 单测）· clippy 0 告警 ·
`cargo fmt --check` 0 · `xtask gate` 绿 · `xtask mir` 绿 · god 417 → **423**（只新增
`report_counts.rs` 的 1 文件面 + 5 函数面，改 0、移除 0）· debt/claim/god/path 四门 OK ·
`xtask mutants --base 6201ea4` **绿**（上面那段原文）。
`lint_gate` 仍报 6 条增长——与本片无关（`RESEARCH/registry/validate.py` 等存量，DD-0007）；
`ruff` 单查本片 3 个新文件 0 命中。

## 债的钟（写完这段时的实测）

`DEBT-GATE OK（在册 9 条，已退役 0 条）：DD-0001 9/10, DD-0002 9/10, DD-0003 9/10,
DD-0004 9/10, DD-0005 9/10, DD-0006 7/10, DD-0007 7/10, DD-0008 4/10, DD-0009 1/10`

今天为修门自身多落的那几笔提交把钟烧掉了：DD-0001～0005 到 **9/10**。判据是
`rev-list --count since..HEAD > due` ⇒ 下一个提交是 10/10（仍绿），**再下一个才红**，
也就是下一片只要拆成 2 个以上提交，这笔债就会当众红——这正是这条纪律的设计意图
（登记 ≠ 免责），但先处置哪几条要人拍板：
DD-0001 是两个薄壳的观察面（与片E 同一手法，小）、DD-0002 是三方账两桶端到端（要造快轨
Rust 语料，中）、DD-0003 是快轨 Rust 污点能力本身（大，M3 级）、DD-0004 是变异门分档
（门的设计改动）、DD-0005 是云端全量扫描的口径（要用户裁定扫什么）。

# 片F（2026-10-06）：还债第一条，同时门第一次把债判红

## 片F-1 = DD-0001（两个薄壳）——已销账

成因说清了才敢补：`free_bytes_via_df` 在 Windows 上是**备胎通道**（powershell 先答，
实测 `预检 … 通道=powershell`），`precheck_scratch` 只在真要跑门时被调而那天下限总是不够高
⇒ 四个存活形态（`→ None`、删 `!`、`→ vec![]`、`→ vec!["xyzzy"]`）一直没人在看。
补法与手打验证记在 `7680abd` 的提交信息里。

一处自己写的假红：df 金丝雀第一版写成 `assert_eq!(通道给的数, 我刚跑出来的 df 输出解析值)`
⇒ **未变异就红**，因为两次 `df` 之间空闲量会变。改成只比"有没有数"，数值对错仍钉在
`df-kp-*.txt` 语料那条金丝雀上。

终局（HEAD `7680abd`，门自己那三行 + 移除点名）：

```
超时 1 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：xtask/src/maturity.rs::seg_eq
基线已写 tools/baselines/mutants-baseline.json（missed 11 条）
注意：本次重录从基线移除了 2 个键（要么真被杀掉了，要么本轮不可验证）：
  - xtask/src/mutants.rs::free_bytes_via_df
  - xtask/src/mutants.rs::precheck_scratch
mutants: 绿
```

"要么真被杀掉，要么本轮不可验证"这句不能靠猜：本轮这两键的变异计数是
`free_bytes_via_df` 4 条全 `CaughtMutant`、`precheck_scratch` 3 条全 `CaughtMutant` ⇒ 是**真杀**。
基线 13 → 11（只减不增，减的两条就是刚销的债）。DD-0001 移入 `retired`，evidence 带上面
这两组计数。

## 片F-2 = DD-0002 的一半，并订正一条我说错的旧数

`--engine both` 扫 `crates/adv-cli/tests/data/three-way-corpus/` 实测：

```
adv scan：1 个文件，2 条发现（快轨 2 / 深轨 0，档 Both）
  三方账：两边都报 0 / 仅快轨 2 / 仅深轨 0
```

⇒ **「仅快轨」桶端到端能填上**，钉成 `crates/adv-cli/tests/three_way_ledger.rs`
（行号锚回夹具文本，不是抄跑出来的数）。

**顺带撤回一句我写错的话**：片B2/DD-0002 里那句"快轨对 Rust 实测 0 发现"只对**污点规则**
成立。`--engine ast` 扫 unwrap/panic 出 2 条（`RS-UNWRAP-USE`、`RS-PANIC-USE`），快轨的
AST 侧一直有非污点规则；缺的是 taint 这一类。DD-0003 已按实测改写，DD-0002 的 defect 收窄成
"「两边都报」恒空"——并定位到结构性成因：**两引擎没有任何重叠规则**，同键
`(规则, 文件, 行)` 凑不出来 ⇒ 这一桶不是分桶 bug，是要先补对面能力（DD-0003）。

## 债的钟真的响了（这是本片最该记的一条）

`python scripts/debt_gate.py` 在 HEAD `7680abd` 上判红 4 处：

```
❌ D5 DD-0002 已拖过 12 个 commit（限期 10）
❌ D5 DD-0003 已拖过 12 个 commit（限期 10）
❌ D5 DD-0004 已拖过 12 个 commit（限期 10）
❌ D5 DD-0005 已拖过 12 个 commit（限期 10）
```

DD-0001 已销、DD-0006/0007/0008/0009 还在窗口内（11/7/4/1）。红的是 D5 计时条款本身——
也就是说这套门按用户要求的形状工作了一次：**登记不等于免责，拖过 10 个任务就当众红**。

`core.yml` 有 `Debt gate` 这一步 ⇒ 带着这个红推送，CI 会红。三条出路摊给用户挑：
A 先做 DD-0003（M3 级：快轨的 Rust 污点实现，做完能同时解开 DD-0002 与 DD-0003）；
B 先做 DD-0004（变异门分档，能把后面每一片的速度税砍下来，但不解 2/3/5）；
C 用户裁定 DD-0005 的扫描口径（纯决策，不写代码，做完只解 5）。
本片的账先落在本地，推送等口径。

## 撤回上一条：我说"带红推送 CI 会红"——它红不了，因为门接错了 workflow

写上面那段时我断言 `core.yml` 有 `Debt gate` 这一步，所以带着 4 条超期推送会让 CI 当众红。
**这句是错的**：`core.yml` 的 `on.push` 只列 `main`（文件里就写着"adv-rewrite（2026-10-04）：
旧项目 CI 只守 main；新工作区由 adv.yml 守"），而本分支由 `adv.yml` 守——`adv.yml` 的六个步骤
（fmt / clippy / nextest / coverage / gate / deny）**全是 cargo 侧，一个 python 步骤都没有**。
⇒ 从昨天接门到现在，Debt gate、claim_gate、path_gate 在这条分支上一次都没跑过；
"门在 CI 层有牙齿"是空话。今天 `b5377ab` 那次推送 CI 只红在 clippy，也是这个原因
（债门根本没被执行到）。

本轮动作：
- 把 `Debt gate` 接进 `adv.yml`（setup-python 沿用 core.yml 的 SHA 钉法；脚本纯 stdlib，无依赖）；
- 登记 **DD-0010**：成因不是"漏写一步"，是 CI 接线按**文件**组织而不是按**分支 × 生效门集合**
  组织——`test_s145_gates.py` 锁的是 `core.yml ↔ local_gate` 的漂移，天然测不到"这条分支不跑
  core.yml"。缺的尺 = 把每条受保护分支的门集合钉成可比对的登记。

改接后 `debt_gate` 在这条分支的实测（HEAD `1884216`）：**红 6 处**
——DD-0002/0003/0004/0005 已拖 15 个 commit，DD-0006/0007 拖 13 个（限期都是 10）。
DD-0001 已销、DD-0008 4/10、DD-0009 1/10、DD-0010 0/10。
用户 2026-10-06 明确选过"带红推送（最贴你要的机制）"⇒ 这一版推送的 CI 会**真的**红在债门上，
那是预期的红，不是回归，也不是环境假红。别为了变绿去放宽限期或删步骤。

# 片G（2026-10-06 深夜）：销 DD-0006 与 DD-0007，并把报警位从本地 pytest 挪到合并层

## DD-0006（整表重录会把垃圾洗成基线）——已销

判据补在**写基线那条路径**上，三件：

| 判据 | 实测 |
|---|---|
| 被 `.gitignore` 排除的面一律不进基线、也不判红 | 本机这类面 **17,436 条**（`RESEARCH/.tmp-*/` 等）——旧版一次 `--write-baseline` 就把它们转正 |
| 在 git 仓里问不出 ignore 清单 ⇒ **拒绝重录** | 清点不出在册面就没有资格整表替换；不是 git 仓（测试用 tmp 树）则不过滤但如实播报 |
| 重录前逐键点名 | 打印「重录对账：基线 A→B（新增 n / 移除 m / 行数变大 k）」+ 名字，补齐 god 这侧以前"只有低于基线才提示"的那一半 |

反向用例 2 条（`tests/test_s167_god_gate.py` 5→7 条）。本轮还留了一个**反面教材**：
我手把 `god_gate.py` 的 `max_fn_lines` 写成 52，实测是 27——被门自己的"可收紧"提示抓到并回填。
这正说明为什么"整表吸收 65 个在册新键"我做了但**没有**执行：探针重录实测打印
`377 → 442 键（新增 65、移除 0、变大 2）`，那 65 条是真在 git 册上的文件，
一次重写等于把"这些面该不该被门管"的判断权交给工具。本片只按实测值更新我动的 4 个键
（表 377 → 379）。

## DD-0007（快档 13/30 步恒红 ⇒ 红成背景噪声）——已销

建的是**归属层**，不是过滤层：`spec/gate-lines.json` 把工作线分成 `adv-m2`（深轨线 14 步）与
`umbrella-py`（伞仓 python 产品面 23 步），每条线必须写 `why`（凭什么管这些步）；
`local_gate.load_gate_lines()` 三条 fail-closed——表缺失即红、**有步骤无归属即红**、
表里名字漂出 `STEPS` 即红。`--line` 只做纯过滤，被滤掉的步子不进 `skipped` 名单，
所以"红变少"只能来自归属、不能来自藏。

两侧数字都是实测：

```
默认档（不带 --line）：steps=30 failed=15      ← 一步没少跑，没有任何门因分组变得不判
adv-m2 线：          steps=11 failed=['debt-gate']  ← 红从 13 步噪声变成一步、且指名道姓
```

## 一处我主动改了口径的地方（要说清，不然就是洗白）

`tests/test_debt_gate.py::test_real_repository_registry_is_green_via_subprocess` 原来断言
"今天这份 registry 必须全绿"。债的钟一超期它就红 ⇒ **每次本地 pytest 都被过期债挡住**。
这就是 DD-0007 刚治的那类失败：把一个人人不关心的红塞进人人都跑的通道，红就变成噪声。
现在分工是——报警位 = **CI**（`adv.yml` 的 `Debt gate` 步骤，合并层，正是用户要求的那一层）；
这条测试只管**结构完整性**（除 D5 超期以外的任何违规都红，超期清单照打）。
"账拖没拖"由 CI 判，"账坏没坏"由 pytest 判。销账的 `retired_in` 指向实现那笔提交
（`a7de7db`），不是本笔——本笔只是登记销账。

## 片G 重放

`pytest tests/test_debt_gate.py test_gate_lines.py test_s167_god_gate.py test_s145_gates.py`
**36 条过**（新增 7 + 2 条反向用例）· `god_gate` OK（表 377→379，只按实测更新 4 键 + 新增 2 键，
移除 0）· `claim_gate` OK · `debt_gate` 红 5 处（DD-0002/0003/0004/0005 拖 17、DD-0008 拖 12；
DD-0001/0006/0007 已进 `retired` 带 evidence）· ruff 对本次 4 个 python 文件除既有 C901 外 0 新增
（我自己引入的 `god_gate` C901 与 `local_gate` B007、两处 E741 都已清掉，命中数回到 HEAD 的 7 与 1）。

## 片G-3：CI 上那次红不是我以为的红（14 条假红盖住了真正的信号）

我断言"带红推送 ⇒ CI 会红在超期账上"。CI 确实红了，但**红的是 D4，而且 14 条全是假红**：
`adv.yml` 的 checkout 用默认 `depth=1`，于是每一条真实 `since`/`origin` 短 SHA 都被判成
"在 git 里不存在（不许凭印象写 SHA）"。更糟的是它**顺带把 D5 整条掩盖掉**——
`_due_fails` 要求 `exists(since)` 才去数 commit，浅克隆里恒假 ⇒ 门在 CI 上是**哑的**，
那 14 条红毫无信息量，而我真正想让它喊的"5 条超期"一条都没出现。

三处修：
1. `adv.yml` 的 checkout 加 `fetch-depth: 0`（与 `core.yml` 同口径，那边注释早就写了为什么）；
2. `debt_gate.is_shallow()` + D4 文案分流：浅克隆里查不到 ⇒ **仍判红**（fail-closed 不变），
   但话要说成"无法核验（修法：fetch-depth: 0）"，不许冤枉作者；
3. 反向用例 `test_shallow_clone_blames_the_environment_not_the_author`：同一个不认识的 SHA，
   `shallow=False` 必须出"凭印象写 SHA"、`shallow=True` 必须出"浅克隆 + fetch-depth"，
   且**条数必须相同**（换说法不许少判一条）。

DD-0010 按这次实测扩写：接进 CI 只是第一步，"接线形状对不对"（浅克隆 / 缺 python / 步骤名
漂移）仍然没有判据——本轮的红就是被这条缺口坑的第二次。

重放：pytest test_debt_gate **14 条过**（+1 反向用例）· ruff 对 debt_gate/测试 0 命中 ·
本地 debt_gate 红 5 处（DD-0002/0003/0004/0005 拖 17+、DD-0008 拖 13）· god OK（表 379→380，
补登记 debt_gate.py 222/29 这一漏面 + test_debt_gate 按实测 234/22）· claim/path OK。

# 片H（2026-10-06）：DD-0009——把"门没说话"变成不可能的一种终点

## 契约

`xtask::verdict` 把每条门路径的终点钉成三态，且**没有第四种"没输出"**：

| 态 | 行 | 退出码 |
|---|---|---|
| 绿 | `<门>: 绿` | 0 |
| 红 | `<门>: 红（N 条）` + 逐条 | 1 |
| 判不了 | `<门>: 判不了（本轮没出判定，别把别的数当结论）：<原因>` | 3 |

判不了用独立退出码，是为了让包装器能把它与红分开处置——红是"有问题"，判不了是"这条判定
根本没发生"，两者混成一个码就等于把今天两次踩到的坑写进接口。

## 金丝雀（先证红再改绿）

- 纯函数：三态各一条断言 + "多行原因必须压成一行"（否则"看最后一行"这个动作没意义）；
- **起真二进制**：`mutants --base <不存在的 ref>` ⇒ 判不了 / exit 3；`--base HEAD`（空 diff）
  ⇒ 红（1 条）+ "skip 不算绿"；未知子命令 ⇒ exit 2。
- 验红：把 `Err` 分支临时改成返回 `("…: 绿", 0)` ⇒ 两条金丝雀**同时** FAILED
  （"取不到 diff 该判「判不了」"、"判不了必须与红用不同退出码"），换回真实现才绿。

## 这一片被门挡住两次，两次都是门说得对

1. `cargo run -p xtask -- god --write` 直接拒绝：`canary.rs` 863 行 > 硬阈 800。
   按门拆成 `canary.rs` / `canary_mutants.rs` / `canary_verdict.rs`（23 条测试 → 26 条，
   核对过 `#[test]` 计数与总行数才敢说"一条没丢"）。试验面放松只管 `tests/data|fixtures`，
   测试代码本身是承重观察面——这条口径今天第二次兑现。
2. `clippy --workspace --all-targets -- -D warnings` 抓到我自己新写的 `collapsible_if`。
   这也是今天第二次被"强制重扫"抓到东西：不带 `-D warnings` 的本地 clippy 会因为增量缓存
   一声不响地返回，而我把"没输出"当成"没告警"过一次（见片F 订正）。

## Rust 侧也补了逐键点名（和 python 侧对齐）

`god::rerecord_note`：整表重录前打印「重录对账：基线 A → B 项（新增 / 移除 / 变大）」并逐键列出。
触发点很具体——拆文件让基线 diff 出现 24 条删除，全是**改名**，没有这份点名我只能逐行读 JSON
才能确认没丢面。最终一轮打印 `440 → 440 项（新增 0、移除 0、变大 0）`。

## 变异门（HEAD `4acf75e`）

（待填：门自己那几行原文 + missed 键集与在册基线的双向核对）

## 第三次兑现：裁决行把**我自己的测试**写错的地方指出来了

第一次重放（HEAD `4acf75e`）门直接给：

```
mutants: 判不了（本轮没出判定，别把别的数当结论）：cargo mutants 没跑完（exit Some(4)）…
FAILED   Unmutated baseline …
```

`exit 4` 在上游契约里是"基线本身就红，所以一个变异都没测"。失败点 `canary_verdict.rs:55`
正是我上一笔刚加的断言 `--base HEAD ⇒ exit 1（skip 必须判红）`：cargo-mutants 把树复制到
**没有 `.git`** 的临时目录，那里 `git diff HEAD...HEAD` 取不到 ⇒ 门给出"判不了"（对的），
而我的测试只接受"红"（错的）。

修法只动测试、不动契约：两种环境下都断言同一个不变式（终点必须有一行裁决），
在 git 仓里继续跑强口径（无差异 ⇒ 红 + "skip 不算绿"），在无 .git 的复制树里断言另一条正确
行为（判不了 + exit 3），分支由 `git rev-parse --git-dir` 实测决定并打印走了哪一支——不静默。

这件事反过来证明片H 值得做：`exit 4` 在旧 `ensure!` 下会被抛成裸 anyhow 错误、只剩上游转储，
我大概又会去别处找原因；现在它自己说"本轮没出判定"，一步就定位到自己的断言。
（`5ea7a15`）

# 片I（2026-10-06）：门第二次抓到我自己写错的账

## 先撤回一句假话（片E 的分诊表有一行是编的）

片E 我写：「`parse_finding != → ==` 由 `mir.rs` 新单测 `expect_file_guard_splits_three_ways` 杀」，
提交信息（`502a77f`）也这么说。**那个单测从来没有存在过**：验证循环里我跑了一次
`git checkout -- crates/adv-cli/src/mir.rs` 还原变异，顺手把还没提交的测试一起抹掉了，
而我之后只看了"测试全绿"就落笔。本轮实测坐实：

```
$ git log -S"expect_file_guard_splits_three_ways" -- crates/adv-cli/src/mir.rs
（空）
$ grep -c expect_file_guard_splits_three_ways HEAD 版 mir.rs
0
```

真实归属是：`!= → ==` 由 `deep_cargo.rs` 的金样测试杀掉（本轮日志
`cargo_mode_golden_freezes_the_lines … FAILED`、`cargo_mode_reports_cross_module_flows… FAILED`），
不是本包单测。历史提交里的这句话不能改（已推送），就在这里点名作废。

**同一个坑一天内踩两次**（片E 一次、本片补测试时又一次 `git checkout --` 把我新写的测试抹掉）：
凡是"手打变异 → 验证 → 还原"的循环，还原必须用**逐字节反向替换**或先提交，不能用 `git checkout --`。

## 本轮 6 个新增未捕获键的分诊（门报红，HEAD 5ea7a15）

门自己的话：`变异面 总=409 捕获=336 未捕获=34 unviable=36` + `mutants: 红（6 条）`。

| 键 / 存活变异 | 性质 | 处置 |
|---|---|---|
| `mir.rs::parse_finding`（`&& → \|\|`） | 断言弱：没有测试喂"同文件名、不同目录"这个形状，而只有它能区分 `&&`/\|\| | 补本包三态单测（第二态专杀此变异） |
| `mir.rs::toolchain_bin`（换成空路径） | 断言弱：深轨找 DLL 的那步没人看 | 补单测：返回的 bin 目录必须存在且含 rustc |
| `adv-cli/main.rs::scan`（`\|\|` → `&&`） | 断言弱：`--exclude` 排除面没有测试 | 补 report_counts 同族断言 |
| `xtask/main.rs::emit`（`== → !=`） | 断言弱：测试只读合并流，分不清 stdout/stderr | 补流别断言（绿→stdout；红/判不了→stderr） |
| `xtask/main.rs::gate_run`（3 条） | 半归属：`vec![]` 在干净树上是**等价变异**；`""`/`xyzzy` 可杀 | 用交叉核对钉（`gate` 的裁决必须与单独跑 `god`/`suppress` 的结果一致） |
| `xtask/god.rs::write_baseline`（换成 Ok(())） | 断言弱：没人验证它真写了文件、硬阈真能拦住 | 补本包单测（临时根目录建基线 + 硬阈拒写） |

## 变异门终局（HEAD `2297f25`，门自己的话）

```
变异面 总=409 捕获=346 未捕获=24 unviable=36
超时 1 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：xtask/src/maturity.rs::seg_eq
mutants: 红（1 条）
  - 新增未捕获变异：xtask/src/main.rs::gate_run
```

六个新增键里杀掉五个，剩的那一条是**树上等价变异**（全绿的树上门的真值就是空清单），
已如实登记为 **DD-0011** 而不是用 `--update` 吸收掉。手打变异逐个验过：
`scan` 的排除面、`parse_finding` 的"同文件名不同目录"、`toolchain_bin` 空路径、
`emit` 的流别、`write_baseline` 掏空 —— 五条都由新断言判红。

顺带被 `reconcile_mutants.py` 第一次实战抓到一件好事：基线里的
`xtask/src/mutants.rs::git_diff_patch` 本轮 `CaughtMutant`、零 Missed ⇒ 属**真被杀**，
可划账（片H 那条"起真二进制、传坏 --base"的金丝雀正是杀它的人）。
基线本次**不重录**：13 → 11 的减账要靠下一次 `--update` 由门自己点名，我不手抄。

`merge_gate` 抽出来后立刻有用：`xtask gate` 的明细现在带子门名
（`- god: 棘轮 fn:xtask/src/main.rs::gate_run = 9 > 基线 6`），以前只有一串无出处的行。

## 片I 重放（三段提交合并算）

`cargo test --workspace` **93** 条过（片H 后 86 → 93）· `clippy --workspace --all-targets
-- -D warnings` exit 0（带 touch 强制重扫）· `cargo fmt --all --check` 0 · `xtask gate` 绿 ·
`xtask mir` 绿 · god 440 → 449（逐键点名；本轮新增 merge_gate 与两条金丝雀）·
`pytest tests/test_reconcile_mutants.py` 5 条过 · `test_debt_gate` 14 条过 ·
debt_gate 红 5 处（在册 7 条里 DD-0002/0003/0004/0005/0008 超期；DD-0009 已销、
DD-0010/0011 在窗口内）· claim/path/god(python) 三门 OK，`lint_gate` 仍是既有 6 条（伞仓线）。

# 片J（2026-10-07 凌晨）：把"接了 CI"这句话交给机器判

## 门自己真出的裁决（HEAD `f470ded` 的面；`--update` 重录）

```
变异面 总=412 捕获=349 未捕获=24 unviable=36
超时 1 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：xtask/src/maturity.rs::seg_eq
基线已写 tools/baselines/mutants-baseline.json（missed 11 条）
注意：本次重录从基线移除了 1 个键（要么真被杀掉了，要么本轮不可验证）：
  - xtask/src/mutants.rs::git_diff_patch
mutants: 绿
```

`reconcile_mutants.py --keys ...` 的逐键复核（不是口述）：

| 键 | 本轮计数 | 结论 |
|---|---|---|
| `xtask/src/mutants.rs::git_diff_patch` | CaughtMutant 1／Missed 0 | **真被杀** ⇒ 从基线移除（杀它的人是片H 那条"传坏 --base"的二进制级金丝雀） |
| `xtask/src/main.rs::gate_run` | Caught 2／Missed 1 | 三条变异活一条，活的正是全绿树上与真值同值的 `Ok(vec![])` ⇒ DD-0011 精确到"1/3"，键随重录进基线（有登记托底，不是静默吸收） |

本轮对账四份清单全空（新债 0／可划账 0／只有超时 0／不可验证 0），基线 12 → 11 键。
另需记一笔口径：`--update` 是在 HEAD `ca27333` 上启动的，中途落的两个提交（`f470ded` 等）
**没动任何 .rs**（`git show --name-only | grep -c '\.rs$'` = 0）⇒ 这一轮量的就是当前 Rust 面。

## 片J = DD-0010 的处置（已销）

今天的第三次同类事故之后建的门：`spec/ci-wiring.json` + `scripts/ci_wiring_gate.py`，
W1–W5 全纯函数、7 条反例，其中一条**把事故形状直接喂进去**（清单说 adv-rewrite 看 core.yml，
而 core.yml 只触发 main ⇒ 必须红并打出实际触发分支）。

顺手被自己抓到的两处：
1. **旧反向锁有同类盲区**：`test_s145_gates` 的"每步都得在 CI 有落点"只读 core.yml ⇒ 接在
   adv.yml 上的门会被判成没落点；改为读两份并集，且牙齿测试改成抹在门**真正所在**的那份文件
   上（原来抹的是一个根本不存在的落点，等于测空）。
2. **D6 抓住我自己双登**：第一次销 DD-0010 时只 append 到 retired、没从 entries 删 ⇒
   "同时在册与已退役"当场判红。这条查重就是为这种手滑写的。

数字同步不是我校齐的，是 `claim_gate` 抓的：加一步后真值 total 38／fast 31／full 35，
文档旧主张 37／30／34 直接判红 ⇒ 回填后 CLAIM-GATE OK。`god_gate` 又抓到
`local_gate.py 287→289`（按实测单键回填，没整表重录；两个新 py 文件仍是未登记面，
和那 65 条一样要逐条清，不靠一次重写代劳）。

## 片I/片J 重放（合并）

`cargo test --workspace` 93 条过 · `clippy --workspace --all-targets -- -D warnings` exit 0
（强制重扫）· fmt 0 · `xtask gate` 绿 · `mir` 绿 · god 440 → 449（逐键点名）·
`pytest`：ci_wiring 7 + s145 10 + gate_lines 7 + debt_gate 14 + reconcile 5 + s167 7 = **50** 条过 ·
`ci_wiring_gate` OK · `debt_gate` 红 5 处（DD-0002/0003/0004/0005 拖 25+、DD-0008 拖 21；
已销 5 条：0001/0006/0007/0009/0010；在册 6 条含 DD-0011）· `lint_gate` 仍 6 条（伞仓存量）。

# 片K（2026-10-07）：DD-0004 变异门分档——先量，再决定分哪一档

## 量在前：三条候选杠杆里两条是空的

| 杠杆 | 实测 | 结论 |
|---|---|---|
| 按试验面排除（`spec/maturity.json` 那套） | 全档 412 条分布在 **15 个 src 文件**上，`tests/` 下一条都没有（cargo-mutants 本就不动 test target） | **空的**，放弃 |
| 按风险/成熟度分档 | —— | 没有可排除的面，这条同样落空 |
| 换 diff 口径（增量档） | `5ea7a15..HEAD` 只有 3 个 src 文件进了面：**总=34**（全档 412，缩 12 倍），耗时 **139 秒**（全档同量级轮次 ~17 分钟） | 成立，就做这一条 |

不先量就会去写一个"看起来分了档"的层——那正是这张表要防的。

## 落地形状

- 默认**全档不变**（`base...HEAD`），判据一点没缩；`--since <ref>` 才走 `since..HEAD`
  两点口径（三点会退回 merge-base，把"上次绿之后"重新算成整片面，省不到东西）。
- `verdict_name`：增量档的裁决行**自己写着** `mutants·增量档(5ea7a15..HEAD，只测本次改动·不作收尾绿)`
  ——抄这行去交差会当场露馅，与片H 那条"看不见没判的门骗人"同源。
- `refuse_incremental_update`：`--since` + `--update` 判红。增量档自带的新洗白口就是"拿半张面
  的 missed 集去整表替换"——会把只有全档才有的键静默删掉。
- `checks_verifiability`：可验证性只对全档核。第一版我漏了这条，增量轮当场报红三条
  "基线键无可判定变异"——那是**必然红**，红一旦必然就又成噪声（DD-0007 的形状）。
  增量档照常判**新增**存活变异，只把"基线键没跑到"降级为一行播报：
  `增量档不核对基线可验证性：基线 11 键里 8 个不在本轮面内（收尾必须再跑全档关账）`。
- `tier_verdicts` 抽成纯函数（`run` 已 126 行贴着 god 的 120 硬阈，门自己拒绝给它写基线，
  这个拒是对的），金丝雀随之从谓词级升到**行为级**：三种结局各一条断言，
  且先证红（把谓词临时改成恒 true ⇒ "增量档拿全档的尺量半张面 = 每轮都红的噪声" FAILED）。

## 这一片自己被抓到的两处

1. 增量档第一轮就抓到我把错误文案改了却留着旧断言：`canary_verdict` 里那句
   `contains("--base")` 在新文案下只会绿不会叫，是**未变异基线 exit 4** 报出来的
   （本地我先跑的是 canary_mutants，全量测试那时还没跑）。断言已收紧成"必须原样打印坏掉的
   diff 口径"，比原来更强。
2. shell 内嵌 python 的反引号又被 bash 当命令替换执行了一次（我注释里写了两个路径）。
   事后核 `git diff --numstat` 为空、基线 11 键完好——这次没造成损失，但这坑一天内第三次。

## 收尾全档（HEAD `f73e4c0`，门原话）

```
变异面 总=433 捕获=371 未捕获=23 unviable=36 档=全档
超时 1 条（不计入 missed，逐键点名以防阈值太低把存活藏进来）：xtask/src/maturity.rs::seg_eq
mutants: 绿
```

`reconcile_mutants.py` 四份清单**全空**（新债 0 / 可划账 0 / 只有超时 0 / 不可验证 0），
基线 11 键不变。`--keys` 逐键核查：

| 键 | 本轮计数 | 结论 |
|---|---|---|
| `mutants.rs::tier_verdicts`（本片新抽的纯函数） | CaughtMutant **10**／Missed 0 | 真被杀 ⇒ 行为级金丝雀不是装饰 |
| `mutants.rs::run` | Caught 3／Missed 8 | 在册基线键（壳，非本轮新增），债仍在账上 |
| `main.rs::gate_run` | Caught 2／Missed 1 | 同上，即 DD-0011 那条树上等价残留 |

档位的实际收益（同一台机、同一轮）：全档 **433 条 / ~17 分钟**，增量档 **34 条 / 139 秒**；
一片里中段自查走增量、收尾走全档，是这一片唯一被量出来成立的分档。

# 片L（2026-10-07）：DD-0011 把壳搬到能喂进真红的位置 + 附一条诊断订正

## 修法用的是登记里预选的那条杠杆

DD-0011 的 `guards` 写着两条候选：「把 root 变成可注入参数让金丝雀能造一次真红的子门」或
「让 merge_gate 一侧的等价性消失」。选前者：`gate_run` 本来就已经收 `root` 参数，真正不可注入的是
**它住在 bin 里**——集成测试 import 不到 bin，所以没有任何测试能把红喂给它。搬进
`xtask/src/gate.rs::run(root)` 后观察面立刻存在，bin 只剩一行 `gate::run(&root)`。

金丝雀两条（`xtask/tests/canary_gate.rs`）：

1. `canary_gate_run_does_not_swallow_a_red_subgate`：合成根——成员目录齐全、源码 3 行、
   god 基线记 1 行 ⇒ 真值是「棘轮恶化」的红。断言 `gate::run` 清单**非空**，且每条明细带
   `god:` / `suppress:` 前缀（前缀漂了就定位不到是哪道子门报的）。
2. `canary_gate_run_reports_a_broken_subgate_as_unjudgeable`：成员目录缺失 ⇒ 必须 `Err`
   （由 `verdict` 落成「判不了」+ 退出码 3）。换成空清单在这条上同样判红。

先证红（老规矩，不靠"测试应该能抓到"）：把 `run` 整体手打成 `Ok(Default::default())` ⇒
**两条同时 FAILED**；逐字节还原（md5 `07524f6ea2f7dc436d4a519b770cfd06` 对账，不用
`git checkout --`）后复绿。

## 门的原话（同一提交两轮，逐位一致）

```
判定轮  变异面 总=433 捕获=372 未捕获=22 unviable=36 档=全档
        mutants: 红（1 条）
          - 基线键本轮无可判定变异（不可验证…）：xtask/src/main.rs::gate_run
重录轮  变异面 总=433 捕获=372 未捕获=22 unviable=36 档=全档
        基线已写 tools/baselines/mutants-baseline.json（missed 10 条）
        注意：本次重录从基线移除了 1 个键…：- xtask/src/main.rs::gate_run
```

与片K 收尾轮（HEAD `f73e4c0`：总=433 捕获=371 未捕获=23）对照：**总数不变、捕获 +1、未捕获 −1**，
差值正是这条债。`reconcile_mutants.py --keys` 给 `xtask/src/gate.rs::run` =
`{'CaughtMutant': 3}`（Missed 0）⇒ 新面是真被杀而不是没测到；旧键「本轮没出现该键」是因为函数搬走，
不是因为构建/链接失败（同轮 unviable 仍是 36，未涨）。重录后默认对账四份清单全空 ⇒ RECONCILE OK。

捕获率按门自己打印的数：372/433 = **85.9%**（含 unviable 分母）；只对可判定面 433−36=397 ⇒
372/397 = **93.7%**。未捕获 22 条按键：`taint.rs` 四键 1+2+1+1、`main.rs::main` 2、
`maturity.rs::pattern_matches` 1、`maturity.rs::seg_eq` 1（另有 Timeout 2 条不计入）、
`mir.rs::run` 3、`mir.rs::rustc_print` 2、`mutants.rs::run` 8 —— 合计 22，与门打印数一致。

god 侧：`xtask god --write` 基线 456 → 462，逐键点名（新增 7 面、**移除
`fn:xtask/src/main.rs::gate_run`**、`file:xtask/src/lib.rs` 11→12）——移除那条正是搬家，
不是丢面。

## 这一片自己撞到的一条（而且是门在起作用）

第一次收尾跑用的是命令默认 `--base`，本机没有 `main` 分支 ⇒ 门**没有**给数字，而是
`mutants: 判不了（本轮没出判定，别把别的数当结论）：diff 口径 main...HEAD 取不到（git 退出码 128）…`
+ 退出码 3。片K 刚立的三态契约第一次在真实误用下兑现：如果它当时像旧版那样把失败当"没跑完"甩一句裸
错误，我很可能又去复算别的数当结论。复跑口径改回本片一直在用的 `--base 6201ea4`。

## 附：DD-0002 / DD-0003 的诊断被实测推翻（订正，不是新增债）

登记里 DD-0003 写「快轨对 Rust 的污点**能力缺失**」、DD-0002 写「没有任何一条规则能同时在 AST 与
MIR 成立 ⇒『两边都报』结构上凑不出来」。今天下午两条固定命令把这两句都判成**错**：

| 语料（只差一处形态） | 快轨 `--engine ast` | 深轨 `--engine mir` | 三方账 |
|---|---|---|---|
| `chained.rs`：`let raw = std::env::var("X").unwrap_or_default(); Command::new(raw)` | **0 条** | 1 条 | 仅深轨 1 / 两边都报 0 |
| `direct.rs`：同一句去掉 `.unwrap_or_default()` | **1 条**（`RS-TAINT-COMMAND`，engine=tree-sitter，行 3） | 1 条 | 同键可得 |
| `crates/adv-ast-rust/tests/fixtures`（4 份污点夹具） | 0 条 | 4 条 | 仅深轨 4 |

复现：`cargo run -q -p adv-cli -- scan <路径> --rules rules --engine ast|mir|both`。

机制（读码定位，与上面 A/B 一致）：`crates/adv-parse/src/rust_lang.rs:142-177` 的 `dotted_callee`
把链式调用的外层 callee 拼成 `std::env::var.unwrap_or_default`；`crates/adv-rules/src/taint.rs:279-280`
判"调用即源点"用 `sources.iter().any(|s| s == callee)` 精确相等，`matcher.rs:163-172` 的裸名档也要求
全等 ⇒ 赋值右侧永远匹配不上源点，声明过的 propagator（`unwrap_or_default`）同样落空。

所以真实形状是：**快轨有 Rust 污点引擎且能用（`adv-rules/src/taint.rs`，Python 的 PY-TAINT-EVAL 正在
用它），缺的是"链式调用上的源点/propagator 匹配"**。这把 DD-0003 从「切片顺序的代价（设计债）」降级
为「产品级漏报（可修的判定缺陷）」——更严重而不是更轻：任何写成链式的 Rust 源点在快轨上静默流空。
DD-0002 那句「结构上凑不出来」同时撤回：同一规则、同一文件、同一行（两边都报 1 条）在两轨是可得的，
现在差的只有这条链。债条按实测改写，处置（含误杀面 A/B）留到片M。

## 片L 重放

`cargo fmt --check` 0 · clippy（先 touch，`--workspace --all-targets -D warnings`）exit 0 ·
`cargo test --workspace` exit 0（44 条 `test result: ok`，xtask 侧 12+2+15+5=34）·
`xtask gate` 绿 · `xtask mir` 绿 · `xtask god` 绿（重录后）· `debt_gate` 红 4 处
（DD-0002/0003/0005/0008；DD-0011 已销，在册 4 / 已销 7）· 变异门全档两轮逐位一致（见上）·
`reconcile_mutants` RECONCILE OK。

## 附二：按线重放才发现的存量真红（公开撤回片J 的一条）

上面那段"片L 重放"我最初也是按老习惯列的单脚本清单。这次改成跑**整条线**
`python -X utf8 scripts/local_gate.py --line adv-m2`，第一步就把两片都没看见的东西抖出来了：

- `test_s167_god_gate.py::test_baseline_exists_and_gate_green` FAILED ⇒ python 侧 god 门
  `✗ tests/test_s145_gates.py: file_lines 172 → 188（不许变胖）`。查账：
  `git show f470ded:tests/test_s145_gates.py | wc -l` = **187**，而**同一次提交**里的
  `god-baseline.json` 该键写 `file_lines: 172` ⇒ 这道门从片J 起每个提交都该判红，一直没被看见。
  **因此片J 重放栏里那句 `pytest … s167 7 … = 50 条过` 是错的**（我今天在同一台机上跑同一批
  文件：49 过 1 红）。错因不是编码、不是环境，是我把"我跑了哪些脚本"当成了"门跑了哪些脚本"：
  `god-gate` 明明在 `adv-m2` 这条线的 15 步里，而我两片都只挑单脚本跑，恰好跳过了它。
- 另有一类是**我自己的调用口径**造成的假红：直接 `python -m pytest` 在本机 cp936 下会让
  子进程读线程解码失败 ⇒ `cp.stdout is None` ⇒ 10 条测试报 `TypeError: NoneType`，看着像门坏。
  加 `-X utf8` 后同一批 49 过 1 红。教训：本仓 python 侧一律 `python -X utf8 -m pytest`。

## 附三：python god 门的一个盲区（登记为 DD-0012）

清上面那条红要用 `--write-baseline`，重录时门自己打印的规模暴露了更大的事实：
基线 **380 → 452 键**，其中**新增 72、移除 0、变大 2**。72 个新增几乎全是历片新建的 Rust 面
（`xtask/tests/canary_mutants.rs` 607 行、`xtask/src/mutants.rs` 537、`crates/adv-cli/src/main.rs` 467、
`crates/adv-rules/src/taint.rs` 421……）。机制核对（`scripts/god_gate.py:303`）：表外新文件只在
**超硬阈**时才 `（新增，无基线）` 报红，没超阈就一声不响 ⇒ 棘轮实际只约束"已经在表里的键"，
一个新面从创建到硬阈之间可以任意生长而不判红。同一形状在 Rust 侧会红
（`god.rs::ratchet_violations` 的 `None => 未登记 …` 分支）。

复跑门：`GOD-GATE OK 超标/变胖=0` ⇒ 这次登记没有把超阈面洗白（这是登记动作唯一的自证，
所以我把它也写进账）。缺的判据本体登记成 **DD-0012**（在册 5 / 已销 7，债门仍红 4 处——
DD-0012 的 `since` 是今天的 HEAD，还没到期）。

## 片L 最终重放（按线）

```
LOCAL-GATE FAIL steps=14 skipped=['cli-bench', 'perf-gate', 'coverage-gate'] failed=['debt-gate'] total=22.9s
```
逐步：path-gate / arch-gate / ci-wiring-gate / claim-gate / name-ledger / bench-anchor /
handoff-gate / deps-lock / **god-gate** / type-gate / quality-pact / cargo-test / clippy 全 OK；
debt-gate 红 4 处（都是 D5 逾期，属"账没处置完"不是回归）；三个计时/覆盖率步按档跳过并写明开关。

变异门在**最终 HEAD `013700e`** 上另起独立第三轮（不带 `--update`，TMP 在 D 盘）：

```
变异面 总=433 捕获=372 未捕获=22 unviable=36 档=全档
超时 1 条（不计入 missed…）：xtask/src/maturity.rs::seg_eq
mutants: 绿
```

同面三轮（判定轮 / 重录轮 / 收尾轮）逐位一致 ⇒ 新基线 10 键不是"录下来正好自洽"，
而是门在没有任何解释性动作的情况下自己说绿。

推送后 CI（run `37554679267`，顶梢 `0b712b1`）：**唯一失败步 = Debt gate**，打出的正是
那 4 条 D5（DD-0002/0003 拖 33、DD-0005 拖 33、DD-0008 拖 28）；DD-0012 今天才登记，未到期
所以不响；无 D4 假红（`fetch-depth: 0` 仍在位）。即"带红推送"这条机制按预期工作：
红的是账，不是回归。

# 片M（2026-10-07）：快轨链式源点匹配——DD-0003 的漏报面收回来，「两边都报」填上

## 改前的固定尺

命令一律 `cargo run -q -p adv-cli -- scan <路径> --rules rules --engine ast|mir|both`（用户要求先出数）：

| 语料 | `ast` | `mir` | `both` 三方账 |
|---|---|---|---|
| `crates/adv-ast-rust/tests/fixtures`（5 文件，含 4 份污点夹具） | 0 | 4 | 两边都报 **0** / 仅快轨 0 / 仅深轨 4 |
| `crates/adv-cli/tests/data/taint-crate`（2 文件） | 0 | 1（1 文件深轨跑失败，退 2） | 两边都报 0 / 仅快轨 0 / 仅深轨 1 |
| `three-way-corpus`（1 文件） | 2 | 0 | 两边都报 0 / 仅快轨 2 / 仅深轨 0 |
| 真实面 `crates`（52 文件） | **27**＝PY-EVAL 2 + PY-EXEC 1 + RS-PANIC 12 + RS-UNWRAP 12，**RS-TAINT-COMMAND 0** | — | 误杀基线 |
| 真实面 `xtask`（14 文件） | 0 | — | 同上 |

先量风险半径：全仓 `crates/*/src` + `xtask/src` 里链式 `env::var(...)` 只有 2 处
（`crates/adv-ast-rust/src/main.rs:184`、`xtask/src/mutants.rs:399`），且都接**未声明**的 `.ok()`
⇒ 严式修法下预期产品面新增 0 条。

## 改法：只扩一档，并把"不许渗到源点/汇点"反向钉住

`crates/adv-rules/src/taint.rs` 加 `tail_segment` + `method_tier_in`，把**链式调用的尾段**纳入
propagator / sanitizer 两档（`std::env::var(x).unwrap_or_default()` 的尾段正是名单里的
`unwrap_or_default`）；`expr_taint` 里只有这两处改用 `method_tier_in`。source / sink 仍走 `name_in`
的精确相等，并由单测 `source_and_sink_tiers_stay_exact` 断言
`name_in("std::env::var.unwrap_or_default", ["std::env::var"]) == false` —— 扩档一旦渗进源点/汇点，
`x.anything(env::var)` 这类形状就会被说成源点，扩面不可控。深轨一行没动。

## 先证红（红测试在前，改动在后）

新夹具 `crates/adv-rules/tests/ruleid.rs::RS_TAINT` 写完后**先跑**，门自己的话：
`rs_taint: 规则注解不一致 / 漏报（注了没报）: [(4, "RS-TAINT-COMMAND")] / 误报（没注却报）: [] /
实际: [(6, "RS-TAINT-COMMAND")]` —— 漏的正是链式那行（4），报的是非链式那行（6）。机制当场坐实，
不靠我读码推断。改后同一夹具两条都中，`.ok()` 与 `"safe".to_string()` 两条反例仍不报。

## 改后同一把尺

| 语料 | `ast` | 三方账 |
|---|---|---|
| `adv-ast-rust/tests/fixtures` | 0 → **3** | 两边都报 0 → **3** / 仅快轨 0 / 仅深轨 **1** |
| `taint-crate`（逐文件档） | 0 → **3** | — |
| 真实面 `crates` | 27 → **34** | 新增 7 条**全是 RS-TAINT-COMMAND 且逐条落在夹具/语料里**：`taint_dual_role.rs:8`、`taint_direct.rs:6`、`taint_operators.rs:16`、`taint_sanitized.rs:19`、`taint-crate/src/inner.rs:5`、`src/lib.rs:10`、`src/lib.rs:15`；**产品代码新增 0 条** ⇒ 没引入误杀 |
| 真实面 `xtask` | 0 → 0 | — |

残留 1 条「仅深轨」是 `taint_branch.rs`（分支合流要 CFG）——两引擎的真实语义差，不是缺陷也不是键问题：
快轨按语句序流敏感 + 未声明即保守杀，要在 AST 档建模合流才追得上，属设计取舍，不记债。

## 这一片判红了 3 条旧测试，而它们钉的是漏报

首轮 `cargo test --workspace` 有 3 条红（`deep_cargo.rs` 两条 + `engine_mir.rs` 一条），改前都是绿的：
- `cargo_mode_reports_cross_module_flows_and_respects_sanitizer` 断言"stdout 全量行数 == 夹具标记数 3"
  且每行 `engine == "mir"`，注释写着"快轨在该 rust 语料 0 条"——**把漏报钉成了期望**；
- `cargo_mode_golden_freezes_the_lines` 拿全量行比金样，同样默认快轨贡献 0；
- `both_reports_three_way_tally_on_stderr` 断言"并集应仍是这 4 条（快轨 0 条）"。

处理不是放宽，而是把口径改对：新增 `lines_by_engine`，深轨行 = 金样 3 行（金样 `deep-cargo.jsonl`
**一个字节没动**），快轨行单独与夹具 `adv-expect: hit` 标记对账（也是 3），并加两条结构性断言——
快轨每条深轨都有同键条目（⇒ 这次修复没造出"快轨独有"的假阳面）、唯一那条仅深轨必须是
`taint_branch.rs`。桶计数不许自己数一套、集合另一套：`three_way_ledger.rs` 新增的自洽断言把三个桶
与 `|fast ∩ deep|` 绑在一起。

## DD-0013：顺带量出来的第二个设计层缺陷（在册，**未修**）

`--deep-via-cargo` 档下深轨 `file` 是 **crate 相对路径**（`src/lib.rs`），快轨是调用者给的长路径 ⇒
三方账的键 `(rule, file, line)` 不同源。同一份 taint-crate 实测：`6 条发现（快轨 3 / 深轨 3）`
但 `两边都报 0 / 仅快轨 3 / 仅深轨 3`——3 个流程被算成 6 条，最有信息量的桶仍是 0。逐文件档没这问题
（才有上面「两边都报 3」）。修法两条待裁：A=在 `scan_crate_via_cargo` 把 file 换回调用者坐标系
（要**有意识重录**金样 3 行的 `file` 字段，属动冻结契约）；B=只在 `three_way_buckets` 的键上归一
（金样不动，但两引擎回包仍不同形，双计只在账里消失）。症状已钉进 `deep_cargo.rs`：桶计数一修就会
判红并指名来改账，不许静默变绿。

## 账与基线

- DD-0003 销（`retired_in d55c62c`）：evidence 带改前/改后两张表 + 先证红原文 + 严式范围。
- DD-0002 销：「两边都报」0 → 3，且断言桶计数与两侧集合自洽，不是抄跑出来的数。
- DD-0013 登记（`origin a9e8212`、`due_after_tasks 10`）。debt_gate 现红 **2** 处
  （DD-0005 拖 35、DD-0008 拖 30）；在册 4 / 已销 9。
- god 两侧重录并逐键点名：xtask `462 → 474`（新增 12、变大 8、移除 0）；python god 452 文件（5 处变大）。
- `cargo test --workspace` **101 passed / 0 failed** · clippy `-D warnings` exit 0 · fmt 0 差异 ·
  `xtask god` / `gate` / `mir` 全绿。

# 片M-3（2026-10-07）：DD-0013 按 B 案修（只归一账的键，不动行契约与金样）

用户拍板选 B。落点：`crates/adv-cli/src/main.rs` 新增 `ledger_file(finding, cargo_base)`，
`three_way_buckets` / `three_way` / `report` 改收 `Option<&Path>`，`scan` 只在 `--deep-via-cargo`
时传 `Some(target)`，逐文件 mir 档传 `None`（不拼接）。

## 三条归一边界，每条都是我自己踩出来的

第一版按"路径是否绝对"无差别 join，**用相对路径调用时当场露馅**：快轨的相对路径
`crates/…/taint-crate/src/lib.rs` 已经含目标目录，再叠一次 base 得到
`crates/…/taint-crate/crates/…/taint-crate/src/lib.rs`，两边都不相等 ⇒ 账仍是 0。
（是我在改完后用 `adv scan crates/adv-cli/tests/data/taint-crate …` 复跑发现的，
不是靠测试通过的绿——测试当时用的是绝对路径，恰好把这条掩盖了。）

于是判据写成三条，各有单测：① 只作用于 `ENGINE_MIR` 且非绝对路径；② `cargo_base = None`
（逐文件档）一个字符都不动；③ base 必须是调用者给的 target —— 反例单测钉住
"不同 crate 的同名 `src/lib.rs` 不许撞上"，因为那种归一会造出**并不存在的相互印证**，
比双计更糟。

## 实测（三档调用形状）

| 调用 | 改前 | 改后 |
|---|---|---|
| `--engine both --deep-via-cargo`（相对目标） | `6 条发现（快轨 3 / 深轨 3）`+`两边都报 0 / 仅快轨 3 / 仅深轨 3` | `两边都报 3 / 仅快轨 0 / 仅深轨 0` |
| 同上（绝对目标） | 同形 0/3/3 | `两边都报 3 / 0 / 0` |
| `--engine both`（逐文件档，fixtures） | `两边都报 3 / 0 / 1` | 不变 `3 / 0 / 1`（那 1 是 `taint_branch` 的 CFG 语义差） |

**B 案的已知边界**（如实记，不算回归）：stdout 仍按两引擎各自的 file 形状出行，3 个流程 6 行——
账里不再双计，回包侧要按 `engine` 分行才能对账（`deep_cargo.rs` 就是这么改的）。金样
`crates/adv-cli/tests/golden/deep-cargo.jsonl` **一字节未改**，这是 B 与 A 的全部差别。

`deep_cargo.rs` 里那条"钉症状"的断言按它自己的要求（改完必须判红并指名来改账）兑现了一次：
先红，再把期望换成归一后的形状，注释里留了 6 行为什么不是 bug。

## 片M-3 重放与账

- DD-0013 销（`retired_in 45a76f4`）。在册 3（DD-0005 / DD-0008 / DD-0012）/ 已销 10；
  debt_gate 红 **2** 处。
- god 两侧重录并点名：xtask `474 → 476`（新增 `ledger_file` 与其测试；`file:main.rs` 466 → 588、
  `fn:scan` 83 → 89、`fn:three_way_buckets` 17 → 20）；python god 452 键（1 处变大）。
- clippy `-D warnings` exit 0 · `cargo test --workspace` **102 passed / 0 failed** ·
  `xtask god` / `gate` 绿 · fmt 0 差异。

## 片M 收尾（全档，HEAD `ec4cc37`）

```
变异面 总=453 捕获=392 未捕获=22 unviable=36 档=全档
超时 1 条（不计入 missed…）：xtask/src/maturity.rs::seg_eq
mutants: 绿
```

面从 433 涨到 453（片M 新增的 4 个函数 + 3 个测试文件的变异面），捕获 372 → 392、未捕获**不变 22**、
unviable 不变 36。`reconcile_mutants.py` 四份清单全空 ⇒ RECONCILE OK；逐键核查本片新面全部"真被杀"：

| 新面 | 计数 |
|---|---|
| `adv-rules/src/taint.rs::method_tier_in`（链式尾段匹配本体） | CaughtMutant **7** / Missed 0 |
| `adv-rules/src/taint.rs::tail_segment` | CaughtMutant **6** / Missed 0 |
| `adv-cli/src/main.rs::ledger_file`（DD-0013 B 案归一） | CaughtMutant **7** / Missed 0 |
| `adv-cli/src/main.rs::three_way_buckets` | CaughtMutant **12** / Missed 0 |

（`--keys` 那一栏对四条都打了"真被杀"，所以这次的 `RECONCILE OK` 不是"没测到"。）

档位耗时观察（同一台机）：本轮全档比片K 那轮**明显更长**——片M 新增的端到端测试会起深轨边车，
cargo-mutants 每变一条都要重跑一遍 adv-cli 测试集。这条属实测记账，供下片决定要不要把
"起边车的端到端"挪出变异面。

# 片N（2026-10-07）：DD-0012 —— python god 门补上"表外新面必须先登记"

修的是一类**盲区**而不是一条数值：旧口径对表外新文件只在**超硬阈**时报
`（新增，无基线）`，没超阈就一声不响 ⇒ 一个面从创建长到硬阈之间可以任意生长而门恒绿。
实测形状见片L：一次重录补登记了 **72 个从未进表的面**（0 移除、2 变大），全是历片新建的 Rust 面。

改法（`scripts/god_gate.py::evaluate`）：`require_registration = bool(base)`，新文件在**文件维度**
报一条 `未登记（新面：跑 --write-baseline 登记并披露）`；只在文件维度点一次名，键维度各报一条
会把同一件事刷成三行。与 Rust 侧 `xtask god::ratchet_violations` 的 `None => 未登记 …` 对齐。

两条边界（都进代码注释 + 金丝雀）：
- **没有基线时不启用**（引导期口径不变，否则新仓第一步就红死、门会被直接关掉）；
- **登记后必须转绿**：门要的是点名，不是堵死。所以新金丝雀是两半——
  `test_unregistered_new_file_reddens_once_a_baseline_exists`（新面 ⇒ 红且输出含
  `newface.py: 未登记`；`--write-baseline` 后 ⇒ 绿）与 `test_no_baseline_keeps_the_bootstrap_path`。

先证红再证绿（拿本次自身增长当输入，不造合成文件）：门报 3 条
`✗ god_gate.py: file_lines 463 → 471`、`✗ god_gate.py: max_fn_lines 68 → 69`、
`✗ tests/test_s167_god_gate.py: file_lines 151 → 179`；`--write-baseline` 后
`基线 452 → 452 键（新增 0、移除 0、行数变大 2）`、门转绿。这条恰好又演示了记忆里那条纪律：
**只改测试/脚本也要跑门**，行数棘轮照算。

DD-0012 销（`retired_in 3ab41b7`）。**在册只剩 2 条**：DD-0005（云端全量扫描口径，等裁）、
DD-0008（提交路径钩子，等快档真能绿）；已销 11。python 分片 **52 passed**（含两条新金丝雀）；
门线除 debt-gate 外全绿。

# 片O（2026-10-07）：DD-0008 —— 装窄版钩子，并让它能自证

用户拍板"装窄版钩子"。先量后装：`local_gate.py --fast` 实测 **31 步里 14 步红**
（secrets / naming-gate / data-flow / secrets-history / audit-freshness / tool-evals /
cli-golden / mcp-surface / selftest / dupe-gate / lint-gate / typos-gate / gitleaks-gate +
debt-gate，全是伞仓 python 面的存量账）⇒ 直接装全档钩子 = 每次提交被 14 条红挡住，
正是要防的"必然红=噪声"。

三件事：

1. **窄版线 `pre-commit`**（`spec/gate-lines.json` 新增）：15 步 = adv-m2 减 `debt-gate` 再加
   `hook-status`。为什么减 debt-gate 写在 `why` 里：超期由 CI 判是既有分工
   （`tests/test_debt_gate.py` 只断结构、CI 的 Debt gate 判超期），本地再判一次等于**在修这笔账
   的时候被这笔账挡住提交**——死锁不是纪律。实测整线 12.7s 绿。
2. **`scripts/hook_status.py`**：提交路径的自证三态——已装 / 未装 / **不一致**，只有"记号说有、
   实际没装"判红（新克隆与 CI 天然没钩子，判红就是噪声；这与本仓"红-by-default 的门会失效"
   同一条纪律）。`--install` 幂等落记号 `adv.hooksInstalledAt`，实测
   `core.hooksPath=.githooks`、记号 `2026-10-07T10:54:59+08:00@6916db1`；`--require` 供显式收口。
   金丝雀三条：未装不红 / `--require` 翻转 / 记号有装而无钩子必红。
3. **真的在提交路径上跑**：`sh .githooks/pre-commit` 与**真实 `git commit`**（`7c6e6b7`）都完整
   跑过 14 步（12.7s / 11.6s，全绿），输出头部就是 `HOOK-STATUS installed`——这条不是"我跑过
   脚本"，是"提交真被拦了一下并放行"。`pre-push` 同改（窄版线 + `cargo run -p xtask -- gate`）。

**claim-gate 当场抓到我把文档数字写旧了**：加一步之后真值变 32/36/39，而 README（38/31/35）与
HARDENING（31/35）还是旧数 ⇒ `CLAIM-GATE FAIL mismatch=3`。按它给的真值源改完转绿。这条恰好
演示了"文档里的数字必须机器可复算"的用处——我改的不只是数字，是别人读到的现状。

**DD-0012 的新规则在片O 当场生效**：新增 `scripts/hook_status.py` 与 `tests/test_hook_status.py`
两个文件后，python god 门立刻报 `✗ …: 未登记（新面：跑 --write-baseline 登记并披露）`；
重录 `452 → 454`（新增 2、变大 2）。

DD-0008 销（`retired_in 7c6e6b7`）。**在册只剩 1 条：DD-0005**（已销 12）。

# 片P（2026-10-07）：DD-0005 口径裁定 + 云端扫描提交（异步）

先量再裁：全仓 **7.7GB**（含 `.git`/`target`）→ 排除 `.git/target/rust/RESEARCH` 后 **16MB**
（其中 `bench` 6MB、`tests` 6MB 是体量大头）→ 用户拍板**只扫承重面**：
`crates/ rules/ spec/ xtask/ scripts/ tools/` + 顶层配置 `Cargo.toml Cargo.lock
rust-toolchain.toml .github`（≈4MB）。理由与登记里那条成因对齐：区分「被扫产品面」与
「构建产物/研究材料」——`bench/`、`tests/` 里的故意脆弱样本不该混进产品面的扫描结论。

提交（异步，本机命令可直接重跑同一口径）：

```
~/.qodersec/bin/qodersec.exe scan crates rules spec xtask scripts tools \
  Cargo.toml Cargo.lock rust-toolchain.toml .github
```

回执：`project=github.com:bfxh/ADV`、`project_id=12229`、`scan_id=53768`、
`task_name=adv-rewrite@9d17d49#d4e7dde166feb7b7230636c8df685fa1`、
报告 <https://qoder.com/security-code-scan/reports/12229/53768>。

取回结果（**本轮按工具规矩没有在提交后立刻轮询**，取回是新的一次请求）：

```
~/.qodersec/bin/qodersec.exe scan poll --project github.com:bfxh/ADV --task-id 53768 --report detailed
```

DD-0005 **未销**：报告是异步的，结果没取回并处置之前不销账（这一条恰好也是本仓纪律——
"提交了"不等于"扫过了"，如同"版本化钩子"不等于"装好的钩子"）。在册 1 / 已销 12，
`debt_gate` 红 1 处（就是它）。










