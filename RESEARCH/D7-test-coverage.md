# D7 深挖：测试执行 / 覆盖率 / Flaky 治理

- 日期：2026-10-03（检索窗口当天；资料优先 2025–2026，机制经典回溯）
- 范围：为 ADV（分支 adv-rewrite）的 test-replay / xtask-gates 提供「测试怎么跑、覆盖率怎么算、flaky 怎么判、改哪测哪」的机制级调研。
- 方法：本域共 14 次检索（4 条因并发限制未返回，已重试补齐）；结论凡来自训练知识未经当日检索复核的，depth 标「中/浅」并在锚一栏注明。说不出机制的没写。
- 关系：nextest + insta + llvm-cov 三件套在 wave-0 06 已定，本文不重开决策，只补机制细节与判据口径。

每对象格式：定位 → 可抄机制 → 档位【吸收|有界|不吸收|watch|reference】→ 第一步动作 → 许可证/成熟度【实验|维护中|弃维护】→ 链接。

---

## 一、测试执行

### D7-01 cargo-nextest（分区 / 重试 / 归档）
- 定位：Rust 事实标准测试执行器（wave-0 已定采用）。这里只补 CI 相关机制细节。
- 可抄机制：
  1. **分区（sharding）**：`--partition hash:K/N` 按测试名哈希分配桶——各分片无需协调即得到一致分配，代价是只平衡"测试个数"不平衡运行时长（小套件易不均）；`--partition count:K/N` 按确定性顺序按数量切分。对 ADV：规则引擎套件测试时长方差大，hash 分片 + nextest 后续的按耗时加权（若有）需在 xtask-gates 里用实际数据复核。
  2. **重试语义**：`.config/nextest.toml` 支持按 filterset 的 per-test 重试覆盖；失败后在退避（backoff）后重跑，"失败→重跑通过"记为 **flaky** 而非失败，默认 flaky 按通过计——这正是"重跑掩盖"的入口，门判据必须在报告层把它单独拎出来（见 D7-02、D7-14）。
  3. **归档执行**（`cargo nextest archive`）：编译一次产出含二进制+元数据的归档，CI 各分片从归档跑——"编译/执行分离"对 ADV 本地多分片同样适用。
- 档位：吸收
- 第一步动作：xtask-gates 固化 `--partition` 与 nextest.toml 重试骨架；flaky 不许静默按通过计（报告层拦截）。
- 许可证/成熟度：Apache-2.0 OR MIT / 维护中
- 链接：https://nexte.st/docs/ci-features/partitioning ；https://nexte.st
- 锚：hash/count 两式与"只平衡个数"来自官方 partitioning 文档页当日检索摘要。

### D7-02 nextest 机器可读输出（JSON 事件流 + JUnit + nextest-metadata）
- 定位：报告统一格式的输入端。nextest 有三层机器输出：`--message-format json` 事件流（内部规范的首选）、JUnit XML（出口/生态互换）、nextest-metadata crate（结构化元数据）。
- 可抄机制：
  1. **JUnit flaky 嵌套**：重试过的用例在 JUnit 里以 `<flakyFailure>` 子元素嵌套保留每次尝试；`JunitFlakyFailStatus`（nextest-metadata）控制 flaky 在 JUnit 里算通过还是失败。第三方（如 Currents）就靠这个结构计价重试。
  2. **消费要点**：按 (file, classname, name) 三元组做聚合键；不同工具的 JUnit 方言不同（nextest 的 flakyFailure 嵌套 vs pytest xunit2 vs surefire），聚合器要按方言适配而不是硬解析单一形状。
  3. ADV 内部建议：**以 nextest JSON 事件流为一等公民**，JUnit 只做对外出口——事件流逐测试、逐尝试、含 stdout 位置，比事后解析 XML 稳。
- 档位：吸收
- 第一步动作：test-replay 落 `--message-format json` 事件流的 schema 快照（insta 固定样例），聚合器先只支持这一种方言。
- 许可证/成熟度：Apache-2.0 OR MIT / 维护中
- 链接：https://docs.rs/nextest-metadata ；https://nexte.st
- 锚：flakyFailure 嵌套与 JunitFlakyFailStatus 来自当日检索（docs.rs 与第三方报告工具文章）。

### D7-03 pytest-xdist（distribution 模式对照）
- 定位：Python 生态多进程测试分发；对 ADV 是"分发策略菜单"的对照样本。
- 可抄机制：`--dist` 六式：`each`（每 worker 全跑，配不同平台）、`load`（默认，逐个派发）、`loadfile`（按文件聚合，保模块级状态）、`loadscope`（按类/模块作用域聚合，保 fixture 语义）、`loadgroup`（`xdist_group` 标记强制同 worker）、`worksteal`（3.1 起，worker 空闲时从队列"偷"测试，削长尾）。核心取舍：**负载均衡 vs 状态亲和**——worksteal 最快但最不保序，loadfile/loadscope 牺牲均衡换隔离。对应 ADV：nextest 分片是静态 hash，长尾问题可借鉴 worksteal 思路在本地 runner 内做动态领取。
- 档位：reference
- 第一步动作：把"状态亲和分级"写进 xtask-gates 的分片设计注释（哪些测试必须同分片：共享 tokio runtime 端口、共享临时目录等）。
- 许可证/成熟度：MIT / 维护中
- 链接：https://github.com/pytest-dev/pytest-xdist
- 锚：六式与 worksteal 版本号来自当日官方仓库检索摘要。

