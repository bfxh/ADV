# ADV 重写蓝图 v0.2（2026-10-04）

> v0.2 = v0.1（2026-10-03）+ 扩展调研（W1–W4，55 域 1959 条，`RESEARCH/PROGRAM.md` 进度账）汇入的新增定案（§12）。
> 一句话：把 unified-rx-mcp（GitHub `bfxh/ADV`）重写为 **Rust 主体的本地优先代码安全/质量平台**——
> 本地化（self-hosted、离线可用）技术全用上，质量与速度对齐 2025–2026 前沿，自建 CI/CD 与合并流程，上帝对象从第一天生效机器门。
> 依据 = `RESEARCH/00–10`（11 份）+ W1–W4 共 66 域 registry **1959 条**（吸收 464/有界 291/P0 52，工程有效 988）；本文只做决策与汇总，细节一律锚到调研文件。

---

## 1. 定位与非目标

**定位**：单机本地运行的开发者安全/质量平台，三个产品面共享一个内核：
1. **扫描**（SAST 规则/污点 + secrets + SCA/供应链）
2. **检索**（遍历/符号/全文/向量四层，混合检索）
3. **工具面**（MCP 服务器为主，CLI 为人用面）

**非目标**（防堆东西）：
- 不做云服务/SaaS；默认档零网络（威胁数据用本地快照，联网功能显式开关）。
- 不做 IDE 插件（CLI + MCP 先行）。
- **不做旧 89 工具一比一兼容**：工具面重新设计，旧工具语义按需迁移（决议 #2：不为兼容降质量）。
- 云 LLM 不进默认档；本地嵌入/推理可以。
- **不搞嵌入/向量检索层**（2026-10-03 拍板）：检索走纯词法/符号路线；将来如需要只作可选扩展档，不进默认。

## 2. 命名与仓库策略（2026-10-03 拍板：分支制）

- **开发分支制**：重写在 `bfxh/ADV` 仓的 **`adv-rewrite` 分支**上进行，本地以 worktree 挂在 `D:\KF\ADV`（基线 = main `9fd1d62`，2026-10-03；一任务=一树=一分支，主 checkout 与其他会话分支不受影响）。
- **替换时机**：分支完善后整体替换 main 并接管 GitHub 仓库名——现在不做改名/转移；旧仓 36 minor 未打 tag 的历史账随替换一并了结。
- crate 前缀 `adv-*`，门禁二进制/xtask 同前缀。

## 3. 硬约束（每条都有旧仓教训作锚）

| # | 约束 | 锚 |
|---|------|----|
| C1 | Rust 为主体；**Python 白名单制**（评测/一次性迁移脚本可申请，登记进 `spec/PY-WHITELIST.md` 才许存在） | 00 §②④ |
| C2 | **god 门从第一天生效**（xtask 内建）：函数硬阈 + **文件硬阈**（旧仓缺）+ 成员数 + 棘轮基线只准减 + 金丝雀先记录后拦截 | 00 §②（382 条祖父化）；06 Top-1（生态空白确认自研） |
| C3 | **版本/tag 锁步门**：版本常量必须与 git tag 对账，断档即红 | 00 §②（SERVER_VERSION 2.94.0 vs tag v2.58.0，36 minor 断档零发布） |
| C4 | 工具注册**单一裁决点 + 出口限幅**；工具面默认三层渐进披露（§9），禁止全量 schema 进上下文 | 00 §②（89 工具全量，toolface 46K 软帽止血） |
| C5 | 默认档确定性可复现：并行段块序合并 + 哈希验证（par.rs 范式延续），判定字段必须与机器无关 | 00 §③ 资产 |
| C6 | 表述红线/质量红线/一任务一树一分支 = 旧仓 AGENTS.md 纪律整体迁移进新仓 | 00 §③ |

## 4. 架构（crate 分层草案）

```
adv-cli ────────────┐（人用面：scan / index / serve / gate）
adv-server (MCP) ───┤（stdio 自研 + 三层渐进披露）
                    │
        ┌───────────┴────────────────────────────┐
        │            adv-core（错误/句柄/路径策略/预算）        │
        └───────────┬────────────────────────────┘
   adv-parse ── adv-rules ── adv-taint        adv-index        adv-secrets / adv-sca
   (tree-sitter  (YAML spec    (快轨 matcher +   (ignore + Tantivy   (NP 收编 / 清单匹配分离
    +归一AST)    编译+matcher)  Rust 深轨 MIR)    + SCIP + 向量)       + OSV/RustSec 快照)
        │              │              │
   adv-sandbox（WIT 插件 ABI + wasmtime/wasmi 双运行时 + 进程隔离）
   adv-bin【v0.2 新增，R1–R6 定案】：二进制面（格式解析/反汇编/符号/相似性/缓解检查/模式扫描）
   adv-ast-rust【边车】：rustc_private + 固定 nightly + salsa，nightly 绝不下渗主引擎
   xtask：god 门 / 锁步门 / mutation / fuzz 语料 / conformance / 基线工具
```

