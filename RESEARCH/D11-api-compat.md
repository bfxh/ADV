# D11 调研：API 表面与兼容性工具（API/兼容性域）

日期：2026-10-03（检索当日；数字锚见各条）。范围：Rust 公开面/semver 工程、ABI、API 文档与示例验证、跨语言 breaking 检测对照。判读口径：机制级对象给全六段；扫视对象给一句。档位＝吸收|有界|不吸收|watch|reference。

---

## 一、机制级对象

### 1. cargo-semver-checks（Rust semver 违规检测）
- **定位**：对比两个版本 crate 的 rustdoc JSON，跑 700+ 条 lint 找 semver 违规（rust-tool.tools 索引页 2026-09 口径"700+ lints"；docs.rs 2026-10-03 查得 crate 版 0.50.0）。
- **可抄机制**：
  1. **双版本 rustdoc JSON diff 作为规则数据流**：规则不是读源码，而是对 `cargo rustdoc -- -Z unstable-options --output-format json` 产出的两个 JSON 做结构化查询——ADV 的"公开面门"可直接复用这条数据流（先落 JSON 快照再比对，基线与产物分离）。
  2. **规则 = Trustfall 查询**：每条 lint 是一条 Trustfall 查询（同作者 obi1kenobi 的查询引擎），经 trustfall_rustdoc 适配层同时支持多个 rustdoc JSON 格式版本——格式演进不推翻规则（goals.rust-lang.org 的 Rust 目标页以此为合并进 cargo 的解阻塞理由）。
  3. **公开库 API**：0.50.0 docs.rs 暴露 `Check`/`SemverQuery`/`Report`/`QueryOverride`/`LintLevel`/`Witness`（witness 可生成"证据"）——ADV 可以内嵌调用而不是只当 CLI 用。
- **规则编写机制（重点①）**：新规则 = 在 `src/lints/` 加一个定义 `SemverQuery` 的文件（含查询文本、`required_update`、`lint_level`、参考链接）+ 对应 rustdoc 测试夹具；仓库有脚手架脚本生成新 lint 文件与测试（tracking issue #236，2022-12 开）。**运行时不支持用户自定义 lint 集**——维护者在 2024-01 GitHub 讨论明确：不改代码就只能用内置集（见 github.com/obi1kenobi/cargo-semver-checks issues/… 讨论）。因此 ADV 给自家 API 写自定义规则的路径有三条：a) fork/vendor 后按上述文件约定加查询（成本最低，跟上游有合流成本）；b) 依赖公开库 API 把 `SemverQuery`/`Check` 嵌进 xtask（查询集仍受限于构建时编译进去的那些）；c) 用 rustdoc-types crate 自建一层轻查询（自由度最高，重造轮子）。对 ADV 建议 a+b 混合：门用上游 CLI，自家特有 API 约定用 b 通道加少量自定义查询。
- **档位**：吸收（wave-0 已定基调，此处补机制与自定义规则路径）。
- **第一步动作**：在 xtask-gates 里以"两份 rustdoc JSON + 内嵌 Check"跑门，规则超集里预留 `QueryOverride` 调严重级的口子。
- **许可证/成熟度**：Apache-2.0 OR MIT；维护中（2026 仍在发版，docs.rs 0.50.0）。
- **链接**：https://github.com/obi1kenobi/cargo-semver-checks ；https://docs.rs/cargo-semver-checks

### 2. cargo-public-api（公开 API 枚举与 diff）
- **定位**：枚举 crate 完整公开 API（含私有依赖链导致的传播项），支持 `cargo public-api diff` 对基线（最近发布版/任意 git ref 的 rustdoc JSON）做差异，被 Firefox 用于发现非预期 API 变化（项目 README 口径）。
- **可抄机制**：
  1. 枚举同样走 rustdoc JSON + rustdoc-types——与 cargo-semver-checks 同一数据源，ADV 可以共享"快照产物"（一次 rustdoc JSON，两个消费者）。
  2. diff 模式输出"签名清单的增删改"——适合作为 ADV CLI 门里**平铺式**（比 semver 规则更白盒）的公开面快照报告，人审友好。
  3. 对 `#[doc(hidden)]` 的处理与局限（semver-checks 一次只见一个 crate 的 JSON，2023-11 lobste.rs 讨论）提醒：门报告要标注覆盖边界。