### D7-04 Go test 缓存（机制经典）
- 定位：构建缓存 + 测试结果缓存的双层设计，是本地优先"改哪测哪"缓存的教科书。
- 可抄机制：
  1. **测试结果缓存键** = 测试二进制的构建缓存键 + 测试旗标 + 环境变量等输入；命中则直接回放上次输出并标 `(cached)`。
  2. **运行时失效（testlog 机制，Russ Cox 2018）**：测试进程对文件与环境的访问被记录（`GOCACHE` 里的 testlog）；下次运行若测试"读过"的文件变了，缓存失效。这是**被动观测式失效**——不用静态分析也能抓到测试的真实副作用，正是 ADV test-replay 可以抄的：按测试记录文件访问集，变更时按交集失效。
  3. `-count=1` 显式绕过缓存（Go 官方给的"强制真跑"开关），缓存键不包含时间——缓存永不主动过期，只随输入失效。
- 档位：吸收
- 第一步动作：test-replay 设计文档加"testlog 式副作用记录"一节：先记录（ tracing 文件 open/read 的路径集），后做失效判定。
- 许可证/成熟度：Go 工具链（BSD-3） / 维护中
- 链接：https://go.dev/cmd/go/#hdr-Build_and_test_caching ；https://research.swtch.com/testlog
- 锚：testlog 机制为 Russ Cox 2018 博文（训练知识，未当日复核原文；机制稳定）。

### D7-05 Bazel test caching（与 D4 交界）
- 定位：action 级缓存的测试特化。机制归 D4（构建系统），这里只记测试特化点。
- 可抄机制：测试也是 action，受 `--cache_test_results` 控制；缓存键含 runfiles 集合、分片号、attempt 号；**flaky 测试的结果不落缓存**（被标 flaky 的结果不作为可复用成功）——"flaky 不缓存"这条对 ADV 是可直接抄的门语义：重试通过的测试，其"成功"不得进入 test-replay 缓存。
- 档位：有界
- 第一步动作：与 D4 拉齐缓存键组成（输入文件内容哈希 + 参数 + 测试名），并写明 flaky 结果的缓存排除规则。
- 许可证/成熟度：Apache-2.0 / 维护中
- 链接：https://bazel.build/docs/user-manual（`--cache_test_results` / `--runs_per_test`）
- 锚：键组成与 flaky 不缓存为训练知识（depth 中）；flag 名可 bazel.build 复核。

### D7-06 vitest / jest 分片
- 定位：前端生态在 CI matrix 上做静态分片 + 报告合并的样本。
- 可抄机制：vitest `--shard=i/n`（按文件静态切）+ `--merge-reports`（blob 报告合并，覆盖率与测试结果都在 shard 内收集后合并）；jest `--shard` 配 `--ci --maxWorkers=1`。要点：**分片是无状态静态切分，合并是唯一全局汇聚点**——ADV 若做多分片，blob 式"每分片自带完整局部报告"比"中央实时汇总"简单可靠。
- 档位：有界
- 第一步动作：xtask-gates 若出分片模式，输出格式定为"每分片一份自包含 JSON，合并器只做并集与去重"。
- 许可证/成熟度：MIT / 维护中
- 链接：https://vitest.dev/guide/improving-performance.html ；https://jestjs.io/docs/cli#--shard
- 锚：flag 名为训练知识（depth 中），vitest blob/merge-reports 机制在官方文档。

---

## 二、覆盖率

### D7-07 rustc instrument-coverage + profraw 合并（cargo-llvm-cov）
- 定位：Rust 覆盖率主链路（wave-0 已定 llvm-cov 三件套）。机制：`-C instrument-coverage` 基于 LLVM source-based coverage，各测试二进制跑完产出 `.profraw`，`llvm-profdata merge -sparse` 合并成 `.profdata`，再由 `llvm-cov` 出报告。
- 可抄机制：
  1. `LLVM_PROFILE_FILE` 支持 `%p`（pid）/`%m`（mangled 二进制名）占位——多测试二进制并发跑不互相覆盖，profraw 按 pid 分文件。
  2. cargo-llvm-cov 的 `--no-report` + 后续 `report` 两段式：先跨多次不同条件的运行攒 profraw，再一次合并出报告——这正是"测试执行与报告生成解耦"，xtask-gates 应采用两段式而非一步到位。
  3. 合并语义：profdata 合并是**计数器累加**（per-region counter 相加），所以"多次部分覆盖运行"可以并出全量覆盖——test selection 子集跑的结果也能并进基线。
