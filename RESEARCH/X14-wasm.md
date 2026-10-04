# X14 域调研：WASM / 组件模型与运行时工程

> 日期：2026-10-04 · 范围：wasm-tools 生态、wasmtime 宿主工程、WASI 0.3、wasmi/wasmer 对照、fuzz 与供应链
> 档位：【吸收|有界|不吸收|watch|reference】；成熟度：【实验|维护中|弃维护】
> 锚点说明：wasmtime 版本节奏为月度发版；本调研检索时点 docs.rs 显示 wasmtime 50.0.0-dev 文档（对应主干），主干发布线约 36→46+（2026）。凡未能本机复算的性能数字一律不引，只给判据。

---

## Top-3（对本项目价值最高的三条）

1. **wasmtime component 宿主 API（`bindgen!` + `ResourceTable` + async）**——adv-sandbox 宿主侧可直接照抄的机制：WIT 世界生成 Rust trait、资源句柄宿主托管、P3 异步。→ 条目 1
2. **pooling + fuel/epoch 双计量起步配置**——决定 adv-sandbox 的资源模型与裁决可复现性；本文给出配置草案与调参判据。→ 条目 5、6 + 重点问答①②
3. **wasm-smith / wasm-mutate fuzz harness 三档接入**——对测试插件宿主直接可用，且 CVE 历史证明"codegen bug 靠 fuzz 发现"这一响应模式成立。→ 条目 13 + 重点问答④

---

## A. 组件模型

### A1. wasmtime component API（Rust 宿主侧：bindgen!/资源类型/异步）——机制级
- **定位**：宿主侧消费 WIT 组件的标准路径，adv-sandbox 宿主的直接实现基础。
- **可抄机制**：
  1. `wasmtime::component::bindgen!` 宏从 WIT 文本生成宿主类型与 `Host` trait：宿主实现 trait，`Linker` 注册，`instantiate_async` 拿到 typed exports；`async: true` 配合 `Config::async_support(true)`（wasmtime ≥24 命名）走异步调用。
  2. 资源类型：`Resource<T>` 是 guest 侧句柄（i32 索引 + 所有权位），宿主侧用 `ResourceTable` 存实际对象；句柄 close/transfer 有 rep 语义校验，guest 伪造索引拿到的是 trap 而非越界——沙箱边界在类型系统里。
  3. P3 时代（WASI 0.3.0，2026-02 发布）：WIT `future`/`stream` 类型可进 bindgen 签名；`Store::epoch_deadline_async_yield_and_update` 与异步运行时协作让出（机制见 A6）。
- **档位**：【吸收】
- **第一步动作**：adv-sandbox 把已定的 WIT 插件 ABI 用 `bindgen!` 生成宿主骨架，`ResourceTable` 托管全部跨边界句柄，禁用裸 `Func::typed` 直调路径。
- **许可/成熟度**：Apache-2.0 WITH LLVM-exception；维护中（wasmtime 月度发版）。
- **链接**：https://docs.rs/wasmtime/latest/wasmtime/component/macro.bindgen.html

### A2. wasm-tools（wit 解析 / wasm-compose）——机制级
- **定位**：组件模型的官方工具箱，wasm-smith/wasm-mutate/wit-parser/wit-bindgen 同仓同节奏。
- **可抄机制**：
  1. `wasm-tools component wit <file.wasm>`：从编译产物反查其 WIT 世界——ADV 加载插件时校验"插件的 world == 声明的 ABI 版本"可直接复用。
  2. `wasm-tools compose`：按导入/导出名与类型匹配把子组件链接成复合组件，自动生成 shim 适配——规则流水线"多插件编排"可参考其匹配算法。
  3. `wasm-tools new`：把 core module 封装成 component（加适配垫片）。
- **档位**：【吸收】（CI 门：wit 校验；不吸收 compose 进运行时，仅作工具）
- **第一步动作**：CI 加一步 `wasm-tools component wit` 校验插件产物与 ABI wit 一致。
- **许可/成熟度**：MIT OR Apache-2.0（以仓库 LICENSE 为准）；维护中。
- **链接**：https://github.com/bytecodealliance/wasm-tools