- **增量统一机制**（01 Top-2、09 Top-2）：内存侧 salsa 统管依赖失效（文件→AST→绑定→summary 全 tracked query）；磁盘侧内容 hash 键控 + 「重建→原子换名」（Zoekt 式）+ Tantivy 段追加 + 向量差分。缓存键绑定 commit/内容 hash/模型版本（时序独立性）。
- **解析与 IR**（01 Top-1）：tree-sitter 0.25 多语言主解析 + 自建最小归一 AST（Semgrep「CST→generic AST」形状 + oxc 的 arena/AstKind 底座）；Rust 深轨独立边车（MIRAI/stack-graphs/DDlog 归档潮是隔离纪律的实证）。
- **污点**（01 Top-3）：YAML taint spec（sources/sanitizers/propagators，Semgrep 兼容形态）编译成 AstKind 分发 matcher 走全语言快轨；Rust 深轨 = MIR + IFDS 需求驱动（SVF 3.0「单一值流图+可插拔 solver」抽象 + Flowistry 机制），不动点用 Ascent（纯 Rust）。Soufflé/Flix/DDlog 只抄思想不进依赖。
- **性能骨架**（07 Top-1/2）：匹配层照抄 rg（aho-corasick/Teddy 字面量预过滤 + memchr 罕见字节 + regex lazy DFA 终判；SIMD 走 stable `core::arch`+运行时检测，std::simd 仍 nightly 不吸收）；文件级不规则并行交 rayon，有序相位保留自建分块+块序合并+sha256；异步文件 IO 不进扫描内核（留 NVMe 深队列对照实验位）；GPU 只留已验证相位（批量熵、XOR 穷举）+ ort/DirectML 承接嵌入推理；PGO 吸收（+10~15% 锚）。
- **检索栈**（09 Top-1/3，按 2026-10-03 拍板裁剪）：`ignore` 遍历 + Tantivy BM25（符号加权迁移为 identifier/content 双字段）+ rust-analyzer 产 SCIP 消费 + RRF 融合 + cAST 式 tree-sitter 递归分块（arXiv 2506.15655）。**嵌入/向量层与神经重排层不搞**。自写 BM25/tf-idf 只留融合层与评测集。负面信号：stack-graphs 已弃维护、tower-lsp 停更 3 年（选续作 tower-lsp-server/async-lsp）。

## 5. 吸收总表（跨域聚合，细节锚 RESEARCH）

| 域 | 头号吸收 | 档位判定要点 | 文件 |
|----|----------|--------------|------|
| 现状资产 | spec/ 判定档 46 文件 + 43 门禁脚本思想 + 棘轮两级制 + claim_gate | PR-DISCIPLINE/god 门原样移植；Rust 引擎 20K LOC 域逻辑评估平移 | 00 |
| 静态分析 IR | tree-sitter+归一 AST+salsa+边车隔离 | MIRAI/stack-graphs/DDlog 归档潮 ⇒ 隔离纪律 | 01 |
| SAST/secrets/SCA | **Nosey Parker 内化源码**（Apache-2.0 Rust：内容寻址 datastore+按 blob 增量+内容去重 10–1000× 降噪；2026-10-03 拍板内化而非外挂依赖）+ Syft→Grype 清单/匹配分离 + 四层误报流水线（LLM 永不前置，IRIS 边界） | TruffleHog=AGPL 只抄思想；CodeQL/商业平台专有 | 02 |
| fuzz/变异 | fuzz 三档制 + 变异 `--in-diff` 进 PR 门 | 全量变异仅周跑；Miri/Kani 周期档；cargo-mutants 无结果缓存 ⇒ 自建缓存外层是差异点 | 03 |
| MCP | 混合协议壳 + 三层渐进披露默认化 | 对齐 Anthropic tool search 实测 −85% token 准确率反升；sampling/roots/logging 已废弃永不采纳 | 04 |
| CI/CD | 阶段化迁移 + bors 思想自制队列（600–1200 行） | Earthly 关停教训 ⇒ 每组件留降级路径；L1 发布闭环，L2/L3 不做 | 05 |
| 质量工具链 | nextest+insta+llvm-cov 三件套 + Bencher 自托管 + cargo-public-api/semver-checks | god 门 xtask 自研（生态空白确认）；Windows 无 mold ⇒ rust-lld+sccache | 06 |
| 性能 | rg 骨架 + rayon/自建混合制 + SQLite 单文件全家桶 | GPU 只留已验证相位；Tantivy 按 语料/增量 判据切换 | 07 |
| 沙箱 | WIT 插件 ABI + wasmtime/wasmi 双运行时 + Windows broker/worker 四件套 | wasmtime CVE-2026-34971 ⇒ 锁版本+公告门；Windows 路径语义专项单测 | 08 |
| 检索 | 三层成熟组件（遍历/全文/符号）+ cAST chunking；**嵌入层不搞（拍板）** | nomic-embed-code 7B 本地 CPU 不现实（已无关紧要）；stack-graphs/tower-lsp 弃维护负面信号仍有效 | 09 |
| 论文 | P0 五篇：SVF 3.0 / IRIS / PrimeVul / cAST / Agentless | PrimeVul ⇒ 评测协议必须配对+防泄漏，否则门禁数字不可信 | 10 |

