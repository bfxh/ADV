# 03 · 模糊测试 / 变异测试 / 动态验证 —— 外部调研（供《重写蓝图》）

> 调研日期：2026-10-03。方法：WebSearch + 一手文档抓取（cargo-fuzz 官方书 CI 页、Depot 分布式 fuzzing 实操文、cargo-mutants 官方 changelog、StrykerJS 官方增量文档、PIT 官方文档检索、Kani releases 检索）。
> 口径：标注【待验证】= 本次未拿到一手文本，仅检索摘要/记忆；标注样本范围的数字不外推。许可证除特别标注外均为"可抄代码"档；AGPL/专有只能抄思想。

---

## 一、逐对象调研

### 1. cargo-fuzz + libFuzzer

**一行定位**：Rust 官方生态事实标准的 libFuzzer 前端（cargo 子命令 + libfuzzer-sys crate），`fuzz/` 目录约定 + `fuzz/corpus`、`fuzz/artifacts` 产物约定。

**值得抄的机制**
1. **预算即 flag**：`cargo fuzz run <target> -- -max_total_time=300`。官方 CI 页给的就是这套（`CARGO_FUZZ_VERSION: 0.12.0`、`FUZZ_TIME: 300` 秒、GitHub Actions matrix 按 fuzz target 展开、失败时 `actions/upload-artifact` 上传 `fuzz/artifacts`）——"预算控制"不需要自研调度器，一个参数就是一档。
2. **回归重放 `-runs=0`**：对既有 corpus 全量重放、不生成新输入——PR 门禁形态：秒级到分钟级，语料里每一份历史 crash 样本等于一条永久回归测试（"崩溃入库即棘轮"）。官方 CI 页未收录此用法（该页只写 `-max_total_time` 冒烟），`-runs=0` 出自社区通行做法与 libFuzzer 语义（本次检索的 binaryexploitation.org/appsec.guide 侧面印证）。
3. **`-merge=1` 语料合并/最小化**：多输入目录合并时只保留"新增覆盖"的输入。Depot 实操文用它在 merge job 里把 N 个 shard 的 `new_findings/` 去重后写回 `fuzz/corpus`——这是多机并行的收口动作。
4. **artifact 命名约定**：libFuzzer 落盘 `crash-<输入sha1>` / `oom-` / `timeout-` 前缀文件。注意：**libFuzzer 自身不做崩溃去重**（文件名是输入哈希），栈级去重是平台层（ClusterFuzz crash state）的活，别指望引擎白送。
5. **覆盖率闭环**：`cargo fuzz coverage` 产出 profraw 覆盖报告 → 定位 corpus 盲区 → 补种子。扫描器项目的语料天然可从"真实仓库文件"来（见第六节）。

**档位【吸收】**——机制成本低、与"解析不可信文件"的主业完全同构，三档制（PR 重放 / 定时冒烟 / 定时深跑）全部可直接落地。

**第一步动作**：为解析入口建第一个 harness `fuzz_parse(data: &[u8])`（目标：解析全程不 panic、不无限循环），CI 先加 `-runs=0` 重放档 + 300 秒冒烟档，corpus 以真实仓库样本初始化。

**许可证**：cargo-fuzz / libfuzzer-sys = MIT OR Apache-2.0（待验证具体双许可表述）；libFuzzer 本体 = Apache-2.0 WITH LLVM-exception。
**链接**：https://rust-fuzz.github.io/book/cargo-fuzz/ci.html ｜ https://github.com/rust-fuzz/cargo-fuzz

---

### 2. AFL++（Rust 接入：afl.rs）

**一行定位**：rust-fuzz 组织下的 AFL 包装（`afl::fuzz!` 宏 + persistent mode），是 cargo-fuzz 之外的"第二引擎"。