### A3. 组件模型的版本化与兼容规则——机制级
- **定位**：ADV 规则包/插件 ABI 升级路径的依据。
- **可抄机制**：
  1. WIT 包带 semver：`package ns:name@x.y.z`；工具链（wit-parser）按 semver 校验包依赖。
  2. 当前兼容性判定=**导入/导出按名称+类型精确匹配**（wasm-compose 也用同一匹配器）；没有运行时版本协商。
  3. 组件二进制层有独立版本字节（与 core wasm 区分）；"版本化/兼容范围"仍是进行中的设计议题（component-model 仓跟踪），未定型。
- **档位**：【有界】——ADV 自定约定：manifest 固定 `abi_version`，加载时 wasm-tools 校验，不赌未来协商机制。
- **第一步动作**：adv-sandbox manifest 增加 `wit_world` + `abi_version` 字段，加载器先校验再实例化。
- **许可/成熟度**：规范草案随 component-model 仓演进；版本化提案=实验。
- **链接**：https://github.com/WebAssembly/component-model

### A4. WASI 0.3（socket/http 子接口现状）——机制级
- **定位**：E7/08 已锚 async；这里补子接口与运行时落地现状。
- **可抄机制**：
  1. WASI 0.3.0 于 2026-02 发布，核心=native async（future/stream 一等类型）+ threads；wasmtime 46 起 P3 API 默认启用（检索锚点：wasmCloud 2026 文档、dev.to 2026-06 综述）。
  2. `wasi:sockets`/`wasi:http` 已随标准化接口集发布于 wa.dev；`wasi:http` 的 P3 实现仍在打磨（2026-08 仍有 outgoing body buffering 控制的开放 issue）——引 http 前先盯该仓。
  3. 能力按接口注入：宿主 `Linker` 只挂需要的 wasi 接口实例，未挂载=插件调用即 trap（能力安全模型）。
- **档位**：【有界】——ADV 规则插件默认零网络；socket/http 仅测试回放插件按能力位显式注入。
- **第一步动作**：adv-sandbox Linker 白名单化 wasi 接口；http 相关等 wasmtime 稳定版 + wasi-http p3 打磨完再评估。
- **许可/成熟度**：规范=已发布（0.3.0，2026-02）；http 子接口=维护中带打磨。
- **链接**：https://github.com/WebAssembly/WASI · https://wa.dev

---

## B. 运行时工程

### B5. wasmtime pooling allocator——机制级（含配置草案）
- **定位**：实例级内存池预分配，ADV 插件宿主"多实例、快实例化、拒绝 OOM 抖动"的核心开关。
- **可抄机制**（默认值锚点：docs.rs `PoolingAllocationConfig`，wasmtime 50.0.0-dev 文档，2026-10-04 检索）：
  1. 池按槽位预划：`total_memories=1000`、`total_tables=1000`、`total_stacks=1000`（async 档）、`total_core_instances=1000`、`total_component_instances=1000`；每内存槽上限 `max_memory_size`（默认 4 GiB），每表 `table_elements=20000`。
  2. 槽位回收复用：`max_unused_warm_slots=100`（热槽保留数）、`decommit_batch_size=1`（默认即逐页归还）；`linear_memory_keep_resident` 等保页参数控制归还节奏。
  3. 隔离加固：`memory_protection_keys`（默认 `no`；Linux MPK 硬隔离可选 `auto`）、`pagemap_scan`（Linux 6.7+，默认 no）。
  4. 池模式下内存上限静态已知 ⇒ 等价于把 ResourceLimiter 的检查前置成分配期拒绝。
- **起步配置草案**（ADV 规则插件场景，全部待基准复算）：
  ```rust
  let mut pool = PoolingAllocationConfig::default();
  pool.total_memories(256);            // 默认 1000 对单机规则宿主偏大；=并发插件上限
  pool.max_memory_size(16 << 20);      // 默认 4 GiB；插件不跑大数组时 16 MiB 起步
  pool.total_stacks(64);               // 仅 async 档需要；同步档可关
  pool.table_elements(10_000);         // 默认 20000 的保守减半
  pool.memory_protection_keys(MpkEnabled::Auto); // Linux 部署位；其他平台回退 no
  ```
  **调参判据**：① 并发峰值 ≤ total_memories×0.7（留拒绝余量）；② 抽样插件 RSS 实测 P99 < max_memory_size，否则只调大该值并重算 VA 预算；③ 池满拒绝率=0（出现即调 total）；④ 实例化 P50 与池命中与否的差值即本机收益（自测，不引外数）。
