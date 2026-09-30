# 编译提速账（S199）——为什么慢、哪几刀能砍、GPU/NPU 能不能接

> 口径：**只写量到的数字**（本机 Windows / 2026-09-30）；**没量的写"未量"**，不拿"应该会快"当结论。
> 被量的仓：ADV（`unified-rx-mcp`，零依赖 11 bin）、BSHSQ（20 crate / 174 依赖）、
> VoxelForge-V3（Bevy 游戏 / 752 依赖）。

## 一、实测数字（本机）

| 场景 | 命令 | 实测 |
|---|---|---|
| ADV 冷编译（现行 `lto="fat"` + `codegen-units=1`） | 全新 target，`cargo build --release` | **24 s**（产物 1.22 MB） |
| ADV 冷编译（**快档**：`lto=false` + `cgu=16`） | 同上，仅改 profile 旋钮 | **6 s**（**4×**；产物 2.38 MB） |
| BSHSQ 全工作区冷编译 | 全新 target，`cargo build --release` | **41 s**（target 452 MB） |
| BSHSQ 改一个 core 文件后重建门面 | `cargo build --release -p vxl-phys` | **3 s** |
| BSHSQ 测试编译（增量） | `cargo test --no-run`（全工作区） | **79 s** ← 单条最贵 |
| BSHSQ clippy（增量） | `cargo clippy --all-targets` | **9 s** |
| VoxelForge-V3 本地 target 体积 | — | **34 GB**（`D:/v3-target`）；`[profile.test/dev] incremental=false` |

**结论一：单次冷编译并不慢**（ADV 24 s / BSHSQ 41 s）。慢的是**这几处的乘积**：
① CI 里**多个 job 各编一遍**（ADV 6 个 job × ~70 s，本地实测 24 s ⇒ runner 慢 + 无缓存）；
② **测试编译**（BSHSQ 79 s 增量 ⇒ 每个 crate 的单元/集成测试各链接一个可执行文件，Windows 上
链接是瓶颈）；③ 本地**每次 push 跑门链**（ADV 全档 28 步 ≈ 210 s，其中 cargo test + clippy）。

## 二、哪几刀能砍（按"质量零损失"排序）

| 刀 | 做法 | 实测/预期 | 质量影响 |
|---|---|---|---|
| **① CI 用"测试档" profile** | workflow 里给 cargo 传 `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`（或定义 `[profile.ci]`） | **ADV 实测 24 s → 6 s（4×）** | **零**：CI 的 exe 只用于跑测试；**发布档 `lto=fat` 原样保留**（LTO 的收益是体积/内联） |
| ② 换链接器 **lld** | `RUSTFLAGS="-C link-arg=-fuse-ld=lld"`（Windows：rustup 自带 `rust-lld`；Linux：`mold`/`lld`） | **未量**（需一次带 `--timings` 的跑，见 §四） | 零（同语义链接） |
| ③ **sccache** | 编译缓存（内容哈希键控） | **未量**。它绕开"artifact 缓存被 mtime 打穿"的老问题（见 §三） | 零（命中即跳过编译） |
| ④ BSHSQ `[profile.test] debug=0` | 测试档去掉 debuginfo | **未量**；VoxelForge 的经验是 debuginfo 能把测试二进制顶到 2.5 GB | 零（测试不需要调试信息；要调试时单独开） |
| ⑤ VoxelForge `[profile.dev/test] incremental=false` 复核 | 他们为躲"2.5 GB 测试二进制 + PE 损坏(os error 193)"关掉了增量 —— 这**直接牺牲日常迭代速度** | 未量；建议只对 `test` 关，`dev` 恢复增量 | 零（调试体验） |
| ⑥ 本地 target 落盘位置 | BSHSQ 钉在 `C:/vxl-wl-target`（7 GB 热缓存）、VoxelForge 34 GB —— C 盘常年 92%+ | 未量（取舍记账：挪一次要全量重编 41 s，换来 C 盘空间） | 零 |