**值得抄的机制**
1. **Persistent mode + 共享内存覆盖反馈**：被测进程常驻、每次迭代只跑函数而非重启进程（fork server 负责脏状态隔离）——与 libFuzzer inprocess 同思路，但反馈统计路径不同。
2. **异构引擎交叉**：libFuzzer 与 AFL++ 消费同格式语料文件。同一 corpus 可以在两引擎间互相喂——卡住时换引擎是零成本操作（OSS-Fuzz 对同一 target 跑多引擎即此逻辑）。

**档位【有界】**——开发活跃度低于 cargo-fuzz/LibAFL（本次检索口径：afl.rs "development activity is modest"）；对 ADV 只在"libFuzzer 长期无新覆盖"时作为换引擎手段，不作为默认门禁组件。

**第一步动作**：暂不接入。语料格式按纯文件目录设计（不绑 libFuzzer 内部格式），保留换引擎自由度。

**许可证**：afl.rs Apache-2.0（待验证）；AFL++ 本体 Apache-2.0。
**链接**：https://github.com/rust-fuzz/afl.rs ｜ https://rust-fuzz.github.io/book/afl.html

---

### 3. honggfuzz（honggfuzz-rs）

**一行定位**：老牌覆盖率引导 fuzzer（软件插桩 + 硬件性能计数器两种反馈），Rust 包装 honggfuzz-rs；社区共识活跃度已落后。

**值得抄的机制**
1. **硬件 perf counter 反馈**：不依赖编译期插桩拿分支覆盖——思想可用于"对无法插桩的黑盒二进制做反馈"，但 Rust 自研工具链里收益低。
2. 低桩开销方向在 Rust 生态已被 LibAFL 的编译期方案覆盖（见下）。

**档位【不吸收】**（工具本体；"低桩反馈"思想标记为有界）——上游长期低维护（本次检索口径："generally considered less actively maintained than cargo-fuzz and LibAFL"；上游具体停更时间【待验证】），引入即背维护风险。

**第一步动作**：无。仅在本文件留档，避免后人重复评估。

**许可证**：Apache-2.0（honggfuzz，待验证）。
**链接**：https://github.com/rust-fuzz/honggfuzz-rs

---

### 4. LibAFL

**一行定位**：AFL++ 团队的 Rust 原生 fuzzing **框架**（库而非 CLI），2025–2026 仍在活跃开发，学术扩展不断（如 LibAFL-DiFuzz，arXiv:2601.22772，定向 fuzzing for Rust/Go）。

**值得抄的机制**
1. **组件化 trait 拆分**：Executor（怎么跑）/ Observers（观测什么）/ Feedback（算不算新覆盖）/ Scheduler（下一个输入谁上）/ Mutator（怎么变）各自独立 trait——"fuzzer 是组合物"这个数据结构设计本身可抄到我们自研扫描引擎的管线设计。
2. **inprocess Executor + Llmp 共享内存多核/多机 broker**：自建分布式 fuzz 集群的参考实现。

**档位【有界】**——ADV 的主业是扫描器不是 fuzzer 产品；cargo-fuzz 已覆盖需求。LibAFL 的接入价值出现在"需要大规模定向 campaign（比如对某规则引擎做定向压力）"的后期。

**第一步动作**：无代码动作；在蓝图里把"自研引擎管线 trait 拆分"记一条 LibAFL 参考位。

**许可证**：Apache-2.0（部分子 crate 许可不同【待验证】）。
**链接**：https://aflplus.plus/libafl-book/introduction.html ｜ https://docs.rs/libafl

---

### 5. Miri

**一行定位**：Rust 官方 MIR 解释器，把 unsafe 代码的 UB（越界、未初始化内存、Stacked/Tree Borrows 别名违规、泄漏、并发数据竞争）变成可复现的运行期报错。

