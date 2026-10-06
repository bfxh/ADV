# 试验面 / 承重面分域 + 设计债到期门（2026-10-06 设计稿，待拍板）

> 起因是一条真实的不满：**对"正在被测试的东西"用"维护承重结构"的尺，会把整条流程拖慢，
> 而拖慢本身不产出正确性**。测试面上的 bug、低可维护度是**预期属性**；
> 真正不能容忍的是**设计层面的问题**——设计不好就是不好，跟它是不是测试材料无关。
> 本稿要同时做到两件事：① 让维护形的那些门别去卡试验面；② 让**设计债必须带到期日**，
> 最迟在下 10 个任务内处置，超期由门自己想起来判红。

## 1. 今天的实例（为什么现在提这个）

| 事件 | 代价 | 性质 |
|------|------|------|
| 合成测试夹具 `crates/adv-cli/tests/data/taint-crate/` 被 god 棘轮当承重面计量 ⇒ 多录一次基线、多一轮披露 | 一轮重录 | **维护形**的税，交给了试验面 |
| `xtask mutants` 每轮 30–40 分钟（今天跑了 4 轮） | 会话一半时间 | 最慢的那道门，对薄壳代码也一样严 |
| 两个"起子进程 + println"薄壳键进不了断言面（`free_bytes_via_df`、`precheck_scratch`） | 记在变异基线里 | **真设计问题**：观察面为空，谁也没给它到期日 |

⇒ 该松的是前两条，该收紧的是第三条。

## 2. 分域：声明而不是猜

沿用 `spec/path-exempt.json` 的先例（**每条必须带 `why`，缺 why 直接红**）。

- `spec/maturity.json`：glob → 域。默认 `load-bearing`（承重）；登记过的才算 `experimental`（试验）。
- 判据不是"看着像测试"，而是**谁读它、变了谁会红**：每条写 `why`（为什么是试验面）
  与 `guards`（它靠哪道门/哪个金样守）。
- 候选（本仓现成）：`crates/*/tests/data/**`、`crates/*/tests/fixtures/**`、`bench/**`、`RESEARCH/**`。

## 3. 门按域分档：松的只有"维护形"，严的仍是"正确性与卫生"

| 门 | 承重面 | 试验面 | 理由 |
|----|--------|--------|------|
| fmt / clippy / 编译 | 严 | **严** | 这是正确性下界，放了就烂，跟成熟度无关 |
| 仓库卫生（path / secrets / naming / dupe / deps-lock） | 严 | **严** | 污染会外溢到整棵树 |
| golden / 金样 / 金丝雀 | 严 | **严** | 金样正是试验面唯一必须成立的设计断言 |
| god 规模与函数棘轮 | 严（只准减） | **不计量** | 试验面允许写得难看；改为计入"试验面总量软帽" |
| 变异门（新增存活即红） | 严 | **不判红，但必须落一条设计债** | 慢门（30–40 分钟）不该为一次性材料反复重跑；但"观察不到"这件事必须变成有到期日的账 |
| 基线重录的逐键披露 | 严 | 免 | 试验面不进基线，就没有重录可披露 |

一句话口径：**省掉的是"把试验面当资产养护"的开销，留下的是"设计上的问题必须显式且会过期"。**

## 4. 设计债登记 `spec/design-debt.json`（核心是到期计数器）

```json
{
  "id": "DD-0001",
  "surface": "xtask/src/mutants.rs::free_bytes_via_df",
  "kind": "coverage",
  "defect": "起子进程的薄壳无观察面：删掉 status 检查在真产物上不可区分",
  "why_from_design": "盘量查询最初按「一条命令一个数」设计，起进程与解析写在一起；分层是补断言时才补的，薄壳是那次没做彻底的残渣",
  "origin": "5b1d862",
  "since": "1fc62d3",
  "due_after_tasks": 10,
  "guards": "tools/baselines/mutants-baseline.json（存量债 11 键中的 2 条）"
}
```

字段口径（2026-10-06 定）：**`since` = 登记起算的 commit**（计时用，防止"补登记时把里程倒推到
一年前"或"重开刷新计时"两种漂移）；**`origin` = 设计成因所在的 commit**（只追溯、不计时）。
`guards` = 现在靠什么守它；`"无"` 是允许的值，但必须诚实写——"没有判据"本身就是这条债的内容。

判据（`cargo run -p xtask -- debt`，同时是 CI 一步）：

1. 必填字段缺 ⇒ 红；**`why_from_design` 缺 ⇒ 红**（没有设计成因的条目就是"贴个标签躲门"，
   正是本设计要防的）。
