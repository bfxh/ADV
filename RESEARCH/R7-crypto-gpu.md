# R7 GPU 加速解剖与密码学工具工程

> 基：R7（GPU 加速解剖与密码学工具工程）｜调研日：2026-10-03｜方法：WebSearch/WebFetch，优先 2025–2026 资料
> 视角：拆 **GPU 计算框架/调度** 与 **密码学工具** 的机制，落点是 perf-core（批量相位）、adv-index（内容寻址/校验层）、adv-secrets（密钥与秘密存储）。
> 证据纪律：所有数字带来源锚（版本/论文/基准页）；检索摘要所得一律标「二手」；落盘前未核的标「待核」。AGENTS.md 表述红线 11–15 适用：不绝对化、不把单窗口结论升格为普遍结论。
> 计数（与 `registry/R7.jsonl` 一致）：**26 条**，其中机制级（deep，能说清算算法/数据结构/工程参数）**14 条**。
> 档位分布：吸收 8 / 有界 7 / watch 6 / reference 5。集成去向：perf-core 7 / adv-index 4 / adv-secrets 3 / watch 6 / reference 6。

## 0. 判定口径

- 本域是「学习目的」：hashcat 等密码学工具**只拆调度与内核工程**，不写攻击教程、不涉密钥恢复流程。
- ADV 的 GPU 档定位为**可选增强档**（Windows 单机、缺席须显式状态、不静默回退——沿用 07 文档口径）。默认 CPU 路径必须独立可用。
- 「值得上卡」的判据是**搬运比**：数据已在 GPU 常驻（无回读）或每项算术量 ≫ 搬运量；IO/编排/树遍历类相位按 07 文档口径不上卡，本域不推翻。

---

## 1. 重点解答（①–⑤）

### ① hashcat「批量同构任务」调度 → ADV 批量哈希/熵/词法统计

hashcat 的调度是三层循环 + 自动调参（来源：hashcat/hashcat 代码结构与 DeepWiki 摘要，二手）：
- **三层循环**：`outer_loop()`（keyspace 大块）/ `inner1_loop()` / `inner2_loop()`（内核批），内核签名统一 `KERN_ATTR`，候选生成在 GPU 内联（"in-kernel rule engine" 为其自称，仓库 README 口径）。
- **自动调参**：autotune 以 `TARGET_MSEC ≈ 96 ms` 为目标内核耗时，联动调整 `kernel_accel` / `kernel_loops`（把同一 dispatch 内做多轮）——即**按目标耗时反推批量参数**，不是写死。
- **主机-设备切分**：workload 左右分工（部分 keyspace 留在 CPU 侧内核，GPU 侧跑主循环），按设备速度比例切。
- **向量化**：`u32x/u64x` 显式向量类型 + `VECT_SIZE` 档位 + 按硬件的 tuning DB（HCTUNE），同一内核源码跨 OpenCL/CUDA/HIP 后端（`inc_vendor.h` 平台抽象）。

映射到 ADV（**不搞嵌入**，改批量统计）：
1. **同构候选 → 同构待统计项**：ADV 侧「一相位 = 一个 WGSL 内核 + SoA 列缓冲」：字节直方图/熵、XOR 穷举、token 频次与 n-gram 计数都是「每项独立、无回读、算术密集」，与 hashcat 的候选-验证循环**同形**。
2. **目标耗时调参**：perf-core 的批量 dispatch 参数（workgroup 大小、每内核处理项数、循环轮数）按「目标内核耗时 ~100 ms 量级」自校准；低于拐点的小批量直接留 CPU（07 文档：1K/10K/100K/1M 拐点曲线判据）。
3. **左右切分**：扫描对象集先按「大小/是否已常驻显存」分左右两组，小项攒批、大项 GPU 常驻；回读只回聚合结果（直方图/计数），禁止逐项回读。

注意（限定）：hashcat 是候选生成型负载，ADV 相位是聚合型负载——可抄的是**调度形状**（分区/目标耗时/批量内循环），不是内核语义。

### ② blake3 / Bao 并行树哈希（内容寻址层直接可用）

机制（来源：BLAKE3 官方 README 与 spec、Bao spec，二手）：
- 输入切 **1024 B chunk**，每 chunk 独立压缩；父节点 = 左右子哈希拼接再压缩，构成 Merkle 树（"Bao tree"）；chunk/父节点压缩均为 BLAKE2s 派生 7 轮。
- **SIMD 并行**：同一向量寄存器内并行 8（AVX2）/16（AVX-512）路 chunk 压缩；官方口径吞吐 ~2599 MiB/s（AVX2）、~4500 MiB/s（AVX-512）（cryptopp-modern 基准页，二手；机器不同不可直接换算到 ADV 本机）。
- **线程并行**：Rayon 多线程在树层并行（适合大文件）。
- **Bao = verified streaming**：`combined`/`outboard` 两种编码，树开销约 **6.25%**；给定根哈希可只校验单个 chunk/切片（认证路径），无需重算全量。