- 档位：吸收
- 第一步动作：xtask-gates 固化两段式管线（run: 攒 profraw → report: merge + export JSON），profraw 目录进缓存。
- 许可证/成熟度：cargo-llvm-cov Apache-2.0 OR MIT / 维护中；rustc 机制随工具链维护中
- 链接：https://doc.rust-lang.org/rustc/instrument-coverage.html ；https://github.com/taiki-e/cargo-llvm-cov
- 锚：占位符与两段式为官方文档口径（训练知识 + 当日检索摘要交叉）。

### D7-08 llvm-cov 粒度与已知口径坑
- 定位：LLVM 原生报告工具，给 line / region / branch 三个百分比口径。
- 可抄机制：
  1. **region（区间）是 source-based coverage 的原生单元**：coverage mapping 把源码切成连续计数区间，一行可能含多区间（三元、短路、宏展开）。line 100% 而 region <100% 是常态——ADV 棘轮必须先选定口径（见重点回答②）。
  2. 已知精度坑：switch 语句 case 收尾大括号被报未覆盖等 mapping 问题，LLVM Discourse 有专门"覆盖率精度问题指南"帖——判据设计要容忍**已知 mapping 噪声**，而不是拿 line 100% 当绝对真理。
  3. `llvm-cov export` 输出 JSON（含每函数记录、segment 列表）——这是把覆盖映射回"变更行"的原料（diff coverage 的实现基础）。
- 档位：吸收
- 第一步动作：用 `llvm-cov export` 在 ADV 仓跑一次，确认 segment 结构能映射到行，做一张 region→line 归属表。
- 许可证/成熟度：LLVM Apache-2.0-with-exceptions / 维护中
- 链接：https://clang.llvm.org/docs/SourceBasedCodeCoverage.html ；https://discourse.llvm.org/t/guidance-on-issues-with-code-coverage-accuracy/86459
- 锚：Discourse 帖为当日检索所得。

### D7-09 grcov（源码级聚合，与 llvm-cov 的机制差异）
- 定位：Mozilla 的 Rust 覆盖率聚合器。差异点：llvm-cov 是 LLVM 侧"读 profdata 出报告"；grcov 是**应用层聚合器**——并行解析多份原始产物（gcov 格式 / profraw），在源码级做合并、过滤（如按目录排除），出 lcov/cobertura/HTML。
- 可抄机制：并行解析产物 + 在源码级（而非报告级）过滤——"过滤发生在 merge 之前还是之后"决定口径；ADV 若要"排除生成代码/测试代码"的自定义口径，应在 merge 前按文件表过滤，避免百分比先算后改。
- 档位：有界（ADV 已定 llvm-cov，grcov 只作聚合思路参照，不引第二套工具链）
- 第一步动作：不引入；把"merge 前过滤"写进 xtask-gates 口径说明。
- 许可证/成熟度：MPL-2.0 / 维护中（Mozilla）
- 链接：https://github.com/mozilla/grcov

### D7-10 覆盖率粒度取舍：function / line / region / MC-DC
- 定位：棘轮门选什么分子的底层问题。
- 机制与取舍：
  1. **function 级**：最粗最稳（该函数进没进），抗格式噪声，但"函数有测试碰过 ≠ 函数逻辑被测"。
  2. **line 级**：人类直觉单位，但受 mapping 噪声影响（D7-08）。
  3. **region 级**：表达式/分支区间，比 line 更接近"逻辑被测"的语义，粒度细到同一行内区分路径。
  4. **MC/DC**：修正条件判定覆盖，LLVM 侧 clang 14 起支持（`-fcoverage-mcdc` 一线机制）；**rustc 未暴露 MC/DC**（截至 2025 stable 的公开文档口径）——对 ADV 是 watch：规则引擎的组合条件恰是 MC/DC 的目标场景，短期用 region + proptest 组合覆盖补位。
- 档位：吸收（口径决策本身）
- 第一步动作：D7 棘轮定稿会上拍板口径表：PR 门=region/line 双口径（见重点回答②），MC/DC 列 watch。
- 许可证/成熟度：LLVM 侧维护中；rustc MC/DC 未暴露（实验性，不成熟）
- 链接：https://clang.llvm.org/docs/SourceBasedCodeCoverage.html#mc-dc ；https://doc.rust-lang.org/rustc/instrument-coverage.html
- 锚：clang 14 MC/DC 为 LLVM 官方文档口径（训练知识）；rustc 未暴露一说需在 ADV 采用的 toolchain 版本上复验。

### D7-11 diff-cover（diff coverage 经典实现）
- 定位：Python 生态最成熟的增量覆盖工具：拿 coverage XML 与 git diff 相交，只对**变更行**算覆盖率，`--fail-under` 出门。
- 可抄机制：
  1. **分子/分母口径**（源码实现可直接读）：分母 = diff 中的新增/修改行中"可执行行"（排除纯注释/空行）；分子 = 其中被覆盖判定命中的行。**删除行天然不在分母**。
  2. 输入格式多态：Cobertura XML / lcov / JSON 都能吃——ADV 用 llvm-cov export JSON 后转 lcov 是低成本路径。
  3. `diff-quality` 把同一 diff 相交框架复用到 lint 违规——"diff 相交"是个通用门形状，可同时承载覆盖棘轮与 lint 棘轮。