## 6. 质量门体系

**从旧仓移植**（原样/改编）：god 门（xtask 化 + 补文件硬阈）、claim_gate（文档数字↔真值源）、棘轮两级制（基线禁恶化 + 硬阈禁历史债）、金丝雀先记录后拦截、快照/金样门、PR 纪律（一功能一 PR、不自行合并）、`绿=SKIP 不算绿` / 判据走真路径 / 时序独立性 三护栏。

**新增强档**（全部进 xtask/CI，来源 03/06）：
1. **默认档三件套**：cargo-nextest（分片/重试/JUnit）+ insta（快照 bless）+ cargo-llvm-cov（覆盖率棘轮）。
2. **变异门**：cargo-mutants `--in-diff` 进 PR（分钟级）+ 全量周跑 `--shard`；自建「mutant 缓存 + 测试变更失效 + 覆盖测试选择」外层。
3. **fuzz 三档**：PR 门语料重放 `-runs=0`（秒级，crash 永久入库）/ 每 6h 定时 4 shard × 10 min / merge `-merge=1` 去重回写。扫描器加结构感知 harness + 输出 oracle。
4. **Miri/Kani 周期档**：Miri 只跑含 unsafe 的 crate；Kani ≤20 个纯函数 proof harness 周跑（s2n-quic 168 为上限参考）。
5. **性能棘轮服务化**：Bencher 自托管接 criterion，替代手工基线。
6. **API 面棘轮**：cargo-public-api + cargo-semver-checks。
7. **LLM 层边界**（IRIS）：LLM 只产规格/候选/初筛，确认权在静态引擎；四层误报流水线顺序不可倒（规则置信度→baseline 账本→确定性可达性→LLM 复核且结论缓存）。
8. **评测协议**（PrimeVul）：配对样本 + 防泄漏，否则门禁数字不可信。
9. **v0.2 新增门**（细节见 §12）：门缓存三段键 + `--explain`、hermeticity 八条自检、panic 隔离档位（unwind 默认/进程池兜底）、Kani 契约 + panic-free、抑制到期四要素、fix 安全三档、registry 校验门（`RESEARCH/registry/validate.py`，调研数据同受门管）。

## 7. CI/CD 与合并（阶段化，05）

- **阶段 0（现在）**：GitHub Actions + Windows self-hosted runner（注意 2026-03 起最低 runner 版本强制）；CI 形状锁 + 判据分片延续旧仓经验。
- **阶段 1（并行熟悉）**：Forgejo 镜像仓跑「熟悉而非兼容」的 Actions 子集 + 自托管 runner。
- **阶段 2（切换）**：Forgejo 主仓 + 合并队列（gitea-mq 优先；自制串行版 ≈600–1200 行 Rust，rust-lang/bors 为参考实现；兜底 auto-merge + 并发压 1）。
- **发布闭环（L1）**：cargo-auditable + CycloneDX SBOM + cosign 签名（静态密钥）；SLSA L2/L3 明确不做。cargo-dist 状态回暖但须评估（断档史）。
- **第一原则**：每个组件都有降级路径（Earthly 2025-07 全线关停、cargo-dist 断档史）。

## 8. 沙箱与安全边界（08）