ADV 用法建议：adv-index 的内容 ID 直接用 blake3；Bao 用于「大文件部分读校验」——适配扫描结果缓存/对象存储的完整性验证。第一步动作：在本机既有语料上出 blake3（SIMD 开/关、单线程/多线程）对 SHA-256 的吞吐曲线（同机同语料、窗口均值，单点不算）。

### ③ wgpu vs CUDA vs OpenCL（Windows 单机、可选增强档）选型判据

| 维度 | wgpu | CUDA（cudarc） | OpenCL | ROCm/HIP |
|------|------|------|--------|----------|
| 覆盖 | Vulkan/DX12/Metal 单 WGSL 代码 | 仅 NVIDIA | 多厂商但驱动参差 | AMD，Windows 生态弱 |
| 上限 | 抽象封顶（无 Tensor Core） | 最高 | 中间 | 中间 |
| 本仓适配 | 与 07 文档已定方案一致，MIT OR Apache-2.0 | 需另维护一套内核源 | 需运行时编译 + 驱动部署 | watch |
| 判据 | **默认档** | **仅当**目标机 NVIDIA + 相位经 wgpu 证明瓶颈且收益 ≫ 双实现维护成本 | 仅当需覆盖老旧多厂商驱动 | 不达 Windows 单机门槛 |

关键工程参数（wgpu 官方 COMPUTE-BACKENDS 文档，二手）：默认 `maxComputeInvocationsPerWorkgroup = 256`、`maxComputeWorkgroupSizeX/Y = 256`、`maxComputeWorkgroupsPerDimension = 65535`、storage buffer 绑定默认 128 MiB——**超默认必须在设备创建时显式 request 并检查 adapter 支持**；DX12 硬上限 1024 invocations/workgroup，Vulkan 规范保底 128（桌面常见 1024）。参考 workgroup 建议：<1K 项用 64；100K–1M 用 256（官方 compute 文档口径）。
实测类旁证（二手，跨厂商）：any-gpu 项目在 AMD RX 5700 XT / RTX 3070 Laptop / M4 上用同一 WGSL 取得 47–89 GFLOPS matmul（54/54 测试通过）——说明单 WGSL 跨厂商可行，但不代表 ADV 目标相位同收益。

### ④ GPU 在扫描器里的适用相位白名单

**值得上卡**（每项独立 + 算术密集 + 聚合回读）：
- 批量字节直方图 / 批量熵（内部锚：unified-rx 实测 31–38×，单机 Windows；07 文档已定）
- XOR 穷举 / 单字节或短密钥空间扫描（同族）
- 大批量内容哈希（blake3 树层；项数 ≫ 搬运比时）
- 多模式简单正则/YARA 字面量（限 HybridSA 类 bit-parallel 内核可覆盖的模式集；见 §6）
- 批量签名/哈希比对（MD5/SSDEEP 类大批量；有先例但硬件代差大，见 §6）

**绝不上卡**：
- IO / 编排 / 树遍历 / 规则判定 / 符号执行 / 反编译（无数据并行形状）
- 小批量或单文件（PCIe/UMA 双程搬运吃掉全部收益；拐点曲线前置判据）
- 高复杂度正则/回溯型匹配（GPU 分支发散模型不友好）
- 嵌入推理（走 ort 的 DirectML/CUDA EP，不自写 wgpu transformer——07 文档已定）

### ⑤ SHA-NI / 硬件指令 CPU 侧加速：Rust 可达性与收益

- **SHA-NI（x86 SHA 扩展）**：RustCrypto `sha2` crate **编译期选后端 + `cpufeatures` 运行时分发**，x86 走 SHA-NI/AVX2，无需 `asm` feature 或 `target_cpu=native`（社区实测 Ryzen 7 2700X 上 ~2080 MB/s，论坛口径二手）；2025 基准口径 SHA-NI SHA-256 ~1.1–1.2 GiB/s（arcanum-primitives，二手）。**SHA-NI 指令数据无关（constant-time）**，适合秘密相关哈希。
- **注意**：x86 上没有广泛可用的 SHA-512 指令；SHA-512 走 AVX2/软件路径；AArch64 有 `sha512` 扩展（跨平台代码别假设对称）。
- **CRC32C**：SSE4.2 `crc32` 指令、三路并行展开提升 ILP（crc32c crate 0.6.8 口径）；基准（librscrc，二手）：硬件 CRC x86_64 10.48 GiB/s / aarch64 21.03 GiB/s，均 ≫ 查表（3.66 / 2.27 GiB/s）。
- 收益判据：**零重架构成本**（同一 API 换 crate/feature）→ 应作为 GPU 档之前的第一道加速（CPU 指令侧先榨干，再按拐点开 GPU 档）。

---

## 2. GPU 计算框架与调度

### 2.1 wgpu（现状/计算路径/limits）｜【吸收】｜perf-core

