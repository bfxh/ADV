# D5 包管理器与依赖解析

- 调研日期：2026-10-03（WebSearch/WebFetch；锚点标注在条目内）
- 服务对象：adv-sca（依赖图/锁定/解析）、ci-ops（离线/vendor/镜像）
- 口径：机制级 = 能落到 ADV 实现或门禁规则的一句话级描述；数字带来源锚；档位五档【吸收|有界|不吸收|watch|reference】

---

## 一、Python 系

### D5-01 uv 解析器与通用解析（uv #1/3）
- 定位：Astral 的 Rust 版 Python 包管理一站式工具（解析/锁/装一体），当前生态工程质量标杆。
- 可抄机制：
  1. PubGrub 求解驱动版本选择（2023 年重写后定型；失败错误报告走根因链，见 D5-18）。
  2. **通用解析（universal resolution）**：一份 `uv.lock` 覆盖全部平台 × Python 版本组合，不可满足处按 `resolution-markers` 把解析 fork 成分支（uv changelog 锚：lockfile 支持 resolution-markers 字段）。
  3. 锁漂移最小化：新版本发布**不**触发锁过期判定（官方文档原话口径："uv will not consider lockfiles outdated when new versions of packages are released"），升级只由 `uv lock --upgrade` 显式触发；重锁时偏好已锁版本。
- 档位：**吸收**
- 第一步动作：adv-sca 解析层直接用 pubgrub-rs（D5-19）；锁过期语义抄"声明变化才过期、版本发布不过期"。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（高频发布）。
- 链接：https://docs.astral.sh/uv/

### D5-02 uv 缓存布局（uv #2/3，本次实抓 docs 页）
- 定位：全局内容缓存，硬链接进各环境；ADV 缓存层/语料库布局的直接模板。
- 可抄机制（2026-10-03 抓 https://docs.astral.sh/uv/concepts/cache/ 锚）：
  1. **桶（bucket）结构**：wheels / sdists / git 仓库等各成一桶，**每桶独立版本号**；破坏性格式变更整桶 bump（文档举 uv 0.4.13 core metadata v12→v13 例），不兼容桶拒绝读写；同版本桶内变更保持前后兼容，多个 uv 版本可共享一个缓存目录。
  2. **按依赖类型的键策略**：registry 依赖尊重 HTTP 缓存头；直连 URL 依赖以 URL 为键；git 依赖以 resolved commit 哈希为键；本地目录以 `pyproject.toml`/`setup.py` 的 mtime 为键；`--find-links` 假定不可变按文件名缓存。
  3. **链接与并发**：缓存须与环境同文件系统才能硬链接，否则回退慢 copy；缓存 append-only、线程安全，目标环境有文件锁；`uv cache prune --ci` 区分"重下 wheel 便宜 vs 源码构建贵"。
- 档位：**吸收**
- 第一步动作：ADV 缓存/语料库目录实现 `<bucket>-v<N>` 布局 + "按解析结果（commit/内容哈希）为键"的哈希账；清理按重建成本分档。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://docs.astral.sh/uv/concepts/cache/

### D5-03 uv.lock 与 exact sync（uv #3/3，本次实抓 sync 页）
- 定位：锁文件与环境的强一致语义；ADV "锁定检查"门禁规则的直接来源。
- 可抄机制（2026-10-03 抓 https://docs.astral.sh/uv/concepts/projects/sync/ 锚）：
  1. 二档校验：`--locked` = 锁过期即报错（等价 `uv lock --check`）；`--frozen` = 不检查过期直接用锁。CI 两档：严格门用 locked，纯提速用 frozen。
  2. **exact sync 默认**：`uv sync` 会移除锁外包（`--inexact` 保留）；`uv run` 默认 inexact、`--exact` 转严格 —— 环境漂移的可执行判据。
  3. 组/extra 语义：dev 组（PEP 735）默认同步，"组排除优先于包含"；导出面：uv.lock 可导出 requirements.txt / pylock.toml（PEP 751）/ CycloneDX SBOM。
  4. preview：`audit.malware-check = true`（或 `UV_MALWARE_CHECK=1`）对锁文件跑 OSV 匹配 —— 包管理器开始内置恶意包检查（2026 信号）。
- 档位：**吸收**
- 第一步动作：xtask 门实现 locked/frozen 两档 + 漂移检测规则；SCA 输入复用 SBOM 导出路径。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://docs.astral.sh/uv/concepts/projects/sync/

