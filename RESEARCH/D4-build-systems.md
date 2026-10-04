# D4 深挖：构建系统与任务运行器

- 域：D4（构建系统与任务运行器），服务 ADV 的 xtask-gates / ci-ops 设计
- 日期：2026-10-03；资料优先 2025–2026（正文数字均带锚，缺锚处已注明"知识库，未复核"）
- 方法：WebSearch 为主；机制描述部分（jobserver / REAPI / 特性统一等）来自官方文档知识库，版本与事件类数字带外部锚
- 纪律：每对象「定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接」；说不出机制的不写

---

## 0. 一页结论与 Top-3

**Top-3（对 ADV 最有行动价值）：**

1. **质量门载体终裁 = xtask**（判据三件套：Windows 支持 / 新增依赖数 / 可测性，见 §2.①）。just 降级为"可选入口薄层"（有界：只转发、零逻辑）；cargo-make 不吸收。工具链锁用 rust-toolchain.toml 单一事实源（本地 mise、CI rustup 双消费）。
2. **Bazel 式门缓存三段键**（输入指纹 → 动作签名 → 输出摘要，§2.②）：不引入 Bazel 本身，只抄它的键设计；配上 Turborepo 式 `--dry=json` 同型的 `xtask gate --explain` 可观测性。sccache 维持 wave-0 现状（有界：只做编译产物缓存），但必须补 env 白名单审计。
3. **hermeticity 自检门**（§2.⑤ 八条清单，做成 `xtask hermeticity` 门，吸收）：封闭性靠"声明输入 + env 白名单 + 双跑一致 + cwd 无关"可执行验证，不靠口号。

---

## 1. 对象目录（25 个）

### 1A 任务运行器

#### 1. just
- **定位**：Rust 写的单二进制命令运行器；justfile 语法近 make 但**不是构建系统**——不追踪文件时间戳、无增量、无文件 DAG，只有"recipe 依赖先跑完再跑本体"的调用序。
- **可抄机制**：① `--jobs N` / justfile 里 `set jobs := 4` 并行执行无依赖 recipe（2025 系列 1.55–1.58 的 changelog 确认 --jobs 并行度限制，RPM 打包页转述 casey/just changelog）；② `[group]` 属性 + `just --list` 自文档（门清单天然可发现）；③ Windows 一等公民（默认 PowerShell，`[windows]` 属性按平台切 shell）；④ `[no-exit-message]`/`@` 静默控制——门输出的信噪比控制可借鉴。
- **档位**：**有界**（只做入口薄层，零逻辑转发 `cargo run -p xtask -- <gate>`；避免门逻辑写进 shell——不可单测）。
- **第一步动作**：若引入：justfile 每条 recipe 一行转发；CI 不依赖 just（直接调 xtask），保证 CI 与本地同路径。
- **许可证/成熟度**：CC0-1.0；维护中（casey/just，2025 年 1.55–1.58 系列滚动）。
- **链接**：https://github.com/casey/just

#### 2. cargo-make
- **定位**：TOML DSL 任务运行器（Makefile.toml），内置大量预制 CI 任务（lint/coverage/docker 流派）与任务继承。
- **可抄机制**：① task 继承/extend/clear 覆盖链——"公共配置 + 项目覆写"的分层思想；② pre/post hook 流派任务接线；③ 内置 CI 元任务（证明" batteries-included"是它唯一不可替代点）。
- **档位**：**不吸收**。判据：依赖树重（cargo 插件安装慢）、DSL 表达力靠特例堆叠、门逻辑不可单测；2026-09 的 Rust Tool Index 快照显示其更新滞后约 10 个月（~1.3k stars），动量低于 just。
- **第一步动作**：无（仅留对比记录，防止日后有人重提）。
- **许可证/成熟度**：Apache-2.0；维护放缓（据 tools.corrode.dev 2026-09 快照）。
- **链接**：https://github.com/sagiegurari/cargo-make ；https://tools.corrode.dev

#### 3. xtask 模式
- **定位**："用 cargo 构建自己"：workspace 内一个 xtask crate，`cargo run -p xtask -- <gate>`；matklad 2020 提出，rust-analyzer 等在用。
- **可抄机制**：① 门逻辑与被测代码同一次编译、同工具链版本——判据可测性的核心（`cargo test` 直接覆盖门逻辑，just/cargo-make 都做不到）；② 工作区元数据消费：`cargo metadata --no-deps`（毫秒级）拿 package/feature/target 清单，替代手工枚举，是元数据驱动门的正道（见 §1D-25）；③ 子命令组织：`xtask::run()` match 分发或 clap；每门一个子命令 + `--explain`，映射"门=纯函数(输入指纹)->判定"。
- **档位**：**吸收**（ADV 门载体已定，本文给出补充：元数据消费 + 自指纹 + 缓存键）。
- **第一步动作**：给 xtask 加 `cargo metadata` 消费层与 `--explain`（键展开），见 §2.②。
- **许可证/成熟度**：模式（matklad/cargo-xtask 文章 MIT）；参考实现多 MIT/Apache-2.0；成熟（多年多项目验证）。
- **链接**：https://github.com/matklad/cargo-xtask

