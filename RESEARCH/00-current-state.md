# 00 · 现状盘点：unified-rx-mcp（D:\KF\unified-rx-mcp → GitHub bfxh/ADV）

> 盘点日期 2026-10-03，只读盘点未改仓。当前分支 `docs/handshake-ledger-resolved`，HEAD `7db9ee8`（S218，2026-10-02）。
> 所有数字均实测（命令/行号随条附注）；标注"推测"的是未复核推断。

## ① 一句话现状

一个纪律密度极高的 Python 单体 MCP 服务器（89 工具 / 53 工具模块 / registry 单一裁决点）+ 已做了一半的 Rust 零依赖引擎（20,168 LOC + 3,883 测试 LOC、11 个 bin），靠 158 条 S 编号轮账和 ~16 个自建 CI 门禁驱动演进；**工程方法（门禁/棘轮/账本/判定档）远超平均水平，但载体本身正被三件事拖住：版本锁步断档、活体部署漂移、文件级上帝对象祖父化**——这正是"全新重写为 Rust 主体"的依据所在。

规模实测：
- Python 侧：`server.py` 569 行 + `registry.py` 753 行 + `tools/*.py` 53 个模块 + 顶层 `hub_*.py` 12 个模块；全仓 .py 53,749 行（含 bench/manual_snaps ~4,600 与 tests）。
- Rust 侧：`rust/src/` 30 个文件 20,168 行；`rust/tests/` 3,883 行；11 个 bin（rx-mcp/taint/fs/ide/search/semantic/scan/audit/appops/sys/svc，`rust/Cargo.toml`）；edition 2024；**[dependencies] 恒空**（红线注释指向 spec/VULN-HUNTING.md 五、spec/LIBRARY-POLICY.md）。
- 工具数：`rg -c '@tool\(' tools/*.py` = **89 个**（41 个文件）；`registry.py:187` 声明式注册（name→handler/group/schema/requires_auth）。
- 测试：tests/ 154 个文件、1,188 个 test 函数（`rg -c "def test_"`）。
- spec/ 判定档 46 个文件；scripts/ 门禁脚本 43 个；.github/workflows 5 个。

## ② 问题清单（每条带证据）

### P1 版本锁步断档：36 个 minor 版本没打 tag
- `server.py:40` `SERVER_VERSION = "2.94.0"`；`rust/Cargo.toml` version = "2.94.0"（两处人工同步）。
- 最新 git tag = **v2.58.0**（2026-09-14，`git log -1 v2.58.0`）；共 70 个 tag（`git tag -l | wc -l`）。
- ⇒ 2026-09-14 至今 ~19 天、2.58→2.94 共 36 个 minor 版本无 tag；而 `.github/workflows/release.yml:11` 只认 `tags: ['v*']` ⇒ **期间零正式发布**。
- 仓内其实已有对账工具：`server.py:338`（VERSION_TAG：SERVER_VERSION↔最新 tag 对账，S91）、`server.py:401-474`（EXE_TAG：11 个 exe `--version` 对账）——纪律工具在，tag 动作本身断了。
- S218（`spec/ROUNDLOG.md:2054` 起）证明后果：活体宿主在跑 2026-09-20 时代的 v2.76.0 副本。

### P2 活体部署漂移：日常驾驶舱跑的是孤岛混合体（S218 实测）
- 证据：`spec/ROUNDLOG.md:2054-2078`——ZCode 插件缓存 `.mcp.json`（2026-09-10）指向 `D:\rj\MCP\server.py` v2.76.0 + tools/ 48 文件（仓 main 是 53），git origin 指向已不存在的 `D:\开发\unified-rx-mcp`（孤岛）。
- 后果：S214 瘦身 / D-2 回包溢出 / D-3 熔断 / S217 UTF-8 硬化**全不在宿主生效**；`~/.ADV/stats.jsonl` 15 万条日常增长是门禁/测试 embedded 流量，不是宿主流量——遥测口径也被污染。
- 教训（进新项目）：**部署路径必须单源**（插件直接指仓内构建产物），版本/落点漂移要有门禁。

### P3 上帝对象门被基线祖父化部分架空：文件级棘轮停走
- `god.gate.json`：max_file_lines=800、max_fn_lines=120（fn 硬阈 `fn_hard_threshold: true`，S170 起）、max_type_members=24、拆函数换行数合法交换 ≤10%。
- 但 `god-baseline.json`（382 条）里实测超 800 的在册文件：`rust/src/appaudit.rs` 1203、`rust/src/sysinfo.rs` 982、`rust/src/scan.rs` 956、`bench/swe_p3.py` 861、`rust/src/ide.rs` 826（另 `rust/src/pyast/lex.rs` 798 逼近）。
- 即：函数超长不接受祖父化（硬阈），**文件超长仍可祖父化**（基线）⇒ 文件级"历史上就这么胖"无解，只有"别更胖"。
- `registry.py` 753 行本身也在逼近 800 帽（单一裁决点在涨）。

