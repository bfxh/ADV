# ADV 工作区纪律（AGENTS-ADV，切换日升为根 AGENTS.md）

> 依据：docs/BLUEPRINT.md（C1–C6 硬约束）+ 旧仓纪律移植。本文件在 adv-rewrite 分支生效。

## 语言与依赖
1. **Rust 为主体**：产品代码全 Rust；Python 白名单制——新 Python 文件必须先登记 `spec/PY-WHITELIST.md` 才许存在（蓝图 C1）。
2. **本地优先**：默认档零网络。联网能力只能以 feature 门控存在，且 CI 断言默认档 `cargo tree` 不含网络栈（RESEARCH X12）。

## 质量门（全部机器执行，判据走真路径）
3. **god 门**：`cargo run -p xtask -- gate`。三轴硬阈（file 800 / fn 120 / members 24）+ 棘轮基线只准减；新面走 `--write` 登记并**在提交信息披露**新增了什么（旧仓教训：--write 会吞掉新超标面 ⇒ 先拆到阈内再登记）。基线在 `tools/baselines/god-baseline.json`。
4. **锁步门**：workspace 版本必须与最新 `v*` tag 精确一致；发版先打 tag（旧仓 36 minor 断档教训）。
5. **金丝雀**：`xtask/tests/canary.rs` 证明门会红——改门逻辑必须连金丝雀一起跑；全绿的门不值得信。
6. **门在提交之前读**：`cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test && cargo run -p xtask -- gate && cargo deny check` 全绿才准提交。
7. 默认档确定性：判定字段必须机器无关；性能门按窗口均值，单点不作数（旧仓测量协议 §7/§10 移植）。

## 流程
8. **一任务=一树=一分支**；本仓当前开发树 = worktree `D:\KF\ADV`（分支 `adv-rewrite`）。
9. **一功能一 PR，不自行合并**（旧仓 spec/PR-DISCIPLINE 四条继续有效）。
10. **表述红线**：数字/结论带锚（版本/日期/复算口径）；依赖关系两端点名；不绝对化（给适用范围）；单点结论不升格总体；解释必须配证据/修法/判据。
11. **先量后改**：判据先于改动；否证留档（标签是时间戳，不是永久事实）。

## 交接与账本
12. 大输出落盘再检索；会话交接只读本文件 + docs/BLUEPRINT.md + RESEARCH/PROGRAM.md。
13. 遗留事项必须带"谁/何时/判据"三要素进档（旧仓"等一句话"惯例）。
