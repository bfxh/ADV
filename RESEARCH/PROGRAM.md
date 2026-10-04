# ADV 扩展调研纲领 v1.0（2026-10-03）

> 缘起：wave-0（11 份、约 190 对象）被用户判为"算少了"。用户 2026-10-03 口径：论文和项目都要看；
> **实验性论文**要分析；**黑客工具/反编译**等要拆开集成；**命令行加速**、**开发者工具**、**实验性项目**全都要；
> **不降低质量**。⇒ 把调研升级为持续研究工程：55+ 新域、三层深度、全部登记、逐波汇入 BLUEPRINT。

## 0. 目标与验收
- **条目目标（以 registry 实数为准）**：深挖层 ≥600 条、扫视层 ≥2000 条，合计**数千条**；不许为凑数灌水——扫视条目也必须"一句机制 + 档位 + license + 链接"。
- 深挖层 = 机制级（说得出算法/数据结构/工程参数），全部进蓝图候选；扫视层 = 家底清点 + watchlist。
- 每条落到 `RESEARCH/registry/<DOMAIN>.jsonl`，可按 域/档位/集成目标 汇总计数（进度账用实数，不用估数）。
- 吸收/有界条目**必须有集成目标**（ADV 模块或里程碑）；watch/reference 明确写"不集成理由"。

## 1. 三层深度
| 层 | 标准 | 每域条数 | 去向 |
|----|------|----------|------|
| deep 深挖 | 机制级：算法/数据结构/工程参数 + license + 第一步动作 | 12–40 | BLUEPRINT §5 + 施工队列 |
| sweep 扫视 | 一句机制 + 档位 + 链接；重点 3–5 条升格 deep | 40–150 | registry + watchlist |
| registry 登记 | JSONL 一行/条（含全部 sweep） | 全覆盖 | 统计与追踪 |

## 2. Registry schema（每个域一个文件，追加写入）
文件：`RESEARCH/registry/<DOMAIN>.jsonl`，UTF-8，一行一 JSON（python `json.dumps(ensure_ascii=False)` 追加）：

```json
{"id":"R1-001","domain":"R1","name":"Ghidra","kind":"project","verdict":"reference","mechanism":"P-code 中间表示：所有指令 lift 到统一 RISC IR，反编译/仿真/分析共用","integration":"adv-bin","license":"Apache-2.0","link":"https://github.com/NationalSecurityAgency/ghidra","depth":"deep","date":"2026-10-03"}
```

- `kind`: project | paper | technique | dataset | product
- `verdict`: 吸收 | 有界 | 不吸收 | watch | reference
- `integration`: adv-parse | adv-rules | adv-taint | adv-ast-rust | adv-secrets | adv-sca | adv-index | adv-server | adv-sandbox | **adv-bin** | perf-core | cli | xtask-gates | ci-ops | test-replay | docs-process | watch | reference
- `depth`: deep | sweep

## 3. 域矩阵（wave-0 已完成 11 个；下表为新增 55 域）

**Wave 1 · 逆向/二进制/安全工程（R）** — 用户点名的"黑客工具/反编译"专线
| ID | 域 | 深度 | 目标 | 产出 | 集成目标 |
|----|----|------|------|------|----------|
| R1 | 反汇编器/反编译器全家 + 反编译研究（LLM4Decompile 等） | deep | ≥22 | R1-decompilers.md | adv-bin / reference |
| R2 | 二进制分析与符号执行（angr/KLEE/Triton/miasm/qiling） | deep | ≥20 | R2-binary-symbolic.md | adv-bin |
| R3 | 调试器与动态插桩（gdb/lldb/x64dbg/WinDbg TTD/Frida/DynamoRIO/PIN/QBDI/rr/eBPF/ETW） | deep | ≥20 | R3-debuggers-instrumentation.md | adv-bin / test-replay |
| R4 | 二进制格式与符号/调试信息（LIEF/goblin/object/PDB/DWARF/符号服务器） | mix | ≥35 | R4-binary-formats.md | adv-bin |
| R5 | 恶意软件分析与检测工程（YARA-X/capa/Cuckoo/CAPE/ATT&CK/取证） | deep | ≥20 | R5-malware-detection.md | adv-bin / adv-rules |
| R6 | 利用工程与缓解机制（ROP/堆利用自动化/CFG/CET/ASLR/沙箱逃逸分类） | mix | ≥25 | R6-exploit-mitigations.md | adv-rules / watch |
| R7 | 密码学工具与 GPU 加速解剖（hashcat/JtR/GPU 调度） | deep | ≥12 | R7-crypto-gpu.md | perf-core |
| R8 | 网络扫描与协议分析（nmap/masscan/ZMap 无状态扫描/Wireshark dissector 架构/mitmproxy） | mix | ≥25 | R8-net-protocol.md | perf-core / adv-sandbox |
| R9 | 红队/双用途工具工程（架构思想与防御对照；只读思想） | mix | ≥20 | R9-dualuse-engineering.md | watch / reference |