- 定位：Rust 的 WebGPU 实现（非浏览器用 Vulkan/DX12/Metal/GLES 后端）；本仓 07 文档已定默认 GPU 档。
- 可抄机制：
  1. **显式 limits 协商**：默认保守（workgroup 256、storage 128 MiB、65535 workgroup/维），上量必须 request + 校验 adapter 能力——把「能力探测」做成启动期显式状态。
  2. **按数据量选 workgroup**（官方建议 <1K→64，100K–1M→256）+ 大批量分多次 dispatch。
  3. **storage buffer 平铺要求**：与 R1 域「SoA = GPU 上传缓冲零转换」结论一致，保持。
- 第一步动作：在既有批量熵相位上出「workgroup ∈ {64,128,256} × 批量 ∈ {1K,10K,100K,1M}」二维拐点表（本机、窗口均值）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（2025–2026 活跃）。链接：https://wgpu.rs ；https://github.com/gfx-rs/wgpu

### 2.2 CUDA 与 Rust 绑定（cudarc/rust-cuda）｜【有界】｜perf-core

- 定位：NVIDIA 专属最高上限路径；Rust 侧以 cudarc（运行时绑定）为主流。
- 可抄机制：1) 模块/内核按需加载（PTX/cubin）与 stream/queue 隔离，与「可选增强档」的显式降级契合；2) 单机优先复用 wgpu 内核形状，只把被证明瓶颈的 1–2 个相位写 CUDA 版（双实现最小化）。
- 判据（限定）：仅当目标机 NVIDIA + wgpu 版相位瓶颈已量化 + 预期收益 > 双实现维护成本；否则不开第二条内核链。
- 第一步动作：不写代码，先记录判据到 perf-core 档位规则；待出现「wgpu 版相位实测不达标」的具体报告再启动。
- 许可证/成熟度：cudarc MIT OR Apache-2.0（**待核**）/ 维护中。链接：https://github.com/coreylowman/cudarc

### 2.3 ROCm / HIP（hip-rs、rocm-rs、cubecl-hip）｜【watch】｜watch

- 定位：AMD 路线；Windows 单机生态弱（ROCm 官方重心 Linux）。
- 可抄机制：1) rocm-rs 提供 HIP/rocBLAS 等安全封装（2026-07 仍有提交，二手）；2) cubecl-hip/burn-hip 由 Burn 深度学习生态带动（2025–2026 活跃，二手）——说明 AMD 路径在「框架级」才有资源，单机工具链不宜赌。
- 第一步动作：无（仅记录：若目标机为 AMD 独显且用户显式要求，再评估）。
- 许可证/成熟度：hip-rs MIT（**待核**）/ 实验；rocm-rs 待核 / 实验。链接：https://github.com/RustNSparks/rocm-rs

### 2.4 OpenCL（ocl / opencl3 / cl3）｜【watch】｜watch

- 定位：多厂商通用路径；hashcat/JtR 的跨平台底座即 OpenCL。
- 可抄机制：1) 运行时编译内核 = 部署灵活但失败面更大（驱动/编译器版本矩阵）；2) 三档如实上报（设备/失败可见）在 OpenCL 生态是惯例（hashcat `-I` 类设备枚举）。
- 判据：不作为 ADV 默认路径——wgpu 已覆盖 Vulkan/DX12 两后端；OpenCL 仅在「目标机显卡驱动缺失 Vulkan/DX12 支持」的兜底场景评估。
- 第一步动作：无。
- 许可证/成熟度：ocl MIT（**待核**）/ 维护中（0.19.x 仍有更新，二手）。链接：https://github.com/cogciprocate/ocl

### 2.5 Vulkan compute（直接使用）｜【reference】｜reference

- 定位：wgpu 的底层后端之一；直接写 Vulkan 只在需要 wgpu 未暴露能力（如特定扩展）时才有意义。
- 可抄机制：1) 桌面 GPU workgroup 上限常见 1024（Vulkan 规范保底 128，二手）——即「上限比 wgpu 默认高一档」，差量靠 request limits 拿到；2) 时间戳查询/间接 dispatch 在 Vulkan 后端最完整（wgpu 后端对照表，二手）。
- 第一步动作：无（保持经 wgpu 间接使用）。
- 许可证/成熟度：Khronos 规范 + loader Apache-2.0 / 维护中。链接：https://www.vulkan.org

### 2.6 内核工程规则集（occupancy / 合并访存 / 共享内存 / cp.async）｜【吸收】｜perf-core

- 定位：跨框架的通用内核工程约束集（CUDA Handbook、LeetCUDA、kernel-skills 等共识，二手）。
- 可抄机制：
  1. **合并访存 + SoA 平铺**：warp 内连续线程映射连续地址；本仓 SoA 列缓冲天然满足（07 文档已定）。
  2. **共享内存 bank 冲突**：fp32 加 1 列 padding（+4B）、fp16 加 2 列；或 XOR-swizzle 布局——ADV 的直方图/归约内核若用 workgroup 共享内存，先做冲突分析再写。
  3. **异步复制（cp.async 类）**：ping-pong 双缓冲 + `commit_group` / `wait_group(1)` 保持一组在飞；**前提**：搬运在关键路径上且计算量足以覆盖（纯访存型内核无收益，cudahandbook 口径二手）。ADV 在 WGSL 侧对应能力有限（无 cp.async），此条主要约束「是否值得为相位迁 CUDA」（见 2.2）。