- **档位**：【吸收】
- **第一步动作**：adv-sandbox 按"总 VA 预算 = total_memories×max_memory_size + total_stacks×stack_size"反推默认档数值，写进池配置单测。
- **许可/成熟度**：Apache-2.0 WITH LLVM-exception；维护中（参数名以所用版本 rustdoc 为准，历史上 `PoolingAllocatorConfig`→`PoolingAllocationConfig` 有过更名）。
- **链接**：https://docs.wasmtime.dev/api/wasmtime/struct.PoolingAllocationConfig.html

### B6. epoch interruption vs fuel（CPU 计量两制对比）——机制级
- **定位**：抢占与配额两条机制，裁决"规则插件用哪个"。
- **可抄机制**：
  1. **epoch**：`Config::epoch_interruption(true)`；宿主侧独立线程周期调 `Engine::increment_epoch()`；guest 代码仅在函数调用/循环回边检查计数，超 `Store::set_epoch_deadline(n)` 期限即 trap（或经 `epoch_deadline_async_yield_and_update` 协作让出）。检查点稀疏 ⇒ 开销近零，但语义是"墙钟近似"，非确定性。
  2. **fuel**：`Config::consume_fuel(true)` + `Store::set_fuel(u64)`；逐指令计量，耗尽→`Trap::OutOfFuel`。同模块+同配额+同输入 ⇒ 执行轨迹确定，可进裁决记录；代价是逐指令开销与配额校准负担。
  3. 两制正交可叠加：fuel 管"算多少"，epoch 管"等多久"；fuel 不计量宿主函数内部耗时，宿主调用挂死要靠 epoch（或宿主自身超时）兜底。
- **裁决（ADV）**：**规则执行=fuel 主计量**（结果可复现、配额可审计）；**epoch 作硬超时兜底**（覆盖宿主调用与异步让出路径）。纯编排/IO 类非裁决任务可只用 epoch。
- **档位**：【吸收】
- **第一步动作**：adv-sandbox 计量层做成"fuel 配额 + epoch 硬限"双参数；用同一条规则语料测 fuel 开销占比，得到本机锚点后再定默认配额。
- **许可/成熟度**：随 wasmtime；维护中。
- **链接**：https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.epoch_interruption · https://docs.rs/wasmtime/latest/wasmtime/struct.Store.html#method.set_fuel

### B7. wasmtime 编译缓存（wasmtime-cache）——机制级
- **定位**：插件冷启动的"第一次之后免费"路径。
- **可抄机制**：
  1. `Config::cache(true)` + `Config::cache_config_load_default()`；条目=盘上的序列化编译产物。
  2. 缓存键=编译输入哈希（模块字节 + 编译器/优化设置 + wasmtime 版本等）⇒ 版本或编译配置一变即全量 miss，天然避免跨版本脏命中。
  3. 默认目录按平台：Linux `$XDG_CACHE_HOME/wasmtime`、macOS `~/Library/Caches/wasmtime`、Windows `%LOCALAPPDATA%\wasmtime`；缓存行为可经 `config.toml`（enabled/directory/worker 线程数）调。
- **档位**：【吸收】（宿主进程内自动用；不给用户目录写权限的场景要显式指定目录）
- **第一步动作**：adv-sandbox 启动时 `cache(true)`，基准对比首次/二次实例化耗时差（=编译时间），留档作冷启动基线。
- **许可/成熟度**：随 wasmtime（crates/cache）；维护中。
- **链接**：https://github.com/bytecodealliance/wasmtime/tree/main/crates/cache