**值得抄的机制 / 成本档位**
1. **`cargo miri test` 直接复用现有测试集**——零新增 harness 成本；官方 GitHub Action（rust-lang/miri-action）一条 job 就能进 CI。
2. **`-Zmiri-many-seeds=0..N`**：对同一测试跑 N 个随机种子，捕非确定性 UB（未初始化内存依赖行为）；成本随种子数线性放大——这是"周期档"flag，不是 PR 档 flag。（flag 语法出自 Miri README，本次未直接抓取原文【待验证细节】。）
3. **成本口径**：MIRI 是解释执行，社区一致口径"非常慢"（数十倍慢 + 显著内存放大；本次检索 Reddit/JVM 项目、Bun 讨论均此口径，属定性非基准）。实测参考：dora-rs 2026-04-09 QA 报告把重型分析挡在 PR 预算外（"would add 30+ minutes per PR"）——Miri 全量测试同量级风险。单点样本：某咨询团队口径 ~0.3 unsafe 块/1000 行、集中在 FFI 边界（样本范围=其服务项目，不可外推到 ADV）。

**档位【有界】**——PR 门只跑"含 unsafe 的 crate 的单测"；nightly 周期档加 many-seeds；全仓 `cargo miri test` 不进门。ADV 重写目标几乎全 safe（Rust 性能引擎除外），Miri 价值集中在 FFI/SIMD/指针操作那几个 crate。

**第一步动作**：识别 unsafe 清单（预计很小）；CI 加 nightly job `cargo miri test -p <unsafe-crate>`；不做棘轮基线（跑面小，直接要求全绿）。

**许可证**：MIT OR Apache-2.0（rust-lang 工具链标准双许可）。
**链接**：https://github.com/rust-lang/miri ｜ https://github.com/rust-lang/miri-action

---

### 6. Kani（Rust 模型检查）

**一行定位**：Amazon 开源的 Rust 有界模型检查器（BMC，CBMC 后端），**证明**性质（panic-free、自定义 assert/contract）而非"找 bug"；2025–2026 处于活跃发版期。

**2025–2026 现状锚点**（本次检索，出处 GitHub releases 页检索摘要）：0.67.0（releases 页面口径 2026-01-15 前后）；0.68.0 新增 `#[kani::loop_decreases]`（证明循环终止）；Autoharness 持续改进——`--bounded-arguments` 下支持 slice/string（#4803/#4805）与引用，仍有限制类 issue 活跃（如 #4808 `&Wtf8` 未支持）；性能改进含 parallel goto-transformation。生产用例：Amazon s2n-quic；2026 arXiv 论文（arXiv:2607.01504）口径其 harness 规模 168 个（2024 年口径，manual+auto 合计）——这是"证明套件该多大"的量级参考。

**值得抄的机制**
1. **`#[kani::proof]` harness + `kani::any()` 符号参数**：把"任意输入下此纯函数不 panic"写成普通测试函数；`#[kani::unwind]`/`--default-unwind` 控制循环展开上界（有界=对该界内 sound，超出不算证毕——写结论时必须带这个限定）。
2. **Autoharness**：自动为函数生成符号参数 harness（复杂类型/递归用 bounded-arguments 截断）——把"写 harness"的人力成本压掉一部分。
3. **Kani contracts**（`#[kani::requires/ensures/modifies]`）：前置/后置条件可同时被证明与被 harness 复用——与 ADV 自建"棘轮账目"门禁的契约思想同构。

**档位【有界】**——SMT 求解成本函数级分钟~小时级，绝不适合全仓、不适合 PR 门；价值在核心纯函数（路径规范化、掩码计算、解析不变量、账目运算）的精挑选题。harness 总量控制在几十个内（s2n-quic 168 为上限参考）。

**第一步动作**：挑 1 个纯函数（建议：文件路径/规范化函数）写 `#[kani::proof]` harness 跑通并计时，用实测耗时定 harness 配额与运行档（nightly 或 weekly）。

**许可证**：MIT OR Apache-2.0（model-checking/kani，待验证双许可表述）。
**链接**：https://github.com/model-checking/kani/releases ｜ https://model-checking.github.io/kani ｜ https://arxiv.org/abs/2607.01504

---

### 7. cargo-mutants（Rust 变异测试）

