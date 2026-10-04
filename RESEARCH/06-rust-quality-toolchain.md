# 06 · Rust 质量工具链与工程纪律（外部调研）

- 日期：2026-10-03 ｜ 调研窗口：2025-01 ~ 2026-10 信息优先
- 方法：WebSearch 为主（17 次检索）。搜索噪声较大处已只保留可交叉印证的机制性事实；查不到原文的数字一律标「待验证」。
- 口径约定：「机制」= 该工具实际怎么干活（数据流/判定方式/输出形态）；「营销话术」= 官网口号，不作为采信依据。所有结论带适用范围，不做绝对化表述。
- 服务对象：unified-rx-mcp 现有门禁（god 棘轮 / ruff 逐规则棘轮 / 快照金样 / perf_lock / 25 本地门 + CI 19 job）→ Rust 化。Python 侧对应关系在每节末标注。

---

## 0. 速览总表

| 对象 | 一行定位 | 档位 |
|---|---|---|
| cargo-nextest | 每测试一进程的并行测试 runner，分片/重试/JUnit 内建 | 吸收 |
| insta + cargo-insta | 快照测试事实标准，review 工作流 + inline 快照 | 吸收 |
| proptest | 属性测试 + 自动收缩 + 回归用例入库 | 吸收 |
| cargo-llvm-cov | llvm 覆盖率事实标准，lcov/cobertura + fail-under-lines | 吸收 |
| cargo-mutants | 变异测试：验证测试真的能抓 bug | 有界 |
| criterion | 墙钟基准事实标准（本地档） | 吸收 |
| Gungraun（原 iai-callgrind） | 指令数基准，负载无关，Linux/Valgrind only | 有界 |
| Bencher | 可自托管的持续基准服务 + 阈值告警 | 吸收 |
| cargo-semver-checks | 基于 rustdoc JSON 的 breaking change 静态检查 | 有界 |
| cargo-public-api | 公共 API 清单/快照/diff，可做 API 面棘轮 | 吸收 |
| clippy + [workspace.lints] + clippy.toml | 官方 lint 主力，阈值可配，无棘轮 | 吸收 |
| dylint | 自定义 lint 打成动态库，可挂 clippy 驱动 | 有界 |
| cargo-hack | feature powerset / each-feature 编译矩阵 | 有界 |
| cargo-deny + RustSec | 依赖审计（advisories/licenses/bans/sources） | 吸收 |
| workspace 先例（ripgrep/tokio/rust-analyzer/uv/gitoxide/Feldera） | 小 crate 拆分 = 编译时间 + 结构纪律 | 吸收（思想） |
| sccache | 跨 CI/本机编译缓存，绕开 GHA 10GB 限制 | 吸收 |
| Windows 链接器（rust-lld/Wild 现状） | Windows 无 mold/Wild，rust-lld 是唯一实际选项 | 有界 |
| 现成「文件/函数/成员数」门 | **不存在**（clippy too_many_lines 是绝对阈值且 pedantic） | → god_gate 自研 |

---

## 1. 测试与覆盖

### 1.1 cargo-nextest
- 定位：每测试独立进程、按二进制并行调度的测试 runner，cargo test 的直接上位替代。
- 值得抄的机制：
  1. **分片内建**：`--partition count:N/idx` / `--partition hash:<bucket>/N`，配 timing 权重让各 shard 负载均匀——对应现有 CI 多 job 的测试分摊，不用自研调度。
  2. **重试 + flaky 标记**：profile 级 `retries`（含 backoff），重试后通过的用例在输出和 JUnit（`flaky` 属性）里单独标出——「绿但 flaky」不再伪装成稳定绿，与「绿=SKIP 不算绿」同一精神。
  3. **JUnit 报告 + nextest-metadata**：机器可读结果（失败帧、耗时），`nextest-metadata` crate 可让 xtask 直接解析分片计划——门禁吃结构化输出不用正则刮日志。