### D5-04 PEP 751 pylock.toml
- 定位：2025-03-31 接受的标准锁格式，统一 requirements 钉版生态。
- 机制：固定 schema 的 `pylock.toml`（包名/版本/哈希/来源的不可变记录）；现实中各工具把它当**互操作/导出目标**而非替代品——uv（Charlie Marsh 2025-04 口径）认为 pylock 不足以替代 uv.lock（缺 uv 需要的元数据）；Poetry 仍用自有 poetry.lock。
- 档位：**有界**（读取器必做；不自研写回标准之外的字段）
- 第一步动作：多语言清单读取器支持 pylock.toml 只读解析。
- 许可证/成熟度：n/a（PEP 标准）/ 标准已接受、工具采用渐进（Pex/Pants 2025-10 仍在讨论支持）。
- 链接：https://peps.python.org/pep-0751/

### D5-05 pip-tools
- 定位：`requirements.in` → compile → 全量钉死 `requirements.txt` 的最小对照系。
- 机制：compile 产出传递闭包钉版；`--generate-hashes` 给每条目附 sha256 哈希账——"声明文件+哈希账"的最简形态，适合做 adv-sca 测试夹具语料。
- 档位：**reference**
- 第一步动作：无（夹具生成器用例）。
- 许可证/成熟度：BSD-3-Clause / 维护中（Jazzband 托管，节奏放缓）。
- 链接：https://github.com/jazzband/pip-tools

### D5-06 Poetry
- 定位：对照系：自有求解器 + poetry.lock。
- 机制：poetry.lock 头部 `content-hash`（pyproject 相关内容哈希）做过期判定——与 uv"声明变化才过期"语义同源，可互为印证。
- 档位：**reference**
- 第一步动作：无。
- 许可证/成熟度：MIT / 维护中（2.x，2025-01 起）。
- 链接：https://python-poetry.org/

---

## 二、JS/TS 系

### D5-07 pnpm 内容寻址 store（本次实抓机制）
- 定位：npm 生态的机制经典：内容寻址 + 硬链接 + 虚拟 store。
- 可抄机制：
  1. **全局内容寻址 store**（Linux `~/.local/share/pnpm/store`）：每包内容全机只存一份，项目内不复制。
  2. **硬链接布局**：`node_modules` 顶层是指向 `node_modules/.pnpm`（虚拟 store）的符号链接；虚拟 store 内每包目录 = 对全局 store 的**硬链接** + 指向其自身依赖的符号链接 ⇒ 严格隔离：未声明的依赖在 Node 解析路径上不可见（Nx 供应链事件讨论中被引用为安全面收益）。
  3. 平台回退：Windows 用 junction；硬链接跨文件系统不可用时回退 copy。
- 档位：**吸收**（对 ADV 缓存层/语料库：内容哈希去重 + 硬链接省盘 + copy 回退，同构方案）
- 第一步动作：ADV 语料库按内容哈希去重存储，引用计数代替猜测式 prune；扫描目标规范化为"仅声明依赖可见"。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://pnpm.io/motivation

### D5-08 pnpm-lock.yaml v9
- 定位：workspace 感知的 YAML 锁；读取器必支持格式。
- 机制：三段结构——`importers`（workspace 内每个包的声明）→ `packages`（版本 + `resolution.integrity` = sha512 SRI）→ `snapshots`（依赖图，键含 peer 依赖后缀，环/对等依赖表达完整）。
- 档位：**吸收**（读取器）
- 第一步动作：读取器解析 importers/packages 两段，integrity 直接入扫描账。
- 许可证/成熟度：n/a（格式）/ pnpm 9+（2024 起）。
- 链接：https://pnpm.io/settings

### D5-09 npm package-lock.json v3
- 定位：事实标准格式；v3（npm 7+）为当前主形态。
- 机制：`lockfileVersion: 3` 扁平化：单一 `packages` 表，键为 `node_modules/...` 相对路径，每项 `version/resolved/integrity`（SRI sha512）`/license/engines`；对比 v2 的 packages+dependencies 双表冗余。
- 档位：**有界**（读取器 + 完整性规则）
- 第一步动作：读取 packages 表；`npm ci`（锁与清单不一致即失败、不写 node_modules）作为门禁语义参照（见 D5-25）。
- 许可证/成熟度：n/a（格式）/ npm Artistic-2.0。
- 链接：https://docs.npmjs.com/cli/v10/configuring-npm/package-lock-json