**一行定位**：Rust 源码级变异测试（缺失函数体、运算符替换等），逐 mutant 跑测试：存活=测试盲区；`cargo mutants` 一条命令全仓生成+验证。

**机制锚点**（本次直接抓取官方 changelog mutants.rs/changelog.html，逐条带版本）：
1. **diff 增量 `--in-diff FILE`**：23.11.1 引入"只测 diff 内的 mutant"；23.12.0 改为零 mutant 时退出成功（CI 友好）；24.7.1 空 diff 不报错；26.0.0 容忍二进制/非 UTF-8 diff。**这就是 PR 门禁档的现成开关**。
2. **分片 `--shard k/n`**（23.12.2）+ `--sharding slice|round-robin`（26.0.0，默认 slice）：多机拆分；26.0.0 默认确定性顺序（可 `--shuffle` 恢复）保证分片可复现。
3. **并行与副本策略**：`--jobs`（1.1.0）、GNU jobserver 限编译并发（24.9.0）、`--jobs>8` 告警（25.0.1）；26.0.0 支持 reflink COW 树复制（Btrfs/XFS/APFS）、25.1.0 `--copy-target`、24.1.2 `--in-place`（快但与 --jobs 不兼容）——"每 mutant 一次干净拷贝"的磁盘成本是被认真工程化过的问题。
4. **超时体系**：自动 timeout = min 20s 或 5×baseline（0.2.2）、`--timeout-multiplier`（24.2.1）；build timeout 24.7.0 引入后 24.7.1 又撤默认（flaky）——**经验：自动超时难调，门禁要固定 multiplier 并记录 baseline**。
5. **排除面**：`#[mutants::skip]`（0.0.2 起）、`#[cfg_attr(test, mutants::skip)]`（0.2.7）、`#[tokio::test]` 类测试属性自动跳过（26.0.0）、`#[mutants::exclude_re]`（changelog 标注 unreleased）。
6. **CI 输出**：GitHub Actions 结构化 annotations（25.3.0，`GITHUB_ACTION` 环境自动生效）、`outcomes.json` 带 start/end_time/版本（26.0.0）。
7. **没有结果缓存**：changelog 全文未见 mutants-cache/持久缓存机制（本次核验范围=该 changelog 页【待验证】）——这正是 Stryker/PIT 思想的插入点（见 §8/§9）。

**成本量级**：每 mutant ≈ 一次测试运行（共享 target 目录摊薄编译成本）；全量 = mutant 数 × 单次测试时长，中型项目数千 mutant → 小时~天级 → 全量必须 `--shard` + 周跑。

**当棘轮门用**：PR 上 `git diff origin/main...HEAD > d.patch && cargo mutants --in-diff d.patch --baseline=skip`，存活数 > 0 即红。语义 = "你改动的函数必须被现有测试杀死"，与统一-rx 已有的"绿=真路径"棘轮同构。

**档位【吸收】**（diff 增量档为主）；全量周跑分片【有界】。

**第一步动作**：在 ADV 核心解析 crate 上跑一次全量基线（记录 mutant 总数/总耗时/存活数），再定 diff 档是否直接进 PR 门。

**许可证**：MIT（待验证——本人记忆口径，未在本次一手核验）。
**链接**：https://mutants.rs/changelog.html ｜ https://github.com/sourcefrog/cargo-mutants

---

### 8. Stryker（JS/TS，增量模式思想样本）

**一行定位**：JS/TS 生态最大变异测试框架，其 **incremental 模式**是"变异结果缓存"的成熟参照实现（官方文档，本次直接抓取）。