- 第一步动作：把「共享内存 padding 规则 + 双缓冲 + 目标内核耗时」写成 perf-core 的内核 checklist，先用于现有批量熵内核的共享内存路径。
- 许可证/成熟度：知识性（无许可证约束）/ 维护中（社区文档持续更新）。链接：https://cudahandbook.com/book/ch5/intra-kernel-asynchronous-memcpy ；https://github.com/xlite-dev/LeetCUDA

### 2.7 多内核流水线 / overlap（批次窗口 + 流水深度）｜【吸收】｜perf-core

- 定位：把「扫描窗口切批 → 队列排队 → 结果异步回读」做成流水，摊掉 dispatch 与搬运死区。
- 可抄机制：
  1. **窗口化批扫描**：KeyHog（2025，Rust GPU 秘密扫描器）用 1 MiB 窗口 + 128 KiB 重叠做 batch windowed scanning（其文档口径，二手）——窗口互叠保证跨窗模式不漏。
  2. **异步流水深度**：KeyHog 的 async peers 暴露 depth 1–4（其 backends 文档口径，二手）——深度以显存预算定，非越深越好。
  3. **队列独占**：GPU 队列由专用线程持有（与 07 文档「自建池角色固定」一致），CPU 侧只投递任务与收聚合结果。
- 第一步动作：在批量熵相位加「双缓冲 + 结果异步回读」，对比单缓冲的窗口均值（判据：吞吐 + p99，单点不算）。
- 许可证/成熟度：KeyHog 待核 / 实验。链接：https://santhreal.github.io/keyhog/backends.html

---

## 3. 密码学工具（GPU 并行范式标本）

### 3.1 hashcat（调度与内核工程，仅拆机制）｜【吸收】｜perf-core

- 定位：批量同构任务上 GPU 的教科书；本条目**只拆调度与内核工程**。
- 可抄机制：
  1. **三层循环 + 目标耗时自动调参**：outer/inner1/inner2 + autotune `TARGET_MSEC ≈ 96 ms` 调 `kernel_accel`/`kernel_loops`（DeepWiki 摘要，二手）。
  2. **主机-设备左右切分**：keyspace 按设备算力比例分工（workload 左右），慢设备不吃满流水。
  3. **统一内核签名 + 显式向量化**：`KERN_ATTR` 统一元数据、`u32x/u64x` × `VECT_SIZE` 按硬件选档 + HCTUNE tuning DB——同一源码跨 OpenCL/CUDA/HIP。
  4. **内联候选生成**（自述 "in-kernel rule engine"）：避免主机-设备往返，代价是内核复杂度上升。
- 第一步动作：把「按目标耗时反推批量参数」写进 perf-core 批量相位调度器（先用 CPU 侧线程池验证形状，再映射 WGSL dispatch）。
- 许可证/成熟度：MIT / 维护中（仓库 2026-09 仍有提交，二手）。链接：https://github.com/hashcat/hashcat

### 3.2 John the Ripper（OpenCL 后端）｜【有界】｜reference

- 定位：与 hashcat 同代的另一标本；其 OpenCL 后端把「主机编排 + 设备内核」分工写得很显式。
- 可抄机制：
  1. **`crypt_all()` 主机编排**：缓冲写入/读取/派发集中一处（便于三档如实上报）。
  2. **拆分内核流水**（crypt_kernel + split_kernel + final_kernel）与 `new_keys` 抑制重复传输——「键未变不重传」与 ADV「列未变不重传显存」同构。
  3. **自动调参口径**：以最大内核耗时（默认 200 ms 量级）调 LWS/GWS；bitcoin-opencl 用例：Vega 64 上 LWS=256/GWS=16384 → 4369 c/s；调到 400 ms 后 GWS=131072 → 4691 c/s（虚拟 3276K c/s）（issue #4910 讨论，二手）——**目标耗时放宽要同时验证收益曲线拐点**。
- 第一步动作：抄「键未变不重传」为 ADV 显存驻留策略的判据（列指纹未变则跳过上传）。
- 许可证/成熟度：GPL-2.0+（jumbo 树内混合，部分格式另有条款，集成前需核）/ 维护中。链接：https://github.com/openwall/john

### 3.3 bitcoin SHA-256 流水线与 ASIC 对照｜【reference】｜reference