#### 4. mise（原 rtx）
- **定位**：asdf 兼容的多语言工具链管理器（Rust 实现），`mise.toml` 单文件声明工具版本 + env + 任务，日历版本月更（2025.11.10 见 2025-12-04 issue；docs.rs 列 2026.9.12）。
- **可抄机制**：① `[tools]` 声明 + 多后端安装（vfox/asdf/aqua/ubi——GitHub release 直装二进制），适合钉 taplo/cargo-binstall 等 Rust 之外的工具；② `[env]` 注入——与 hermeticity 的 env 白名单同一件事的开发者体验面；③ 任务系统 `mise tasks`（与 just 同型，ADV 不需要——xtask 已覆盖）。
- **档位**：**有界**（本地开发体验用；CI 不经 mise，直接按版本下载固定 URL+校验和，见 §2.④）。
- **第一步动作**：验证 mise 的 rust core plugin 是否消费仓库根 rust-toolchain.toml（行为确认后再定 mise.toml 里是否重复声明版本——单一事实源原则）。
- **许可证/成熟度**：MIT；维护中（2025–2026 月更节奏）。
- **链接**：https://mise.jdx.dev ；https://github.com/jdx/mise

#### 5. asdf
- **定位**：老牌多语言版本管理器，shell shim 注入 PATH；0.16 起（2025 初）用 Go 重写提速（据官方 changelog，未复核细节）。
- **可抄机制**：仅插件模型这一条值得知道——per-language plugin + shim。对 ADV 无增量价值：shim 依赖 shell profile，Windows 支持弱，功能被 mise 全面覆盖。
- **档位**：**不吸收**。
- **第一步动作**：无。
- **许可证/成熟度**：MIT；维护中但生态位被 mise 取代。
- **链接**：https://asdf-vm.com

#### 6. GNU make（图执行 / jobserver）
- **定位**：时间戳驱动的规则系统，DAG 隐式（模式规则推导）；ADV 不引入，但两个机制是门并行的正解来源。
- **可抄机制**：① **jobserver**：`-j N` 的并发度由管道上的 token 池承载，递归子 make 经 MAKEFLAGS 继承同一池——cargo 复刻了同一机制（jobserver crate），rustc 构建脚本与 cargo 共享 token 池。ADV 门并行直接采用：**全局 token 池限制总并发，防"8 个门 × 门内 -j8 = 64 超订"**；② 反面教材：时间戳不可靠（时钟回拨/克隆重置）→ 门缓存键必须用内容摘要，不用 mtime；③ 隐式 DAG 难审计 → 门的依赖图要显式声明。
- **档位**：**reference**（机制已吸收进 §2.③，不引入工具）。
- **第一步动作**：xtask 图执行器里实现 token 池（跨进程信号量或 jobserver crate）。
- **许可证/成熟度**：GPLv3+；维护中。
- **链接**：https://www.gnu.org/software/make/manual/

#### 7. ninja
- **定位**：为大型生成式构建设计的图执行器（Meson/CMake 的后端），不引入，机制是门调度器的理想型。
- **可抄机制**：① build.ninja 是**已解析的静态边表**（无变量展开计算），调度 = 拓扑序 + 就绪边池，成本 O(就绪边)——xtask 的门依赖图照此做：图先显式物化，再调度；② **deps log**：跨运行持久记录依赖（.ninja_deps），重启后增量——对应 ADV 的门缓存键落盘；③ **restat**：边执行后若输出未变则跳过下游——门缓存的"输入指纹未变则短路下游门"同型；④ **pool**：console 独占池与 jobserver 池并存——"人机交互任务独占"的隔离思想（门日志/进度独占通道）。
- **档位**：**reference**。
- **第一步动作**：无（图执行器设计评审时对表 §2.③）。
- **许可证/成熟度**：Apache-2.0；维护中（ninja-build/ninja）。
- **链接**：https://ninja-build.org/manual.html

### 1B 构建系统（monorepo 级）