- **档位**：吸收（作为 semver-checks 的"人读报告"补充，同一快照管线）。
- **第一步动作**：xtask-gates 增加二级命令：先出 public-api diff 文本，semver-checks 只对 diff 命中的项做规则判定。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（Enselic 长期维护，2025–2026 crates.io 持续发版）。
- **链接**：https://github.com/Enselic/cargo-public-api

### 3. rustdoc JSON 格式稳定性
- **定位**：rustdoc JSON 输出仍是 **unstable**（需 `-Z unstable-options --output-format json`），格式跨 rustc 版本会变；格式版本由 rustdoc-types crate 跟踪（`FORMAT_VERSION` 常量随版本递增）。
- **可抄机制**：cargo-semver-checks 的 trustfall_rustdoc 适配层同时支持多格式版本（见上）——ADV 若自建消费者，必须把"rustdoc JSON 格式版本"记进快照元数据，格式不匹配时降级为 SKIP 而不是绿（呼应"绿=SKIP 不算绿"）。
- **档位**：有界（不直接产门，但决定快照元数据字段）。
- **第一步动作**：adv 快照 schema 增加 `rustdoc_json_format_version` 字段，消费端校验。
- **许可证/成熟度**：rustdoc-types 为 MIT OR Apache-2.0；维护中（随 rustc 演进）。
- **链接**：https://github.com/rust-lang/rust/issues/90483 （rustdoc JSON 跟踪 issue）；https://github.com/Enselic/rustdoc-types

### 4. trycmd（CLI 快照测试）
- **定位**：assert-rs/snapbox 家族的 CLI 快照 harness（docs.rs 2026-10-02 查得 v1.2.1），启发自 trybuild 与 cram；枚举测试文件、跑命令、对 stdout/stderr 做快照比对。
- **可抄机制（重点②）**：
  1. **发现与格式**：`trycmd::TestCases::new().case("tests/cmd/*.trycmd")` 枚举 `.trycmd`/`.md`（markdown 围栏）/`.toml`（精细控制）三类文件；`.md` 里围栏外内容忽略——ADV 的门文档可以**直接当测试用例**。
  2. **dump→review→overwrite 三段流**：`TRYCMD=dump cargo test` 先产出 `.stdout/.stderr` 待审文件，人工过目后拷回 `tests/cmd`；回归时 `TRYCMD=overwrite` 重建。门输出冻结判据 = **基线文件入库 + CI 只跑比对（无 overwrite）+ 变更必须走 PR 审阅**。
  3. **省略与占位符**：`...`（整行通配）、`[..]`（行内通配）、`[EXE]`/`[ROOT]`/`[CWD]`、自定义 `insert_var`——版本号、时间戳、绝对路径这类动态内容在基线里写成占位符，避免每次构建重写基线；`.fail("...")` 给已知失败留标记；`fs.sandbox` 还能校验文件系统副作用（`.out/` 目录比对）。
- **对 ADV xtask 门的适用性**：直接把 xtask 子命令的门输出（含错误信息格式）冻结成 `.trycmd` 用例——门"输出契约"从此有机器判据；建议只冻结**格式**（占位符化动态值），不冻结**数值结果**（数值归门判定逻辑管）。
- **档位**：吸收。
- **第一步动作**：给 xtask 选 1–2 个最核心的门命令建 `tests/cmd/*.trycmd`，dump 流走一遍基线入库。
- **许可证/成熟度**：MIT OR Apache-2.0；维护中（epage/rust-cli 团队，v1.2.1 2026-10-02）。
- **链接**：https://docs.rs/trycmd ；https://github.com/assert-rs/snapbox