- 档位：吸收
- 第一步动作：xtask-gates 的 diff-coverage 门以 diff-cover 口径为准绳（不引 Python 工具，按其口径在 Rust 侧实现），实现后用 diff-cover 对同一 diff 复算对齐。
- 许可证/成熟度：Apache-2.0 / 维护中
- 链接：https://github.com/Bachmann1234/diff-cover
- 锚：口径描述来自当日检索摘要 + 该工具源码结构（训练知识）。

### D7-12 Codecov / SonarQube 的 new-code 口径（含 2025 坑清单）
- 定位：托管 diff coverage 的两条主流口径（patch coverage / new code coverage），ADV 本地优先不引托管服务，抄判据与坑。
- 可抄机制/坑：
  1. Codecov 的 "patch coverage"（diff 上覆盖）与 "commit coverage" 分开报，门可以只钉 patch。
  2. 坑（社区实录）：测试文件重组后报 0%（路径映射断）；删除行被计入导致覆盖下降假象。→ **行归属要在 merge 前对齐到当前工作树路径**。
  3. Sonar 的 "new code definition"（按版本/分支基准圈定"新代码"）说明：**棘轮基准本身是个可配置对象**（基准分支 + 窗口），不是天然常量。
- 档位：reference
- 第一步动作：把两条坑写进棘轮实现的自测用例（路径重组、删除行）。
- 许可证/成熟度：商业 SaaS（uploader 侧 Apache-2.0）/ 维护中
- 链接：https://community.codecov.com ；https://docs.sonarsource.com
- 锚：0% 重组坑为 Codecov 社区 2025-12 帖（当日检索）。

### D7-13 tarpaulin
- 定位：Rust 另一套覆盖率实现（ptrace 跟踪，不依赖 LLVM 插桩）。
- 机制：运行时 ptrace 跟踪执行流计行覆盖；与 llvm-cov 的插桩式差异在于零编译开销、但精度与 `no_std`/部分场景支持受限，且社区口径常年建议以 llvm-cov 为准（insta/nextest 生态默认亦是）。
- 档位：不吸收（wave-0 已定 llvm-cov；第二套只添噪声）
- 第一步动作：无；仅在 llvm-cov 遇到硬阻塞时重开。
- 许可证/成熟度：MIT / 维护中（迭代慢）
- 链接：https://github.com/xd009642/tarpaulin

---

## 三、Flaky 治理与 test selection

### D7-14 flaky 判定的统计口径
- 定位：给 ADV 门判据的底座。
- 机制（判据模板，非绝对数）：
  1. **同轮混合结果判定**：同一提交内"失败→重跑通过"（nextest 已给出这个信号）即记 flaky 候选——这是成本最低的第一层。
  2. **窗口判定**：per-test 在最近 N 次运行内 pass/fail 混合即 flaky（Azure DevOps 与多家实践同构）。N 取 20–30 次、窗口内出现 ≥2 次"结果翻转"提权为高置信 flaky——具体阈值要在 ADV 自己的窗口数据上标定后再冻结。
  3. **置信度**：重跑 K 次全过只能证"本次不是必然失败"；要估 flake 概率需按二项观测处理（K 次重跑通过次数的 Wilson 区间），K=1 的"重跑通过"是低置信证据——门判据应区分"候选/高置信"两级，不许拿单次重跑过当无罪判决。
  4. **后果分级**：nextest 默认 flaky=pass（不挡 PR）+ 报告层单列；quarantine 名单里的测试跑但不挡门；连续窗口无翻转后自动解除。
- 档位：吸收
- 第一步动作：在 ADV CI 事件流上先做**只记录不设防**的 flaky 计数器，攒 2–4 周数据再定阈值（先量后改）。
- 许可证/成熟度：判据本身无 license；参考实现见 D7-15/17。
- 锚：Google 体量参照——约 1.5% 测试运行 flaky、近 16% 测试有某种程度 flakiness（Micco 2016 演讲口径，2026-09 shiftasia 转引复核；厂商口径 RNF 2025-07 称 26% 团队受 flaky 影响，量级仅供参考）。数字锚齐了再引用，不复述为普遍规律。

### D7-15 Google 的 flaky 治理（quarantine 机制）
- 定位：工业界最常被引用的 flaky 治理流程。
- 机制（公开口径）：重跑与自动标记 → 独立的 flaky 测试追踪/看板（Test Analytics 一类内部工具）→ **quarantine**：确认 flaky 的测试移出主门（仍跑、仍记录、不阻塞），限期修复或删除 → 按趋势复盘。要点不是"要不要重跑"，而是**重跑结果必须进统计、flaky 必须有名册、quarantine 必须有出口**。
- 档位：有界（流程语义可抄，内部工具不可得）
- 第一步动作：xtask-gates 加 `--flaky-report` 输出（who/when/翻了几次），quarantine 名单做成版本化文件（走评审）。
- 许可证/成熟度：实践口径，无代码产物
- 链接：https://shiftasia.com（2026-09 转引 Google 数据的复核锚）；原始口径：John Micco（Google Testing Blog/演讲，2016）