- 插件 ABI 定 **WIT（WASI P2 语义）**；WASI 0.3（2026-06，async 原生）锁 P2 跟进。
- 双运行时：wasmtime 主（pooling + fuel/epoch + host 函数白名单=唯一能力出口）、wasmi 回退；rquickjs 只作 JS 规则前端。
- Windows 隔离 MVP = broker/worker 池四件套：full AppContainer + Job Object kill-on-close + 受限 token + alternate desktop（rappct 可达）；Linux 用 Landlock+seccomp 同构；LPAC 二档。
- 攻击面分档表：**执行面沙箱 / 解析面 fuzz 兜底 / 能力面三张白名单表**；Windows 路径语义专项单测（RUSTSEC-2024-0438 教训）；敏感数据 zeroize+secrecy。

## 9. MCP 工具面（04）

- 协议壳 = 判据触发的混合制：**stdio 主路径自研**（2026-07-28 无状态语义把手写成本降到最低）；官方 conformance 套件（rust-mcp-stack 110/110）当 CI 验收门；远程 HTTP+OAuth 需求出现才评估 rmcp 3.x（4,845 RPS 锚证明性能非变量）。
- **三层渐进披露做成默认机制**（旧仓 opt-in 教训，00 §②）：
  - L0 轻目录：全部工具 ≈1K token 目标；
  - discover/load 元工具对按需加载完整 schema；
  - outputSchema/structuredContent 输出通道（大结果落盘+句柄，不进上下文）。
- spec 第一梯队：tool annotations / structured output / server discover；sampling/roots/logging 已于 2026-07-28 废弃——永不采纳；tasks 二梯队扩展。
- 安全：Invariant Labs 四类攻击模型（tool poisoning 等）缓解清单 + 工具面最小权限。

## 10. 里程碑（精选，每步判据先行）

| 里程碑 | 内容 | 验收判据 |
|--------|------|----------|
| M0 骨架 | workspace + xtask 门（god/锁步）+ 三件套 + clippy/deny + CI 阶段 0 + AGENTS.md 迁移 | ✅ 2026-10-04（`0f53a99`，CI 绿；suppress 门片3 并入成三门） |
| M1 规则内核 | adv-parse + 归一 AST + YAML taint spec 编译 + AstKind matcher + 规则测试门禁 | ✅ 2026-10-04 片1–3（`0fd48fb`/`3423c81`/`d5c1e54`；FP 会计账 ×3，方法沉淀 spec/FP-ACCOUNTING.md） |
| M2 Rust 深轨 | 边车 crate + MIR IFDS + Ascent | 与旧 Rust 引擎对拍（行为等价金样） |
| M3 secrets+SCA | Nosey Parker 内化 + 清单/匹配分离 + OSV/RustSec 本地快照 | 增量扫描降噪比 + 工具间分歧口径明示 |
| M3b 二进制面（v0.2 新增） | adv-bin L0–L1：ImageFacts（object+pelite+gimli）+ 缓解检查（winchecksec 字段集）+ cargo-auditable 读取；L2–L3（反汇编/函数识别/P-code 深轨）按 M3b 验收后再排 | ImageFacts 原型跑通 PE/ELF；缓解检查字段集逐项判据 + 金丝雀二进制 |
| M4 检索栈 | Tantivy + SCIP + RRF + cAST chunking + 增量索引（无嵌入层，拍板） | 评测集配对协议（防泄漏）+ 增量正确性判据 |
| M5 MCP 服务器 | stdio 自研 + 三层渐进披露 + conformance 门 | L0 目录 token 计量 + conformance 全绿 |
| M6 沙箱/插件 | WIT ABI + 双运行时 + Windows 四件套 | 攻击面分档表逐项验收 |
| M7 自托管 | Forgejo 主仓 + 合并队列 + L1 发布闭环 | 队列串行化验证 + 发布物可验签 |

## 11. 决议记录（2026-10-03 用户拍板，原决策队列已闭环）

1. **GitHub 路径**：现在不动（没有搞好前不换）；重写在 `adv-rewrite` 分支（worktree `D:\KF\ADV`）上进行，**完善后整体替换 main 并接管仓库名**。
2. **旧 89 工具语义兼容面**：**不为考虑以前的就降低质量**——工具面自由重设计，旧语义按价值选择性迁移，不设兼容包袱。
3. **Nosey Parker**：**内化源码**（Apache-2.0 允许；保留上游 LICENSE/署名与出处锚）。
4. **宿主孤岛副本**（D:\rj\MCP）：随分支替换 main 的时点一并切换。
5. **嵌入模型**：**不搞**。检索栈砍掉向量层与神经重排层，走 遍历+全文+符号 纯词法/符号路线（§1 非目标、§4）。
6. **Python 白名单**：维持少数场景原则（评测/一次性迁移脚手架须登记进 `spec/PY-WHITELIST.md`）。

## 12. v0.2 新增定案（W1–W4 扩展调研汇入，细节锚 registry）