### 5. buf breaking（protobuf breaking 检测）
- **定位**：buf CLI 的 breaking 子命令，把当前 schema 编译成 image 与基线（git ref/模块/image）的描述符比对；规则按"严格度阶梯"分四类（buf.build/docs/breaking/rules 2026-10-03 查）。
- **规则清单（重点③，2026-10-03 官方文档口径）**：
  - **FILE**（最严，默认；保护"生成代码按文件"不破）：删除类 ENUM_NO_DELETE / MESSAGE_NO_DELETE / SERVICE_NO_DELETE / FILE_NO_DELETE / EXTENSION_NO_DELETE / ONEOF_NO_DELETE / FIELD_NO_DELETE / ENUM_VALUE_NO_DELETE / RPC_NO_DELETE；同值类 FIELD_SAME_TYPE/NAME/CARDINALITY/JSON_NAME/ONEOF/DEFAULT、ENUM_VALUE_SAME_NAME、ENUM_SAME_TYPE、MESSAGE_SAME_REQUIRED_FIELDS、RPC_SAME_REQUEST_TYPE/RESPONSE_TYPE/CLIENT_STREAMING/SERVER_STREAMING/IDEMPOTENCY_LEVEL；文件选项类 FILE_SAME_PACKAGE/SYNTAX/GO_PACKAGE/JAVA_PACKAGE…；预留类 RESERVED_ENUM_NO_DELETE / RESERVED_MESSAGE_NO_DELETE。
  - **PACKAGE**：同族规则但以包为界，允许类型在同包内跨文件移动（PACKAGE_NO_DELETE / PACKAGE_ENUM_NO_DELETE / PACKAGE_MESSAGE_NO_DELETE / PACKAGE_SERVICE_NO_DELETE / PACKAGE_EXTENSION_NO_DELETE + 同值/选项/预留族）。
  - **WIRE_JSON**（官方推荐最低档）：wire + JSON 双兼容；关键设计是**有条件删除**——FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED / FIELD_NO_DELETE_UNLESS_NAME_RESERVED / ENUM_VALUE_NO_DELETE_UNLESS_NUMBER_RESERVED / ENUM_VALUE_NO_DELETE_UNLESS_NAME_RESERVED，即"删了但编号/名字已进 reserved 就放行"；类型宽容 FIELD_WIRE_JSON_COMPATIBLE_TYPE/CARDINALITY（如 int32↔uint32）。
  - **WIRE**（最宽）：只保二进制可解；FIELD_WIRE_COMPATIBLE_TYPE/CARDINALITY（repeated↔map 通过；int32/uint32/int64/uint64/bool 互通）。
  - **选档哲学**：官方明确"选一档就锁一档"，不鼓励混选/逐条排除——规则的组合要保持连贯保证。
- **对 adv-sca 的启发**：lockfile/manifest 的 breaking 检测可套这套三段式——a) 分层（锁文件内容级 / 依赖图级 / 构建可解级）对应 FILE/PACKAGE/WIRE 的"谁会破"分层；b) **UNLESS_*_RESERVED 的例外通道**对应旧仓"编号断档"教训：删除必须有前置的"预留/废弃"动作才放行；c) 规则命名（对象_NO_DELETE / 对象_SAME_属性 / 对象_WIRE_COMPATIBLE_属性）是现成的规则命名法。
- **档位**：吸收（机制与规则分类法；buf 本体不引入）。
- **第一步动作**：adv-sca 写 lockfile/manifest breaking 规则清单时按 `对象_动作_条件` 命名 + 三层分档 + 预留豁免通道，先出规则表再写代码。
- **许可证/成熟度**：buf CLI Apache-2.0；维护中（bufbuild，Go）。
- **链接**：https://buf.build/docs/breaking/rules/ ；https://github.com/bufbuild/buf

### 6. oasdiff（OpenAPI diff）
- **定位**：Go 写的 OpenAPI diff/breaking 检测，支持 3.0–3.2；变更分类数有两个口径：470+ 类（SSRN 2026-06 "API Governance as a Pre-Registration Protocol" 论文口径）与 755 种（codingtocontent 博客口径）——量级为数百，具体以仓库枚举为准。
- **可抄机制**：每类变更标 severity（breaking / non-breaking）分级输出；CI 走 oasdiff-action 在 PR 上出报告并可设失败阈值；带 deprecation 相关检查（deprecation 宽限期思路——给"标记废弃→真正删除"留窗口）。
- **档位**：有界（机制借鉴：变更类别枚举 + severity + 宽限期三件套，与 buf 档位思想互证）。
- **第一步动作**：adv-sca 规则表把"breaking/非 breaking/宽限期内"三态做进报告输出。
- **许可证/成熟度**：Apache-2.0；维护中（oasdiff/oasdiff；注意 oasdiff-action 0.0.51 前有 CVE 记录，引用需固定新版）。
- **链接**：https://github.com/oasdiff/oasdiff