**值得抄的机制**（全部出自官方 incremental 文档）
1. **结果文件 + git-like diff 失效判定**：历史结果落 `stryker-incremental.json`（`--incrementalFile` 可改路径；`--incremental` 或配置 `"incremental": true` 开启）；下次运行对"被测文件 + 测试文件"做 diff 匹配 mutant/测试。
2. **复用条件的键设计**（关键）：killed 的 mutant 仅当"元凶测试（killing test）仍存在且未变"才复用；survived 的 mutant 仅当"无新测试覆盖它且测试未变"才复用——即**缓存键 = mutant 源码 + 覆盖它的测试集状态**，测试侧变更会使缓存失效。这是 PIT"类哈希"键的加强版。
3. **测试追踪精度分级**：按 runner 能力分 4 级（full=逐测试定位 / tests-per-file / names-only / none）——告诉我们：自建时缓存失效粒度可以直接按"测试文件"级起步，够用。
4. **dry run 永远重跑**（基线不能缓存）+ **中断续跑**（CTRL+C 也落盘部分结果）+ `--force` 强制全跑。官方示例效果：3965 个 mutant 复用 3731、实跑 234（文档示例值，非承诺值，样本范围=该示例）。

**档位【吸收思想】**——不是 JS 工具本身，而是把"survived 也缓存 + 测试变更失效 + killing-test 记录"做成 cargo-mutants 外层包装（或自研变异门禁的核心数据结构）。

**第一步动作**：在蓝图"棘轮体系"章节定义变异缓存 schema：`{mutant_id, source_hash, covering_tests_hash, outcome, killing_test}`，先给 cargo-mutants 的 outcomes.json 做外层 diff/缓存包装实验。

**许可证**：Apache-2.0（stryker-mutator 组织口径【待验证】）。
**链接**：https://stryker-mutator.io/docs/stryker-js/incremental/

---

### 9. PIT（Java，coverage 过滤思想样本）

**一行定位**：Java 生态事实标准变异测试器，两大可抄点：增量 history 与**行覆盖测试过滤**（官方文档，本次检索直认 pitest.org）。

**值得抄的机制**
1. **coverage 过滤（默认行为）**：每个 mutant 默认只拿"行覆盖命中它所在行的测试"去杀它，而非全测试集（全矩阵要显式 `--fullMutationMatrix`）——单 mutant 成本从 O(全部测试) 降到 O(覆盖测试)。这是"测试选择"而非"结果缓存"，两者正交可叠加。
2. **增量 history**：`historyInputLocation` / `historyOutputLocation`（0.29 起，2013+），按"类代码哈希未变→复用上次结果"；Maven 侧 `withHistory` 一键双写。Arcmutate 博客同时记录了 CI 中 history 文件的坑（跨 run 一致性/缓存失效）——自建时要把历史文件的"键包含哪些输入"写死，避免脏复用。
3. **思想合并公式**：PIT 的 coverage 过滤（选测试）× Stryker 的 killing-test 失效键（判失效）= 自建变异门禁的最小正确实现。

**档位【吸收思想】**——Java 工具不引入；两机制是 cargo-mutants 当前（经本次核验的 changelog 范围内）缺失的能力。

**第一步动作**：同 §8 第一步（合并为一件事：变异缓存 schema 设计中把"covering_tests"字段定义为 PIT 式行覆盖计算出的测试子集）。

**许可证**：Apache-2.0（pitest）。
**链接**：https://pitest.org/quickstart/incremental_analysis ｜ https://blog.arcmutate.com/history

---

## 二、专项回答

### ① 持续 fuzz 的自托管 CI 形态（预算 / 时长 / 收敛 / 归因）

**三档制**（各档耗时量级为检索/文档锚定值）：
- **PR 门（重放档）**：`-runs=0` 重放入库 corpus，秒~分钟级，每 PR 必跑。保证"已发现的崩溃永不复发"，这是把 fuzz 从"随机抽奖"变成"棘轮"的关键一格。
- **冒烟档**：每 target `-max_total_time=300`（官方 CI 页数值），PR 或每日；官方页用 matrix 按 target 横向展开。
- **深跑档（定时）**：Depot 实操形态——cron 每 6 小时，4 个 shard × `-max_total_time=600`（每 runner 10 分钟）+ `-fork=$(nproc)` 吃满单机核；merge job（`needs: fuzz`、`if: always() && !cancelled()`）下载各 shard `new_findings/`，用 `-merge=1` 去重合并，语料写回 Actions cache（时间戳 key + `restore-keys:` 前缀恢复最新，旧条目自动滚动淘汰——作者原文口径"age out or be removed when the cache is over size"）。折算成本：每 6h ≈ 40 runner-min，作者明说这是"比一小时大 job 容易过审"的预算心理学，并警告别变成"budget fire"。
- **语料分层**：`corpus/`（小、入 git，人类策展的种子）+ CI 缓存层（机器滚动的生成语料）。crash 修复后把触发样本转入 corpus，即永久回归。