### D7-16 Predictive Test Selection（Google）与 Meta CI test impact（"Savanna"正名）
- 定位：test selection 的两大工业实现公开锚点。
- 正名：**检索未找到名为 "Savanna" 的 Meta 测试选择论文**（多组关键词均无命中，命中的是无关生态学/ML 论文）。最接近的公开锚是：Google 的 **Elbaum et al., "Predictive Test Selection", ICSE-SEIP 2019（pp. 91–100）**；Meta/Facebook 的公开口径散见其 CI 工程论文（Petrović & Ivanković，ICSE-SEIP 2018 一线）描述的 test impact analysis。"Savanna" 记忆疑有误，列 watch 待人工复核。
- 机制（算法综述，四类）：
  1. **静态依赖图式**：变更文件 → 反向依赖 → 受影响测试目标（Bazel `rdeps` 查询、monorepo affected tests；实现简单、保守）。
  2. **覆盖率映射式**：每测试 × 覆盖的源码区间（D7-07 的 profdata 可提供），变更行落在谁的映射里谁跑——精度高，代价是基线维护。
  3. **动态记录式**：记录测试运行时的真实文件/网络访问（Go testlog 式），按访问集失效——把"跑过什么"当真值。
  4. **ML 预测式**（Elbaum/Google；Launchable 商业化）：以历史通过/失败特征预测本变更的失败概率，选 top-p 百分位跑——本质是**允许漏报换吞吐**，必须配保守兜底（定期全量 + 回归补跑）。
- 档位：吸收（1–3 类机制对 ADV 落地现实；第 4 类列 watch）
- 第一步动作：ADV "改哪测哪" v0 = 覆盖率映射式（用 cargo-llvm-cov 每测试 export 建 test→file/line 基线）+ mtime/内容哈希失效；全量跑每夜兜底。
- 许可证/成熟度：论文/内部系统，无直接可用实现；同类商业 Launchable 维护中
- 链接：https://dl.acm.org（检索词 "Predictive Test Selection" ICSE-SEIP 2019）；https://www.launchableinc.com

### D7-17 Azure DevOps flaky test management（自动化判定的参考实现）
- 定位：把 D7-14 口径产品化的公开样本。
- 机制：同一分支近期运行内某测试 pass 与 fail 混合 → 自动标记 flaky，给优先级与统计视图；支持 quarantine 工作流（标记后不阻塞）。可抄其**判定窗口 + 自动标记 + 人工确认**三段式。
- 档位：reference
- 第一步动作：无代码动作；判定窗口语义并入 D7-14 判据模板。
- 许可证/成熟度：商业功能 / 维护中
- 链接：https://learn.microsoft.com/en-us/azure/devops/pipelines/test/flaky-test-management
- 锚：混合结果判定语义为官方文档口径（depth 浅，未逐字复核）。

### D7-18 BuildPulse
- 定位：从 CI 事件流（JUnit/XML 结果）统计 flaky 的商业工具，开源生态常见其 reporter。
- 机制：聚合历次 CI 运行结果，按测试计算稳定性指标，找出"时好时坏"的测试并排序——事件流输入 + 稳定性统计的做法与 ADV 的 JSON 事件流兼容。
- 档位：watch（license 细节未核实，商业为主）
- 第一步动作：仅在其文档里确认"稳定性指标的定义"，用于校准 D7-14 口径。
- 许可证/成熟度：商业（license 未核实）/ 维护中
- 链接：https://buildpulse.io

### D7-19 trunk.io flaky tests（2025 信号）
- 定位：2025 年活跃的"自动检测 + quarantine + 修复追踪"一体化产品。
- 机制：CI 结果摄取 → 自动识别 flaky → 自动隔离（quarantine 名单化）→ 趋势看板。代表 2025 的产品化方向：flaky 治理从"规则"走向"托管管线"。
- 档位：watch
- 第一步动作：订阅其 changelog，观察其 quarantine 语义是否出现可抄的新判据。
- 许可证/成熟度：商业 / 维护中
- 链接：https://trunk.io（2025-04 检索锚）

### D7-20 rr（record/replay，确定性回放）
- 定位：syscall 级录制回放调试器，与 R3 TTD 交界。
- 机制：录制一轮执行（记录 syscall 与非确定性源），回放时消除调度/时钟非确定性，得到逐指令可重放、可反向调试的轨迹。对 flaky 的价值：**偶发失败录制一次，回放到根因**；限制明显：Linux、x86-64/ARM（新版）、需要关闭 ASLR 类特性，Rust 生态只能包着用。
- 档位：watch（与 R3 TTD 合并评估；CI 内不可用，本地排障用）
- 第一步动作：R3 域对接：在 ADV 开发者手册记录"flaky 二次复现 → rr 录制"的本地流程，不进自动门。
- 许可证/成熟度：MIT / 维护中
- 链接：https://rr-project.org