### 7. abidiff / libabigail（ELF ABI 检查，Linux 角度）
- **定位**：libabigail 的 abidiff 对比两个 ELF 共享库的 ABI（从 DWARF/CTF/BTF 调试信息重建类型）出差异报告；abipkgdiff 扩展到包级并把变化分"有害/无害"（Fedora 文档口径）。
- **可抄机制**：1) 调试信息作为 ABI 真相源（对应 Rust 侧的 rustdoc JSON——"从编译产物元数据反推表面"同构）；2) 抑制文件（abignore）允许声明"已知豁免"——对应 ADV 门需要豁免清单机制；3) 有害/无害分级输出。BTF 处理比 DWARF 快且小（Linux Plumbers 2024 报告口径）。
- **档位**：有界（ADV 本体是 Rust，若 adv-sca 未来扫 C/C++ 目标或 FFI 边界才引入；机制现在就抄分级+豁免清单）。
- **第一步动作**：在 ADV FFI/extern 块相关规则里引用"ABI 报告 + suppression 豁免"模式。
- **许可证/成熟度**：LGPL-3.0-or-later；维护中（2.7/2.8 均于 2025 发布，GNU Tools Cauldron 2025 有"ABI change analysis in Libabigail 2.8"报告）。
- **链接**：https://sourceware.org/libabigail/

### 8. cbindgen（C 头文件生成）
- **定位**：从 Rust crate 语法树（syn 解析，非 rustdoc JSON）生成 C/C++ 头文件；`#[cbindgen::annotate]`/配置文件控制导出项与命名。
- **可抄机制**：头文件本身即"FFI 公开面快照"——对生成的头文件做 git diff 就是最朴素的 FFI 表面门（生成物入库 + diff = 门）；注释标注驱动的导出选择，可借鉴为 ADV 的"显式标记才进公开面"思想。
- **档位**：有界（ADV 若提供 C ABI 就启用；否则 reference）。
- **第一步动作**：若 adv-sca 需要被 C/FFI 嵌入，先跑一次 cbindgen 建头文件基线。
- **许可证/成熟度**：MPL-2.0（GitHub 仓库许可标注）；维护中（mozilla 侧项目）。
- **链接**：https://github.com/mozilla/cbindgen

### 9. MSRV 工程化（rust-version 字段 + cargo-msrv + resolver）
- **定位（重点④）**：三层机制——a) 声明层：`Cargo.toml` 的 `rust-version` 字段（Cargo 1.56 起）；b) 解析层：MSRV-aware resolver 于 Cargo 1.84 落地，edition 2024 的 resolver v3 把 `incompatible-rust-versions` 默认从 "allow" 改为 "fallback"（Rust 1.84 发布说明 / edition-2024 resolver v3 口径）——依赖解析自动避开超 MSRV 的依赖版本；c) 实证层：cargo-msrv 用二分法在真实工具链上编译测出实际 MSRV（cargo-msrv Book）。
- **可抄机制**：ADV 三层全抄：字段声明进模板 + CI 矩阵跑"MSRV 那一档最低工具链 + 最新稳定"两端 + 门里校验依赖图与声明的 MSRV 一致（cargo-msrv `verify` 语义）。文档侧：crate 文档明示 MSRV 政策（哪些 bump 算 breaking 在 0.x 下需要口头约定，见 §13）。
- **档位**：吸收。
- **第一步动作**：xtask-gates 增加 msrv 门：读 `rust-version` → `cargo +<msrv> check`（或 cargo-msrv verify）→ 失败即红。
- **许可证/成熟度**：cargo-msrv MIT OR Apache-2.0；维护中（cargo-msrv Book：gribnau.dev）。resolver 行为随 Cargo 稳定通道演进，以所用 Cargo 版本发布说明为准。
- **链接**：https://rust-lang.github.io/rustup/…/（Cargo 1.84 changelog）；https://github.com/rust-lang/cargo-msrv ；https://users.rust-lang.org（2025-10 "SemVer philosophies in ecosystems" 讨论认为生态 MSRV 处理"基本已解决"——机制成熟度旁证）

