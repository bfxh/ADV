# Vendored: Nosey Parker（内化源码，M3-1）

- 上游：<https://github.com/praetorian-inc/noseyparker>
- 取用提交：`2e6e7f36ce36619852532bbe698d8cb7a26d2da7`（2026-02-21 的上游 HEAD，`--depth 1` 浅克隆）
- 许可证：Apache-2.0（`LICENSE` 与 `NOTICE` 原样保留，按上游要求随源码分发）
- 取了什么：上游 `crates/` 中**除 `noseyparker-cli` 外**的全部 crate + 根 `Cargo.toml`（workspace 定义）
  + `Cargo.lock`。**为什么不取 CLI**：① 我们只用核心库与规则集（`adv-secrets` 的薄壳），
  CLI 会把它自己那 473 包的依赖树一起拖进来；② CLI 的测试快照
  （`crates/noseyparker-cli/tests/scan/appmaker/snapshots/*.snap`）里带着上游自己种的**密钥形状样本**，
  GitHub 推送保护会把它判成 "Push cannot contain secrets" 而拦下整条分支的推送（2026-10-07 实测
  02bac5f 被拦）。去掉 CLI 后：实测 54 个 `.rs`、约 1MB。
  改这个范围=改本文件的一句话，不动任何源码。
- 用途：ADV 的 secrets 面（`crates/adv-secrets`）以 **path 依赖**方式接它的核心库与规则集；
  **不是**我们的 workspace 成员——上游代码不进本仓的 fmt/clippy/god 计量面，我们也不改它。
- **与上游的唯一差异（脱敏，可复核）**：内建规则数据（`crates/noseyparker/data/default/builtin/rules/*.yml`）
  里的 `examples` / `negative_examples` 段已被**删除**（87 个文件、约 1.8k 行）。两条理由：
  ① GitHub 的 push protection 不认"这是上游文档示例"，把它们判成真实密钥拦下整条分支的推送
  （2026-10-07 实测）；② 本仓自己的纪律也是"能删的密钥形状字面量就删，别加白名单"。
  **行为影响：无**——匹配只用 `pattern`，这两个字段是规则 AST 里的文档字段
  （`noseyparker-rules/src/rule.rs` 的构造示例里就是 `examples: vec![]`）。
  复核：`python -X utf8 scripts/vendor_redact.py --check`（无残留段）+ adv-secrets 的金丝雀
  （它加载并解析整包规则，解析不过就判红——等于每天在 CI 上替这次脱敏背书）。
  **重新同步上游后必须重跑 `scripts/vendor_redact.py`。**
- 我们不维护这份代码：升级=重新同步上游并更新本文件的提交号；要改行为请提 issue/PR 到上游，
  或在 `adv-secrets` 一侧包一层。出处锚以本文件的提交号为准（不许"凭印象说来自哪里"）。
