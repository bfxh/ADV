# RESEARCH 索引（66 域，2026-10-02/04）

> **扩展调研已收官并过审计**：W0 + W1–W4 共 66 域，registry 总账 **2241 条**
> （deep 884 / sweep 1357；吸收 561 / 有界 367 / P0 56 / P1 208 / P2 362 / watch 306 / reference 303 / 不吸收 78；
> 工程有效 = 吸收+有界+P0+P1 = **1192**），`RESEARCH/registry/validate.py` 校验门 **0 违规**。
> 抽样审计见 `RESEARCH/AUDIT-registry.md`（样本 20%：0 灌水/0 错配/0 重复；链接可核验 74.6%，5 条真错链已修）。
> 纲领与进度账见 `RESEARCH/PROGRAM.md`；每域两件套：`RESEARCH/<ID>-*.md` + `RESEARCH/registry/<ID>.jsonl`。

## W0（wave-0，11 份）

| 文件 | 主题 | 对象数 | 一句话核心结论 |
|------|------|--------|----------------|
| 00-current-state.md | 旧仓 unified-rx-mcp 盘点 | — | 36 minor tag 断档 / 宿主孤岛副本 / god 基线 382 条祖父化 / 89 工具全量进上下文；spec 判定档+门禁思想是最大资产 |
| 01-static-analysis-ir.md | 静态分析引擎与 IR | 24 | tree-sitter+归一 AST+salsa 增量+rustc 边车隔离；污点 YAML spec→matcher 快轨 + MIR IFDS 深轨 + Ascent |
| 02-sast-secrets-sca.md | SAST/secrets/SCA 对标 | 21 | Nosey Parker 内化（拍板）；Syft→Grype 清单/匹配分离；四层误报流水线 LLM 永不前置 |
| 03-fuzz-mutation.md | fuzz/变异/动态验证 | 9 | fuzz 三档制（PR 重放/6h 深跑/merge 去重）；变异只 `--in-diff` 进 PR；Miri/Kani 周期档 |
| 04-mcp-ecosystem.md | MCP 生态与工具面 | 13 | stdio 自研 + conformance 当 CI 门；三层渐进披露默认化（−85% token 锚）；sampling/roots/logging 永不采纳 |
| 05-selfhost-cicd.md | 自托管 Git/CI/CD/合并 | 24 | 阶段化：GH runner → Forgejo 镜像 → 主仓+自制合并队列（600–1200 行）+ L1 发布闭环；每组件留降级路径 |
| 06-rust-quality-toolchain.md | Rust 质量工具链 | 22 | nextest+insta+llvm-cov 三件套 + Bencher + API 面棘轮；god 门确认生态空白 ⇒ xtask 自研 |
| 07-performance-localfirst.md | 性能与本地优先 | 29 | rg 骨架（aho-corasick+memchr+lazy DFA）；并行混合制（X6 收敛为 rayon 唯一入口）；SQLite 全家桶；GPU 只留已验证相位 |
| 08-sandbox-plugins.md | 沙箱与插件隔离 | 16 | WIT 插件 ABI + wasmtime/wasmi 双运行时；Windows broker/worker 四件套；wasmtime CVE ⇒ 锁版本+公告门 |
| 09-retrieval-index-lsp.md | 检索/索引/LSP | 22 | ignore+Tantivy+SCIP+RRF（嵌入层拍板砍掉）；stack-graphs/tower-lsp 弃维护负面信号 |
| 10-papers.md | 论文清单 2023–2026 | 16 | P0=SVF 3.0 / IRIS / PrimeVul / cAST / Agentless；评测协议必须配对+防泄漏 |

## W1–W4（55 域，条数=registry 实数）

