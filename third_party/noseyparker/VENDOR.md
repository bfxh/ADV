# Vendored: Nosey Parker（内化源码，M3-1）

- 上游：<https://github.com/praetorian-inc/noseyparker>
- 取用提交：`2e6e7f36ce36619852532bbe698d8cb7a26d2da7`（2026-02-21 的上游 HEAD，`--depth 1` 浅克隆）
- 许可证：Apache-2.0（`LICENSE` 与 `NOTICE` 原样保留，按上游要求随源码分发）
- 取了什么：上游 `crates/` 中**除 `noseyparker-cli` 外**的全部 crate + 根 `Cargo.toml`（workspace 定义）
  （上游那把独立 `Cargo.lock` 在 M3-3c 剪 git 面时**重新生成过**：它原先记着已被删除的
  `input-enumerator`，我们自己的 LC-2 判据当场判红 ⇒ 用 `cargo generate-lockfile` 按裁剪后的清单
  重出一份（278 包）。**注意它从此是我们的产物、不再是上游的锚**；我们自己的根 `Cargo.lock` 才是
  构建与审计的依据，这份锁只服务于"有人想独立构建这棵树"的场景）。**为什么不取 CLI**：① 我们只用核心库与规则集（`adv-secrets` 的薄壳），
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
- **与上游的第二处差异（剪掉 git 面，M3-3c，2026-10-08）**：上游 `gix` 与 `input-enumerator` 都是
  **非可选依赖**，于是我们不用的 git 面照样进依赖图并吃 advisory —— 实测命中 `gix-date 0.10.7`
  （RUSTSEC-2025-0140），而修复只在 `>=0.12.0`、vendored 的 `input-enumerator` 钉 `"0.10"`、
  上游已于 2026-02-21 退役 ⇒ **等不到上游修**。处置（用户拍板「剪掉 git 面」）：
  ① 新增 `crates/noseyparker/src/object_id.rs`（本地 20 字节 newtype，替代 `gix::ObjectId`），
  `blob_id_map.rs`/`blob_id_set.rs` 的容器从 `gix::hashtable::{HashMap,HashSet}` 换成
  `std::collections` 的对应物（**语义逐字等价**；代价是哈希器换成 std 的 SipHash，20 字节键上的
  常数差，未做基准）；② `blob_id.rs` 删掉 4 个 `gix::ObjectId` 转换 impl；
  ③ `provenance.rs` 把上游的 `Arc<CommitMetadata>` 窄化成它唯一被用到的字段 `commit_id`（`Display` 只用它），
  并删掉 `input-enumerator` 依赖与整个 `crates/input-enumerator/`（含它的 `git_metadata_graph` 等）；
  ④ `crates/noseyparker/Cargo.toml` 去掉 `gix`；⑤ `smallvec` 显式补 `serde` feature——
  那个 feature 原来是**借 gix 的依赖图**打开的，gix 一走就断供（"特性靠邻居打开"的坑，实测踩到）。
  **实测结果**：`cargo tree -i gix` / `-i gix-date` 都查无此包，依赖包数 **305 → 209**，
  `cargo deny check` 四项全 ok（此前那条单 id 窄豁免已撤回，树回到零例外）；
  `adv-secrets` 的金丝雀与扫描测试全绿（去重表换了实现、行为不变）。
  **上游已退役 ⇒ 这次分叉不会产生同步成本**（这是选它的关键论据）；若将来要 git 面，
  按上游原样接回 `input-enumerator` + `gix` 并撤销本段。
  **连带处置**：上游那份独立 `Cargo.lock` 已重新生成（见上文"取了什么"末句）——我们的 LC-2 判据
  （`adv sca` 的"锁里有、清单没声明"）当场把它判红（`gix, input-enumerator` 已不在清单里），
  等于自己抓到了这次裁剪的连锁面。裁决口径：**判据没错，是那把锁过期了**。
- 我们不维护这份代码：升级=重新同步上游并更新本文件的提交号；要改行为请提 issue/PR 到上游，
  或在 `adv-secrets` 一侧包一层。出处锚以本文件的提交号为准（不许"凭印象说来自哪里"）。