### B8. AOT（Cwasm 预编译分发形态）——机制级
- **定位**：`wasmtime compile x.wasm -o x.cwasm` 产出可反序列化的编译产物；对照缓存路径。
- **可抄机制**：
  1. 运行时 `Module::deserialize_file` 跳过 Cranelift 编译直接加载（或 `Engine::precompile_module` 自产自销）。
  2. 产物强绑定：wasmtime 版本、目标架构与 CPU 特性——**不能**当跨机分发格式用。
  3. 信任边界：官方文档明确只应反序列化可信来源的产物；cwasm 不是安全边界，ADV 若用，输入必须出自本机/本包校验过的编译步骤。
- **档位**：【有界】——仅"同机预编译自用"场景（安装后首次编译一次，后续 deserialize）；规则包分发仍发 .wasm。
- **第一步动作**：与 B7 一起测：同机冷启动三态（无缓存/缓存命中/cwasm）耗时对比，决定是否值得引入。
- **许可/成熟度**：随 wasmtime；维护中。
- **链接**：https://docs.rs/wasmtime/latest/wasmtime/struct.Module.html#method.deserialize_file

### B9. wasmer（2026 现状对照）——扫视
多后端运行时（LLVM/Cranelift/解释），2026-09 检索见 FreeBSD ports 已更到 7.4.0，未见官方 6.0 发布稿；MIT；对照位：ADV 主线已定 wasmtime，wasmer 不吸收，作后端对照【reference】。https://github.com/wasmerio/wasmer

### B10. wasmi（回退档现状）——扫视
2026-09 发布 v2.0.0（工程博文 "Engineering of the fastest WebAssembly interpreters"，自建 wasmi-benchmarks 对 ~20 个运行时）；主打解释器性能与嵌入友好；MIT OR Apache-2.0；08 定的"wasmi 回退档"继续成立，升级 2.0 前先跑双运行时一致性（spectest + 规则语料差分）再切【吸收（既有决定）/ 维护中】。https://github.com/wasmi-labs/wasmi

### B11. WAMR（C 对照）——扫视
C 生态嵌入式事实标准（解释/AOT/JIT 三模式，超小 footprint）；Apache-2.0；ADV 为 Rust 主体，不吸收，作 C 侧嵌入与体积基准对照【reference】。https://github.com/bytecodealliance/wasm-micro-runtime

---

## C. 安全工程

### C12. wasmtime CVE 历史与响应模式——机制级
- **定位**：给 ADV 的运行时升级与后端策略提供依据。
- **清单（检索到锚点者）**：
  - CVE-2026-34971 / GHSA-jhxm-h53p-jm7w / RUSTSEC-2026-0096（2026-04）：aarch64 Cranelift 误编译 guest 堆访问 ⇒ 沙箱逃逸；修于 wasmtime 36.0.7。（08 已收录，此处补编号链）
  - CVE-2026-34987（SUSE 通告，2026-05）：Winch 后端可产生沙箱外内存访问。
  - CVE-2023-41880（2023-09）：fuzzing 发现的 Cranelift 缺陷，通告明确**非**沙箱逃逸。
  - CVE-2022-39392（2022）：<2.0.2 的实现缺陷（GHSA 渠道）。
  - 更早：CISA 通告（2023-03）提及 Cranelift x86_64 误编译类缺陷。
  - 完整清单以官方页与 RUSTSEC 为准（本调研未逐条穷举）。
- **响应模式（可抄的组织机制）**：GHSA↔RUSTSEC 双发布 + 补丁打到维护分支（36.0.7 式小版本）；发现主渠道=fuzzing；严重项集中在**代码生成器**（Cranelift 各架构、Winch）而非宿主 API。
- **对 ADV 判据**：① CI 挂 cargo-audit/cargo-deny 盯 RUSTSEC 的 wasmtime 通报；② 非默认后端不开（Winch 默认关，减少暴露面）；③ 跟踪某修复后评估升级窗口。
- **档位**：【吸收】（响应模式+升级策略，不是吸收 CVE 本身）
- **许可/成熟度**：wasmtime Apache-2.0 WITH LLVM-exception；维护中。
- **链接**：https://github.com/bytecodealliance/wasmtime/security/advisories · https://rustsec.org