| 组 | 域 | 条数 | 各域头号定案（摘） |
|----|----|------|--------------------|
| R 逆向/二进制/安全 | R1 反编译器 · R2 符号执行 · R3 调试器插桩 · R4 二进制格式 · R5 恶意软件检测 · R6 利用与缓解 · R7 GPU/密码工具 · R8 网络扫描协议 · R9 红队工程（防御对照） | 324 | Ghidra P-code/SLEIGH 深档路径+BSim；angr 引擎分解标本+GTIRB；LLDB/DAP 会话层+rr 记录内核+Windows 观测三件套；object+pelite+gimli 三件套+cargo-auditable 机制；YARA-X 规则→IR+capa 多粒度；winchecksec/BinSkim 字段集+CWE 2025 数据模型；hashcat 批量调度+blake3+Bao；无状态扫描范式+Wireshark/Zeek 插件化；XZ 9 条检测规则；**EDR 规避类明确否决** |
| C 命令行加速 | C1 CLI 性能解剖 · C2 管道/结构化 · C3 启动与二进制 · C4 TUI · C5 文件系统 IO | 183 | rg 预过滤全栈+fzf 动态工作队列+启动相位；GNU parallel 批处理语义+JSONL-first；rust-lld on MSVC+PGO 链路+**BOLT 否决（无 PE 支持）**+启动预算书；ratatui 0.30+三态输出契约+Windows 终端 9 坑；ignore::WalkParallel+USN 双轨+原子写 9 坑/6 规范+mmap 分档表 |
| D 开发者工具 | D1 Linter · D2 格式化 · D3 类型/LSP · D4 构建 · D5 包管理 · D6 Git/VCS · D7 测试覆盖 · D8 Profiling · D9 本地 CI · D10 编辑器 · D11 API 兼容 | 316 | `#[expect]` 抑制到期+fix 安全三档+路径无关缓存键；treefmt 组织模式+格式门验收三件套+gofmt 哲学；clangd 背景索引 shard+pyright 沙箱解析+诊断节流（tsgo 已发）；**xtask 终裁**+Bazel 三段键门缓存+hermeticity 八条；uv 三件套+pubgrub-rs 直接用+pnpm 内容寻址；gix 读路径+difftastic 语法 diff 收窄增量+merge-tree 预检/CAS 推送；nextest 语义+diff-coverage 口径+proptest shrinking；samply+tracelogging+dhat 连续剖析闭环；Forgejo Runner Windows 无容器隔离⇒信任三档+预热池；ropey 定档+Zed MultiBuffer+先 LSP 后插件；cargo-semver-checks+trycmd+buf breaking 阶梯+deprecation 机制 |
| E 实验性项目 | E1 新语言/编译器 · E2 增量计算 · E3 新 VCS · E4 数据引擎 · E5 AI4Code · E6 验证 · E7 OS/隔离 | 164 | rustc 诊断契约+annotate-snippets（404 已解除）+rowan 零失败解析+工具链自带质量门形态；DBSP 算子微分进 adv-index+rustc dep-graph 选层+SAC early cutoff 失效协议；worktree-per-agent 并发基座+Reviewable 评审失效状态机；**sled 事实冻结排除**+FTS5 调参具体化+index/cache 分库判据；aider repo-map+SWE-agent ACI 四原则+仲裁权归确定性门；Kani 契约+panic-free 门+Creusot+SMT 增量求解；gVisor 纠偏三条+能力白名单数据结构+**Firecracker/libkrun 无 Windows 宿主**⇒强隔离 runner 绑 Linux |
| P 论文群 | P1 安全四大 · P2 SE 五大 · P3 PL 四大 · P4 系统 · P5 ML4Code · P6 二进制研究 · P7 fuzzing · P8 供应链 | 658 | 129/147/90/66/72/56/54/44；**P0 共 52 篇逐篇映射设计决策**：ZIPPER 两级污点、Bond 告警接自动验证、Mammoth LLM 评测协议、SAILR 源码对齐度量、S3-FIFO、StreamCache 扫描 IO、BWoS work-stealing、SPFresh 增量索引、IncIDFA、egglog、MalGuard、Ladisa 攻击树对账表、From Noise to Signal 补丁级归属等 |
| X 底层库与平台 | X1 数据结构 · X2 SIMD · X3 序列化 · X4 正则匹配 · X5 压缩 · X6 运行时 · X7 分配器 · X8 错误日志 · X9 FFI · X10 密钥卫生 · X11 Windows · X12 网络 · X13 存储 · X14 WASM · X15 CLI UX | 314 | id-arena+slotmap 句柄化 AST+lasso 驻留+jiff；memchr 三层分级+SIMDjson 相位+StringZilla SWAR；rkyv+blake3 零拷贝+serde 6 规范+bincode 2.0；regex-automata 选档树+HIR literal 预抽取+fst；zstd 三段定档+先哈希后压缩；**rayon 唯一 CPU 并行入口（par.rs 不吸收）**+MCP stdio 同步方案+LazyLock 替代 once_cell；std+Segment Heap+mimalloc 门控+bumpalo per-file arena；错误三面分层+表驱动码表+panic=abort 调和；pyo3 abi3+Miri/ASan 测试矩阵分工；keyring→DPAPI+zeroize 两类判据+env 白名单两道闸；Job Object 总账+路径总规范+windows crate 特性门；ureq 3.x 定默认档+rustls ring 后端+离线 feature 门 CI 断言；pragma 起步档+≤100KB BLOB 判据+refinery 迁移；**fuel 主计量+epoch 兜底**+pooling 起步配置+wasm-smith/mutate 进 fuzz；clap 判据+figment 合并模型+globset 裁决+CJK 表格 comfy-table |

## 遗留补扫清单（登记于各文档尾注）

CCS 2023–25 录取页 404 缺口 · USENIX/NDSS 2025 全量补扫 · FP 过滤/代码摘要专题（P5 尾注）· ATC 2026 官网 404 · DBLP 反爬补 DOI · CAV 卷 Crossref 未命中 · E1 的 Vale/Hare 仓库 404 复核。