**Wave 2 · CLI 加速与开发者工具（C/D）** — 用户点名"命令行加速""开发者工具都要分析"
| ID | 域 | 深度 | 目标 | 产出 | 集成目标 |
|----|----|------|------|------|----------|
| C1 | 现代 CLI 工具性能解剖（ripgrep/fd/fzf/jq/hyperfine/delta/tokei…） | mix | ≥55 | C1-cli-acceleration.md | cli / perf-core |
| C2 | shell/管道/结构化数据 CLI（GNU parallel/xargs/qsv/miller/arrow-cli） | sweep | ≥40 | C2-shell-pipelines.md | cli |
| C3 | 启动延迟与二进制瘦身（LTO/PGO/mimalloc/静态链接/Windows 控制台） | deep | ≥20 | C3-startup-binary.md | cli / ci-ops |
| C4 | TUI/终端渲染（ratatui/crossterm/notcurses/alacritty/wezterm） | sweep | ≥30 | C4-tui-rendering.md | cli |
| C5 | 文件系统与 IO 加速（mmap 分档/IOCP/io_uring/USN journal/并行遍历） | deep | ≥20 | C5-fs-io.md | perf-core |
| D1 | Linter 全家（ESLint/Ruff/Biome/Oxc/Clippy/Staticcheck/golangci/Pylint…） | mix | ≥45 | D1-linters.md | adv-rules |
| D2 | 格式化器全家（Prettier/Biome/Oxc/rustfmt/dprint/gofmt/Black…） | sweep | ≥30 | D2-formatters.md | cli |
| D3 | 类型检查与 LSP（tsc/pyright/mypy/rust-analyzer/gopls/clangd + LSP 框架） | deep | ≥25 | D3-typecheck-lsp.md | adv-parse |
| D4 | 构建系统与任务运行器（Bazel/Buck2/Pants/Turbo/Nx/Ninja/Just/xtask） | deep | ≥25 | D4-build-systems.md | xtask-gates / ci-ops |
| D5 | 包管理器与依赖解析（uv/pnpm/bun/cargo/vcpkg/PubGrub/renovate） | deep | ≥20 | D5-package-managers.md | adv-sca |
| D6 | Git/VCS 工具（gitoxide/jj/libgit2/delta/lazygit/packfile/bitmap） | deep | ≥20 | D6-vcs-tools.md | ci-ops / cli |
| D7 | 测试/覆盖率/flaky 治理（nextest/pytest/vitest/llvm-cov/grcov） | deep | ≥20 | D7-test-coverage.md | test-replay |
| D8 | Profiling 与可观测性（perf/vtune/tracy/samply/tracing/OTel/pyroscope） | sweep | ≥30 | D8-profiling.md | perf-core |
| D9 | 本地 CI 与 runner（act/Woodpecker/Forgejo Actions/Dagger） | mix | ≥20 | D9-local-ci.md | ci-ops |
| D10 | 编辑器架构（helix/neovim/zed、rope、tree-sitter 集成） | sweep | ≥20 | D10-editors.md | adv-parse |
| D11 | API/兼容性工具（cargo-semver-checks/public-api/rustdoc-json/ABI） | sweep | ≥15 | D11-api-compat.md | xtask-gates |