### C13. wasm-smith / wasm-mutate（fuzz 基建）——机制级
- **定位**：wasm-tools 仓的两个 fuzz 输入引擎；wasmtime 自身 Cranelift fuzzing 的弹药库（见 C12 响应模式）。
- **可抄机制**：
  1. `wasm-smith`：按语法规约**结构化生成**语法合法的 wasm 模块，可约束模块特性与预算（内存/表大小），生成的模块保证可通过校验器——fuzz 目标不被解析噪音淹没。
  2. `wasm-mutate`：对给定模块做**保有效性变异**（语义保持/类型保持变换），真实规则语料 ⇒ 大量近亲变体。
  3. 标准用法=配 libfuzzer（cargo-fuzz）in-process 跑；wasmtime 仓的 fuzz target 即此模式，可直接参考其 harness 结构。
- **ADV fuzz 三档接法（问答④，详见下节）**。
- **档位**：【吸收】
- **第一步动作**：test-replay 仓加 fuzz target：输入→wasm 模块→ADV 宿主加载→fuel+epoch 双限执行→断言"宿主不 panic、trap 正确归档"。
- **许可/成熟度**：MIT OR Apache-2.0（以仓库为准）；维护中。
- **链接**：https://github.com/bytecodealliance/wasm-tools（crates: wasm-smith / wasm-mutate）

### C14. spectest 套件——扫视
WebAssembly/spec 官方一致性测试集（core），wasmtime/wasmi 仓内都有现成 runner；ADV 用作**双运行时一致性底线**：wasmi 2.0 升级门=全量 spectest + 规则语料差分对拍【吸收（进 test-replay）】；Apache-2.0（spec 仓，以 LICENSE 为准）。https://github.com/WebAssembly/spec/tree/main/test/core

---

## D. 工具链

### D15. cargo-component——扫视
Rust 组件开发工具链（`cargo component new/build`，Cargo.toml 内声明 world）；2026-08 仍有教程将其作为首选路径，未见弃维护信号；底层绑定生成已部分让位给直接用 wit-bindgen（两者可组合，wasmCloud 2026-06 文档有 `with` 复用绑定的示例）。Apache-2.0（以仓库为准）；维护中【reference（插件作者路径）】。https://github.com/bytecodealliance/cargo-component

### D16. wit-bindgen——扫视
多语言 WIT 绑定生成（guest 侧 Rust 为主）；wit-bindgen-core v0.62.0（crates.io，2026-09）发布节奏活跃；MIT OR Apache-2.0；维护中【reference】。https://github.com/bytecodealliance/wit-bindgen

### D17. wkg（WIT 包注册与分发）——机制级
- **定位**：Wasm Package Tools（wasmCloud 系），WIT 包/组件的取发 CLI；回答"要不要自建分发格式"。
- **可抄机制**：
  1. `wkg get/resolve/publish`：WIT 包以 **OCI artifact 布局**存取，任意兼容 registry（GHCR/ECR/Harbor…）皆可作后端；warg 协议也支持但生态动量在 OCI。
  2. 生态锚点：wash v0.36（2024-11）起弃 wit-deps 转 wkg；wa.dev 提供 wasi:* 标准包源——插件作者 `wkg get wasi:*` 即可 bindgen。
  3. 组件分发同走 OCI 媒体类型（CNCF WASM OCI artifact 布局已是事实标准）。
- **裁决（ADV，问答⑤）**：**ABI 层吸收，规则包本体不自建注册系统**——把 ADV 的插件 wit world 发成 OCI artifact（wkg publish/get），蹭现成 registry 与鉴权；规则包（.wasm+manifest）用自建轻量格式（tar+zstd + manifest 内 sha256 + abi_version），存储后端可插拔（本地目录/OCI 皆可），不绑死 wkg。
- **档位**：【吸收（ABI 分发）/ 有界（规则包格式）】
- **第一步动作**：把 adv-sandbox 的 wit 文件用 wkg publish 到项目自己的 registry 命名空间，CI 校验可拉回。
- **许可/成熟度**：Apache-2.0（wasmCloud 系）；维护中。
- **链接**：https://github.com/wasmCloud/wkg · https://wa.dev