**新增面**：
- **adv-bin**（二进制面，R1–R6 定案）：L0 格式解析（object+pelite+gimli，`ImageFacts` 一次解析）→ L1 缓解检查（winchecksec/BinSkim 字段集）+ 模式扫描（YARA-X 内化评估）+ cargo-auditable 读取 → L2 反汇编（iced-x86/yaxpeax）+ 函数识别（capa 范式）→ L3 Ghidra P-code/SLEIGH 深轨（Rust 侧 jingle_sleigh 类试接）。相似性 = BSim 思想最小骨架。里程碑挂 M3 后（M3b）。

**运行时与并行（改判）**：
- 并行栈终裁：**rayon 为唯一 CPU 并行入口**（indexed collect 保序覆盖 sha256 相位）；旧仓 par.rs 手写范式**不吸收**（X6）。
- MCP stdio = 同步最小方案（reader 线程 + crossbeam-channel + 串行 writer）；**tokio 不进默认档**，需求触发才经 `net-async` feature 门控进（X6/X12，CI 断言默认档 `cargo tree` 无 tokio）。
- 网络栈：默认 ureq 3.x；rustls 默认 ring 后端（aws-lc-rs 仅 FIPS 触发）；离线优先落成依赖结构而非纪律（X12）。

**数据与存储**：
- 序列化：报告层 JSONL（serde 6 条字段演进规范）；索引层 rkyv+blake3 内容寻址（bincode 2.0 为对照档）；压缩 zstd 三段定档（热 L3/温 L9–12/冷 L19）+ 先哈希后压缩规范（X3/X5）。
- SQLite pragma 起步档 + ≤100KB 进 BLOB 官方锚 + refinery 迁移三规范；内容寻址抄 Git loose+pack 两级（X13）。
- 增量失效协议：SAC early cutoff + 内容 hash 值重验（保守上界保声音、便宜复验出精度，E2）；DBSP 算子微分用于线性摘要（~200 行）。

**门与验证**：
- 门缓存 = Bazel 式三段键（输入指纹→动作签名→输出摘要）+ `xtask gate --explain`；hermeticity 八条进自检门（D4）。
- god 门载体终裁 xtask（just 仅薄入口）；`#[expect]` 抑制到期四要素进规则 schema；fix 安全三档（D1/D2/D4）。
- panic 隔离调和：默认 unwind+线程隔离（量得 abort 收益 ≥2–3% 才翻），不可信输入下沉 worker 进程池（X8）。
- Kani 契约模式（requires/ensures）+ panic-free 门；SMT 增量求解（push/pop+unsat core 作规则冲突解释器）（E6）。

**二进制与安全工程**：
- WASM 插件：**fuel 主计量（确定性可审计）+ epoch 兜底**（宿主调用挂死）；pooling 起步配置草案；wasm-smith/wasm-mutate 进 fuzz 三档（X14）。
- Windows 总账：Job Object 能力→ADV 字段映射表；路径总规范（长路径/reparse/ADS/大小写/短名）直接进 spec；elevated 判定+无管理员降级清单（X11）。
- 密钥卫生：keyring→Windows 凭据管理器（DPAPI）；spawn 出口"默认空环境+白名单"两道闸（GIT_* 教训泛化）；报告掩码抄 GitHub 语义（X10）。
- 供应链新增检查项：XZ 9 条规则（5 条纯静态先行）+ Ladisa 攻击树对账表 + 包幻觉/slopsquatting（R9/P8）。

**检索与索引（收敛后）**：
- aider repo-map（tree-sitter 标签→PageRank→预算内文件集→树渲染）= ADV repo 摘要 MCP 工具的最短移植路径（E5）。
- difftastic 语法 diff 把增量扫描从文件级收窄到函数/块级（先实测收窄比与误收率，D6）。
- 嵌入层确认不搞（拍板不变）；FTS5 调参走列权重+分词器（unicode61 tokenchars '_-' + trigram）路径（E4/X13）。

**论文 P0（52 篇）已逐篇映射模块**；先读顺序：SVF 3.0 → IRIS → PrimeVul → cAST → Agentless（wave-0 10）→ ZIPPER → SAILR → Ladisa。

**工具链裁定（2026-10-04 用户拍板，写入 AGENTS-ADV.md §3）**：`rust-toolchain.toml` `channel = "1.99.0"` patch 级钉定；本机 default-host=gnu（无 MSVC link.exe）、CI=msvc，同一 channel 各自解析可用工具链；channel 里**禁止**带目标三元组（CI rustup 拒收实测）。依赖保持最高兼容，lockfile 入库。