#### 8. Bazel
- **定位**：机制经典的增量构建系统；ADV 不引入（Rust 单语言 workspace 用 Bazel 迁移成本远超收益），但它有四个"机制级可抄件"。
- **可抄机制**：① **Action Cache**：每个 action 的键 = hash(动作签名 + 输入摘要集合 + **显式声明的 env**)，值 = ActionResult（输出文件摘要 + 元数据）；本地 `--disk_cache` 与远端缓存共用同一键，命中即跳过执行；② **Skyframe 增量求值**：节点=文件/配置/glob/target，变化沿依赖边最小重算——门依赖图增量短路的思想源头；③ **sandboxing**：linux-sandbox（namespaces）/ darwin-sandbox / Windows 沙箱（实验），配合 `--incompatible_strict_action_env`（未声明 env 不进键不进环境）——hermeticity 的可执行形态；④ **visibility 默认强制**（Bazel 8 起）：模块边界在构建系统层可执行——对应 ADV 各 gate crate 之间的依赖边界（cargo 侧可用 workspace 依赖白名单近似）。
- **档位**：**有界**（机制吸收：键设计进 xtask-gates 门缓存、strict action env 进 hermeticity 门；构建系统本身不引入）。
- **第一步动作**：按 §2.② 实现三段键缓存（`xtask gate --cache`），含 `--explain`。
- **许可证/成熟度**：Apache-2.0；维护中（8.0 LTS 2024-12 发布；9.x 线 2026 活跃：9.1.0 于 2026-04-20 前后、9.2.0 计划 2026-06，据 bazelbuild GitHub release issue #28365/#29355；2025 年生态工具兼容性多列到 8.3.x）。
- **链接**：https://bazel.build ；https://releases.bazel.build

#### 9. Buck2
- **定位**：Meta 的 Rust 写构建系统，Starlark 配置；理论视角最干净（Build Systems à la Carte 分解）。
- **可抄机制**：① **DICE 统一增量图**：loading/configuration/analysis/execution 全是同一张增量图上的节点，无"阶段"概念——增量粒度最细的参照；② 增量基于**输入指纹变化**而非 mtime（内容感知重算）；③ 与 Bazel 的差异点：无隐式 host 依赖、package 边界更严、prelude 注入内建规则——"隐式行为越少，缓存键越可信"的实证。
- **档位**：**watch**（对 ADV 无落点：生态小、Windows 支持非一等、无 2025 重大更新信号——本次检索未见 2025 专属公告）。
- **第一步动作**：仅跟踪（若未来 ADV 多语言化再评）。
- **许可证/成熟度**：Apache-2.0；维护中（Meta 内部生产使用）。
- **链接**：https://github.com/facebook/buck2

#### 10. Pants
- **定位**：Rust 引擎 + Python 规则的 monorepo 构建系统；`@rule` 纯函数 + 图增量 + 进程长驻（nailgun）。
- **可抄机制**：① 规则=纯函数、缓存=图节点——与 xtask"门=纯函数(指纹)->判定"同构，验证了该抽象可行；② 局部执行（local process cache）与远程缓存同一接口。
- **档位**：**watch**。锚：2025-09-16 社区 chat 里用户讨论为 3–4 分钟的性能 bug（约 30–40% 管道时间）悬赏——主维护带宽吃紧的信号；发布仍滚动（2.26.x 发布笔记在案）。
- **第一步动作**：无（半年后复查一次维护状态）。
- **许可证/成熟度**：Apache-2.0；维护中但人力吃紧（2025-09 信号）。
- **链接**：https://www.pantsbuild.org ；https://github.com/pantsbuild/pants

#### 11. Turborepo
- **定位**：JS monorepo 任务运行器；价值在**远程缓存设计**与缓存键可观测性。
- **可抄机制**：① 任务哈希 = hash(锁文件 + 任务脚本 + 输入 glob 快照 + **显式声明的 env 清单（env/globalEnv）** + framework 推断)——"env 必须显式进键"与 Bazel strict action env 殊途同归，是三段键里输入段的清单来源；② 远程缓存走简单 HTTP 协议（Bearer token + artifact PUT/GET），有多个社区自托管实现——若 ADV 未来要团队共享门缓存，这是最省的协议形态；③ `--dry=json` 输出每个任务的哈希与依赖图——`xtask gate --explain` 的模板（缓存为什么 miss，一眼可查）。
- **档位**：**reference**（键设计与 --explain 模板吸收；工具本身 JS 生态）。
- **第一步动作**：无（设计稿引用其 hash 组成文档）。
- **许可证/成熟度**：MPL-2.0；维护中（Vercel）。
- **链接**：https://turborepo.com/docs