### P4 工具面成本：89 工具默认全量进上下文，profile 是可选项不是默认
- 每会话 `tools/list` 把全部工具定义塞进宿主上下文（`scripts/toolface_budget.py` 文档头：外部实测 58 工具 ≈55K token；本仓 S143 摸底 72 工具 ≈12.7K token）。
- 软帽 `_DEFAULT_CAP = 46000` 字符（S204 抬帽实测 45,873）——**软帽只是止血，成本仍随工具数线性涨**；工具描述以中文为主（CJK 按 ~1 token/字计）。
- 渐进披露已实现但靠 opt-in：`registry.py:215-252`（S149，UNIFIED_RX_PROFILE env + `profile_enable`/`profile_status` 工具 + tools/list_changed 钩子）；core 档仅 6 组 fs/scan/ide/search/ops/guard（`scripts/toolface_budget.py:52`）。
- 另有出口限幅资产：`registry.py` MAX_RESULT_ITEMS=200 / _MAX_BYTES=50K / MAX_STR_CHARS=64K / 入口 _MAX_STR_ARG 2M / _MAX_LIST_ARG 1 万（S10/S62）。

### P5 账本编号断档（ROUNDLOG）
- 实测：`spec/ROUNDLOG.md` 标题扫描共 158 条，S38–S218，缺 **S49、S54–S71（18 个）、S111、S112、S115、S161–S173（13 个）**。
- S1–S37 在 ROUNDLOG 之外（推测记于 spec/UPGRADE.md，未复核）。
- 推测：S161–S173 缺口与 S170"fn 硬阈开闸"同期，可能是并轮合并记录或漏记——无论哪种，**编号连续性没有门禁兜着**，事后无法机械对账。
- 另有封印账本 `spec/audit-ledger.json`（Mimosa 副本审计封印，scripts/audit_ledger.py 对账，首轮 59 findings verdict=inconclusive）——账本形态本身是资产。

### P6 零依赖自我限制是双刃剑
- Python 纯 stdlib + Rust [dependencies] 恒空 ⇒ 自带 JSON 解析（`rust/src/json.rs` 383 行）、手写 Python 词法/语法解析（`rust/src/pyast/lex.rs` 798 行 + `parse_expr.rs` 688 行）、手写 SHA-256（`sha256.rs`）。
- 收益：审计面完整、供应链零风险、离线可用——这正是新项目要的。
- 税：每覆盖一个新语言/新格式都要自建轮子（appaudit.rs 1203 行、sysinfo.rs 982 行的体量就是这么来的）；Windows 编码坑连续三次付费（S214 type-gate gbk 假红、S217 子进程 cp936、S218 顺序依赖）。

### P7 Rust 化是"跨进程薄壳"而非"进程内"：每工具一次 spawn 税
- 模式：Python 薄壳定位 exe → subprocess 单行 JSON 透传（`tools/astscan.py`：找 `%TEMP%\rx-rs-target\{release,debug}\rx-scan.exe`，超时 600s，退出码契约 0/2）。
- 11 个 exe × 每调用一次 spawn + 编解码；engine 缺失时"清晰报错不静默降级"（好纪律，但也就停在报错）。
- `server.py` stdio 主循环 + `registry.call()` 单一裁决点仍在 Python——协议层、授权门、沙盒、熔断、打点全在 Python 侧（`registry.py` 753 行）。

### P8 CI 形状：单 job 巨石 + 无超时
- `core.yml`：**1 个 job（core）、52 个 step、全串行、windows-latest、触发 branches: ['\*\*']**（每个分支每次 push 全量跑），rg 无 timeout-minutes 命中。
- 52 步内含 16+ 个自建门：secrets(:99)/naming(:102)/self-attack(:106)/taint(:109)/toolface(:112)/secrets-history(:115)/path(:118)/arch(:123)/claim(:128)/guard-canary(:133)/name-ledger(:137)/bench-anchor(:141) 等。
- 其余：`release.yml`（4 jobs：version-lock/build-windows/build-linux/release，tags 触发）、`codeql.yml`（main post-merge）、`health.yml`（schedule+dispatch）、`scan.yml`。
- 形状推断（未实测耗时）：1188 测试 + cargo 构建 + 16 门全串行，反馈慢、单点红全红；`pipelines/adv.gate-fast.json` 说明本地已有"快档"意识，但 CI 没有分档。

