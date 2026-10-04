# X9 FFI/跨语言绑定 调研

- 日期：2026-10-04（当日 WebSearch 检索；数字/版本均带锚）
- 范围：Rust 侧对外/对内的 FFI 机制盘点，服务 ADV（本地优先，Rust 主体 + Python 白名单场景）。
- 与 wave-0 的关系：X3（schema 生成）已定 FlatBuffers，本文不重复；08 插件 ABI 只从 FFI 角度补证。
- 口径：机制级条目给全格式；扫视条目一句话。license/成熟度逐条标注。

---

## 一、机制级条目（9 条）

### 1. PyO3 —— Rust 扩展 Python 的主路径
- **定位**：Rust 库编译为 CPython 原生扩展（.pyd/.so），#[pyfunction]/#[pymodule] 宏直接暴露 Python 对象模型。
- **可抄机制**：
  1. Bound API（0.23 起）：`Python::attach` / `Bound<'py, T>` 取代旧 GIL 生命周期 API——同一套代码兼容有 GIL 与自由线程构建（检索到 0.28.x 已用 attach 风格，2026-10）；
  2. abi3 特性（abi3-py38…）：按 CPython Limited API 编译，一个 wheel 通吃多版本（pyo3.rs「Building and distribution」文档）；
  3. `Python::allow_threads`：长任务中释放 GIL/线程令牌，避免 Python 侧卡死。
- **档位**：主推（Python 白名单场景若走"Rust 引擎 + Python 壳"，这是默认路径）。
- **第一步动作**：在 ADV 评测脚手架里做一个 pyo3 demo 模块：abi3-py311 + maturin 出 wheel，量构建时间与 wheel 体积，与 ctypes 方案（见 15 条）对比。
- **许可证/成熟度**：MIT OR Apache-2.0；成熟（crates.io 头部 crates，0.26 于 2025-09 发布——检索时 crates.io 元数据约 11 个月前；持续活跃）。
- **链接**：https://github.com/PyO3/pyo3 · https://pyo3.rs

### 2. maturin —— pyo3 的构建/分发后端
- **定位**：pip-installable 构建后端，负责把 Rust crate 打成 wheel（含 manylinux/musllinux 标签），支持 abi3 与 PyPy。
- **可抄机制**：
  1. abi3 wheel：每平台架构 1 个 wheel（如 cp311-abi3-win_amd64）替代"每 Python 版本 × 平台"矩阵（maturin issue #2893 明确无 abi3 时需逐解释器构建）；
  2. `maturin develop` 本地迭代 + `maturin build --strip` 控制体积；
  3. musllinux/manylinux 策略即分发判据（maturin.rs 分发文档）。
- **档位**：主推（与 pyo3 绑定使用）。
- **第一步动作**：CI 加 maturin 构建 job，先只出 win_amd64 + manylinux_2_28 x86_64 两个 abi3 wheel，记录构建耗时基线。
- **许可证/成熟度**：MIT OR Apache-2.0；成熟（PyPI 生态标准工具之一）。
- **链接**：https://github.com/PyO3/maturin · https://www.maturin.rs

### 3. PyO3 子解释器（sub-interpreter）现状核查
- **定位**：PEP 684（3.12 per-interpreter GIL）/ PEP 734（3.14 `concurrent.interpreters` 入标准库，2025-10 随 3.14 落地）之后，扩展模块需 multi-phase init + 无全局静态才能支持子解释器。
- **现状**：PyO3 尚无完整子解释器支持——官方 issue #1474（PEP 587 安全初始化封装）长期开放；discuss.python.org「Supporting Per-Interpreter GIL from Rust (PyO3)」(2023-10) 列出 thread-state 切换等硬问题；主要障碍是 PyO3 宏产物里的静态状态。
- **档位**：观察（若 ADV 的 Python 白名单需要多租户/多解释器隔离，pyo3 不是现成答案——那是 adv-sandbox 的 WIT/wasmtime 路线的加分项）。
- **第一步动作**：无（订阅 pyo3#1474 即可；出现 multi-phase init 支持再评估）。
- **许可证/成熟度**：随 pyo3；此子能力未成熟。
- **链接**：https://github.com/PyO3/pyo3/issues/1474