#### 12. Nx
- **定位**：JS monorepo 构建编排（项目图 + 任务图分离，named inputs 进哈希）；2025 年最重要的产出是**反面教材**。
- **可抄机制**：① named inputs 显式声明输入集（对应三段键的输入段）；② runtime 环境求值（哈希前执行命令取值进键）——比隐式 env 更诚实，但成本在每次求值，门缓存可用"env 白名单 + 版本断言"替代。
- **2025 供应链事故（s1ngularity）**：2025-08-26/27，npm 上 8 个被植入恶意 postinstall 的 nx 与 @nx/powerpack 版本存活约 5 小时：窃取 env/GitHub/npm token/SSH key/钱包，经 GitHub gist 外传；**攻击脚本武器化 AI CLI（Claude Code / Gemini CLI / Copilot CLI）来定位凭据**，并尝试篡改本地 AI 工具捕获后续 prompt（Wiz 2025-08-28 分析；Nx 官方 postmortem 2025-09-05）。一个月后同赛道出现 Shai-Hulud npm 蠕虫（2025-09）。对 ADV ci-ops 的直接教训：构建/发布工具链的**版本必须锁定 + 内容校验 + 发布令牌最小权限**；本地 AI 代理本身成了攻击面。
- **档位**：**reference**。
- **第一步动作**：把 s1ngularity 复盘条目写入 ci-ops 的供应链清单（版本锁定/校验和/最小权限令牌）。
- **许可证/成熟度**：MIT；维护中。
- **链接**：https://nx.dev ；https://www.wiz.io/blog/s1ngularity-supply-chain-attack（Wiz 分析）

#### 13. Meson
- **定位**：受限 Python 方言定义构建、固定生成 ninja 后端；2025 年两个版本有 Rust 相关信号。
- **可抄机制**：① "定义语言受限求值 + 生成静态边表"的两层结构（求值期/执行期分离——对应 xtask 的"配置解析→图物化"分离）；② 1.9.0（约 2025-08-25，Phoronix 报道）增强 Rust 支持；1.8.0（约 2025-05-01）Wayland 模块转正；1.9.1（约 2025-10/11）PyPI 缺 sdist（issue #15236，GitHub Releases 为准——分发渠道单点的现成案例）。
- **档位**：**reference**（Rust 主 workspace 不适用；C/C++ 伴生组件若出现再评）。
- **第一步动作**：无。
- **许可证/成熟度**：Apache-2.0；维护中（1.8/1.9 于 2025 年发布）。
- **链接**：https://mesonbuild.com

### 1C 缓存与可复现

#### 14. ccache
- **定位**：C/C++ 编译缓存老牌；价值在**键的两级设计**。
- **可抄机制**：① 直接模式（manifest：编译器+参数+头文件集指纹 → 直接给 .o，绕过预处理）vs 预处理模式（预处理后源码内容为键）——**先试廉价指纹、miss 再付重哈希成本**的两级查询，门缓存可复用（廉价段=路径+参数+env，贵段=文件内容哈希）；② `base_dir` 归一化绝对路径——键里不能出现仓库绝对路径（否则克隆位置不同全 miss）；③ 依赖用 deps 文件而非递归 stat。
- **档位**：**reference**。
- **第一步动作**：无。
- **许可证/成熟度**：GPLv3+；维护中。
- **链接**：https://ccache.dev

#### 15. sccache（wave-0 已部署，本文补键设计）
- **定位**：跨语言编译缓存（C/C++/Rust…），Rust 用它缓存 rustc 编译；wave-0/06 已有 sccache 自托管，此处只补**键设计细节与边界**。
- **可抄机制/键细节**：① 键 ≈ hash(编译器可执行摘要 + 预处理参数 + 规范化公共参数 + 输入文件内容 + 语言 + **相关 env**)——注意 env 一项：CI 变量漂移会导致全 miss 或（更糟的）错 hit，**必须显式审计进键的 env 清单**；② Rust 支持的边界：处理 rustc 时绕过/禁用增量编译（增量产物与 sccache 键不相容）——ADV 若开 sccache 就不要指望增量编译同时命中；③ 0.9.1（2025-02，Mozilla Engineering Effectiveness Newsletter；rust-lang CI 同步升级 PR #137023）起有 **preprocessor cache mode**（`--debug-preprocessor-cache` 调试），0.10.0 约 2025-04（Arch 打包记录）；④ 自托管后端 redis 最简（wave-0 现状）。
- **档位**：**有界**（只做编译产物缓存，不做门结果缓存——两者键语义不同，勿混层）。
- **第一步动作**：审计 sccache 进键的 env 清单并写入 ci-ops 配置注释；门缓存与 sccache 分开命名空间。
- **许可证/成熟度**：Apache-2.0；维护中（Mozilla；2025 年 0.9.x/0.10.x）。
- **链接**：https://github.com/mozilla/sccache