### D5-10 bun
- 定位：Zig 单二进制一体化运行时（运行时+安装器+bundler+测试器），安装器性能常报 10–25× npm（第三方复测口径，非本方测量）。
- 机制：bun 1.2（2025-01）把二进制 `bun.lockb` 换成**文本** `bun.lock`（JSONC）——锁格式从二进制回退文本是"可 diff/可合并"方向信号；lifecycle 脚本默认不执行、需 `trustedDependencies` 白名单（供应链位）；全局缓存 + 并行下载。
- 档位：**有界**（读取器 watch bun.lock）
- 第一步动作：读取器加 bun.lock 解析。
- 许可证/成熟度：MIT / 维护中。
- 链接：https://bun.sh/docs/install/lockfile

### D5-11 Yarn PnP
- 定位：激进方案对照系：彻底取消 node_modules。
- 机制：用单个 `.pnp.cjs` loader 文件替代 node_modules，包解析交给运行时钩子；严格依赖可见性 + zero-install。**Yarn 4 起默认回到 node-modules linker，PnP 变 opt-in**——Vite/JetBrains/TS 生态摩擦（2025–2026 issue 持续）使 PnP 未成为主流。
- 档位：**不吸收**（机制课：改运行时解析路径的生态代价 > 收益）
- 第一步动作：无。
- 许可证/成熟度：BSD-2-Clause / 维护中。
- 链接：https://yarnpkg.com/features/pnp

---

## 三、Rust 系

### D5-12 cargo sparse index
- 定位：crates.io 元数据获取协议（RFC 2789），Cargo 1.70（2023-06）起对 crates.io 默认，git 索引废弃。
- 机制：索引 = 每个 crate 一组 JSON 行（每行一个版本记录），经普通 HTTP GET 按分桶路径取（`/{首2字符}/{次2字符}/{crate 名}`）；只拉所依赖 crate 的条目，带宽 O(依赖) 而非 O(全索引)；协议与求解器解耦（协议选择不影响版本选择）。
- 档位：**吸收**（adv-sca 若直接拉 crates.io 元数据，走 sparse+https 端点而非整库 clone）
- 第一步动作：依赖图构建走 `cargo metadata`（其下即 sparse）；自索引镜像按同布局缓存。
- 许可证/成熟度：MIT OR Apache-2.0（cargo）/ 维护中。
- 链接：https://rust-lang.github.io/rfcs/2789-sparse-registry.html

### D5-13 cargo 解析器（resolver v3 / PubGrub 迁移 / -Zminimal-versions）
- 定位：ADV 是 Rust 项目，cargo 本体即解析权威；本条为规则来源而非自研对象。
- 机制：`resolver = "3"`（Cargo 1.84 默认）MSRV-aware：优先选满足 `rust-version` 的版本；pre-release 仅显式请求可选；`-Zminimal-versions`（nightly）：取满足约束的**最低**版本，测试依赖下界真实性；PubGrub 迁移 = Rust 官方 Project Goal（2024 设计文档，2025 延续），要求向后兼容。
- 档位：**reference**
- 第一步动作：xtask 门加 `--locked` 强制；可选 `-Zminimal-versions` 下界冒烟（ci-ops）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中。
- 链接：https://doc.rust-lang.org/cargo/reference/resolver.html

### D5-14 Cargo.lock v4
- 定位：Rust 锁文件当前格式（v4，Rust 1.78 起，2024）。
- 机制：TOML，`[[package]]` 逐条 `name/version/source/checksum/dependencies`；路径依赖无 source/checksum 字段（本地包也入表）；`cargo --locked` 锁过期即失败；`cargo update --precise x.y.z` 钉指定版本。
- 档位：**吸收**（读取器 + 门禁规则来源）
- 第一步动作：adv-sca 读 Cargo.lock 建依赖图；门禁：CI 一律 `--locked`。
- 许可证/成熟度：n/a（格式）。
- 链接：https://doc.rust-lang.org/cargo/reference/config.html

### D5-15 cargo vendor 离线工程化
- 定位：离线可重复构建的标准做法。
- 机制：`cargo vendor` 把全部依赖源码落 `vendor/` 并生成 `.cargo/config.toml` 的 source replacement 配置；之后任何构建 `--offline` 可重复；checksum 校验照常生效。
- 档位：**吸收**（ci-ops）
- 第一步动作：ADV 自举 CI 采用 vendor + `--offline` 双保险。
- 许可证/成熟度：属 cargo / 维护中。
- 链接：https://doc.rust-lang.org/cargo/commands/cargo-vendor.html