- 档位【吸收】。理由：CI 可判定输出、分片、flaky 语义三项都是现有门禁已有思想的现成实现；2025-12 有真实采用例（dbt fusion 在其 CI profile 里加 nextest retries）。
- 第一步动作：`cargo nextest run` 设为默认测试入口；`.config/nextest.toml` 定义 `ci` profile（retries=1 + junit + partition）与 `local` profile（retries=0）；xtask 门禁读 JUnit。
- 许可证：MIT / Apache-2.0（双许可，可抄代码）。
- 链接：https://nexte.st/docs/configuration ｜ https://nexte.st/docs/features/partitioning
- Python 侧对应：替代 pytest-xdist 分片手搓 + 自研失败报告解析。

### 1.2 insta / cargo-insta
- 定位：快照（approval）测试事实标准：断言值对 `.snap` 参照文件，配 `cargo-insta` 交互式 review。
- 值得抄的机制：
  1. **pending 快照不静默通过**：新快照落成 `.snap.new`，CI 用 `--unreferenced=reject` / `INSTA_FORCE_UPDATE` 显式控制——「快照只能显式接受」这一点正好接金样门的 bless 语义（对比现有 Python 金样门手工 bless 脚本）。
  2. **inline 快照**：小输出直接写在测试源内（`assert_snapshot!`），大输出走文件；1.31+ 有 instafmt 重排 inline（还不如 rustfmt 稳，大块仍建议文件快照）。
  3. **已知坑**：历史上 `cargo insta test` 在有 pending 快照时 exit 0（issue #222，2022），CI 里必须显式加判据而不是裸跑——这恰是「判据走真路径」要盯的点（待验证最新版是否已改）。
- 档位【吸收】。理由：现有快照/金样门的 Rust 侧直接换血，review 工作流（accept/reject diff）比自研 bless 体验完整。
- 第一步动作：解析器/格式化输出类模块先接 insta；CI 规定 `cargo insta test --unreferenced=reject`，pending 不清零不许合并。
- 许可证：MIT。
- 链接：https://insta.rs/docs ｜ https://github.com/mitsuhiko/insta

### 1.3 proptest
- 定位：属性测试：随机生成输入 + 失败自动收缩 + 把最小失败用例持久化到 `proptest-regressions/` 文件。
- 值得抄的机制：
  1. **回归用例入库**：失败的生成用例自动写入 `proptest-regressions/*.txt` 并随代码提交——「否证要留档」的机制化版本，比人工记录强。
  2. **Strategy 组合**：输入按类型组合生成，适合解析器/调度器这类「输入空间大、不变式明确」的模块。
- 档位【吸收】。理由：成本低（依赖一个 crate），只对核心逻辑开，不必全仓铺开；corrode.dev Rust Tool Index（2026-09 更新）仍列为属性测试首选。
- 第一步动作：挑 1-2 个纯函数核心模块（解析/编码）写属性测试，regression 文件提交并纳入门禁（文件出现新条目 = 有新否证，需人工归档）。
- 许可证：MIT / Apache-2.0。
- 链接：https://docs.rs/proptest ｜ https://altsysrq.github.io/proptest-book/

### 1.4 cargo-llvm-cov
- 定位：基于 llvm-tools（llvm-cov）的覆盖率事实标准，2025 年主流推荐（Zellij 等项目明确弃 tarpaulin/kcov 转向它）。
- 值得抄的机制：
  1. **`--fail-under-lines`**：覆盖率直接参与 CI 判定，配一个入库的基线值即可做「覆盖率只准升」的简易棘轮（与 ruff 逐规则棘轮同构：数字入库、只准单向）。
  2. **多格式**：lcov/codecov/cobertura/html，自托管时 html 落盘即可，不必依赖 Codecov 服务。
- 档位【吸收】。理由：tarpaulin 是 Linux-only 且维护疲态、GoCover 型替代品无优势；llvm-cov 是编译器自带数据源，Windows 可用。
- 第一步动作：本地 `cargo llvm-cov --html` 养习惯；CI 档 `cargo llvm-cov --lcov --fail-under-lines <基线>`，基线入库、只准升。
- 许可证：MIT / Apache-2.0。
- 链接：https://github.com/taiki-e/cargo-llvm-cov