#### 16. Bazel 远程缓存协议（REAPI / CAS）
- **定位**：内容寻址缓存的事实标准协议（build/bazel/remote/execution/v2）；ADV 只抄键模型，不实现协议。
- **可抄机制**：① CAS：blob 以 `Digest{hash(sha256), size_bytes}` 寻址，**不可变**——存储布局即 `.cache/<hash>/...`；② ActionCache 以 ActionKey 寻址 ActionResult；写路径 UploadMissingBlobs → UpdateActionResult；读 GetActionResult 后逐 blob 校验下载——**命中也要校验**（对应门缓存命中后用输出摘要验盘）；③ 键不可变 + 值自描述（元数据随存）——门缓存 result.json 的字段模板。
- **档位**：**有界**（键模型吸收进 §2.②；协议不实现，单机磁盘缓存起步）。
- **第一步动作**：无（随 §2.② 落地）。
- **许可证/成熟度**：spec Apache-2.0；维护中（Bazel 生态核心，随 9.x 演进）。
- **链接**：https://github.com/bazelbuild/remote-apis

#### 17. Nix 内容寻址构建（与 E 组交界）
- **定位**：derivation 全输入序列化 → sha256 → store 路径；内容寻址 derivation（CA）让输出内容参与路径，等价构建可合并；fixed-output（网络产物）仅摘要进路径。
- **可抄机制**：① "路径即内容摘要"消除"同名不同物"；② fixed-output 的隔离语义：网络产物必须声明摘要，否则不进确定性世界——门缓存的网络产物（下载工具链等）同规则。
- **档位**：**watch**（CA 仍在实验旗标后（官方手册表述），Windows 无原生支持；若 E 组做环境即代码可再评）。
- **第一步动作**：无（E 组交界处交接）。
- **许可证/成熟度**：LGPL-2.1+；维护中；CA 部分实验。
- **链接**：https://nixos.org/manual/nix/stable/

#### 18. hermeticity（封闭性）判定
- **定位**：Bazel 把封闭性拆成"声明输入 + 封闭 env + 沙箱无网络"三件可执行保障；ADV 把它做成自检门（细则见 §2.⑤）。
- **可抄机制**：① `--incompatible_strict_action_env`：未声明的 env 既不进环境也不进键——"白名单外即违规"的可执行定义；② sandboxing 的本质 = **未声明输入读取会被发现**（沙箱失败即构建失败，而非静默）；③ 封闭性测试两条金标准：双跑一致（同键两次执行输出逐字节相同）与隔离性（改无关文件键不变）。
- **档位**：**吸收**（`xtask hermeticity` 门）。
- **第一步动作**：实现 §2.⑤ 八条清单中最便宜的三条先跑（env 白名单、双跑一致、cwd 无关）。
- **许可证/成熟度**：机制归属 Bazel（Apache-2.0）+ reproducible-builds.org 实践；成熟。
- **链接**：https://bazel.build/docs/hermeticity ；https://reproducible-builds.org

### 1D Rust 构建工程

#### 19. cargo 特性图与统一化（feature unification）
- **定位**：特性是**可加的**且在编译单元内跨依赖取并集——"特性组合"不是开关矩阵而是并集语义，这是大量"特性不生效/行为漂移"误报的根源。
- **可抄机制/坑清单**：① 并集语义：同一编译图里某 crate 的 feature 是所有依赖者要求的并集——"关掉"不成立；② resolver 演进：v2（Rust 1.51+）起 build/dev 依赖特性隔离，edition 2024（Rust 1.85，2025-02）默认 resolver v3——ADV 应显式写 `resolver = "3"` 并在文档记录判据（dev-deps 特性不泄漏）；③ 缓存互踢：不同 feature 组合互相作废 target 产物——门并行跑多 feature 组合时用 `CARGO_TARGET_DIR` 分桶。
- **档位**：**有界**（xtask-gates 加 feature 白名单检查 + 分桶策略文档化）。
- **第一步动作**：xtask 加 `xtask features` 列出 workspace 全特性图（来自 cargo metadata），标出 build-dep 引入的特性。
- **许可证/成熟度**：cargo 本体 MIT/Apache-2.0；成熟。
- **链接**：https://doc.rust-lang.org/cargo/reference/resolver.html