2. 超期 ⇒ 红，文案点名"DD-xxxx 已拖过 N 个任务"。计数 =
   `git rev-list --count <since>..HEAD`（见 §6 待拍板①）。
3. 销账必须被引用：删除条目的那个提交/PR 的 message 里要有 `DD-xxxx`，否则红
   （复用 `scripts/quality_pact_gate.py` 的 `EVIDENCE:` 纪律，不另发明格式）。
4. 每条 `due_after_tasks` 只允许 **≤ 10**（本稿的硬约束），要更长必须写理由并升级为
   承重面讨论，不能默认放宽。

## 5. 落在"请求合并那一层"

- **CI**：`core.yml` 加一步 `Debt gate`；同一把尺挂在 `scripts/local_gate.py` 的 fast 档 ⇒ pre-commit 自动覆盖。
- **必需检查**：branch protection 把它设为合并前必需——⚠️ 这是改共享状态，
  要单独授权；命令给出来但默认不执行。
- **PR 模板**（本仓现在没有 `.github/PULL_REQUEST_TEMPLATE.md`，新建）两栏：
  ① 这次碰到试验面了吗？登记/引用了哪条 DD？② 处置了哪条旧债（写 DD-id）？
- **提交路径**：`.githooks` 已启用 ⇒ 同一把尺也挂在 pre-commit，让"忘了"在提交时炸而不是合并时。
- 文档：本文件 + `AGENTS-ADV.md` 一行摘要（门的存在理由要写在Agent会读的地方）。

## 6. 拍板结果（2026-10-06，用户选定三格全 A）

| # | 岔口 | 定案 |
|---|------|------|
| ① | "任务"计数单位 | **commit 数**：`git rev-list --count <since>..HEAD`。纯本地可核、零新账本、CI 秒级。已知代价：一次 squash 会把多个任务计成 1 ⇒ 补偿口径是 `since` 只能是**登记那次的 commit**（见 §4），既不往前倒推里程，也不许重开刷新计时。 |
| ② | 放松范围 | **只松 god + 变异门**（§3 表）。编译/clippy/卫生门/金样/金丝雀/lockstep 一律照旧严。 |
| ③ | 合并门禁 | **先只加 CI 步 + 本地 hook**，不动 branch protection。真要设必需检查时再单独提。 |

## 7. 落点（按实现后的实际形态回填，2026-10-06）

判据落在 **Python 侧**（与既有 26 道门同层）而不是 `xtask debt`：pre-commit 里跑 Rust
要先编译，而这道门只需要 JSON + git 计数。放松那一半仍在 Rust 侧（god 计量）。

- `scripts/debt_gate.py`：七条判据 D1–D7——必填缺 / `why_from_design` 空 / 限期 > 10 /
  **SHA 在 git 里不存在** / 超期 / id 重复 / 销账无 evidence。纯判据函数接受注入的
  `counter`/`exists`，所以"超期"是不是真来自 commit 计数可被单独测（`tests/test_debt_gate.py`）。
- `spec/maturity.json`：试验面 pattern 登记（`why`/`guards` 空即红；`**` 至少吃一层 ⇒
  放松不许比登记字面更宽）。
- `xtask/src/maturity.rs` + `xtask god`：跳过登记的 pattern——②的实际生效点。
- `.github/workflows/core.yml` 的 `Debt gate` 一步 + `scripts/local_gate.py` fast 档
  （⇒ `.githooks/pre-commit` 自动覆盖，不另挂一次）。
- `.github/PULL_REQUEST_TEMPLATE.md`：两栏（碰到试验面？处置了哪条 DD？）+ 证据栏。

## 7.1 重录基线的纪律（本次实测逼出来的，见 DD-0006）

`scripts/god_gate.py --write-baseline` 是**整表替换**：今天量到重录会把 201 条未登记面
（含 `RESEARCH/.tmp-p1/` 这类遗留垃圾）一并转正，表从 377 → 443。所以本片的处理是
**单条有意识更新**（`scripts/local_gate.py` 的 `file_lines` 230→233，条目数保持 377）+
把"整表替换会吞垃圾面"登记成 DD-0006 限期处置。绕过不是判据，故必须留账。

## 8. 明确不做什么

- 不新造第二套基线文件、不改现有 26 道门的判据。
- 不给试验面开"跳过测试"的口子（慢的是**门**，不是**测试**；测试照跑）。
- 不把设计债当成免责凭证：登记 ≠ 可以永远不动，到期判红就是它的牙齿。
- `crates/*/tests/*.rs`（真测试代码）**不算试验面**——它们是被变异门计量的判据本体，
  只有 `tests/data/**`、`tests/fixtures/**`、`tests/golden/**` 这类材料才登记。