**发现怎么收敛**：覆盖率引导 fuzz 的新增输入收益随时间指数衰减；稳定项目长跑主要靠①语料积累②多 shard/多起点分化（Depot 口径：同起点语料各 shard 仍会走出不同路径，避免局部极小）③换引擎。**降频判据（建议，非检索结论）**：连续 N 个深跑周期零新 crash → 深跑从 6h 降为 daily/weekly，预算让位给变异测试。

**崩溃自动归因**：分层——libFuzzer 引擎层只落 `crash-<sha1>`/`oom-`/`timeout-` artifact；**最小化**用 `-minimize_crash=1` 把触发输入缩到最小；**栈级去重与 issue 去重是平台层能力**（ClusterFuzz 的 crash state = 归一化栈顶帧，Apache-2.0，思想与代码皆可抄）。自托管最小可行形态：crash artifact → 自动最小化 → 入库为回归测试 → 按栈顶帧哈希去重 → 通知。OSS-Fuzz 级别（issue 生命周期管理、修复验证、LLM 辅助归因）对单项目自托管属过度建设，标【不吸收】。

### ② 变异测试当门禁的可行档位

| 档位 | 机制 | 耗时量级 | 适用 |
|---|---|---|---|
| 全量 | `cargo mutants` 全仓 | mutant 数 × 测试时长；中型项目小时~天级（无结果缓存，本次 changelog 核验范围内） | 周跑/发布前，必须 `--shard` 多机 |
| diff 增量 | `--in-diff d.patch --baseline=skip` | 改动函数数 × 测试时长，分钟级 | **PR 门主档** |
| 分片/抽样 | `--shard k/n`（slice 默认，确定性顺序） | 全量 ÷ n | 全量的并行化，不是独立档 |

跨语言增量思想（Stryker 结果缓存 + 测试变更失效、PIT 行覆盖测试选择）可把"全量"折到接近 diff 档的成本——cargo-mutants 暂无此机制，**自建外层缓存是 ADV 的差异点机会**。门禁语义统一为棘轮："改动面内存活 mutant 数 = 0"，允许以显式 `#[mutants::skip]`（须注释理由）为白名单，白名单本身入棘轮台账。

### ③ Miri / Kani 的 CI 档位

- **Miri**：PR 门 = 仅"含 unsafe 的 crate"的 `cargo miri test`（预期跑面小、分钟级）；周期档 = nightly + `-Zmiri-many-seeds`（种子数 ≤ 8 起步，成本线性）；全仓 Miri 不进门（解释执行慢，数十倍口径，见 §5）。
- **Kani**：PR 门不进（SMT 函数级分钟~小时）；周期档 = nightly/weekly 跑 proof harness 套件；harness 配额几十个（s2n-quic 168 个为上限参考，2024 口径）；每个新 harness 先实测耗时再收编，防止周期档爆炸。
- 共同纪律：两者的红都不设"豁免过期"，要么修复要么把对应输入/函数挪出验证面并记录——与棘轮"否证带时间戳"同构。

---

## 三、对扫描器自身的黑盒/灰盒 fuzz 用法（综合）

