# FP 会计账 — M2 片3（2026-10-04）

> 片3 交付：**变异门装配**（cargo-mutants 27.1.0 `--in-diff` 档进 xtask：`cargo run -p xtask -- mutants [--base ref] [--update]`）
> + 深轨边车设计稿（docs/PLAN-deep-track.md：RUSTC_BOOTSTRAP=1 + rustc-dev 组件方案，不引入 nightly）。

## 变异基线首录（限片2 差异面：base 6201ea4，75 变异 / 48 捕获 / 16 missed→基线收敛 8 键去重）

未捕获（missed）= 未被任何测试杀死的变异 = **补测试的目标清单**（基线入库棘轮，
新增 missed 即红）：

| 变异函数 | 解读 |
|----------|------|
| taint.rs::body_touches_source | 源引用检测的穷举缺口（walk_check 分支无测试区分） |
| taint.rs::expr_taint | 污点分类臂覆盖不全（净化工/传播子接收者等） |
| taint.rs::root_var | 根变量提取的下标/Other 分支无断言 |
| taint.rs::summarize | 摘要旗标的部分分支无断言 |
| xtask/main.rs::main | 子命令分派的 else 臂（用法输出） |
| xtask/mutants.rs::git_diff_patch / missed_key / run | 变异门自身的失败通道无断言（元规则自证缺口） |

**裁定**：这 8 条是诚实债账入库棘轮（只准减），不粉饰为"已验证"。

## 开发中被抓出来的三个契约事实（都是 wrapper 首跑撞出来的）

1. **outcomes.json 路径**：`<out>/mutants.out/outcomes.json`（嵌套一层）。
2. **条目格式**：`{scenario: {Mutant: {file, function: {function_name}}}, summary:
   Success|CaughtMutant|MissedMutant|Unviable}`——首版按记忆写的
   `{mutation{file,function}}/summary=missed` 全不匹配。
3. **退出码契约**：0=全捕获，**2=存在未捕获变异**（正文打 stdout）——wrapper 原来把
   非零一律当失败且只读 stderr，报错为空。修后 0/2 走棘轮，其余 fail-closed。
4. **cargo-mutants 拒绝脏工作树**（Diff content doesn't match source，报错在 stderr）：
   基线首录必须先提交。这一条吃了两次（uncommitted 修复 + 退出码修复各自挡一次）。

## 收口补记（2026-10-05：前会话 quota 耗尽中断，接手把 CI 修绿）

**中断点**：`model-io-sess_deff4641` 第 167 次请求报 `TerminalStreamChunkError: exceed quota limit`
（2026-10-04T19:17:49Z）。它死在"变异门跑绿 + 本账落盘"之后、todo 里的"提交推送 + 核 CI"之前。

**CI 事实**（`gh run list --branch adv-rewrite` 实测）：`e381074`、`cdc36b7` 两个 run **failure**，
`630c8b8` 仍 success。红点在 `cargo run -p xtask -- gate`，**不是变异门**——是 god 门棘轮（函数体长度禁恶化）：

| 键 | 基线 | 实测 | 涨的原因 |
|----|------|------|----------|
| `fn:xtask/src/mutants.rs::missed_key` | 3 | 11 | 修正一把 outcomes 键提取改成 JSON pointer 取 `file` + `function_name` |
| `fn:xtask/src/mutants.rs::run` | 58 | 65 | 修正二加退出码 0/2 契约的错误通道（双流报错） |

本地 `cargo run -p xtask -- gate` 复现同 `exit=1`、同 2 条 ⇒ 排除 CI 环境差异。

**处置**：`xtask god --write` 重录（沿用既有做法——`git log` 上 god-baseline 每片都含 increase：
`0fd48fb`/`83515dc`/`d5c1e54`/`6201ea4`/`5a75373`/`630c8b8`）。重录前后逐键 diff（`target/god-baseline.pre.json`
留了快照可比对），实测 226 → 224 项：

| 变化 | 条数 | 明细 |
|------|------|------|
| 涨 | 2 | 上表两条，无第三处 |
| 降 | 0 | — |
| 新键 | 0 | — |
| 掉键 | 2 | `type:…mutants.rs::MutationKey(fields)`、`::Outcome(fields)`——源码已无这两个结构体
（改走 `serde_json::Value` + `o["summary"] == "MissedMutant"` 字符串比较后成死面），`rg` 零命中确认 |

**不粉饰**：这 2 个函数同时还在未捕获变异清单里（`missed_key`、`run`）——**长度涨了而测试断言没跟着涨**，
瘦身与补断言是同一笔债，留给"修掉一个从基线划掉一个"的棘轮通道追。

**另一笔待清**：Mimosa 完整扫描自 `e381074` 起未跑完（hook 报 `scanner_enobufs`，按兼容策略放行）。
不作门，记在此处。

## 下一步

按 PLAN-deep-track.md：片A1（rustc-dev 组件 + bootstrap 配置 + 空驱动跑通）。