- 定位：极大批独立同构任务的另一极；JtR 的 bitcoin-opencl 格式是可直接读的标本。
- 可抄机制：
  1. **批量内循环**：`HASH_LOOPS ≈ 2000` 把多轮迭代塞进单内核（摊销派发）；实测把 2000→500 反而降速（4300 c/s，二手）——「更多迭代/内核」在此负载有益，但非普适（要按相位实测）。
  2. **中段状态复用**：先算前 64 B 的 midstate 再按 nonce 批量展开（SHA-256 分段压缩的通用套路）——ADV 侧对应「同一前缀多次哈希」的预处理。
  3. **ASIC 对照**：ASIC 比 GPU 高数个数量级（算力/功耗口径），说明「极度同构 + 固定算法」最终会离开 GPU——反向判据：ADV 相位若变得极其稳定且量极大，先考虑专用指令/专用实现，而非堆 GPU 内核。
- 第一步动作：无。（仅作调度形状参照。）
- 许可证/成熟度：JtR 格式随主仓 GPL-2.0+；cgminer/bfgminer GPL-3.0（**待核**）/ 维护中。链接：https://github.com/openwall/john

### 3.4 Fitcrack（分布式 hashcat 调度）｜【watch】｜watch

- 定位：把 mask/Markov/rule 任务切成分布式作业的学术工程（Brno UT）。
- 可抄机制：1) 大 keyspace 切块 + 检查点续跑；2) 引擎/代理分离的作业模型——对应 ADV 未来「批量扫描作业」的持久化与断点设计（单机阶段用不上，先登记）。
- 第一步动作：无。
- 许可证/成熟度：开源（许可证**待核**）/ 实验。链接：https://github.com/nesfit/fitcrack

---

## 4. 密码学库与硬件信任

### 4.1 BoringSSL｜【reference】｜reference

- 定位：Google 自持的 OpenSSL 分支（TLS 场景），API 不承诺稳定。
- 可抄机制：1) 「按需裁剪 + 不承诺 ABI」= 大厂自维护剪裁策略；2) 对 ADV 的启示仅在「若未来需 TLS 传输 MCP，优先选 Rust 侧 rustls 栈而非 C 库链路」。
- 第一步动作：无。
- 许可证/成熟度：OpenSSL License + ISC 混合（Google 自持）/ 维护中。链接：https://boringssl.googlesource.com/boringssl/

### 4.2 OpenSSL 3.x｜【reference】｜reference

- 定位：事实标准 C 库；3.x 起 Apache-2.0，provider 架构把 FIPS 模块化。
- 可抄机制：1) **provider 架构**（算法实现可插拔）——与 ADV「档位化后端」思想同构的工程参照；2) FIPS 合规在部分审计场景仍是硬要求（选型时先问「有没有 FIPS/体积约束」）。
- 第一步动作：无。
- 许可证/成熟度：Apache-2.0 / 维护中。链接：https://www.openssl.org

### 4.3 RustCrypto（sha2 等）｜【吸收】｜adv-secrets

- 定位：纯 Rust 密码学原语的默认来源（ADV secrets 相位 CPU 侧底座）。
- 可抄机制：
  1. **运行时分发**：`cpufeatures` 探测 SHA-NI/AVX2/AArch64 扩展，同一 API 自动吃硬件指令；`no_std` 默认。
  2. **多后端矩阵**：x86 SHA-NI/AVX2、AArch64 sha2/sha3 扩展、RISC-V 实验——「如实上报可用后端」可直接进 ADV 的三档状态。
- 第一步动作：审计 ADV 现有哈希调用点，确认走 sha2 crate 且无 `target_cpu` 依赖错误；出 SHA-NI 开/关吞吐对照（本机）。
- 许可证/成熟度：MIT OR Apache-2.0 / 维护中（MSRV 1.85，edition 2024 口径，二手）。链接：https://github.com/RustCrypto/hashes

### 4.4 ring / aws-lc-rs / rustls（若需 TLS 栈）｜【watch】｜watch

- 定位：Rust 侧 TLS 栈；rustls 由 ISRG/Rust Foundation 背景，推荐 aws-lc-rs 作默认 provider。
- 可抄机制：1) **显式 crypto provider**（rustls 0.24 起构造 config 必须显式给 provider，二手）——与 ADV「不静默回退」同构；2) aws-lc-rs 的 FIPS 路径经 `aws-lc-fips-sys`，是 Rust 生态唯一现成 FIPS 故事（二手）。
- 第一步动作：无（ADV 当前本地优先、无 TLS 需求；登记备查）。
- 许可证/成熟度：rustls Apache-2.0 OR MIT OR ISC；aws-lc-rs ISC AND (Apache-2.0 OR ISC) / 维护中。链接：https://github.com/rustls/rustls

### 4.5 age / rage 与 age-plugin-tpm｜【有界】｜adv-secrets

- 定位：现代文件加密工具（age Go / rage Rust，互操作）；插件模型接硬件密钥。
- 可抄机制：
  1. **插件协议**：收件人/身份类型前缀（如 `age1tag1…`）+ 插件经 stdin/stdout 状态机与主程序通信——「算法可插拔且标识自描述」，可借到 ADV 的密钥后端抽象。
  2. **流式常量内存加密**：大文件不整读（`age` crate v0.10 口径，二手）。
  3. **TPM 密封策略**：age-plugin-tpm 把密钥在 TPM 内生成、**密封数据留在 TPM 外**（+ PIN + TPM 会话加密）——「机器绑定身份」与「可迁移密钥」要分开设计（其 README 明确不适用于跨机迁移；v1.0.1 已进 Debian，二手）。