### 1.5 cargo-mutants（附加）
- 定位：变异测试——往源码注入小变异（去掉边界检查、翻转布尔等）重跑测试，验证测试真的能抓 bug。
- 值得抄的机制：按 crate 圈定范围跑（库 crate 全量 / 二进制 crate `test_workspace` 模式）；社区实践（dora-daemon 等）都是「只对关键 crate 变异」，全仓跑是几十倍测试时长，不现实。
- 档位【有界】。理由：本质是「测试质量的自检」，开销大，只配进夜间档；且它测的是测试，不直接当门禁。
- 第一步动作：夜间档对 1 个核心 crate 跑，先看 killed/missed 比例作为测试盲区报告，不设门。
- 许可证：MIT。
- 链接：https://github.com/sourcefrog/cargo-mutants

---

## 2. 基准与性能棘轮

### 2.1 criterion
- 定位：统计化墙钟基准 harness（去噪、回归检测、HTML 报告），Rust 基准事实标准。
- 值得抄的机制：
  1. 本地开发档友好：多次采样 + 统计判定，Windows/macOS/Linux 全平台。
  2. 输出可被 Bencher 等持续基准层直接吃（criterion adapter）——本地工具与 CI 棘轮解耦。
- 档位【吸收】（本地档）。理由：CI 墙钟在共享/自托管 runner 上噪声大，criterion 的「回归检测」在 CI 里不可靠——它只做本地测量，判定交给 Bencher 层。
- 第一步动作：`benches/` 目录 + criterion；只给热路径 crate 写基准。
- 许可证：MIT / Apache-2.0。
- 链接：https://github.com/bheisler/criterion.rs ｜ https://bheisler.github.io/criterion.rs/book/

### 2.2 Gungraun（原 iai-callgrind）
- 定位：基于 Valgrind/Callgrind 的**指令数**基准：one-shot、负载无关、跨运行稳定——CI 上判性能回归的「硬尺子」。
- 值得抄的机制：
  1. 指令数是确定量：不受机器负载/频率影响，天然适合「性能棘轮」（对比现有 perf 门靠 perf_lock 机器独占 + 多次采样求稳）。
  2. 官方专章 "Detecting Performance Regressions" + iai-callgrind → Gungraun 迁移指南（2025 年改名，注意文档里两个名字并存）。
- 档位【有界】。理由：**依赖 Valgrind，Linux-only**——Windows 开发机跑不了，只能进 Linux CI 档；Windows 主开发环境的性能判定仍要靠 Bencher 墙钟 + 机器锁定。
- 第一步动作：若 CI 保留 Linux job，对 3-5 个热点函数建 Gungraun 指令数基线；否则标「待验证：Windows 等价物」暂缺。
- 许可证：开源（Apache-2.0/MIT，待验证具体双许可组合）。
- 链接：https://gungraun.github.io
- Python 侧对应：perf_lock 的「机器独占」思想在这里换成「换一把不受负载影响的尺子」。

### 2.3 Bencher（self-hostable）
- 定位：可自托管的持续基准平台：`bencher run` 收集基准结果 → 存档按分支/提交 → 阈值（threshold）触发告警/失败。开源仓库可自部署。
- 值得抄的机制：
  1. **branch baseline + threshold**：对每个基准设上界（如 +10%），超标即 CI 失败——这就是「性能棘轮门」的现成服务化形态，且基线随主干滚动，不用手工维护数字。
  2. **criterion adapter**：`bencher run` 直接解析 criterion 输出，本地工具链与 CI 判定层分开。
  3. 自托管后数据不出内网，符合本项目「判据本地可复现」的纪律。
- 档位【吸收】。理由：现有性能门（perf_lock + 基线）在 Rust 侧最省力的等价物；「少而硬」里性能这一环直接落到它 + criterion。
- 第一步动作：验证自托管部署（Docker）与 criterion 对接，先对 1 个热路径 crate 开 threshold；验证许可条款（待验证：仓库 license 具体文本）。
- 许可证：开源可自托管（具体许可证文本待验证；专有云版只抄思想）。
- 链接：https://bencher.dev ｜ https://bencher.dev/docs/