### D7-21 Launchable
- 定位：Elbaum 一线（predictive test selection）的商业化延续。
- 机制：吃历史构建/测试结果，按变更预测失败概率、推荐子集；配"高风险全跑"兜底。作为 D7-16 第 4 类机制的活体参照。
- 档位：reference
- 第一步动作：无；跟踪其公开的口径文档即可。
- 许可证/成熟度：商业 / 维护中
- 链接：https://www.launchableinc.com

---

## 四、Rust 专项

### D7-22 insta（snapshot 测试的 review 流）
- 定位：Rust 快照测试事实标准（wave-0 已定）。补的是 review 工作流而非用法。
- 可抄机制：
  1. **pending 快照两段式**：断言不匹配时产出 `.snap.new`（不阻塞本地迭代），`cargo insta review/accept` 人工裁决后才落 `.snap`——**快照变更显式过人**，天然适配 PR diff 审查（`.snap.new` 若未处理即失败可设为门）。
  2. `INSTA_UPDATE=always/unseen/new/no` 环境档控制行为——xtask-gates 里 CI 档必须钉 `no`/失败即报，防止 CI 自动吞快照。
  3. inline snapshot（`assert_inline_snapshot!`）把期望写回源码位置，diff 审查粒度与代码 diff 一致。
- 档位：吸收
- 第一步动作：xtask-gates 钉 CI 档（INSTA_UPDATE=no + 未处理 .snap.new 视为失败），并给 `.snap` 文件加"变更必须出现在 diff 中"的软检查。
- 许可证/成熟度：Apache-2.0 OR MIT / 维护中
- 链接：https://insta.rs ；https://github.com/mitsuhiko/insta

### D7-23 proptest 的 shrinking 机制拆解
- 定位：属性测试 + 最小反例生成，服务 ADV 规则引擎的不变量测试。
- 机制拆解：
  1. **生成与收缩分离于类型**：每个 `Strategy` 产出 `ValueTree`，其上定义 `simplify`（收小）/`complicate`（放大）/`current`——失败后沿 simplify 链贪心下降，直到固定点或预算耗尽。与 QuickCheck 的 per-type `shrink` 不同，proptest 是 per-value，同一类型可有不同生成/收缩策略。
  2. **变换保关系**：`prop_map` 等变换保留源 Strategy 的关联，收缩仍发生在**源类型域**（如 `u32`）再映射回来——文档原话口径：变换后收缩发生在底层值类型上。反例是"源域最小值"，可复现。
  3. **反例持久化**：失败用例写入 `proptest-regressions/*.txt`（种子/值），后续每次测试先回放历史反例——这就是"确定性回放"的现成机制，test-replay 直接受益。
  4. **预算**：默认 `cases = 256`（Config）；收缩有迭代/时间预算可调（Config 相关字段，取默认值前按 docs.rs 复核，勿凭记忆写死）；社区 issue 记录过收缩时间过长问题（issue #145）——ADV 门里要给属性测试设显式超时。
- 档位：吸收
- 第一步动作：给规则引擎挑 3 条不变量（如"规则输出必为合法事件集"）写 proptest；CI 事件流里记录收缩后反例与 cases 数。
- 许可证/成熟度：Apache-2.0 OR MIT / 维护中
- 链接：https://proptest-rs.github.io/proptest/（book，含 Transforming Strategies 章）；https://github.com/proptest-rs/proptest
- 锚：`cases=256` 与收缩机制为 book/docs.rs 口径（训练知识 + 当日检索摘要交叉）；issue #145 为当日检索所得。

### D7-24 proptest-stateful（状态机属性测试）
- 定位：社区 crate，把 proptest 扩到"模型状态机"式测试（顺序化抽象状态迁移）。
- 机制：定义参考模型与被测状态机的迁移表，随机生成操作序列，逐步与模型比对——适合 ADV 规则引擎会话/流水线这类有状态对象。
- 档位：有界（社区 crate、采用度小；先用裸 proptest，模型复杂后再上）
- 第一步动作：规则引擎 v1 稳定后评估；当前仅记录。
- 许可证/成熟度：实验（社区 crate，小众）
- 链接：https://crates.io/crates/proptest-stateful

### D7-25（并入 D7-02 的消费侧）——见 D7-02。JUnit 报告统一格式的消费与聚合要点已在 D7-02 给出。

---

## 五、跨语言参考

### D7-26 Go -race（向量时钟竞态检测）
- 定位：语言内置竞态检测的机制样本。
- 机制：基于 ThreadSanitizer 的 happens-before 向量时钟算法：对每次内存访问记录"时钟"，读-写对无 happens-before 序即报竞态；开销官方口径：内存 5–10x、执行时间 2–20x（随程序）——所以 Go 社区惯例是 `go test -race` 常驻 CI 但接受其成本，且 `-race` 与覆盖缓存互斥类语义（-race 下测试结果不缓存，因为非确定性）——"竞态检测运行不进结果缓存"这条对 ADV 有界可抄。
- 档位：reference
- 第一步动作：xtask-gates 若加 sanitizer 档（D7-27），对齐"检测档不进缓存"语义。
- 许可证/成熟度：Go 工具链（BSD-3，TSan 移植）/ 维护中
- 链接：https://go.dev/doc/articles/race_detector ；https://go.dev/cmd/go/#hdr-Build_and_test_caching
- 锚：开销数字为 Go 官方文档口径（训练知识，未当日复核）。