#### 20. build script 机制与坑
- **定位**：build.rs 是构建期任意代码——缓存键与 hermeticity 的最大漏洞源。
- **可抄机制/坑清单**：① 未输出 `cargo::rerun-if-changed` 时默认"**任何文件变化都重跑**"——缓存杀手；2024-03（Cargo 1.77）起新语法 `cargo::` 前缀（老 `cargo:` 仍在）；② `cargo::rerun-if-env-changed` 显式声明读取的 env——与 §2.⑤ env 白名单同构，build script 是白名单的第一执行者；③ 坑：`rustc-link-search` 顺序污染、输出行未识别被静默忽略、build script 里读系统路径导致跨机不可复现。
- **档位**：**有界**（rg 级自检：仓库内 build.rs 必须含 rerun-if 声明；进 xtask-gates）。
- **第一步动作**：加一条静态检查门（`xtask gate build-script-hygiene`）。
- **许可证/成熟度**：cargo 本体；成熟。
- **链接**：https://doc.rust-lang.org/cargo/reference/build-scripts.html

#### 21. proc-macro 编译成本
- **定位**：proc-macro 是宿主工具链编译单元，恒为 dylib 并被 rustc `dlopen` 加载（长-standing 设计，非 2025 新变化）；它阻塞所有依赖者的编译起点——是关键路径头部。
- **可抄机制/数字**：① proc-macro **不做增量缓存**（宏可带副作用，rustc 每次重跑）——宏重仓库增量的主要痛点（SDR Podcast 2024-10 "rubicon" 期有解释）；② 2025 实质改善：Rust 1.88（2025-06）宏解析器减少分配，宏重 crate 编译平均约 -5%（release notes 口径）；Rust 1.90（2025-09）Windows x64 rustc 发行版上 PGO；③ Wasm 沙箱加载宏的提案仍未落地（检索时点无稳定化信号）。
- **档位**：**有界**（监控类：`cargo build --timings` 看宏链关键路径；依赖选择上限制 proc-macro 数量，derive 尽量归并）。
- **第一步动作**：xtask 门里加 `--timings` 产物归档（可选开关），作为编译预算测量起点。
- **许可证/成熟度**：rustc 本体；成熟（沙箱化方向实验）。
- **链接**：https://sdr-podcast.com/episodes/fixing-build-times-with-rubicon/

#### 22. artifact dependencies（bindeps）
- **定位**：RFC 3028：`[dependencies] pkg = { version, artifact = "bin", lib = true }`——依赖另一个包的**构建产物**（bin/cdylib/staticlib），产物路径映射进 `CARGO_BIN_FILE_*` env。
- **机制/现状**：截至 Cargo Book 快照（2025-10 之后）仍需 nightly `-Z bindeps`；跟踪 issue cargo#10444；1.90 dev-cycle 博客（2025-10-01）提到静态参数 + artifact-dependencies 的后续设计——**活跃推进但未稳定**，registry 支持是待解项之一。过渡方案：xtask 里 `cargo install --path` / 预构建产物 + 显式路径 env。
- **档位**：**watch**（稳定即重评；ADV 的"xtask 构建辅助二进制"目前走过渡方案）。
- **第一步动作**：在 xtask 的"辅助工具分发"设计注释里标注：bindeps 稳定后迁移。
- **许可证/成熟度**：cargo 本体；实验（nightly-only）。
- **链接**：https://github.com/rust-lang/cargo/issues/10444 ；https://blog.rust-lang.org/2025/10/01/this-development-cycle-in-cargo-1.90/

#### 23. cargo build-dir / target 目录缓存分桶
- **定位**：`-Z build-dir`（Cargo Book unstable features 在列——截至该快照未稳定）：target 目录内容按哈希分桶，解决多 profile/feature 组合互相覆盖。
- **机制**：不稳定期等价物 = `CARGO_TARGET_DIR` 按门/按 feature 组分桶（`$TARGET_DIR/<gate>-<hash8>/`）；代价是磁盘放大与首次全量编译——桶策略要和门缓存的键对齐（同键同桶，否则缓存短路后 cargo 还要重建）。
- **档位**：**watch**（build-dir 稳定后可能改变分桶策略；现状用 CARGO_TARGET_DIR）。
- **第一步动作**：无（ci-ops 文档记录现状分桶规则即可）。
- **许可证/成熟度**：cargo 本体；实验（unstable）。
- **链接**：https://doc.rust-lang.org/cargo/reference/unstable.html

#### 24. 工具链锁：rust-toolchain.toml + CI 一致性
- **定位**：rustup 读 workspace 根 `rust-toolchain.toml`（channel 钉到 patch 版 + components + targets）；mise 的 rust core plugin 同样消费该文件——**单一事实源**。
- **机制**：① channel 写精确 patch（`1.90.0` 而非 `stable`）——stable 浮动会破坏键稳定性（rustc 版本是三段键输入段的一部分）；② CI 侧 rustup 直读同文件（或 dtolnay/rust-toolchain action 读同文件），不引入第二份版本声明；③ 非 Rust 工具走 mise.toml（本地）+ CI 按版本固定 URL+校验和下载（不经 mise，防供应链漂移，见 §1B-12 事故教训）；④ 一致性判据门：`xtask toolchain-check` 断言本地 `rustc -vV` 与 rust-toolchain.toml 一致、CI 侧比对 hash。
- **档位**：**吸收**。
- **第一步动作**：固化 rust-toolchain.toml（patch 级）+ `xtask toolchain-check`。
- **许可证/成熟度**：rustup MIT/Apache-2.0；成熟。
- **链接**：https://rust-lang.github.io/rustup/overrides.html