---

## 3. API 纪律（API 面棘轮）

### 3.1 cargo-public-api
- 定位：枚举 crate 公共 API（基于 rustdoc JSON）并支持两 commit 间 diff——API 面棘轮的最直接数据源。
- 值得抄的机制：
  1. **快照 diff 即棘轮**：`cargo public-api` 输出入库为 blessed 快照，CI diff；新增公共项 = API 面变大 = 门禁亮灯（对内 workspace crate 比 semver-checks 更实用：内部 crate 没有「发布」语义，关心的是「面有没有偷偷变大」）。
  2. 输出是精确文本清单，可按 crate 逐个门控。
- 档位【吸收】。理由：与 god 门「只准减」完全同构，数据源现成；对 workspace 内部 crate 的适配性优于 semver-checks（待验证 workspace 批量跑的命令面）。
- 第一步动作：对每个对外 crate 生成 blessed API 清单入库；xtask 门「diff 非空即失败，扩大需显式重录基线」。
- 许可证：MIT。
- 链接：https://github.com/Enselic/cargo-public-api

### 3.2 cargo-semver-checks
- 定位：基于 rustdoc JSON 的 breaking-change 静态 lint 集合（规则式判 semver 违例），有官方 GitHub Action；2026-08 Kobzol 正在把它接进 rustc 自身 CI（Sovereign Tech Fellowship）——工具可信度的强背书。
- 值得抄的机制：规则库（major/minor 破坏模式的枚举）本身是「API 纪律」的知识清单，值得通读作为设计输入。
- 档位【有界】。理由：核心价值在「对外发布不破坏下游」——本项目以内部 workspace 为主，破坏面小；workspace 全量跑有 target-metadata 局限（有项目为此打补丁绕行，见 lspf 仓库案例）；先让 cargo-public-api 承担主门。
- 第一步动作：暂不接 CI；在真正拆出对外发布 crate 时启用（配合 RFC 3516 的 `pub` 依赖标注，Cargo 1.83+ 稳定，可区分「泄漏进公共 API 的依赖」）。
- 许可证：MIT / Apache-2.0。
- 链接：https://github.com/obi1kenobi/cargo-semver-checks ｜ https://rust-lang.github.io/rfcs/3516-public-private-dependencies.html

---

## 4. Lint 与依赖纪律

### 4.1 clippy + [workspace.lints] + clippy.toml
- 定位：官方 lint 主力（750+ lints，Trail of Bits 2025 统计口径）。
- 值得抄的机制：
  1. **`[workspace.lints]`**（Cargo 1.74+）：lint 配置集中在 workspace 根，成员 `lints.workspace = true` 继承——单文件管全仓，且存在「强制所有成员继承 workspace lints」的第三方检查 crate。这是 ruff 逐规则棘轮的 Rust 对应物（lint 清单入库）。
  2. **clippy.toml 阈值可配**：`too-many-arguments-threshold`、`too-many-lines-threshold`、`type-complexity-threshold`、`enum-variant-size-threshold`、`large-error-threshold` 等（具体清单待验证）——把「复杂度预算」显式写进仓库。
  3. `clippy::too_many_lines`（pedantic，默认 ~100 行）：唯一现成的函数级长度 lint，但是**绝对阈值、非棘轮**，且对「膨胀是渐进的」无感知。
- 档位【吸收】。理由：CI `-D warnings` + workspace lints 是底线，零自研。
- 第一步动作：workspace 根 `[workspace.lints]` 定死全组；`cargo clippy --workspace --all-targets -- -D warnings` 进 CI 快档；clippy.toml 记录阈值。
- 许可证：Apache-2.0/MIT（工具链随 Rust）。
- 链接：https://doc.rust-lang.org/clippy/ ｜ https://doc.rust-lang.org/cargo/reference/resolver.html#workspace-lints（待验证准确 URL）