- 第一步动作：只登记「插件协议 + 机器绑定/可迁移二分」两条设计约束，不引依赖。
- 许可证/成熟度：age BSD-3-Clause；rage 与 age-plugin-tpm **待核** / 维护中–实验。链接：https://github.com/FiloSottile/age ；https://github.com/str4d/rage ；https://github.com/foxboron/age-plugin-tpm

### 4.6 TPM / HSM Rust 集成（tss-esapi、cryptoki）｜【watch】｜watch

- 定位：TPM 2.0（TSS ESAPI）与 PKCS#11（HSM/令牌）的 Rust 绑定，均属 Parsec 项目。
- 可抄机制：1) **硬件信任根只在需要时接**：密封/解封密钥、Unseal 策略（PCR 等）放 TPM，业务秘密不进 TPM 存储；2) HSM 走 PKCS#11 统一面，避免厂商专有 API 渗透核心。
- 第一步动作：无（ADV 本地优先暂无硬件信任需求；登记为「秘密存储升级路径」）。
- 许可证/成熟度：两者 Apache-2.0 / 维护中（tss-esapi 7.x，8.0.0-alpha；cryptoki 0.11–0.12；Fedora/Debian 有打包，二手）。链接：https://github.com/parallaxsecond/rust-tss-esapi ；https://github.com/parallaxsecond/rust-cryptoki

---

## 5. 哈希 / 校验的工程加速

### 5.1 blake3 + Bao（树哈希 / 验证流）｜【吸收】｜adv-index

- 定位：BLAKE3 = 树形可并行加密哈希；Bao = 其 verified streaming 编码。ADV 内容寻址存储的哈希层首选。
- 可抄机制：
  1. **1024 B chunk + 树并行**：SIMD 8/16 路、Rayon 线程树并行（官方 README，二手）；大文件吞吐随核数与向量宽度近线性扩展（官方口径）。
  2. **Bao 验证流**：combined/outboard 编码（树开销 ~6.25%）；给定根哈希只校验切片——**「缓存命中校验不重算全量」的直接机制**。
  3. **XOF/派生**：同一树可当 PRF/KDF/XOF 用（ADV 侧慎用：密钥语境仍走 RustCrypto 标准件）。
- 第一步动作：在目标语料出 blake3 vs SHA-256 吞吐曲线（本机、同语料、窗口均值）；确定 adv-index 内容 ID 从 SHA-256 迁 blake3 的判据（兼容期双写）。
- 许可证/成熟度：blake3 CC0-1.0 OR Apache-2.0；bao MIT（**待核**）/ 维护中。链接：https://github.com/BLAKE3-team/BLAKE3 ；https://github.com/oconnor663/bao

### 5.2 xxHash / XXH3｜【有界】｜adv-index

- 定位：非加密极速指纹（索引/去重桶散列）。
- 可抄机制：1) XXH3 的 SIMD 累加结构与短输入特化——ADV 侧用于「大对象快速指纹 → 候选分桶」，碰撞由后续加密哈希兜底；2) 严格限定在**非安全**用途（内容寻址 ID 不得用）。
- 质量警示（二手，来自社区 SMHasher3 讨论）：XXH3 有约 15% 测试项不过的口径——非加密哈希的碰撞质量在对抗场景不设防，定位必须保持。
- 第一步动作：无（如索引需要再引 `xxhash-rust`，单点评审）。
- 许可证/成熟度：xxHash 参考实现 BSD-2-Clause；`xxhash-rust` crate **待核** / 维护中。链接：https://github.com/Cyan4973/xxHash

### 5.3 rapidhash｜【有界】｜adv-index

- 定位：wyhash 后继（hoxxep/rapidhash），Rust 生态 2024–2025 上升的快速哈希；仓库自述「通过全部 SMHasher3 项的最快算法」「比 SipHasher 快 3–4×」（自述口径，二手）。
- 可抄机制：1) 跨 AMD64/AArch64 优化 + no_std + streaming——可做 ADV 索引的默认 HashMap 哈希器候选；2) 定位同 XXH3：**非密码学**。
- 质量警示（二手，**待核**）：2025–2026 有论文（arXiv 2609.06022 口径）分析 wyhash/rapidhash/XXH3 类在随机秘密下的碰撞/差分性质——进入前保持「非对抗用途」边界。
- 第一步动作：无。
- 许可证/成熟度：MIT（**待核**）/ 维护中。链接：https://github.com/hoxxep/rapidhash

### 5.4 SHA-NI 硬件指令（Rust 可达路径）｜【吸收】｜perf-core