### D5-16 国内镜像（rsproxy / TUNA / USTC）
- 定位：crates.io sparse 镜像端点；网络受限环境的 ci-ops 备件。
- 机制：镜像提供 `sparse+https://` 端点，`.cargo/config.toml` source replacement 指过去；索引与 crate 文件镜像，checksum 校验仍在本地做（完整性不依赖镜像诚实）。
- 档位：**reference**
- 第一步动作：仅当 CI 出境受限时启用。
- 许可证/成熟度：n/a（服务）/ 各镜像维护中。
- 链接：https://rsproxy.cn/

### D5-17 cargo-hack
- 定位：feature 组合测试工具（对照系）。
- 机制：对 feature 集合跑 `--each-feature` / `--feature-powerset` 的 check/build，暴露特性组合断裂——adv-sca 的"清单/匹配分离"同样存在组合面。
- 档位：**reference**（xtask 抄思路，不引依赖）
- 第一步动作：视 xtask 门数量再定。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（taiki-e 高频维护）。
- 链接：https://github.com/taiki-e/cargo-hack

---

## 四、通用机制与安全

### D5-18 PubGrub 算法
- 定位：版本求解下一代算法，源自 Dart pub（Natalie Weizenbaum 2018 文章）；npm 的旧式深回溯的问题在于错误不可解释。
- 机制：不盲目回溯；维护**不相容集（incompatibilities）**，冲突驱动学习（冲突链固化为 derived incompatibility），单元传播快速剪枝；失败时沿 **derivation tree** 输出"根因链"级人读解释（谁与谁在什么约束下冲突）。
- 档位：**吸收**（错误报告格式学它：解释必须带证据链——与 AGENTS.md 表述红线 15 同构）
- 第一步动作：adv-sca 求解失败输出对齐 derivation-tree 形态。
- 许可证/成熟度：n/a（算法文献）/ 算法稳定（2026 仍是主流选择）。
- 链接：https://medium.com/@nex3/pubgrub-2fb647050446

### D5-19 pubgrub-rs（本次实抓 docs.rs）
- 定位：PubGrub 的 Rust 参考实现；uv 生产在用，cargo 官方迁移目标。
- 机制（2026-10-03 抓 https://docs.rs/pubgrub 锚）：版本 0.4.0（页面日期 2026-09-22）、MPL-2.0、文档 100%；API：`DependencyProvider` trait（`choose_version`/`prioritize`/`get_dependencies`）+ 内置 `OfflineDependencyProvider`（内存依赖图）；失败返回 `PubGrubError::NoSolution(DerivationTree)`，`collapse_no_versions()` 简化报告；`prioritize` 可调排序（文档注明"可选版本最少的包优先"加速求解），`PackageResolutionStatistics` 支持冲突驱动排序；测试带 proptest + varisat（SAT 交叉验证）。
- 结论（重点②）：**直接用，不自写**。uv 生产验证 + cargo 官方迁移目标背书表达力；MPL-2.0 文件级 copyleft 对链接使用友好。
- 第一步动作：adv-sca 以 pubgrub 0.4 做依赖求解原型，用 OfflineDependencyProvider 跑离线场景。
- 许可证/成熟度：MPL-2.0 / 维护中（2026-09 仍有发布）。
- 链接：https://docs.rs/pubgrub

### D5-20 Resolvo
- 定位：受 uv 求解器启发的通用求解库（Rust），被 conda 采用（2025 检索锚）。
- 机制：环境标记（conda 式 condition）表达力强于纯版本约束；求解器层"库化"趋势的又一证据。
- 档位：**watch**（出现多生态求解需求再看）
- 第一步动作：无。
- 许可证/成熟度：BSD-3-Clause / 维护中。
- 链接：https://github.com/mamba-org/resolvo

### D5-21 Go MVS（最小版本选择）
- 定位：与"最大版本+求解器"对立的哲学参照（Russ Cox，go.dev/ref/mod）。
- 机制：列出全部需求后每包取**已列出的满足版本中最高者**（而非图上可达的最新版），求解确定性、无回溯；Go 1.17（2021）模块图剪枝 + 惰性加载：主模块直接/间接依赖全记录于 go.mod，间接依赖的 go.mod 免加载，工作量从 O(全图) 降为 O(直接依赖)。
- 档位：**reference**（可复现优先于最新的价值观；adv-sca 无需自研求解）
- 第一步动作：无。
- 许可证/成熟度：BSD-3-Clause（Go）/ 维护中。
- 链接：https://go.dev/ref/mod