### 4. napi-rs —— Node 侧预编译原生模块
- **定位**：基于 N-API（Node 稳定 ABI）的 Rust→Node 绑定框架，核心卖点是 CI 矩阵产出的 per-platform 预编译 npm 包，用户 npm install 即得，无需本机编译。
- **可抄机制**：
  1. N-API 稳定 ABI ⇒ 每个"平台×架构"一个 .node 二进制，与 Node 版本解耦（区别于 NAN/node-gyp 的逐版本编译）；
  2. v3（2025-07-07 官宣，napi.rs）增加 WASM 支持 + 无预编译平台的 fallback 包（纯 JS/wasm 兜底）；
  3. 官方模板（napi-rs/package-template-pnpm）即标准预编译矩阵：linux-gnu/musl × x64/arm64、darwin x64/arm64、win32 x64/arm64。
- **档位**：观察（ADV 目前无 Node 生态交付物）。
- **第一步动作**：无。watch 判据：ADV 决定发布 Node/JS 生态工具（如 eslint/CLI 插件）时启用；届时直接抄官方模板 CI 矩阵，先出 win/linux/mac 六包 + wasm fallback。
- **许可证/成熟度**：MIT OR Apache-2.0；v3 已稳定（2025-07 官宣；SWC/rspack/turbopack 系生产在用）。
- **链接**：https://napi.rs

### 5. cxx —— C++ 双向安全互操作
- **定位**：dtolnay 的 C++/Rust 双向桥：`#[cxx::bridge]` 宏描述共享类型/函数，生成两侧对称绑定；类型层强制（String/&str/UniquePtr/Box/Vec 等）。
- **可抄机制**：
  1. "桥描述即契约"：bridge 模块同时生成 Rust 侧与 C++ 侧代码，两侧类型不匹配即编译失败；
  2. 不透明类型（Opaque<T>）+ UniquePtr 包装跨边界所有权，边界外仍是各自语言语义；
  3. panic 跨 FFI 边界是 UB ⇒ cxx 生成层自动 catch_unwind（同族问题在自写 extern "C" 时必须自己扛）。
- **档位**：备用（ADV 纯 Rust，无 C++ 主体；仅当引入 C++ 依赖库时用）。
- **第一步动作**：无；若将来包 C++ 库，先查该库是否已有 cxx 桥现成品。
- **许可证/成熟度**：MIT OR Apache-2.0；成熟（Chromium 2021 起生产使用；常年 1.0.x）。
- **链接**：https://github.com/dtolnay/cxx

### 6. diplomat —— IDL 先行（以 Rust 为 IDL）的多语言生成器
- **定位**：单向（Rust→外部）多语言绑定工具：proc-macro 标注 Rust API，diplomat-tool 生成 C/C++/JS(wasm) 高层开箱绑定；ICU4X 的绑定层。
- **可抄机制**：
  1. "不用外部 IDL"：以 Rust 类型系统当 IDL，单一定义源（Manish 2026-06 博文重申该定位）；
  2. 生成的是高层 ready-to-use 绑定而非裸头文件——C++ 侧有 RAII 包装、JS 侧有 wasm 胶水；
  3. 与 cbindgen 分工：cbindgen 只出 C/C++ 头，胶水自写；diplomat 连胶水一起出。
- **档位**：观察（0.x 版本，API 未冻结）。
- **第一步动作**：无。判据见第三节（③ cbindgen vs diplomat）。
- **许可证/成熟度**：MIT OR Apache-2.0；0.x（ICU4X 生产使用是成熟度背书，但工具 API 未 1.0）。
- **链接**：https://github.com/rust-diplomat/diplomat · http://manishearth.github.io（2026-06 Diplomat 文）