### D18. 插件签名与供应链（wasm 插件 hash 锁定）——扫视
现行实践：OCI artifact + cosign 签名/attest 做发布侧供应链；消费侧用 manifest 内哈希锁定（sha256 pin）保证"装的就是验过的那个"。ADV 判据：规则包 manifest 必含 sha256 且加载时强校验（这是吸收线）；签名通道（cosign/minisign 二选一）等分发形态定型再定【watch】。锚点：wasmCloud packaging 文档（OCI artifact + cosign 生态位）。

---

## 重点问答

**① wasmtime pooling + epoch 起步参数（配置草案见 B5/B6）**：`total_memories=256`、`max_memory_size=16MiB`、`total_stacks=64`（async 档）、`table_elements=10_000`、MPK=auto（Linux）；epoch 侧：独立线程 10ms `increment_epoch()`，`set_epoch_deadline` 即超期 trap。调参判据四条见 B5（并发余量/RSS P99/拒绝率/实例化收益自测）。

**② fuel vs epoch 裁决**：规则插件（结果进裁决记录、要求跨机可复现）⇒ **fuel 主计量**；epoch 作硬超时兜底（fuel 不计量宿主函数内部耗时，宿主调用挂死只能靠 epoch 或宿主自身超时）。非裁决的编排/IO 任务可只用 epoch。两制正交可叠加。fuel 开销占比须在本机用规则语料实测得锚，不引外推数字。

**③ 编译缓存对插件冷启动的收益路径**：`Config::cache(true)` → 首次 Cranelift 编译后按"模块字节+编译配置+wasmtime 版本"哈希落盘（默认 `%LOCALAPPDATA%\wasmtime` 等，见 B7）→ 同版本同模块二次起跳过编译。收益=编译时间本身（模块越大越明显），升级即全量 miss 是设计而非缺陷。cwasm 是更强形态但版本/CPU 强绑定且有信任边界，只做"同机自编译自用"，不进分发。

**④ wasm-smith/wasm-mutate 进 ADV fuzz 三档**：档1（CI 快档）= wasm-mutate 变异真实规则语料（每插件固定预算），fuel+epoch 双限，断言宿主不 panic / trap 正确归档；档2（夜档）= wasm-smith 结构化生成（限内存/表预算）+ 随机 host-call 序列，重点覆盖 Resource 句柄误用（伪造索引/重复 close/跨 store 传递）；档3（发布前长跑）= cargo-fuzz in-process，池参数故意调小（total_memories 压到个位数）逼出分配拒绝路径。语料落盘可复现（fuel 保证确定性重放）。

**⑤ wkg/包注册对规则包分发的适用性**：ABI（wit world）走 wkg+OCI 吸收（见 D17）；规则包本体**不自建 registry、也不套 OCI 全家桶**——自建最小格式（tar+zstd+manifest{sha256, abi_version}），存储可插拔。判据：需要离线/受限网分发与本地索引时，OCI 只作可选传输层而非格式依赖。

---

## 附：条目一览

| # | 条目 | 档位 | 深度 |
|---|------|------|------|
| 1 | wasmtime component API | 吸收 | 机制级 |
| 2 | wasm-tools | 吸收 | 机制级 |
| 3 | 组件模型版本化 | 有界 | 机制级 |
| 4 | WASI 0.3 | 有界 | 机制级 |
| 5 | pooling allocator | 吸收 | 机制级 |
| 6 | epoch/fuel | 吸收 | 机制级 |
| 7 | wasmtime-cache | 吸收 | 机制级 |
| 8 | cwasm AOT | 有界 | 机制级 |
| 9 | wasmer | reference | 扫视 |
| 10 | wasmi 2.0 | 吸收（既定） | 扫视 |
| 11 | WAMR | reference | 扫视 |
| 12 | wasmtime CVE | 吸收 | 机制级 |
| 13 | wasm-smith/wasm-mutate | 吸收 | 机制级 |
| 14 | spectest | 吸收（test-replay） | 扫视 |
| 15 | cargo-component | reference | 扫视 |
| 16 | wit-bindgen | reference | 扫视 |
| 17 | wkg | 吸收/有界 | 机制级 |
| 18 | 签名与供应链 | watch | 扫视 |