### 10. doctest 机制（rustdoc / mdBook）
- **定位**：rustdoc 把每个代码示例编译成独立 crate 并运行（`cargo test --doc`），示例从此不会烂；标记体系：`no_run`（编译不跑）、`ignore`（完全跳过）、`compile_fail`（断言编不过才算过——反例文档的机器判据）、`should_panic`、`#` 前缀隐藏行。
- **可抄机制**：1) **compile_fail 作为"反例规则"测试法**——ADV 文档里"错误用法示例"可以用同样语义做门；2) mdBook 的 `mdbook test` 复用 rustdoc 跑书内示例（mdBook 自身处于维护模式，社区口径，2023 起更新放缓——选型注意）；3) doctest 与 semver 的交叉点：公开 API 示例在 API 变更时编译失败＝天然回归门。
- **档位**：吸收（ADV 文档与 xtask 门的示例验证直接用 doctest + compile_fail 语义）。
- **第一步动作**：ADV 仓库约定：公开命令/规则文档的示例一律可被 `--doc` 跑；反例标 compile_fail 语义。
- **许可证/成熟度**：rustdoc 属 rust-lang（Apache-2.0 OR MIT）；mdBook MIT OR Apache-2.0，维护中（放缓）。
- **链接**：https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html ；https://rust-lang.github.io/mdBook/

---

## 二、扫视对象（一句机制 + 档位 + 链接）

11. **Rust ABI 稳定性现状**——语言层无跨编译器版本稳定 ABI（Rust Reference ABI 章口径），`repr(C)` 与 `extern "C"` 是边界内稳定通道 → ADV 的 FFI 表面检查只承诺 C ABI 层 | 有界 | https://doc.rust-lang.org/reference/abi.html
12. **semverver**（rust-lang/semverver）——rustc 插件式 semver 检查，更新滞后于现代 rustc（ crates.io/仓库最后活动口径，2024 前后停滞）| 不吸收（已被 cargo-semver-checks 取代路线）| https://github.com/rust-lang/semverver
13. **cargo-breaking**——静态分析源码枚举公开项再比对版本出 breaking 报告（crates.io 条目口径）| watch（与 semver-checks 数据流不同：源码 vs rustdoc JSON，若后者不满足再回头看）| https://crates.io/crates/cargo-breaking
14. **cargo-smart-release**（ferrous-systems）——semver 感知的发布自动化（版本 bump 建议/changelog/发布执行）| watch（ADV 发布流水线设计时回看）| https://github.com/ferrous-systems/cargo-smart-release
15. **abi-compliance-checker（ABICC）**——C/C++ 库源级+二进制级兼容检查、出 ABI dump 报告；Debian 2025-11 仍有更新（搜索结果口径）| reference（与 libabigail 二选一时优先 abidiff）| https://github.com/lvc/abi-compliance-checker
16. **snapbox / insta-cmd / expect-test**——CLI 快照除 trycmd 外的路线：snapbox 是 trycmd 底座、适合"单点+定制"；insta-cmd 是 insta 家族的命令快照；expect-test（rust-analyzer 系）内联 expect! 快照 | reference（选型依据：大量粗粒度用例→trycmd，单个定制→snapbox）| https://docs.rs/snapbox ；https://crates.io/crates/insta-cmd ；https://docs.rs/expect-test
17. **gorelease**（golang.org/x/exp/cmd/gorelease）——Go 模块发布前对比上一版本 API，自动建议下一个 semver 版本号（x/exp，BSD 许可）| watch（"diff→建议版本号"的自动化思路可进 ADV 发布流程）| https://pkg.go.dev/golang.org/x/exp/cmd/gorelease
18. **semver 0.x 陷阱与依赖 pinning**——Cargo 对 0.x 特殊处理（0.2.3↔0.2.4 兼容、0.3.0 不兼容，Cargo 文档 semver 章）；npm 的 `^`/`~` 与 Go v0"零保证"口径各不同 → ADV 的依赖政策必须写明自家 0.x 约定，区间（`>=x, <y`）与锁文件双轨 | 吸收（写进 ADV 版本政策文档）| https://doc.rust-lang.org/cargo/reference/semver.html
19. **#[deprecated] 通道**——`#[deprecated(since = "x.y.z", note = "...")]`：since 锚 + 说明必填；rustdoc 渲染、使用者触发编译警告，形成"标记→警告→删除"的自然阶梯（Rust Reference 口径）→ **重点⑤**：ADV 工具面/规则废弃照抄此形状：规则删除前必须先带 `since` 标记进废弃名单并保留至少一个版本周期，diff 门对"未废弃即删除"报警——把旧仓"编号断档"教训机制化为"无 since 锚的删除＝breaking"（与 buf 的 UNLESS_*_RESERVED 例外通道同构）| 吸收 | https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-deprecated-attribute
20. **cargo-semver-checks 自定义规则的运行时限制**——维护者 2024-01 明确不提供运行时加载自定义 lint（不改代码只能内置集）；0.50.0 起公开库 API（`SemverQuery`/`Check`/`QueryOverride`，docs.rs 2026-10-03 查）部分缓解：可编程调严重级与内嵌执行，但新增查询仍需编译期进入 | 有界（ADV 自定义规则走 fork/vendor + 库 API 双通道，见 §1）| https://docs.rs/cargo-semver-checks