### D7-27 ASan / TSan / MSan 的 CI 成本档
- 定位：sanitizer 在 CI 的档位设计依据。
- 成本锚（google/sanitizers wiki 口径，**该仓库已归档**，运行时本体在 LLVM 维护）：ASan 约 2x CPU / 约 3x 内存（官方文档口径）；TSan 约 5–15x CPU / 5–10x 内存（clang/AMD 文档口径，另见内存 5x+1MB/线程的变体口径）；MSan 约 3x CPU 且**要求全部依赖（含 libc）插桩**——单独容器/环境。结论（限定在本域参照内）：ASan(+UBSan) 可作每提交档；TSan/MSan 归 nightly/scheduled 档。IREE 等大项目实践同为"MSan/TSan 只进专用 CI 构建"。
- 档位：有界
- 第一步动作：xtask-gates 定义 `gate-sanitizers` 档：PR 档 ASan+UBSan（若走 C/C++/FFI 面），nightly 档 TSan；Rust 纯侧优先用 Go/-race 式内置替代品（`-Zsanitizer` 实验性，先 watch）。
- 许可证/成熟度：LLVM 运行时 Apache-2.0-with-exceptions / 维护中；google/sanitizers 文档仓库已归档（弃维护——只作数字参考）
- 链接：https://clang.llvm.org/docs/AddressSanitizer.html ；https://clang.llvm.org/docs/ThreadSanitizer.html ；https://github.com/google/sanitizers ；https://iree.dev
- 锚：全部为官方文档/归档 wiki 口径（当日检索 + 训练知识交叉）。

### D7-28 LLVM sanitizer 组合矩阵
- 定位：能不能"一把插桩同时抓两类错"的依据。
- 机制（互斥根源）：ASan/TSan/MSan 各自占用独立的 shadow memory 与运行时，**两个 shadow-memory sanitizer 不能共存于同一二进制**；合法组合：ASan+UBSan（常用）、ASan+LSan（LSan 内嵌于 ASan）、UBSan 单独或与任一 shadow sanitizer 组合；HWASan 是 ASan 的低开销替代（约 2x 档）供内存检测降本。
- 档位：有界
- 第一步动作：把矩阵写成 xtask-gates 的档位常量表（PR: ASan+UBSan / nightly: TSan；MSan 独立容器），禁止在门里拼出互斥组合。
- 许可证/成熟度：LLVM / 维护中
- 链接：https://clang.llvm.org/docs/AddressSanitizer.html
- 锚：组合规则为 clang 官方文档口径（训练知识，depth 中）。

---

## 六、重点回答（对 ADV 的判据设计）

### ① 受影响测试选择（test selection）算法综述与可用实现
- 综述四类：静态依赖图（Bazel rdeps/nx，保守）/ 覆盖率映射（test×line 基线，精确但需维护）/ 动态记录（Go testlog 式，被动真值）/ ML 预测（Google Elbaum 2019、Launchable，允许漏报）。公开工业锚：ICSE-SEIP 2019（Google）；Meta 侧公开口径在 CI 工程论文；**"Savanna" 未检索到，疑似记忆误差（watch）**。2025 学术面仍有 RTS 论文引用该线（Weeraddana & Chandra, 2025, ACM，"Mitigating Waste…in CI"）。
- ADV 落地（v0）：覆盖率映射式——用 cargo-llvm-cov 对每个测试（或每测试组）单独 export，建 `test → file/line` 基线；变更时求交集选测试；基线随代码变更用内容哈希失效；每夜全量兜底 + 选择漏报的回归补跑。v1 再上 testlog 式文件访问记录（D7-04）。
- 判据：选择门必须输出"选了多少 / 跳过多少 / 兜底何时跑"三数，不许静默缩小测试面（与"绿=SKIP 不算绿"同源）。

### ② 覆盖率棘轮的分子/分母口径
- 分母：git diff 中**新增或修改**的可执行行（删除行不进分母；注释/空行/纯声明剔除）。行集合由 `llvm-cov export` 的 segment 区间归属到行后与 diff 相交得到。
- 分子：分母中被覆盖判定命中的行（region 命中则该行算覆盖）。
- "绿=SKIP 不算绿"的延伸：**被排除的行（`#[cfg]` 关闭、`#[ignore]`、平台特定）不得静默消失**——必须单列一份"豁免行清单 + 豁免原因"随报告输出，豁免需走评审。分母口径变更（如新增豁免类别）要过版本化配置，不许实现内散写。
- 双口径并行：PR 门钉 line 口径（直观、抗 mapping 噪声弱化处理：已知 switch-brace 类噪声列白名单），周报看 region 口径趋势；MC/DC watch（rustc 未暴露）。
- 阈值：棘轮值=当前主分支口径值的版本化常量（只升不降由评审控制），PR 门= diff 口径 ≥ 阈值 T；T 首月观测后冻结（先量后改，不拍脑袋）。
- 工具锚：判据以 diff-cover（Apache-2.0，维护中）实现为准绳复算对齐；Codecov 2025-12 的路径重组/删除行坑作为自测用例。

