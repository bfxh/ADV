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
| CI（msvc 档） | `gh run list/view`：670fd70 → run 37306492822、cef520d → run 37307126960 | 两轮均 **success**（fmt/clippy/nextest/coverage/gates/deny 全过） |

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

## 下一步

片A2：GenKill 污点到达分析（源/汇同源 YAML，复用夹具的 `flow`）。