### 4.2 dylint
- 定位：Trail of Bits 的自定义 lint 框架：lint 编译成动态库，`cargo dylint` 加载运行，可与 clippy 驱动组合。
- 值得抄的机制：
  1. lint 独立成库、按 collection 组织，配 dylint-link 安装；可挂在 clippy 驱动上一遍跑。
  2. 实战案例生态（solana-lints 等）证明「项目私有 lint」可行。
- 档位【有界】。理由：工具链耦合重（lint 库要跟 rustc/rustc-private 内部 API 版本走，toolchain 升级即重编译/可能重写），维护成本对自研门禁不划算；且「成员数/文件行数」这类结构性检查用 syn 扫描即可，不需要 rustc 内部 API。若未来要语义级私有 lint（如「禁止某模式调用」）再评估。
- 第一步动作：不引入；xtask 用 syn 实现结构检查（见 §6）。留档：dylint 作为后备方案。
- 许可证：Apache-2.0/MIT（待验证）。
- 链接：https://github.com/trailofbits/dylint ｜ RFC 3808 register-tool（未来 lint 注册方向的信号）：https://github.com/rust-lang/rfcs

### 4.3 cargo-hack
- 定位：feature 组合矩阵检查：`--each-feature` / `--feature-powerset`（可 `--depth` 限深）验证 feature 可加性。
- 值得抄的机制：powerset 组合爆炸的处理方式——大项目放夜间档/仅 PR，配 `--depth` 控制组合数。
- 档位【有界】。理由：单二进制+少量 crate、feature 面小的项目收益低；等 feature 面长大再启用。
- 第一步动作：先 `--each-feature` 进夜间档，powerset 待 feature 变多再开。
- 许可证：MIT / Apache-2.0。
- 链接：https://github.com/taiki-e/cargo-hack

### 4.4 cargo-deny + RustSec
- 定位：Cargo.lock 级依赖审计四件套：advisories（RustSec 库）、licenses、bans、sources，单 `deny.toml` 配置。
- 值得抄的机制：
  1. advisories 门：RustSec 通告直接变 CI 失败（可配 `--deny warnings` 的 cargo-audit 等价行为）。
  2. bans/sources 门可做「依赖白名单 + 只增需审批」——依赖数棘轮的现成骨架。
  3. crates.io 已上线基于 RustSec 的 Security 页，与 CI 工具互补。
- 档位【吸收】。理由：现有 CI 已有 cargo-deny/zizmor 把尺（质量红线 8），Rust 侧沿用同一工具即零迁移成本。
- 第一步动作：`deny.toml` 从 advisories + licenses 起步，bans 白名单随后；每次 CI 跑。
- 许可证：MIT / Apache-2.0。
- 链接：https://embarkstudios.github.io/cargo-deny/ ｜ https://rustsec.org

---

## 5. 大项目 crate 分层先例与编译时间

### 5.1 workspace 结构先例（合并叙述，均为「思想吸收」）
- **ripgrep**：根是薄二进制 `rg`，`crates/` 下按能力拆（grep-matcher trait 抽象、grep-searcher、grep-regex、grep-printer、grep-cli，加 ignore/globset/walkdir）——globset 被 bat 复用证明「组件可独立成活」；拆分让 CLI 与库的重建成本分离。
- **tokio**：数十个小 crate（tokio-util、tokio-stream、tokio-macros、tokio-test…），主 crate 按 feature 门控——「运行时聚合 + 生态细分」的结构。
- **rust-analyzer**：约 60 crate（parser/syntax/hir/hir-ty/ide…分阶段分层），matklad 既是 xtask 模式作者也是该结构的实践者——「编译阶段分层 = crate 分层」。
- **uv（Astral）**：大型 workspace，按功能域拆（resolver/distribution/installer 等）；注意 2026-10 新闻：OpenAI 收购 Astral，长期组织归属存变数（不影响结构先例价值）。
- **gitoxide**：自称「workspace of small crates rather than one monolith」，gix 是薄入口。
- **Feldera**（最详细案例，2025-05）：**把 workspace 拆到约 1000 个 crate，编译时间 30 分钟 → 2 分钟**（数字来自其博客标题口径，原文细节待验证）。机制：细粒度 crate → rust-analyzer/cargo 增量只重编触及的最小集合 + crate 间天然并行。
- 共同机制提炼：① 薄入口二进制 + 库 crate 全在 `crates/`；② trait 抽象 crate（如 grep-matcher）隔离实现；③ `[workspace.dependencies]` 统一版本；④ 拆分的收益主要是**增量和编辑器响应**，不是全量编译。
- 档位【吸收（思想）】——各仓库结构事实来自其公开仓库布局，颗粒度数字（如 rust-analyzer 60 crate）为约数。