### P9 hub 平台层是半成品：文档三轮、代码未落
- `spec/HUB.md`（2026-09-28 三轮指令："本轮只交文档，M0 代码挂下一轮"）：一张能力两张脸（MCP stdio + server_web HTTP 共用 registry.call）。
- 顶层 12 个 `hub_*.py`（hub_core 467 行等，~100KB）已在仓根——**与 tools/ 平级的顶层模块群**，架构上尚未收敛进包结构。

## ③ 资产清单（出处 + 带进新项目的方式）

### A1 spec/ 判定档群（46 文件，仓里最值钱的东西）
| 文件 | 主题 | 移植方式 |
|---|---|---|
| `spec/PR-DISCIPLINE.md` | 一 PR 一功能四硬规则（搭车修禁令、依赖自成 PR、捆包拆分） | **原样移植**（仓库无关） |
| `spec/ROUNDLOG.md` | S 编号轮账法（158 条实证：悬案破案、前提纠错、活体漂移都靠它追） | 思想移植 + **编号连续性做成门禁**（补 P5 的洞） |
| `spec/LIBRARY-POLICY.md` | 依赖选型三问 | 改编（新项目 Rust 主体要重写依赖政策） |
| `spec/god.gate.json` + `scripts/god_gate.py` | 上帝对象门（文件/fn/成员三阈 + fn 硬阈 + 10% 合法交换），_doc 自称"可移植：抄到别的仓即可用" | **原样移植**（补文件级硬阈，见 P3） |
| `spec/arch-rules.json` + `scripts/arch_gate.py` | 分层禁向 × import 边 | 改编（阈值按新架构重定） |
| `spec/UPGRADE.md` / `ACHIEVEMENTS.md` / `ADVANCES.md` | 逐轮对账 / 成果归类 / 进展档案 | 思想移植 |
| `spec/BREAKER.md` | 工具熔断（per-key 重复 + 全局 QPM + 日总量） | 改编 |
| `spec/VULN-HUNTING.md` / `SCAN-POLICY.md` / `HARDENING.md` / `PLAYBOOKS.md` | 漏洞狩猎纪律 / 扫描策略 / 加固账 / 剧本 | 改编 |
| `spec/CD-PLATFORM.md` / `FRONTIER-CI.md` / `BUILD-SPEED.md` / `EVAL.md` | 平台化 CI / 前沿 CI / 构建速度 / 评测口径 | 改编进新 CI/CD 蓝图 |
| `spec/CONSOLIDATION.md` / `DESIGN-REVIEW.md` / `EXTERNAL-ALIGNMENT.md` / `CLAIM-INTEGRITY.md` | 收敛记录 / 设计评审 / 外部对齐 / 主张完整性 | 思想移植 |

### A2 门禁脚本群（scripts/ 43 个）——思想清单（新项目用 Rust/cargo 原生重写）
- `claim_gate.py`：文档数字 ↔ 真值源可复算（表述红线 11–15 的机器化）。
- `guard_gate.py` + `spec/guard-corpus.json`：守卫判定金丝雀，真值语料逐项复算（防扫描器自身腐化）。
- `taint_gate.py` + `spec/taint-baseline.json`：污点 definite 不超基线（棘轮）。
- `dupe_gate.py` + `dupe-baseline.json` / `lint_gate.py` + `lint-baseline.json` / `coverage_gate.py` + `coverage-baseline.json`：**"基线只禁恶化 + 硬阈禁历史债"两级棘轮制**（S170 教训：硬阈要同时管文件级，见 P3）。
- `bench_anchor_gate.py` + `spec/bench-anchors.json`：锚登记表 ↔ 引用名实相符（防"量空气"）。
- `toolface_budget.py`：工具面体量软帽 + top 大户仪表。
- `mcp_surface_gate.py` / `name_ledger_gate.py` / `naming_gate.py`：工具面改名 Ledger / 命名纪律。
- `path_gate.py` / `ci_secrets_gate.py` / `secrets_history.py` / `gitleaks_gate.py`：路径卫生 / 明文红线 / 历史 diff 面回扫。
- `stat_boot.py` / `stat_family.py` / `stat_judge.py`：统计判定（估计量口径、样本范围——对应表述红线 14）。
- `perf_gate.py` / `pytest_shards.py` / `shard_plan.py`：性能门 + 测试分片。
- ⇒ 移植方式：思想 100% 保留，实现重写；快慢两档（`pipelines/adv.gate-fast.json` vs `adv.gate-all.json`）直接进新 CI 设计。