### 7. WIT / WebAssembly Component Model（wasmtime 宿主）—— 插件接口的 FFI 角度
- **定位**：类型化组件 ABI：WIT 文件定义接口（record/variant/enum/resource），wit-bindgen 生成宿主与 guest 双向绑定；wasmtime 自 2024 初随 WASI 0.2 提供首个完整稳定实现（`wasmtime::component` API，docs.wasmtime.dev）。
- **可抄机制**：
  1. 版本化即接口化：WIT world + package 版本替代"手写 ABI 版本号字段"，接口演进可 diff；
  2. resource 类型：宿主与 guest 间传句柄而非裸指针，天然避开 C ABI 的所有权/生命周期坑；
  3. 宿主调用 guest 前无共享内存 ⇒ 与 adv-sandbox 的隔离目标同构（untrusted 插件不用再叠一层方案）。
- **对 C ABI 的对比（⑤ 的答案）**：C ABI 零依赖、任何语言 dlopen 即用，但指针/字符串/资源/版本全自管，接口一大就脆；WIT 换来类型安全 + 沙箱 + 自动绑定，代价是宿主内嵌 wasmtime（二进制体积与编译时间显著上涨，wasmtime 依赖本身重）以及 guest 必须能编译到 wasm。实测批评点：2026-02 Lobsters 讨论指出组件模型对引用类型支持仍弱——插件接口应避免借用密集型设计。
- **档位**：主推（adv-sandbox 插件面；与 wave-0 08 决议互证而非纠偏：08 若定 C ABI 作最小自托管路径，与 WIT 并存不冲突——C ABI 给可信自托管 CI，WIT 给不可信插件）。
- **第一步动作**：用 wasmtime 写一个 20 行的 WIT host+guest demo（一个 `analyze(path: string) -> list<finding>` world），量宿主二进制增量与冷启动，作为 adv-sandbox 决议的实测锚。
- **许可证/成熟度**：WIT 规范公开（Bytecode Alliance）；wasmtime Apache-2.0 WITH LLVM-exception；成熟（Component Model 稳定已逾两年半，wasmtime 1.0+）。
- **链接**：https://component-model.bytecodealliance.org · https://docs.wasmtime.dev · https://eunodium…（状态综述：eunomia.dev「WASI and Component Model: Current Status」2025-02）

### 8. unsafe/FFI 边界测试矩阵（Miri 盲区 + Windows sanitizer 可用性）—— ④ 的答案
- **定位**：FFI 的 unsafe 边界无法靠单一工具全覆盖，需要矩阵化分工。
- **可抄机制（矩阵本身）**：
  1. **Miri（nightly）**：查 Rust 侧 UB/别名违规，但**不检查外部函数调用触发的违规**——FFI 调用仅限内置 shim（部分 libc），网络不支持（Rust Project Goals 文档原话：不支查找 foreign function calls 触发的别名违规）。⇒ 放 Linux CI；FFI 路径用 `#[cfg(miri)]` 以纯 Rust stub 替换外部库后再跑。
  2. **ASan**：`-Zsanitizer=address` 在 x86_64-pc-windows-msvc 可用（nightly + `-Zbuild-std`，依赖 VS 自带 clang_rt.asan 运行时）；**LSan 在 windows-msvc 不支持**（rust#88132）。⇒ Windows CI 跑 ASan 只查内存错误，不查泄漏。
  3. **valgrind/LSan**：仅 Linux/macOS。⇒ 泄漏检测统一放 Linux CI（valgrind 或 `-Zsanitizer=leak`）。
  4. **TSan**：windows-msvc 不可用（windows-gnu 有实验性支持，2024 起推进）；数据竞争检测同样放 Linux。