#### 25. 工作区元数据消费（cargo metadata）
- **定位**：`cargo metadata --format-version 1 --no-deps` 一次拿到 workspace 的 package/target/feature/依赖清单，JSON 输出、毫秒级；是 xtask 元数据驱动门的正道（避免 rg 扫 TOML 的脆弱解析）。
- **可抄机制**：① `--no-deps` 控制成本（只要本 workspace 时禁开依赖解析全图）；② metadata JSON 里 feature 集合直接喂 §1D-19 的特性图检查；③ 它是门"输入完备性"的登记处——新 package/target 自动进入门输入集，防"加了 crate 忘了加检查"。
- **档位**：**吸收**。
- **第一步动作**：xtask 统一 metadata 消费层（缓存 metadata JSON 进三段键的输入段）。
- **许可证/成熟度**：cargo 本体；成熟（稳定接口多年）。
- **链接**：https://doc.rust-lang.org/cargo/commands/cargo-metadata.html

---

## 2. 五问裁决

### ① xtask vs just vs cargo-make（质量门载体终裁）

| 判据 | xtask | just | cargo-make |
|---|---|---|---|
| Windows 支持 | cargo/rustc 原生保障（ADV 主平台，无 shell 语义缝隙） | 原生支持（默认 PowerShell，[windows] 属性），但需分发安装二进制 | 支持，但 DSL 内嵌 shell 语义跨平台易踩坑 |
| 新增依赖数 | 0（可选 clap） | 1 个静态二进制（CC0，安装面小） | cargo 插件，依赖树重（corrode 索引显示更新滞后约 10 个月，2026-09 快照） |
| 可测性 | 门逻辑 = Rust，`cargo test`/nextest 直接覆盖 | 只能 shell 断言 | TOML DSL，无单测 |

**裁决：质量门本体 = xtask（吸收）；just = 可选入口薄层，只转发零逻辑（有界）；cargo-make = 不吸收。** 红线：门逻辑一旦写进 justfile/shell，就失去可测性与三段键的自指纹能力（键里要求 xtask 实现哈希，shell 逻辑无法自指纹）。

### ② Bazel 式 action cache 的最小可抄子集（ADV 门缓存键设计）

三段键（伪代码）：
```text
in_fingerprint = sha256( concat(
    sorted( 路径 || sha256(内容) )      # 门输入文件集（清单来自 cargo metadata + 显式声明）
    || toolchain_fingerprint            # rustc -vV + 辅助工具 --version（patch 级锁定）
    || env_whitelist_kv                 # 显式 env 白名单 k=v（strict action env 语义）
    || normalized_args                  # 门参数规范化排序
) )
action_sig    = gate_name || xtask_self_hash  # 自指纹：xtask crate 自身源码 sha256
action_key    = sha256( in_fingerprint || action_sig )
value         = { verdict, outputs: [path||sha256], meta: {xtask_ver, created_at 仅展示} }
```
设计要点（每条对应一个反模式）：键用内容摘要不用 mtime（时钟回拨）；env 显式白名单（漂移 → 错 hit/全 miss）；输出摘要随值存储 → 命中后验盘（REAPI GetActionResult 后校验语义）；存储布局 `.gate-cache/<action_key>/`（CAS 不可变）；`xtask gate --explain` 展开键组成（Turborepo --dry=json 同型），miss 原因可审计。sccache 与门缓存分两层：编译产物缓存（sccache，已有）与门判定缓存（本设计），键语义不同不混用。

### ③ 门依赖图与并行执行（进程池 vs 线程池 vs 图执行）

机制对照：make/ninja 的正解 = **显式物化的 DAG + 拓扑序就绪队列 + token 池**（jobserver：并发度全局恒定，子任务继承同一池）。cargo 自身就是这个模式的现役实现（jobserver crate）。三方案裁决：
- **门间 = 图执行 + 进程池**（两层）：xtask 顶层做图调度（就绪队列 + 增量短路：输入指纹未变 → 连下游一起跳过，ninja restat 同型）；每门独立子进程执行（崩溃/信号隔离，通信只走退出码 + JSON 结果）。
- **门内 = 线程池**（rayon）：文件级并行；崩溃域限于本门进程，可整门重试。
- **全局并发 = jobserver 式 token 池**：防"8 门 × 门内 8 线程 = 64 超订"，总并发 = 机器物理预算。
判据：崩溃隔离（进程边界）、总并发恒定（token 池）、增量（键缓存短路）。