- 定位：x86 SHA 扩展的 CPU 侧加速；Rust 生态经 RustCrypto 自动分发（见 4.3）。
- 可抄机制：1) **零代码改动拿硬件路径**（crate 自动探测）；2) **constant-time**（SHA-NI 指令数据无关）对秘密哈希安全；3) 注意 SHA-512 无对应 x86 指令（走 AVX2）。
- 第一步动作：SHA-NI 开/关对照基准（本机）→ 写进 perf-core 基线表（判据：同机窗口均值的吞吐比）。
- 许可证/成熟度：Intel 指令集文档（无许可证约束）/ 维护中。链接：https://github.com/RustCrypto/hashes

### 5.5 CRC32C 硬件指令｜【有界】｜adv-index

- 定位：存储/索引完整性校验的 CPU 侧硬件加速。
- 可抄机制：1) **SSE4.2 `crc32` 三路并行**提升 ILP（crate 口径，二手）；2) **AArch64 CRC 扩展**（基准 21.03 GiB/s vs x86 10.48 GiB/s，librscrc 二手）——跨平台基准须分别测量；3) slice-by-8 软件兜底保正确性。
- 第一步动作：若 adv-index 校验层用 CRC32C，选 `crc32c` 0.6.8 口径 crate 并出硬/软对照。
- 许可证/成熟度：crate Apache-2.0 OR MIT（**待核**）/ 维护中。链接：https://github.com/zowens/crc32c

---

## 6. GPU 在安全扫描的适用性（研究线）

### 6.1 HybridSA（多模式正则 GPU 化，OOPSLA'24）｜【有界】｜perf-core

- 定位：多模式正则匹配的 GPU bit-parallel 方案（ACM PACMPL，DOI 10.1145/3689771）。
- 可抄机制：
  1. **CPU/GPU 模式集切分**：把适合 bit-parallel 的模式放 GPU，其余留 CPU——避免「全量上卡」的形状错配。
  2. **ShiftAnd 系列位并行内核**（ShiftAnd/ShiftAndDist/ShiftAndGap/ShiftAndOps）：用宽寄存器一次推进多个模式状态。
  3. **YARA 子集实测**：其 YARA 数据集口径 6907–7044 MB/s，对照 Hyperscan 1300 MB/s、前代 GPU 方案 HotStart 30 MB/s（论文口径，二手；数字为论文特定语料，不可外推为普遍结论）。
- 第一步动作：无代码；记录「字面量/简单模式集可上卡、复杂正则留 CPU」的切分判据，供 perf-core 正则相位白名单（与 07 文档 Teddy 结论互补）。
- 许可证/成熟度：学术成果（artifact 可用性**待核**）/ 实验。链接：https://dl.acm.org/doi/10.1145/3689771

### 6.2 GPU 自动机/正则先例族（iNFAnt / HotStart / ngAP）｜【reference】｜reference

- 定位：HybridSA 之前的谱系；理解「为什么早期 GPU 正则方案慢」。
- 可抄机制：1) iNFAnt（2010）：共享内存 + symbol-first 转移表示——首个 GPU 多模式匹配；2) HotStart（2020）：热/冷状态分离（热状态占 CUDA 线程）；3) ngAP（2024）：非阻塞大规模自动机，Snort 上 115 MB/s（RTX 3090，论文口径二手）——**仍低于 CPU Hyperscan 量级**，是「简单模式才是 GPU 甜点」的反证。
- 第一步动作：无。
- 许可证/成熟度：学术 / 实验。链接：https://dl.acm.org/doi/10.1145/3689771

### 6.3 KeyHog（2025 Rust GPU 秘密扫描器）｜【吸收】｜adv-secrets

- 定位：与本仓形态最接近的公开先例（Rust + GPU 档扫描器，2025）。
- 可抄机制：
  1. **autoroute 校准**：CPU（Hyperscan）与 GPU 双后端按语料规模自动切换，公开文档给出 **8 MiB 交叉点**（其 autoroute 文档口径，二手）——即「拐点判据 → 运行时路由」的成品范例。
  2. **显式加速器选择 + 失败可见**（初始化失败不静默回退）——与 07 文档已录的收敛结论一致。
  3. **批次窗口与流水**（1 MiB 窗口 + 128 KiB 重叠；async depth 1–4）。
- 基准锚（其公开基准页，二手）：RTX 5090 上 CUDA 后端正则扫描 24.59 ms vs Hyperscan 69.56 ms（中位数，100 对留出）——**单机单卡单窗口数字**，不可外推为普遍结论，但确立「同机交叉点存在且可校准」。
- 第一步动作：抄袭 autoroute 形状——ADV 三档路由：CPU 默认 / GPU 增强 / 显式缺席；交叉点按本机语料出曲线后写死初值（复用 07 拐点判据）。
- 许可证/成熟度：**待核** / 实验（2025 新项目）。链接：https://lib.rs/crates/keyhog ；https://santhreal.github.io/keyhog/

### 6.4 GPU 恶意软件检测研究（2025 线）｜【watch】｜watch