- **测试矩阵（落地判据）**：Linux CI = Miri（stub 化 FFI）+ valgrind/LSan + TSan；Windows CI = ASan(msvc)（内存错误）+ 手工/Dr. Memory 补泄漏抽查；矩阵必须在真实 C 依赖存在时才判定"绿"——全部 SKIP 不算绿（同 workspace 护栏）。
- **档位**：主推（perf-core 质量门）。
- **第一步动作**：在 ADV CI 加一个 `ffi-gate` job：ubuntu 跑 `cargo +nightly miri test -p <ffi 层>` + ASan 构建；windows 跑 `-Zsanitizer=address` 冒烟。Miri 慢，只圈 FFI 边界 crate，不跑全仓。
- **许可证/成熟度**：Miri/rustc 官方工具链（MIT OR Apache-2.0）；均为 nightly 功能，属"官方但未稳定"档。
- **链接**：https://github.com/rust-lang/miri · https://github.com/rust-lang/rust/issues/88132

### 9. UniFFI —— 一份定义出 Kotlin/Swift/Python
- **定位**：Mozilla 的多语言绑定生成器：UDL 文件或 proc-macro 定义 API，生成 Kotlin/Swift/Python 绑定 + 底层 C ABI 胶水。
- **可抄机制**：
  1. proc-macro 派生（`#[derive(ObjectRecord)]` 风格）直接在 Rust 类型上标绑定面，无独立 IDL 文件漂移问题；
  2. scaffolding（C ABI 层）+ bindings（各语言层）双层生成——与"先定最小 C ABI、再包各语言"的手工流程同构，可直接借鉴其分层命名；
  3. 官方指南声明 Kotlin/Swift/Python 三语全支持、Ruby 部分支持（mozilla.github.io/uniffi-rs）。
- **档位**：备用/参考（ADV 无移动端交付；Python 路径已有 pyo3/ctypes 两案，UniFFI 是第三选项——其价值是"若将来同时要 Python+Kotlin/Swift"才成立）。
- **第一步动作**：无；登记其分层设计供 08 插件 ABI 的"最小 C ABI + 语言胶水"分层参考。
- **许可证/成熟度**：MPL-2.0；成熟（Mozilla Firefox 移动组件 Glean/Nimbus 生产；iroh 2025-02 也公开其 FFI 绑定实践）。
- **链接**：https://github.com/mozilla/uniffi-rs

---

## 二、扫视条目（7 条）

| # | 名称 | 机制一句 | 档位 | license / 成熟度 | 链接 |
|---|------|----------|------|------------------|------|
| 10 | cbindgen | 从 Rust `extern "C"` 注解生成 C/C++ 头文件（build.rs 集成），只管头、胶水自写；Servo 出身 | 备用（自托管 CI/插件要 C ABI 时主用） | MPL-2.0 / 成熟 | https://github.com/mozilla/cbindgen |
| 11 | safer-ffi | proc-macro 把 Rust 类型转 `#[repr(C)]` 并生成导出符号清单（inventory），比手写 extern 更不易错 | 参考 | MIT OR Apache-2.0 / 0.1.x，活跃度一般 | https://github.com/getditto/safer-ffi |
| 12 | wasm-bindgen | Rust↔JS 高层绑定：wasm ABI + 自动生成 JS/TS 胶水与 .d.ts（浏览器/bundler 场景，与 napi-rs 的 Node 场景互补不重叠） | 观察 | MIT OR Apache-2.0 / 成熟 | https://github.com/rustwasm/wasm-bindgen |
| 13 | bindgen | C 头文件→Rust 声明（libclang 驱动），Rust 调 C 的标准入口；坑：C++ 支持有限、需处理布局与不完整类型 | 参考（ADV 引 C 库时才需要） | BSD-3-Clause / 成熟（rust-lang 官方仓库） | https://github.com/rust-lang/rust-bindgen |
| 14 | wasmtime 宿主成本 | 内嵌 wasmtime 的代价项：二进制体积/编译时间显著上涨、冷启动开销——X9-07 决议前必须实测记账 | 有界（随 07 走） | Apache-2.0 WITH LLVM-exception / 成熟 | https://wasmtime.dev |
| 15 | Python ctypes/cffi | Python 侧 `dlopen` 加载纯 C ABI 动态库：**零编译链**（无需用户装 Rust 工具链）、随主程序一个 dll/so 分发；代价是类型封装手写、GIL 管理自己扛——pyo3 的轻量替代路径 | 有界（白名单场景若只调 3–5 个入口，优先此案） | PSF-2.0（ctypes 为 CPython 标准库）/ 成熟 | https://docs.python.org/3/library/ctypes.html |
| 16 | FlatBuffers / schema 生成 | 跨语言 schema 生成对照——X3 已定，此处仅留指针，不展开 | — | — | 见 X3 文档 |