### A3 Rust 引擎（`rust/src/` 20,168 LOC，Rust 写的可整体评估搬迁）
- `json.rs`（零依赖 JSON）、`search.rs`（BM25）+ `sem.rs`（tf-idf 语义）、`taint/`（污点引擎）、`pyast/`（手写 Python 解析器）、`sandbox.rs`（沙盒校验）、`nameres/`、`repomap.rs`、`sketch.rs`、`near_dupes`（MinHash/Jaccard 在 `scan.rs` 系）。
- 新项目若放开依赖：json/sha256/pyast 可换成熟库；taint/search/sandbox 等域逻辑可平移。**只留思想还是搬代码，蓝图阶段逐 crate 定**。

### A4 registry 设计（`registry.py`）
- 声明式注册（:187 @tool：name/handler/group/schema/requires_auth）+ `call()` 单一裁决点（错误隔离、自动打点 duration_ms→stats.jsonl）+ 出入口限幅（MAX_RESULT_ITEMS=200、_MAX_BYTES=50K、入口 2M 字符/1 万项）。
- 渐进披露（:215-252，S149）：UNIFIED_RX_PROFILE + profile_enable/status 恒在 + list_changed 钩子。
- ⇒ **思想移植**：新项目 Rust 服务器同构实现；把"默认全量"翻转成"默认按需装载"（P4 教训）。

### A5 版本对账工具思想
- `server.py:338` VERSION_TAG（版本↔最新 tag）、`server.py:401` EXE_TAG（11 exe --version 对账）。
- ⇒ 思想移植：新项目 **tag 即发布、单一真值源自动生成版本号**（SERVER_VERSION/Cargo.toml 双写手同步就是 P1 根因），对账门进 CI。

### A6 SWE-bench Verified 执行验证体系（bench/ + spec/ACHIEVEMENTS.md）
- S21–S30：94 run 双臂全判、判官去通胀（软判官 34%→12.8%）、S/R 块协议可应用率 5%→30%、fail-to-pass 真执行（uv per-task venv）、WSL 后 feasible 44/47。
- ⇒ 这是独立于语言的评测资产：**留档 + 思想移植**（新项目重写执行层成本高，先作为"模型判据"档案带过去）。

### A7 域文档 skills/（17 个 md，每域一张脸）+ console/index.html + hub 设计
- `skills/*.md`：工具域人读文档，与 registry group 对齐 ⇒ 改编。
- `spec/HUB.md`：一张能力两张脸（MCP + Web 共用单一裁决点）⇒ **思想直接进新蓝图**（新项目正是重做这件事的机会）。
- `docs/`（REASONIX 审计、MODEL-FIT、STRESS-AND-PR-GATES）⇒ 留档参考。

## ④ 给蓝图的 5 条硬结论

1. **版本锁步从 day-1 自动化**：tag 即发布（release 只认 tag 是对的，错的是 36 个版本没人打 tag）；版本号单一真值源自动同步到 Cargo.toml/产物，tag 连续性与 EXE 对账进 CI（沿用 server.py:338/401 思想）。旧仓 P1+P2 证明：锁步断 → 活体漂移 → 遥测口径全废。
2. **Rust 化要"进程内"不要"跨 exe 薄壳"**：旧仓验证了 Rust 引擎可行（20K LOC 零依赖跑通 11 bin），但每工具一次 subprocess spawn + 编解码 + Windows 编码坑是持续税；新项目单一 Rust MCP 服务器进程内直调所有域，Python 只留脚手架脚本。
3. **上帝对象门开箱即带文件级硬阈**：两级制（基线禁恶化 + 硬阈禁历史债）保留，但文件和函数都吃硬阈（旧仓只给函数开了，5 个 >800 行文件停在祖父化）；`god.gate.json + god_gate.py` 原样可抄。
4. **工具面默认按需装载，且工具总数设硬上限**：旧仓 89 工具默认全量进上下文、46K 字符软帽只是止血；新项目以 profile 为默认、新增工具=记账动作（ROUNDLOG 条目 + name-ledger 门），CJK 描述成本计入工具面预算。
5. **最大的资产是方法论不是代码**：spec/ 判定档 + 43 门禁脚本 + 棘轮/金丝雀/账本/主张可复算这一整套，是仓里可移植性最高的部分——移植时做两个升级：人肉门 → CI 原生 step（旧仓 core.yml 52 步巨石改为分档并行），账本编号连续性 → 也做成门（补 P5）。