## 三、缓存 CI：盘点（用户 2026-09-30 指令「加 cache 到我的项目里」）

| 仓 | 现状 | 判定 |
|---|---|---|
| BSHSQ | `Swatinem/rust-cache` **8 处**（Cargo.lock 键控） | ✅ 已有，无需加 |
| qingjian / qingjian-gates | rust-cache **各 7 处** | ✅ 已有 |
| physarena | 1 处（node） | ✅ 够用（无 Rust 构建） |
| unified-rx-mcp | **故意不加**：S163–S165 四轮实测**否证**——零依赖 + checkout 刷源码 mtime ⇒ cargo 判据必然重编（恢复 86 MB target 后仍旧 "Finished in 1m15s"，与冷跑无差） | ✅ 不加（附证据） |
| VoxelForge-V3 | `.github/` **未入库**且**无 remote** ⇒ 那份 `ci.yml` 根本不生效 | ❌ 无处可加 |
| TWO-TOWEI / 泰拉科技 | 只有 Python 名门 / 无 remote | ❌ 无编译可缓存 |

⇒ **没有"再加一层缓存"的缺口**。若将来 VoxelForge 建 remote 并启用 CI：**第一件事就是加
`Swatinem/rust-cache`**（752 依赖，冷跑是这个 job 的绝对大头）；注意它的默认行为——
**只缓存依赖、不缓存工作区自己的产物**（rust-cache 的 `cache-workspace-crates` 是另一个档），
而工作区产物受 mtime/指纹影响，通常不值得缓存。

## 四、还没量的两刀（要一次 `--timings` 或一次 CI 才作数）

- **链接到底占多少**：`cargo test --no-run --timings` 的 JSON 里有 per-unit 的 `link` 秒数；
  拿到份额才知道 ② 值不值。
- **lld / sccache 在 CI 上的真实收益**：得在 CI 上 A/B（本地单机数字不能外推——
  与 estimate 口径无关的"应该更快"不算证据）。

## 五、GPU / NPU 卸载编译：判定 = **不采纳**（给理由，不给人云亦云）

用户提的三条路线我都对过：
1. **"GPU 内核当原生函数、CPU/GPU 统一编译"** —— 那是在编**被执行的程序**，不是在编**编译器**；
   对"让 Rust 编得更快"零贡献。
2. **绕开 LLVM、Rust→PTX 直接生成** —— 产物是 **GPU 代码**，不是 x86 可执行文件；本仓/你项目的
   交付物是 Windows/Linux 二进制 ⇒ 换不了。且现状**只支持 CUDA**，跨平台性受限（用户也提到了）。
3. **"用算力换时间"把编译阶段搬 GPU/NPU** —— 编 x86 的目标是 LLVM 后端 + 链接器；
   这两者在 GPU/NPU 上没有可用实现（NPU 更没有通用工具链）。
   **反例要认**：LLVM 有 GPU 目标（NVPTX/AMDGPU）但那是"生成 GPU 代码"；`rustc_codegen_cranelift`
   是"换后端"（可提速 dev 构建，但**非 GPU**、且对全部语义不等价 ⇒ 只在 dev 用属有界）。
⇒ **本项记为"不采纳"**，理由如上；真正能"用算力换时间"的是 §二 那六刀（都是纯工程项）。

## 六、给你的动作单（各仓一句）

- **ADV**：CI 加 `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
  （实测 4×，发布档不动）——**唯一还没榨的**。
- **BSHSQ**：缓存已就位；下一步只在"要不要 `[profile.test] debug=0` + lld"（需 A/B）。
- **VoxelForge**：`dev` 恢复 `incremental`（只对 `test` 关）；建 remote 后第一件事挂 rust-cache。
- **本地**：`cargo check` 代替 `cargo build` 做迭代；不要给日常迭代设 `CARGO_INCREMENTAL=0`
  （那是给 CI 的）。