### 5.2 编译时间管理（Windows 特殊性重点）
- **链接器现状（2025-2026，Windows 是短板）**：
  - mold、Wild 均为 ELF/Linux 生态：Wild 在 2026-10 的 20 轮对比中全部快于 mold（dev.to，2026-10，约数），但 **Windows/PE 无生产就绪支持**（待验证：Wild COFF 路线图）。
  - Rust 1.90（2025-09 稳定）把 **LLD 设为 x86_64-unknown-linux-gnu 默认链接器**——这是 2025 年的大新闻，但 **Windows MSVC 目标默认仍是 link.exe**，未被同批更换。
  - Windows 实际选项：`.cargo/config.toml` 对 `x86_64-pc-windows-msvc` 设 `linker = "rust-lld"`（lld 的 MSVC flavor）；收益小于 Linux 侧，属「可用、非免费午餐」。
- **sccache**：Mozilla 编译缓存，后端可 S3/GCS/Azure/Redis/本地磁盘，可自托管——绕开 GHA **10GB/仓** 缓存上限导致的驱逐冷miss（2026-06 已有 issue 记录该痛点）。自托管 runner + 本地磁盘/MinIO 是 2025 年标准做法。gotcha：tag ref 上别开 sccache（缓存污染）。
- 其余机制（常识层，无需引用）：dev profile `debug = "line-tables-only"`；`cargo check` / `clippy --no-deps` 做编辑循环；target 放快盘；`[workspace.dependencies]` 收敛版本减少重复编译单元。
- 第一步动作（Windows 档）：① rust-lld 进 config.toml 实测 link 时间；② 自托管 CI 用 sccache + 本地磁盘缓存；③ 测一次「20 crate 拆分 vs 单 crate」的增量编译差，作为 crate 粒度决策的本地证据（先量后改）。

---

## 6. 上帝对象/模块膨胀防线 → god_gate Rust 化

### 6.1 现成方案盘点（结论：不存在等价物）
- clippy：函数级 `too_many_lines`（pedantic、绝对阈值、默认关）、`too_many_arguments`、`type_complexity`、`result_large_err` 等——**无文件级长度 lint、无成员数 lint、无棘轮语义**（clippy 的立场：文件长度是风格问题）。
- 通用棘轮工具（Betterer、ratchet 类 GHA bot）：语言无关基线棘轮思想可借鉴，但无 Rust 结构感知（数不了成员/函数）。
- rustfmt：不强制长度。cargo-deny：管依赖不管代码结构。
- **差距结论**：「文件行数 + 函数行数 + 成员数 + 每 crate 公共 API 面」四维棘轮 + 只准减 + 金丝雀，生态里没有现成工具——god_gate 必须自研，但组件成本低（行数纯文本统计；结构计数用 syn 解析）。

### 6.2 xtask vs build.rs vs CI step（取舍表）

| 载体 | 何时跑 | 失败形态 | 本地/CI一致性 | 适合度 |
|---|---|---|---|---|
| build.rs | **每次编译** | 编译错误（污染构建输出） | 高 | ✗ 门禁不该在构建热路径 |
| CI step（裸 shell/脚本） | 仅 CI | CI 红灯 | ✗ 本地难复现，双实现漂移 | ✗ 单独用不行 |
| **xtask** | 按需 `cargo xtask gate` | 结构化退出码 + 报告 | ✓ 同一份 Rust 代码 | ✓ 主宿主 |
| just（薄入口） | 按需 | 透传 | ✓ | ✓ 仅作 1 行别名层 |