### D5-22 依赖图 DAG 存储与环处理
- 定位：adv-sca 依赖图的数据结构选型依据。
- 机制：三套参照——cargo **拒绝包间循环**（crate 图必须是 DAG，同 crate 内模块循环允许）；pnpm snapshots 以"包名@版本+peer 后缀"为节点键的显式边表；uv.lock 每包条目自带依赖边（声明世界无环）。adv-sca 扫描侧面对的是**实际 import 图**，允许环 ⇒ 需要 SCC（tarjan）分解后再拓扑序。
- 档位：**吸收**
- 第一步动作：图存储用邻接表 `{节点: name@version, 边: Vec<节点>}` + tarjan SCC，环不报错、标记后走 SCC。
- 许可证/成熟度：n/a（设计模式）。
- 链接：https://doc.rust-lang.org/cargo/reference/resolver.html#dependency-resolution

### D5-23 SemVer 求解实践
- 定位：版本范围语法对照，供清单读取器正确解释约束。
- 机制：cargo caret `^`/tilde `~` 与"同大版本内兼容"规则；npm range + dist-tag；Python 用 PEP 440 specifier（epoch/通配符）。adv-sca 不自写解释，把范围交给 pubgrub 的 `VersionSet`（`Ranges` + `SemanticVersion`）。
- 档位：**reference**
- 第一步动作：读取器统一产出 pubgrub 版本集合，不自行解释范围。
- 许可证/成熟度：n/a（规范）。
- 链接：https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html

### D5-24 lockfile 合并冲突工程
- 定位：lockfile 单一大文档 ⇒ 双分支都动依赖即大面积冲突；ADV 自身工程与被扫描仓库共同面对。
- 机制：解法三件——git 自定义 merge driver（如 npm-merge-driver 类）；CI 用 `--locked`/`--frozen` 语义重生成代替手工合并；lockfile lint 类工具（safeguard 等，2025 检索锚）把"锁文件 vs 声明策略漂移"前置到 CI。
- 档位：**有界**
- 第一步动作：ADV 仓规：rebase 锁冲突一律"取一侧后重跑 lock 命令"，门禁挡漂移。
- 许可证/成熟度：n/a（工程实践）。
- 链接：https://git-scm.com/docs/gitattributes#_built_in_merge_drivers

### D5-25 lockfile 完整性与确定性安装判据（重点④）
- 定位：adv-sca "锁定检查"门禁规则的直接来源；全部走真路径可验证。
- 判据清单（六条）：
  1. cargo：`--locked` 强制（锁过期即失败）+ registry/vendor checksum 校验。
  2. npm：`npm ci` 语义（锁与清单不一致即失败、不写 node_modules）+ package-lock `integrity`（SRI sha512）。
  3. pnpm：CI 下 frozen-lockfile 默认生效 + `resolution.integrity`。
  4. uv：`--locked`（= `uv lock --check`）+ exact sync（锁外物即漂移）。
  5. 下界真实性：cargo `-Zminimal-versions` 冒烟（可选档）。
  6. 哈希账核对：锁表记录的 SRI/sha512 与本地实际文件哈希对照 ⇒ "锁表与磁盘一致"可测。
- 档位：**吸收**
- 第一步动作：门禁规则表落地以上六条，逐条绑定对应命令验证。
- 许可证/成熟度：n/a（规则集）。
- 链接：https://docs.npmjs.com/cli/v10/commands/npm-ci

### D5-26 供应链防线（包管理器侧；与 P8 交界）
- 定位：dependency confusion / typosquatting 的**包管理器侧防线**（执行侧归 P8）。
- 机制：
  1. npm trusted publishing GA（2025-07-31，GitHub Blog 锚）：CI 用 OIDC 短期令牌发布，取消长期 token；**provenance 默认开启**——Sigstore 签名的 SLSA 证明，记录 repo/workflow。
  2. pnpm `minimumReleaseAge`（2025-09 前后，Shai-Hulud 事件同期）：可配置延迟安装"新发布"版本，规避刚投毒窗口。
  3. 名称防线：scope 占位/私有 registry allow-list（Artifactory/Nexus 类防火墙）对抗内部名被公共源顶替。
  4. typosquat：发布侧对新包名与既有包的相似度校验；消费侧=adv-sca 输出"名称混淆/新发布年龄/无 provenance"信号。