### ④ 工具链锁与 CI 一致性

单一事实源 = `rust-toolchain.toml`（channel 钉 patch 版 + components）。本地 mise（rust core plugin 消费该文件，行为待第一步动作确认）+ CI rustup 直读同文件。非 Rust 工具：mise.toml 管本地，CI 不装 mise、按版本固定 URL+校验和直接下载（s1ngularity/Shai-Hulud 教训：工具分发渠道本身是攻击面）。一致性由 `xtask toolchain-check` 门断言（本地/CI `rustc -vV` 与锁文件一致；rustc 版本同时进三段键输入段——工具链漂移会自动使缓存失效，这是锁与缓存的闭环）。

### ⑤ hermeticity 自检清单（ADV 门的封闭性验证，八条）

1. **工具绝对路径**：PATH 扫描断言所有工具来自白名单目录；`rustc --version` 等与锁文件比对。
2. **env 白名单**：门声明读取的 env 集合；白名单外 env 不得影响判定与键（strict action env 同构；build.rs 的 rerun-if-env-changed 是第一执行者）。
3. **无网络**：门进程 egress=deny 沙箱或运行时断言无外连。
4. **时钟无关**：TZ=UTC、SOURCE_DATE_EPOCH 钉死、判定结果不含本地时间。
5. **双跑一致**：同键连续两次执行，输出逐字节相同（金标准 1）。
6. **隔离性**：触碰门输入集之外的文件 → 键不变、门不重跑（金标准 2，检验指纹覆盖度）。
7. **cwd 无关**：不同 cwd 各跑一次结果一致。
8. **输入完备**：文件集 + 工具链指纹 + xtask 自指纹三源齐备才允许发键；缺源即报门失败而非降级。

---

## 3. 2025–2026 前沿信号

1. **供应链武器化升级**（ci-ops 直接相关）：s1ngularity（2025-08-26/27，nx/@nx/powerpack 8 个恶意版本，~5 小时，武器化 AI CLI 找凭据，经 gist 外传；Wiz 2025-08-28，Nx postmortem 2025-09-05）+ Shai-Hulud npm 蠕虫（2025-09）→ 版本锁定 + 校验和 + 最小权限发布令牌是 2026 的基线不是加分项。
2. **cargo bindeps 仍在推进未稳定**：Cargo Book 快照仍列 `-Z bindeps`；tracking cargo#10444；1.90 dev-cycle（2025-10-01）继续 artifact-dependencies 设计——watch 档不变。
3. **编译器侧性能**：Rust 1.88（2025-06）宏解析器分配优化（宏重 crate 约 -5%）；Rust 1.90（2025-09）Windows x64 rustc PGO——Windows 上宏重 workspace 受益。
4. **sccache 0.9.x/0.10.x（2025）**：preprocessor cache mode 落地（0.9.1 2025-02，rust-lang CI 同步升级）；env 进键清单的审计必要性同步上升。
5. **任务运行器格局固化**：just 成为 Rust 项目默认入口（Rust Tool Index 2026-09 快照双雄之一，cargo-make 动量下滑）——ADV 用 xtask 做本体 + 可选 just 薄层与此一致。
6. **Bazel 9.x 滚动**（9.1.0 2026-04，9.2.0 计划 2026-06；2025 年生态兼容多列到 8.3.x）+ Bazel 8 起 visibility 默认强制——模块边界可执行化成为构建系统标配，ADV gate crate 边界照此收口。
7. **mise 月更、mise.toml 单文件化**（工具+env+任务；日历版本 2025.11.10 → 2026.9.12）——本地工具链体验的事实选择，但 CI 不依赖它是纪律。
8. **Pants 人力吃紧**（2025-09-16 社区悬赏 3–4 分钟性能 bug 讨论）——monorepo 构建系统赛道对 Rust 单语言项目持续无吸引力，反向支持"xtask 不迁移"。

## 4. 与其他域的交界

- **E 组**：Nix 内容寻址（§1C-17）为环境即代码预留接口；CA 仍在实验期，Windows 无原生支持。
- **wave-0/06**：sccache 自托管已在运行（§1C-15 只补键设计/env 审计，不重复部署方案）。
- **C 组**（CLI 启动/性能）：jobserver token 池实现注意 Windows 侧信号量语义（make 的管道 token 在 Windows 是句柄继承问题，cargo jobserver crate 已处理，直接复用）。