- 定位：xtask（matklad 模式）= workspace 内一个独立 crate，用 cargo 跑，贡献者零安装成本；ratatui 2025 年已从 cargo-make 迁到自写 xtask（案例）。just 是「不是构建系统」的命令薄壳，适合 `just ci` → xtask 的分层。
- Rust 化 god_gate 设计要点（从现有 Python god 门迁移）：
  1. **数据源走真路径**：从 `cargo metadata` 枚举 workspace members 扫 `crates/**/*.rs`，不维护文件白名单（防白名单漂移作弊）。
  2. **三层计数器**：行数（文本级，零依赖）→ 符号结构（syn：函数数/成员数/单函数体行数）→ API 面（可复用 cargo-public-api 输出当数据源，不自造）。
  3. **基线文件入库**（JSON/JSONL）：键 = 相对路径/符号路径，值 = 上限；新值 > 基线 → fail；新值 < 基线 → `cargo xtask god-gate --tighten` 生成基线收缩 diff，人工提交（「棘轮只能人拧紧」，防基线被脚本悄悄改）。
  4. **金丝雀语义**：对新型膨胀（如新维度）先记录不拦截，观察 N 天后转硬门——迁移现有金丝雀语料的期望值锚定做法。
  5. **时间戳否证**：基线文件带 last-tightened 时间戳，与「否证标签是时间戳」纪律一致。
  6. 与 clippy 的分工：clippy 阈值管**绝对红线**（如单函数 >300 行直接红灯，不需要基线），god_gate 管**相对棘轮**（当前水平只准减）。两层别合并——绝对红线是纪律底线，棘轮是持续改进。
- 档位【自研（xtask 内建门）】，生态无替代。

---

## 7. 重点回答

### ① 测试/基准/覆盖默认档组合（少而硬）
| 层 | 默认档 | 理由（一句话） |
|---|---|---|
| 测试 runner | nextest（local profile + ci profile） | 分片/重试/flaky 标记/JUnit 全内建，替代三件自研 |
| 快照/金样 | insta，CI `--unreferenced=reject` | 金样门 bless 语义的现成实现，pending 不静默过 |
| 属性测试 | proptest，仅核心纯函数模块，regressions 入库 | 否证留档机制化，成本低 |
| 覆盖 | cargo-llvm-cov `--fail-under-lines` 基线入库只准升 | 编译器数据源 + Windows 可用 + 判据可入库 |
| 基准 | criterion（本地）+ Bencher 自托管 threshold（CI 棘轮） | 墙钟归本地，判定归持续基准层 |
| 深精度基准 | Gungraun 仅 Linux CI（若保留 Linux job） | 指令数不受负载影响，Windows 跑不了 |
| API 面 | cargo-public-api blessed 快照棘轮 | 对内部 crate 比 semver-checks 贴合 |
| Lint/依赖 | clippy -D warnings + [workspace.lints] + clippy.toml 阈值；cargo-deny 每次 CI；cargo-hack 夜间 | 零自研底线 + 依赖白名单棘轮骨架 |
| 结构门 | xtask 自研 god_gate（§6.2） | 生态空白，必须自研但便宜 |
| 夜间档 | cargo-hack powerset + cargo-mutants 单 crate | 贵的检查放无人时段 |