### ③ proptest shrinking 拆解（对规则引擎）
- 链路：Strategy（生成分布）→ ValueTree（`current/simplify/complicate`）→ 失败后沿 simplify 贪心下降至固定点或预算 → 反例写入 `proptest-regressions/` → 后续运行先回放历史反例。`prop_map` 变换保源域关系，收缩发生在源类型（如 usize 索引）上。
- 对 ADV：规则引擎的不变量（输出合法性、单调性、无死锁）写成 proptest；**收缩预算显式化**（默认 cases=256；收缩超时配 Config 字段，勿凭记忆写默认值），防止 issue #145 类"收缩跑不完"在门里拖死 CI；反例文件入库版本化，作为最便宜的确定性回放资产。

### ④ flaky 判定的统计口径（给门判据）
- 两级判定：**候选** = 同提交内失败→重跑通过（nextest flaky 信号，成本零）；**高置信** = 最近 20–30 次运行窗口内 ≥2 次结果翻转（窗口大小与翻转次数在 ADV 数据上标定后冻结，先量后改）。重跑 K 次全过只证"非必然失败"，单次重跑通过是低置信证据，不得据此豁免。
- 处置：候选→报告单列；高置信→进版本化 quarantine 名单（仍跑、不挡门、限期处理）；窗口内无翻转自动解除。nextest 的 flaky 默认按 pass 计，门层必须重标。
- 锚参照：Google 约 1.5% 运行 flaky / 近 16% 测试有 flakiness（Micco 2016；2026-09 转引复核）。

### ⑤ 测试报告统一格式（JUnit/JSON）消费与聚合
- 内部一等公民：nextest `--message-format json` 事件流（逐测试逐尝试 + stdout 定位）；JUnit 仅对外。聚合键 (file, classname, name)；方言差异（nextest 的 `<flakyFailure>` 嵌套、pytest xunit2、surefire）必须显式适配而非单形状硬解析。
- 多分片：blob 式——每分片自包含局部报告（vitest `--merge-reports` 形状），合并器只做并集去重；不搞中央实时汇总。
- 工程要点：时长字段单位归一；stdout/system-out 截断策略；flaky 状态字段端到端保留（不许在合并时丢）；报告 schema 用 insta 固样例防漂移。

---

## 七、2025–2026 前沿信号
1. **flaky 治理产品化加速**：trunk.io（2025-04 检索锚）自动检测+quarantine+修复追踪一体化；厂商口径称受 flaky 影响团队占比上升（RNF 2025-07：26%，厂商调查，量级参考）。
2. **RTS 学术活跃度未衰减**：2025 年 ACM 论文仍在引用并扩展 Predictive Test Selection 线（Weeraddana & Chandra 2025）。
3. **JUnit flaky 语义成为生态事实标准**：nextest 的 `<flakyFailure>` 嵌套被第三方报告服务消费（2025 检索：Currents/test-wisdom 类工具按此计价重试）。
4. **托管 diff-coverage 的口径坑在社区高频曝光**：Codecov 2025-12 路径重组致 0% 帖——本地实现对齐行归属的自测价值上升。
5. **MC/DC 在 LLVM 侧成熟、Rust 侧缺席**：clang 14+ 有 `-fcoverage-mcdc` 一线支持，rustc 尚未暴露（watch：跟踪 rustc 覆盖率提案）。
6. **pytest-xdist `worksteal`（3.1 起）成为长尾优化默认推荐**，"动态领取"思路可平移到 nextest 分片之上的本地 runner。
7. **sanitizer 数字锚的文档仓库（google/sanitizers）已归档**：数字口径以 LLVM 官方文档为准，旧 wiki 只作历史参考。

## 八、Top-3（对本域最高杠杆）
1. **nextest 机器可读输出 + JUnit flaky 语义（D7-01/02）**：xtask-gates 执行层的地基——事件流为一等公民、flaky 端到端保留、分片 blob 合并；一次设计三处受益（门、报告、flaky 名册）。
2. **diff-coverage 棘轮口径（D7-11/12 + 重点回答②）**：分子分母 + 豁免行单列 + 双口径并行，直接继承"绿=SKIP 不算绿"的教训；以 diff-cover 为准绳复算对齐，实现风险可控。
3. **proptest shrinking + regressions 回放（D7-23）**：规则引擎属性测试的最小反例机制自带"确定性回放"资产，与 test-replay 天然咬合；显式收缩预算是唯一要盯的坑。

---
*档案说明：登记表 D7.jsonl 共 27 行（D7-01..D7-24 与 D7-26..D7-28；D7-25 已并入 D7-02，编号跳空保留），与本文 27 个小节一一对应。depth 含义：深=当日检索复核；中=训练知识+检索交叉；浅=训练知识未复核。*