---

## 三、2025–2026 前沿信号

- **cargo-semver-checks 走向官方化**：Rust 目标页（goals.rust-lang.org）把"解决合并进 cargo 的阻塞项"列为目标（trustfall 多格式适配是关键解法）；lint 数到 700+（2026-09 corrode.dev 索引口径）。ADV 应按"上游会吃进 cargo"预期设计门（CLI 兼容层要薄）。
- **MSRV-aware resolver 成为默认**：Cargo 1.84（2024-12）+ edition 2024 resolver v3 默认 `fallback`——MSRV 从"自觉"变"机器默认"，cargo-msrv 的 verify 用法在 2025 生态讨论（users.rust-lang.org 2025-10）中被视为基本解决。
- **libabigail 活跃迭代**：2.7/2.8 于 2025 连发（Cauldron 2025 报告口径），BTF 路线（快、小）代表"从重调试信息转向轻量元数据"方向，与 rustdoc JSON 路线同构。
- **快照测试工具链持续活跃**：trycmd v1.2.1（2026-10-02，docs.rs 查）——snapbox 家族仍是 rust-cli 官方推荐位。
- **oasdiff 进入学术评估口径**：SSRN 2026-06 论文以其 470+ 变更类别为 API 治理评估基准——"变更类别枚举 + severity"已成可引用的方法论。
- **buf 规则哲学显式化**：官方文档强调"选一档锁一档、不做逐条混排"——breaking 规则集的连贯性优先于灵活性。

## 四、Top-3（对本项目）

1. **cargo-semver-checks 的规则数据流与自定义规则三通道**（§1/§20）：双版本 rustdoc JSON + Trustfall 查询是主数据流；ADV 自家 API 规则走"vendor 加查询 + 库 API（Check/SemverQuery/QueryOverride）内嵌"双通道，快照元数据记 rustdoc JSON 格式版本。
2. **trycmd 冻结 xtask 门输出契约**（§4）：`.trycmd`/`.md` 用例 + dump→review→overwrite 流 + 占位符省略动态值；判据＝基线入库、CI 只比对、格式冻结而数值归门逻辑。
3. **buf breaking 的规则分类法移植 adv-sca**（§5）：`对象_动作_条件` 命名 + 严格度分档（内容级/结构级/可解级）+ UNLESS_*_RESERVED 预留豁免通道——直接承载"废弃先留锚、未废弃即删＝breaking"的 deprecation 机制（§19）。