### ② crate 粒度与编译时间（Windows）
- 粒度策略：**学「薄入口 + crates/ 细分 + trait 隔离层」结构，但不学 Feldera 一步到 1000 crate**——从 20-40 个功能域 crate 起步，用 god_gate 的 per-crate 行数上限天然约束粒度（单 crate 超限 = 该拆了），拆分决策用实测增量编译时间驱动（先量后改）。crate 分层的额外收益是把「模块边界」变成编译器强制的边界——反上帝对象的物理屏障。
- Windows 特殊性：① 链接器红利吃不到 Linux 级别（1.90 LLD 默认只给 linux-gnu；Windows 仍 link.exe，rust-lld 需手动配且收益小）；② mold/Wild 不可用（ELF-only）；③ GHA 10GB 缓存上限在 Windows target 体积下更易触顶——自托管 runner + sccache 本地磁盘/MinIO 是唯一稳态解；④ dev profile 降 debuginfo（line-tables-only）在 Windows 收益比 Linux 更明显（PDB 生成贵）。
- 适用范围声明：编译时间数字（Feldera 30→2min、Wild vs mold 毫秒差）均为**其各自仓库/机器口径**，本项目的可复现基线必须自测。

### ③ god 门 Rust 化设计要点与现成方案差距
- 差距一句话：clippy 只给绝对阈值（函数级、pedantic），没有文件级/成员数/棘轮语义——四维棘轮在 Rust 生态是空白，自研是唯一路径，载体选 xtask（理由见 §6.2 表）。
- 设计要点浓缩：cargo metadata 定范围 → syn 结构计数 + 文本行数计数 → 基线 JSON 入库只准减 → 收缩需人工提交带时间戳 → 金丝雀先记录后拦截 → 与 clippy 绝对红线分层不合并。预估规模：数百行 Rust（syn + serde + tempfile），xtask crate 不参与主构建图。

---

## 8. 2025–2026 前沿信号

1. **Rust 1.90（2025-09）**：LLD 成为 x86_64-unknown-linux-gnu 默认链接器——官方开始把「快链接」当默认体验；Windows 未同步（linux-gnu 专属）。
2. **Wild linker**（David Lattimore）：2026-10 基准 20/20 快于 mold（Linux ELF）；Windows/PE 支持未见生产就绪（待验证）——短期不影响本项目，长期值得跟。
3. **iai-callgrind → Gungraun 改名**（2025）：文档/仓库双名并存期，引用时注意。
4. **cargo-semver-checks 进 rustc CI**（2026-08，Kobzol / Sovereign Tech Fellowship）：语言级背书，规则库持续壮大。
5. **Feldera 千 crate 案例**（2025-05）：「拆小 crate = 增量编译数量级改善」的 quantified 证据。
6. **GHA 10GB 缓存上限痛点实锤**（2026-06 issue）：Rust 仓缓存驱逐/冷 miss，社区答案收敛到 sccache + 外部后端/自托管。
7. **RFC 3808 register-tool**（进行中）：第三方 lint（dylint 等）注册进工具链生态的方向信号——若落地，私有 lint 的集成成本会降，届时重评 dylint 档位。
8. **crates.io Security 页**（RustSec 数据）：依赖审计从 CI 工具向平台层下沉。
9. **dbt fusion（2025-12）在 CI profile 里加 nextest retries**：nextest 重试语义在大仓真实采用的旁证。
10. **OpenAI 收购 Astral**（2026-10 新闻）：uv/ruff 的组织归属变化；结构先例价值不变，但「跟 uv 学工程实践」的持续性需观察。

---

## 9. Top-3

1. **xtask 内建 god_gate 自研**：生态确认空白（clippy 只有绝对阈值、无文件级/成员数/棘轮），这是现有质量体系里唯一无法买到的部分；设计要点已给（§6.2），载体 xtask + just 薄入口。
2. **nextest + insta + cargo-llvm-cov 三件套 = 测试/快照/覆盖默认档**：现有门禁（分片、金样 bless、覆盖率棘轮）三个自研部件的现成等价物，CI 判据全部结构化可入库。
3. **Bencher 自托管接 criterion = 性能棘轮的服务化**：branch baseline + threshold 直接替代 perf_lock 手工基线；Windows 侧墙钟噪声靠「本地 criterion + CI 阈值上界」消化，Linux 档可加 Gungraun 指令数硬尺。

---
*引用均为检索日期可达的公开来源；标注「待验证」的数字/许可条款在采纳前需一次原文核验。*