1. **种子 = 真实语料**：ADV 扫描的目标就是代码文件，所以 fuzz 种子天然来自真实仓库样本（源码、配置、asar/zip 容器、二进制样例）。语料策展规则：入库种子小而典型；每类解析路径至少 1 个种子；历史 crash 样本永久入库。
2. **结构感知（灰盒）**：扫描器解析多层格式（容器 → 内层文本 → 规则匹配）。给"容器层"写 `arbitrary` derive 的中间表示 harness（先解容器再对内层负载 fuzz），避免随机字节永远死在 zip 校验上——这是扫描器 fuzz 与普通库 fuzz 的最大差异点。
3. **覆盖率闭环**：`cargo fuzz coverage` 定期出报告 → 找"规则引擎/解析分支"盲区 → 定向补种子或新增 harness；语料定期 `-merge=1` 最小化。
4. **对扫描"输出"也要有 oracle**：不只断言不 panic——对"掩码不外泄原始秘密"这类性质，可在 harness 内对 fuzz 输入做往返断言（输入含已知 token → 输出必不含明文），把 fuzzer 变成扫描器正确性的差分测试台。

---

## 四、2025–2026 前沿信号

1. **Kani Autoharness 成熟化**：0.67→0.68（2026-01 前后）连发 autoharness 参数类型扩展（`--bounded-arguments` 的 slice/string/引用）与 `#[kani::loop_decreases]`；parallel goto-transformation 性能线——自动证明的人力门槛在快速下降（出处：GitHub releases 检索摘要）。
2. **LLM × 验证 harness**：HarnessLLM（ACM 2025/2026）自动生成 Rust 验证 harness；OSS-Fuzz 系 oss-fuzz-gen 同方向【待验证最新状态】——"写 harness"正在从人力成本变成提示词成本，蓝图可为"自动补 harness"留接口位。
3. **LibAFL 仍是 Rust 侧最活跃框架**，学术扩展持续（LibAFL-DiFuzz，arXiv:2601.22772，Rust/Go 定向 fuzzing）——自建大规模 campaign 时的技术底座首选。
4. **cargo-mutants 26.0.0 是工程可用性大版本**：sharding 策略化（slice/round-robin）、reflink COW 副本、确定性顺序、`exclude_re`（unreleased）——2026 年内"变异测试进 CI"的摩擦显著下降（出处：官方 changelog）。
5. **托管 fuzz 服务出现**（fuzze.rs 等 cargo-fuzz/afl.rs 托管）——证明"持续 fuzz 即服务"是真实需求；自托管应按其成本结构（小预算高频 + 语料云持久化）设计。
6. **MirI 并发/多种子能力**（`-Zmiri-many-seeds`、并发解释器）使"周期性深检"而非"每 PR 浅检"成为合理档位（Miri README 方向【待验证细节】）。

---

## 五、Top-3（对新项目最值钱的 3 条）

1. **持续 fuzz 三档制直接照抄**：PR 门 `-runs=0` 重放（秒~分钟，崩溃样本永久入库=棘轮）+ 每 6h 定时 4 shard × 10 min 深跑 + merge job `-merge=1` 去重回写语料缓存（Depot 形态，≈40 runner-min/周期）。扫描器 fuzz 必须补结构感知层（容器→内层 `arbitrary` harness）与输出 oracle（掩码不回显明文的往返断言）。
2. **变异门禁以 `--in-diff` 为唯一 PR 档**（分钟级，`--baseline=skip` + `--timeout-multiplier` 固定口径），全量只做周跑 `--shard`；同时自建"mutant 结果缓存 + 测试变更失效 + 行覆盖测试选择"外层（抄 Stryker/PIT 思想）——这是 cargo-mutants 经核验缺失、而 ADV 棘轮体系正好需要的差异点。
3. **重动态验证全部锚在周期档，不进 PR 门**：Miri 只跑含 unsafe 的 crate（nightly，many-seeds ≤8）；Kani 精选 ≤20 个核心纯函数 proof harness（周跑，实测耗时定配额，s2n-quic 168 为上限参考）；两条都与棘轮同构——红即修复或显式记录挪出，不留"豁免过期"。