**Wave 3 · 实验性项目与论文群（E/P）** — 用户点名"实验性论文/项目都要分析"
| ID | 域 | 深度 | 目标 | 产出 | 集成目标 |
|----|----|------|------|------|----------|
| E1 | 新语言/编译器/验证栈（Roc/Gleam/Hylo/Vale/Zig/Carbon/Mojo/Verus…） | sweep | ≥40 | E1-new-languages.md | reference |
| E2 | 增量计算与响应式（salsa/adapton/DBSP/Feldera/Materialize/Nix） | deep | ≥20 | E2-incremental.md | adv-parse / adv-index |
| E3 | 新 VCS 与协作实验（jj/pijul/radicle/stacked diffs） | sweep | ≥20 | E3-new-vcs.md | ci-ops |
| E4 | 数据引擎实验（DuckDB/DataFusion/redb/fjall/流处理） | mix | ≥25 | E4-data-engines.md | adv-index |
| E5 | AI4Code 实验（SWE-agent/aider/openhands/continue/RepoGraph/CodexGraph） | mix | ≥25 | E5-ai4code.md | reference / watch |
| E6 | 形式化验证工具链（TLA+/Alloy/Lean/Z3/cvc5/Kani/Verus/Creusot/Prusti） | mix | ≥20 | E6-verification.md | xtask-gates / reference |
| E7 | OS/内核/隔离实验（Redox/Theseus/Unikraft/WASI 运行时/seL4 类） | sweep | ≥20 | E7-os-isolation.md | adv-sandbox |
| P1 | 安全四大（S&P/CCS/USENIX/NDSS）2023–26 | sweep | ≥80 | P1-sec-conf.md | all |
| P2 | SE 五大（ICSE/FSE/ASE/ISSTA + TOSEM）2023–26 | sweep | ≥80 | P2-se-conf.md | all |
| P3 | PL 四大（PLDI/POPL/OOPSLA/CAV）2023–26 | sweep | ≥60 | P3-pl-conf.md | adv-parse / adv-taint |
| P4 | 系统顶会（SOSP/OSDI/EuroSys/ATC）2023–26 | sweep | ≥60 | P4-sys-conf.md | perf-core |
| P5 | ML4Code/LLM×SE（NeurIPS/ICLR/ACL/EMNLP + arXiv） | sweep | ≥80 | P5-ml4code.md | reference |
| P6 | 二进制/反编译研究 | sweep | ≥50 | P6-binary-research.md | adv-bin |
| P7 | Fuzzing 研究前沿 | sweep | ≥50 | P7-fuzzing-research.md | test-replay |
| P8 | 供应链安全研究（依赖混淆/typosquatting/SBOM 实证） | sweep | ≥40 | P8-supplychain-research.md | adv-sca |

**Wave 4 · 底层库与平台（X）**
| ID | 域 | 深度 | 目标 | 产出 | 集成目标 |
|----|----|------|------|------|----------|
| X1 | 高性能数据结构/算法库（hashbrown/rapidhash/simd-json/ryu/bitvec…） | sweep | ≥40 | X1-perf-datastructs.md | perf-core |
| X2 | SIMD 生态（std::simd/wide/simde/highway/stringzilla） | sweep | ≥30 | X2-simd.md | perf-core |
| X3 | 序列化与零拷贝（serde/bincode/postcard/rkyv/flatbuffers/capnp） | sweep | ≥25 | X3-serde.md | perf-core |
| X4 | 正则/多模式匹配引擎（regex/aho-corasick/hyperscan/RE2/pcre2） | deep | ≥20 | X4-regex-matching.md | adv-rules |
| X5 | 压缩（zstd/lz4/brotli/ouch） | sweep | ≥15 | X5-compression.md | perf-core |
| X6 | 并行/异步运行时（rayon/tokio/glommio/monoio/compio） | deep | ≥20 | X6-runtimes.md | perf-core |
| X7 | 内存分配器（mimalloc/jemalloc/snmalloc/talc/bumpalo） | sweep | ≥15 | X7-allocators.md | perf-core |
| X8 | 错误处理/日志生态（anyhow/thiserror/eyre/miette/tracing） | sweep | ≥20 | X8-errors-logging.md | cli |
| X9 | FFI/跨语言绑定（pyo3/napi-rs/cxx/uniffi/cbindgen） | sweep | ≥15 | X9-ffi.md | reference |
| X10 | 密钥/内存卫生（zeroize/secrecy/mlock/rustls 用法） | sweep | ≥12 | X10-secrets-memory.md | adv-sandbox |
| X11 | Windows 平台工程（windows-rs/WIL/Detours/MinHook/ETW/PDB/调试） | deep | ≥25 | X11-windows.md | perf-core / adv-sandbox |
| X12 | 网络栈（rustls/quinn/hyper/tonic/HTTP3） | sweep | ≥20 | X12-network.md | ci-ops |
| X13 | 数据库/存储引擎（SQLite/DuckDB/redb/sled/LMDB/fjall） | mix | ≥30 | X13-storage.md | adv-index |
| X14 | WASM/组件模型深入（wasmtime/wasmer/component model/wasm-tools） | sweep | ≥15 | X14-wasm.md | adv-sandbox |
| X15 | 终端/CLI UX 库（clap/ratatui/indicatif/console/globset/ignore） | sweep | ≥20 | X15-cli-ux.md | cli |