- 档位：**吸收**（规则信号侧）
- 第一步动作：扫描报告输出 provenance 有无 + 发布年龄 + 名称编辑距离三类信号字段。
- 许可证/成熟度：n/a（机制集）/ npm、pnpm 均维护中。
- 链接：https://docs.npmjs.com/trusted-publishers ；https://pnpm.io/settings

---

## 五、2025–2026 前沿信号

1. **PEP 751 接受（2025-03-31）**：pylock.toml 成为标准；2025 内以"导出面"铺开（uv 支持 export pylock + CycloneDX SBOM），替代式采用仍渐进（uv 明确不弃 uv.lock，2025-04 口径）。
2. **npm trusted publishing GA（2025-07-31）**：OIDC + 默认 Sigstore provenance；npm 2025-09 公布供应链安全路线图（检索锚：github.blog / npm roadmap）。
3. **pnpm minimumReleaseAge（2025-09 前后）**："延迟安装新版本"成为包管理器内置新防线（Shai-Hulud npm 蠕虫事件驱动，时间锚为检索期口径）。
4. **cargo 解析器 PubGrub 迁移**延续为官方 Rust Project Goal（2024 设计文档 → 2025 goal）；resolver v3（MSRV-aware）自 Cargo 1.84 默认。
5. **包管理器内置恶意包检查**：uv `audit.malware-check`（preview）对锁文件跑 OSV 匹配（2026-10-03 实抓文档锚）——SCA 能力正在被包管理器前移吸收。
6. **Yarn PnP 退潮**：Yarn 4 默认 node-modules，PnP opt-in——改运行时解析路径的激进方案被生态否决，对 ADV 是"别自造解析路径"的反面教材。
7. **求解器库化**：pubgrub-rs 0.4.0（2026-09-22 页面锚）活跃；Resolvo（uv 系）被 conda 采用——自研求解器已无必要。

## 六、Top-3（直接可抄）

1. **uv 三件套**（D5-01/02/03）：缓存桶布局 `<bucket>-v<N>` + 按解析结果为键的哈希账 + 锁过期语义（声明变才过期）+ locked/frozen 二档 + exact sync —— adv-sca 缓存层与锁定检查的全套模板。
2. **pubgrub-rs 直接采用**（D5-18/19）：MPL-2.0、0.4.0 维护中、uv 生产验证；依赖求解 + derivation-tree 根因报告一次到位，不自写求解器。
3. **pnpm 内容寻址 store**（D5-07）：内容哈希去重 + 硬链接 + 跨文件系统 copy 回退 + 引用计数清理 —— ADV 语料库/缓存层的同构蓝本。

## 七、多语言清单读取器要点（重点⑤）

| 格式 | 载体 | 完整性字段 | 要点 |
|---|---|---|---|
| uv.lock | TOML | 各包 sdist/wheel 哈希 | 全平台一份锁；resolution-markers/fork 表达跨平台分支 |
| pylock.toml | TOML | hashes 数组 | PEP 751 标准；导出目标形态为主 |
| Cargo.lock | TOML（v4） | 逐包 checksum | 路径依赖无 checksum；本地包也入表 |
| package-lock.json | JSON（v3） | integrity（SRI sha512） | 扁平 packages 表，键=node_modules 路径 |
| pnpm-lock.yaml | YAML | resolution.integrity | importers/packages/snapshots 三段；snapshot 键含 peer 后缀 |
| bun.lock | JSONC | — | bun 1.2（2025-01）起替代二进制 bun.lockb |
| requirements.txt | 文本 | 可选 --generate-hashes | 最简夹具格式 |
| go.mod / go.sum | 文本 | go.sum 校验和行 | MVS 语义 + Go 1.17 图剪枝 |

## 八、来源（主要）

- uv 文档 cache/sync/projects 页（2026-10-03 抓取）：https://docs.astral.sh/uv/concepts/cache/ 、https://docs.astral.sh/uv/concepts/projects/sync/
- pubgrub-rs docs.rs（0.4.0，2026-09-22 页面锚）：https://docs.rs/pubgrub
- cargo：RFC 2789 https://rust-lang.github.io/rfcs/2789-sparse-registry.html ；resolver https://doc.rust-lang.org/cargo/reference/resolver.html
- PEP 751：https://peps.python.org/pep-0751/
- npm trusted publishing：https://docs.npmjs.com/trusted-publishers ；GitHub Blog 2025-07-31
- pnpm：https://pnpm.io/motivation 、https://pnpm.io/settings
- Go MVS：https://go.dev/ref/mod
- Yarn PnP：https://yarnpkg.com/features/pnp
