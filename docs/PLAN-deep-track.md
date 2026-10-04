# PLAN: adv-ast-rust 深轨边车（M2 片4 设计稿，2026-10-04）

> 目的：Rust 代码的**真正数据流污点**——MIR 级 IFDS/摘要分析，补 tree-sitter 快轨
> 的语义盲区（宏展开后的事实、所有权/借用即传播路径、闭包捕获）。
> 验收总纲（蓝图 M2 行）：与旧 Rust 引擎对拍（行为等价金样）；旧引擎就在本树
> `rust/` 目录（已从 workspace exclude，只读作对拍基准）。

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

## 2. 驱动形态

- adv-ast-rust 是**二进制 crate**（adv-ast-rust-driver）：经 `rustc_driver::RunCompiler`
  以库方式驱动 rustc，Callbacks 里在 after_analysis 阶段取 MIR。
- 输入形态（分两步走）：
  1. 片A：合成 crate 驱动——参数收 .rs 文件列表，拼 `--crate-type=lib --edition=2024`
     调 rustc（自包含文件可分析；真实仓库按 crate 驱动，片B）。
  2. 片B：cargo 集成——RUSTC_WORKSPACE_WRAPPER 环境变量方式挂进目标仓构建
     （对拍 rust/ 时用）。
- 输出：MIR 事实 → JSON → 主引擎（adv-taint）消费；**边车不进主进程**
  （nightly/不稳定 API 崩溃隔离在子进程——RESEARCH 01 边车纪律的实现形态）。

## 3. 分析内容（片A 最小闭环）

- MIR 数据流：借用 rustc_mir::dataflow 框架的 GenKill 分析实现「污点值到达」：
  - 源点：`extern fn` 调用返回值（规则集与 adv-rules 的 taint spec 同源——
    从 YAML 读，单一事实源）。
  - 传播：move/copy/引用 deref；`Rvalue::Use`/`UnaryOp`/`BinaryOp` 直传。
  - 汇点：调用图匹配 spec 的 sinks（callee 路径按 DefPathInfo 拼）。
- 产出 findings 与 adv-taint 快轨同 schema（复用 Finding::to_jsonl），来源字段
  加 `"engine": "mir"` 供对拍分账。

## 4. 对拍（验收核心）

- 基准 = `rust/`（旧引擎，20K LOC）上跑同一 YAML 规则集：
  快轨（tree-sitter）与深轨（MIR）的结果做**三方账**：两边都报 / 仅快轨 / 仅深轨。
  仅快轨 = 快轨误报或深轨漏报（宏/控制流盲区），逐条进 FP 会计账；
  仅深轨 = 快轨 FN 清单（这正是深轨的价值证明）。
- 金样：挑 3–5 个含 `unwrap/panic/expect` 的真实文件冻结两端输出。

## 5. 里程碑切分

| 片 | 内容 | 验收 |
|----|------|------|
| 片A1 | rustc-dev 组件 + bootstrap 配置 + 空驱动跑通（打印一个 crate 的 MIR 函数名） | 对给定 .rs 输出函数名列表；xtask 断言组件在位 |
| 片A2 | GenKill 污点到达分析（源/汇同源 YAML） | 合成夹具：直接流/ sanitizer 零发现/跨基本块流 |
| 片A3 | 子进程隔离 + JSON 对接 adv-taint | adv-cli `--engine mir` 档；快轨/深轨三方账出账 |
| 片B | cargo 集成 + 对拍 rust/ | 行为等价金样 |

## 6. 已知风险

- rustc_private API 逐版本变动：已钉 1.99.0 ⇒ 冻结期内 API 稳定；升版 = 边车适配
  （这正是边车隔离纪律的存在理由）。
- rustc-dev 体积与 CI 时长：组件缓存策略待测（actions/cache 对 rustup components
  的命中）；预算超标则 CI 只在周期档跑深轨（RESEARCH 03 Miri/Kani 同款分档）。
- windows-gnu 工具链的 llvm-tools 兼容性：片A1 第一件事就是验证，不通则
  片A1 改用 msvc 侧工具链跑边车（本机 msvc 1.99.0 已修复可用）。