## 4. 波次与节奏
- 并发上限 = 1（实测：子智能体只能串行）⇒ 每轮 2–4 域，逐轮推进，跨会话继续（记忆有锚）。
- 每域 1 个 agent，≤~20 次工具调用，回复 ≤12 行（全文落盘）。
- 每波收尾：更新本文件进度账（registry 实数）+ RESEARCH-INDEX + BLUEPRINT §5。

## 5. 质量红线（不降低质量的具体含义）
- license 必标；AGPL/专有 = 只抄思想，代码不可内联。
- 实验性项目标成熟度【实验/维护中/弃维护】；「弃维护」本身是结论（stack-graphs、tower-lsp 先例）。
- 机制 or 不收：说不出算法/数据结构/工程参数的不写；查不到标「待验证」。
- 数字带锚（来源+日期+口径）；区分实测与推测；不绝对化；两端点名（A 对 B 的机制）。
- 扫视层也不许灌水：一句机制 + 档位 + 链接 缺一不可，空壳剔除。

## 6. 新增集成面（候选，待调研定案后进 BLUEPRINT v0.2）
- **adv-bin**：二进制分析面（反汇编/符号/相似性/模式扫描）——由 R1–R6 定案。
- **adv-diff**：二进制/补丁差分——由 R6/相似性调研定案（若启，并入 adv-bin 里程碑）。
- 反向集成：扫描器自身组件（IR/matcher/检索）被新面复用，登记时写清方向。

## 7. 进度账（每波更新，以 registry 实数为准）
| 波 | 域 | 状态 | 条数（registry 实数） | 落盘 |
|----|----|------|----------------------|------|
| W0 | 00–10（11 域） | ✅ 2026-10-02/03；**10-04 补登记 219 条** | 219 | RESEARCH/00–10 + registry/00–10.jsonl |
| W1 | R1–R9 逆向/二进制/安全工程 | ✅ 2026-10-03 | 324 | R1–R9*.md + registry |
| W2 | C1–C5 + D1–D11（CLI 加速 + 开发者工具） | ✅ 2026-10-03 | 183+316 | C*.md D*.md + registry |
| W3 | E1–E7 + P1–P8（实验性项目 + 论文群） | ✅ 2026-10-03/04 | 164+658+补扫 43 | E*.md P*.md + registry |
| W4 | X1–X15 底层库与平台 | ✅ 2026-10-04 | 314 | X*.md + registry |
| **合计** | **66 域** | ✅ | **2241**（deep 884 / sweep 1357；吸收 561 / 有界 367 / P0 56 / P1 208 / P2 362 / watch 306 / reference 303 / 不吸收 78；工程有效 = 吸收+有界+P0+P1 = **1192**） | |

**收尾与审计（2026-10-04）**：
- 档位/字段归一：verdict、kind、depth、link、domain、id 全部统一；`RESEARCH/registry/validate.py` 校验门建成，**2241 条 0 违规**（首跑抓出 2350 处词汇漂移，已归一；这是门抓自家数据的第一次实战）。
- **抽样审计（`RESEARCH/AUDIT-registry.md`，样本 441/2241=20%**）：内容面 0 灌水/0 错配/0 重复；链接可核验 74.6%、真错链 5 条（含 P0 LIBAFL venue 错）**已按证据修正**；结论=可信高档 ~74% / 中 ~24% / 低 ~1.2%。
- 补扫：CCS 2023–25（Crossref 拉全 +25）、USENIX Sec25/NDSS25 全量核对（P6 +7 / P7 +11）、P5 弱项专题（FP 过滤 +10 / 代码摘要 +10）、待验证链接回填（124→84）。
- **遗留（诚实记档，不挡 M0）**：ATC 2026 议程未上线（0 条）、待验证链接 84 条、license=unknown 约 11%、CCS AsiaCCS 混入已剔 1 条改标。
