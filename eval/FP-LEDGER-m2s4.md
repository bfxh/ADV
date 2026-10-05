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
  查 `unviable.txt` 坐实原因：它们这轮的变异被判 **Unviable（17 条命中这两个文件）**，
  即变异编不过 ⇒ 按框架规则"不是问题"。所以键消失是**分类变化，不是测试变强**。

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

1. **补门的洞**（优先，成本小）：变异门增加一条判据——基线里的键若本轮既不出现在 caught
   也不出现在 missed（全被判 unviable），报"该键失去可验证性"并判红/警告，不得默认绿。
   本片实测已经有 5 个存量键因 17 条 unviable 而静默消失，这就是活样本。
2. **片B**：cargo 集成（RUSTC_WORKSPACE_WRAPPER）+ 对拍 `rust/` 旧引擎出真三方账。
   深轨当前只能吃自包含文件（见"适用边界"），片B 才是价值证明的那一步。