- 定位：GPU 在检测侧的公开研究；与 ADV 的关系是「批量原语可用，检测判定仍在 CPU」。
- 可抄机制：
  1. **批量签名比对**：AFIT 学位论文用 9500 GT 批量 MD5 + 共享内存分块比对，报告文件分桶下 82%–85% 于 CPU（论文口径，二手；硬件代差大，仅证明形状可行）。
  2. **ML 特征侧**：IEEE Access 2025 CUDA CNN-DNN（IoT 恶意检测，batch 32，训练时间 −62%，GTX 1080，二手）——GPU 吃的是训练/推理，不是扫描调度本身。
  3. **模糊哈希 + ML**（SSDHash，Computers & Security 2025，二手）。
- 判据：ADV 侧只吸收「批量 MD5/SSDEEP 比对」这一形状进相位白名单；检测规则判定不上卡。
- 第一步动作：无（纯 watch）。
- 许可证/成熟度：学术 / 实验。链接：https://scholar.afit.edu/cgi/viewcontent.cgi?article=2990&context=etd

---

## 7. 2025–2026 前沿信号

1. **GPU 扫描器成品化**：KeyHog（2025，Rust，多后端 + autoroute 校准 + 显式失败）——「扫描器 GPU 档」从设想变为有公开同类的工程范式（与 07 文档结论互证）。
2. **正则 GPU 化进入位并行成熟期**：HybridSA（OOPSLA'24 后持续被引）、ngAP（2024）把「模式集切分 + 位并行内核」确立为可行路线；GPU 不是全量正则的替代，而是**简单模式集 + 大语料**的加速器。
3. **Rust GPU 计算栈在 2025–2026 明显活跃**：wgpu 持续迭代（发布节奏活跃）、cuBLAS/HIP 侧由 Burn/cubecl 生态带动（rocm-rs 提交到 2026-07，二手）——Rust 单语言 GPU 计算的维护面在变好，但 Windows/AMD 路径仍弱。
4. **哈希质量研究回头审**：2025–2026 出现对 wyhash/rapidhash/XXH3 类随机秘密碰撞/差分性质的论文分析（arXiv 2609.06022 口径，**待核**）——「非加密哈希进安全边界」的做法持续被反证，支持本仓「内容寻址用 blake3」的分离设计。
5. **加密哈希的树并行成为默认**：blake3 生态（Bao 验证流、AVX-512 路径）在内容寻址/去重场景的采用继续扩大；「哈希层 = 内容 ID + 切片校验」两用是 2025 后的常见形态。
6. **硬件信任的可达性提升**：age-plugin-tpm v1.0.1 进 Debian（2025，二手）；tss-esapi 8.0.0-alpha、cryptoki 0.12 活跃——TPM/HSM 在 Rust 侧不再需要自写 FFI（登记备查）。

## 8. Top-3

1. **hashcat 批量同构调度三件套 → perf-core**（吸收）：三层循环 + 目标耗时（~96 ms 量级）自动调参 + 主机/设备左右切分；先以 CPU 线程池验证调度形状，再映射到 WGSL 批量 dispatch（批量熵/直方图/XOR/词法计数）。产出判据：目标内核耗时 vs 批量参数的二维表（本机窗口均值）。
2. **blake3 + Bao → adv-index**（吸收）：内容 ID 用 blake3 树哈希（1024 B chunk、SIMD/线程树并行），大对象切片校验用 Bao 认证路径（~6.25% 树开销、不重算全量）。第一步：blake3 vs SHA-256 本机吞吐曲线 + 迁移判据（兼容期双写）。
3. **CPU 指令侧先榨干 + 显式档位（SHA-NI/crc32c + KeyHog autoroute 形状）→ perf-core/adv-secrets**（吸收）：sha2 crate 自动 SHA-NI、crc32c 三路并行，零重架构成本；GPU 档只保留已验证相位（批量熵/XOR + 大批量哈希），交叉点按本机曲线校准、缺席显式上报（不静默回退）。

## 9. 相位白名单（收口）

| 相位 | 档位 | 依据 |
|------|------|------|
| 批量字节直方图 / 批量熵 | **上卡** | 内部锚 31–38×（单机 Windows）；SoA 零转换 |
| XOR 穷举 / 短密钥空间 | **上卡** | 同族形状（每项独立、聚合回读） |
| 大批量内容哈希（blake3 树层） | **上卡** | 树并行 + 回读仅根/叶摘要 |
| 多模式字面量/简单正则（限位并行模式集） | **有界上卡** | HybridSA 口径（论文语料数字，不外推） |
| 批量 MD5/SSDEEP 比对 | **有界上卡** | AFIT 论文形状（硬件代差，仅形状先例） |
| 小批量 / 单文件 / IO / 编排 / 树遍历 | **不上卡** | 搬运比判据（07 文档口径） |
| 复杂正则 / 回溯匹配 | **不上卡** | GPU 分支发散不友好（ngAP 反证：115 MB/s 级） |
| 嵌入推理 | **走 ort EP** | 不自写 wgpu transformer（07 文档已定） |