---

## 三、重点回答汇总

1. **pyo3 在 Python 白名单场景的成本**：编译链要求 CI 装 Rust 工具链（用户侧不用——拿到的是 wheel）；abi3 使分发矩阵从"版本×平台"缩到"平台"（每平台 1 个 abi3 wheel，pyo3.rs 官方文档）；自由线程 Python 已到"可用"档：CPython 3.14（2025-10）将 free-threaded 升为官方支持（Phase II），pyo3 0.26（2025-09）起适配、0.28.x 全面 attach 化——但**生态 wheel 对 3.13t/3.14t 的覆盖仍不全**，ADV 若用需自行构建并单测 3.14t。判据：白名单 Python 侧入口多、要 Python 对象深度互操作 ⇒ pyo3+maturin+abi3；入口少（≤5 个）⇒ ctypes 加载 C ABI（15 条）更省。
2. **napi-rs 预编译矩阵（watch 判据）**：v3（2025-07-07）稳定，标准矩阵 = linux-gnu/musl × x64/arm64 + darwin × x64/arm64 + win32 × x64/arm64（官方模板 CI），外加 wasm fallback 包兜无预编译平台。触发条件：ADV 决定出 Node 生态工具。在此之前不投入。
3. **cbindgen vs diplomat**：只需给自托管 CI/插件一个 C ABI 头 ⇒ cbindgen（成熟、最小假设）；要 C++/JS 开箱高层绑定且能接受 0.x 工具 ⇒ diplomat；插件面若走 wasm ⇒ 两者都不用，直接 WIT（07 条）。判据核心是"胶水谁写"：cbindgen 把胶水留给调用方，diplomat 替你生成，wasmtime 连隔离一起给。
4. **FFI unsafe 边界测试矩阵**：见 08 条——Linux：Miri（stub 化 FFI）+ valgrind/LSan + TSan；Windows：ASan(msvc) 只查内存错误、LSan 不可用（rust#88132）故泄漏检测放 Linux；Miri 对 FFI 外部代码盲（需 stub）；结论：**没有任何单工具覆盖 unsafe FFI 边界，绿必须来自矩阵且无 SKIP**。
5. **WIT vs C ABI（与 adv-sandbox 互证）**：互证为主——wasmtime 的 Component Model 自 2024 初稳定（WASI 0.2），类型系统（variant/resource）+ 隔离 + 自动绑定使其成为"不可信插件"接口的机制正确解；C ABI 保留为"可信自托管 CI 的最小依赖路径"。纠偏一条：接口设计要避开组件模型当前弱项（引用/借用密集类型，2026-02 Lobsters 批评点），传值/句柄优先。

---

## 四、Top-3

1. **pyo3 + maturin（abi3）**：Python 白名单场景的默认绑定路径；abi3 控分发矩阵，free-threaded（3.14t）已可用但生态覆盖不全——评测脚手架先出 demo wheel 记账（构建时长/wheel 体积），再与 ctypes 轻路径对比定案。
2. **WIT Component Model（wasmtime 宿主）**：adv-sandbox 插件接口的机制正确解（类型化 + 隔离 + 版本化），与 08 决议互证；先做 20 行 host+guest demo 量 wasmtime 体积/冷启动增量，C ABI 保留给可信自托管 CI。
3. **unsafe/FFI 测试矩阵**：Miri（Linux，stub 化）+ valgrind/LSan（Linux）+ ASan（Windows msvc，无泄漏检测）分工覆盖 Miri 的 FFI 盲区；落成 CI `ffi-gate` job，真实 C 依赖在位才算绿。

---

### 本域条目数：16（机制级 9 + 扫视 7；X3 FlatBuffers 仅留指针）
