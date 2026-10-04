# R4 — 二进制格式与符号/调试信息解析（深度调研）

- 域：R4（纲领见 `PROGRAM.md`）；服务两条线：**adv-bin**（逆向/二进制专线，格式解析子模块）与 **adv-sca**（二进制成分分析）。
- 日期：2026-10-03；方法：WebSearch/WebFetch 定向检索，优先 2025–2026 资料；单源/未经复核的数字均标「待复核」，版本号带检索口径。
- 边界：R1（反编译）、R2（二进制分析/符号执行）、R3（调试器）已覆盖动态交界；R3-025 已登记 gimli 一行，本文不重复该粒度，只从「结构/版本/工程参数」面深挖。R5（恶意软件）、R6（利用缓解）、P6/P8（研究）后续交叉引用。
- 每对象格式：**定位 → 可抄机制 1–3 → 档位【吸收|有界|不吸收|watch|reference】→ 第一步动作 → 许可证/成熟度【实验|维护中|弃维护】→ 链接**。

---

## 0. 总览表（44 个对象）

| id | 对象 | 类 | 档位 | integration | 许可 | 成熟度 |
|---|---|---|---|---|---|---|
| R4-001 | object（crate） | 解析库 | 吸收 | adv-parse | MIT/Apache-2.0 | 维护中 |
| R4-002 | goblin | 解析库 | 有界 | adv-parse | MIT | 维护中 |
| R4-003 | LIEF / lief-rs | 解析库(读写) | 有界 | adv-bin | Apache-2.0 | 维护中 |
| R4-004 | pelite | 解析库(PE) | 吸收 | adv-bin | MIT | 维护中(低频发版) |
| R4-005 | pe-parser (iscc) | 解析库(PE) | reference | reference | MIT(待复核) | 待复核 |
| R4-006 | windows-rs（windows/windows-metadata） | 绑定/官方路径 | 有界 | adv-bin | MIT/Apache-2.0 | 维护中 |
| R4-007 | Go debug/pe,macho,elf,buildinfo | 对照实现 | 有界 | reference | BSD-3-Clause | 维护中 |
| R4-008 | Kaitai Struct | 规格 DSL | reference | reference | 编译器 GPL-3.0/运行时 MIT(待复核) | 维护中 |
| R4-009 | wasmparser / wasm-tools | 解析库(WASM) | 吸收 | adv-bin | Apache-2.0 with LLVM exception | 维护中 |
| R4-010 | DWARF 5（规范） | 格式规范 | 吸收(规范面) | adv-parse | 规范自由分发 | 稳定 |
| R4-011 | gimli | DWARF 读层 | 吸收 | adv-bin | MIT/Apache-2.0 | 维护中 |
| R4-012 | addr2line | 地址→位置 | 吸收 | adv-bin | MIT/Apache-2.0 | 维护中 |
| R4-013 | pdb crate（pdb2 0.10.0） | PDB 读层 | 吸收 | adv-bin | MIT/Apache-2.0(待复核) | 维护中 |
| R4-014 | symbolic (Sentry) | 符号化统一层 | 有界 | adv-bin | MIT | 维护中 |
| R4-015 | microsoft-pdb | 参考源码 | reference | reference | 专有(公开参考) | 冻结参考 |
| R4-016 | dbghelp/DbgEng 符号 API + DIA SDK | 官方消费路径 | reference | watch | 专有(系统组件/SDK) | 维护中 |
| R4-017 | Breakpad .sym | 符号格式 | 吸收(格式) | adv-bin | BSD-3-Clause | 维护中(低频；Crashpad 后继) |
| R4-018 | Crashpad | 崩溃捕获 | watch | watch | Apache-2.0 | 维护中 |
| R4-019 | minidump + rust-minidump | 转储格式/处理 | 吸收 | adv-bin | MIT | 维护中 |
| R4-020 | dSYM 包 | 格式(macOS) | 有界 | adv-bin | 格式(工具 Apache-2.0) | 维护中 |
| R4-021 | rustc split-debuginfo 三档 | Rust 机制 | 吸收(策略) | adv-bin | MIT/Apache-2.0(rustc) | 维护中 |
| R4-022 | .gnu_debuglink + build-id | 格式约定 | 吸收 | adv-parse | 格式(binutils GPL-3.0 为工具) | 维护中 |
| R4-023 | 压缩调试段（SHF_COMPRESSED） | 格式 | 有界 | adv-parse | 格式(zlib/zstd 许可) | 维护中 |
| R4-024 | GSYM | 索引格式 | watch | watch | Apache-2.0 with LLVM exception | 维护中 |
| R4-025 | blazesym | 符号化库 | watch | watch | BSD-2-Clause(待复核) | 维护中 |
| R4-026 | Rust v0 mangling + rustc-demangle | Rust 机制 | 吸收 | adv-parse | MIT/Apache-2.0 | 维护中 |
| R4-027 | cargo-auditable | 供应链机制 | 吸收 | adv-sca | MIT/Apache-2.0 | 维护中 |
| R4-028 | Go buildinfo | 供应链机制 | 吸收 | adv-sca | BSD-3-Clause | 维护中 |
| R4-029 | syft binary cataloger | 供应链工具 | 吸收 | adv-sca | Apache-2.0 | 维护中 |
| R4-030 | blint | 供应链工具 | 有界 | adv-sca | Apache-2.0(待复核) | 维护中 |
| R4-031 | dnfile / dnfile-rs / SRM / Cecil | .NET 解析 | 吸收 | adv-sca | 各 MIT 系(待复核) | 维护中 |
| R4-032 | Java class/jar 解析 | 供应链格式 | 有界 | adv-sca | 规范开放(JVMS) | 维护中 |
| R4-033 | Python wheel/native 组件 | 供应链格式 | 有界 | adv-sca | 规范(PSF) | 维护中 |
| R4-034 | SPDX / CycloneDX | SBOM 载体 | 有界 | adv-sca | 规范(CC-BY-3.0/Apache-2.0) | 维护中 |
| R4-035 | 二进制→源码 SCA 研究（2025） | 研究 | 吸收(路线判据) | adv-sca | 论文/学位论文 | 前沿 |
| R4-036 | BinDiff | 差分工具 | 有界 | adv-bin | Apache-2.0 | 维护中 |
| R4-037 | BinExport | 差分中间格式 | 有界 | adv-bin | Apache-2.0 | 维护中 |
| R4-038 | Diaphora | 差分工具 | reference | watch | GPL-3.0(待复核) | 维护中 |
| R4-039 | QBinDiff | 差分研究工具 | reference | watch | LGPL-3.0(待复核) | 维护中 |
| R4-040 | DeepDiff（2025） | 差分研究 | watch | watch | 论文 | 前沿 |
| R4-041 | Patch diffing（技术） | 技术方法 | 有界 | adv-bin | 技术 | — |
| R4-042 | MS 符号服务器协议 + symsrv crate | 协议/分发 | 吸收 | adv-bin | 协议文档公开；crate MIT/Apache-2.0(待复核) | 维护中 |
| R4-043 | srcsrv（Source Server） | PDB 内数据流 | 有界 | adv-bin | crate MIT(待验证) | 实验(crate)/稳定(格式) |
| R4-044 | debuginfod | 调试文件分发 | 吸收 | adv-bin | GPL-3.0(服务端；协议公开) | 维护中 |

---

## 1. 重点回答

### ① Rust 生态解析栈选型：object vs goblin vs LIEF 的覆盖与缺口

**结论（建议）**：主读层选 **object**（统一 facade、生态最广、被 addr2line/gimli/backtrace 依赖）；PE 细节补位选 **pelite**（Windows 特有结构、零分配）；深挖/写回/多格式超集用 **LIEF 独立子进程**（不把 C++ 大依赖链进主链）；**goblin** 只做快速嗅探备选与对照实现（其前缀首发匹配在歧义文件上会误判格式，不宜当安全边界）。WASM 独立走 **wasmparser**。

| 维度 | object (0.38.x) | goblin (0.10.x) | LIEF (0.17.x + lief-rs) | pelite (0.10.x) |
|---|---|---|---|---|
| 读覆盖 | ELF/PE/COFF/Mach-O/XCOFF（32/64） | ELF/Mach-O/PE/unix archive | +COFF/OAT/DEX/VDEX/ART/dyld 共享缓存 | 仅 PE/COFF（含 in-memory） |
| 写覆盖 | 可重定位目标文件 + ELF/PE 可执行 | 基本无 | 全格式重建（Builder） | 无（只读） |
| 符号/DWARF | 节/符号/调试目录原语（DWARF 交 gimli） | 符号表原语 | 自带 DWARF **Editor** + PDB reader + LLVM 反汇编 | 符号/Rich header/重定位等 PE 细分 |
| 解析风格 | 统一 trait，sniff 按 magic | 零拷贝借用 + 16B 首发 sniff（有歧义风险） | C++ 描述隐式，格式无关 Binary 立面（有损） | pe32/pe64 双模块静态分派、零分配视图 |
| 重量 | 纯 Rust 中量 | 最小（no_std 可） | 重（C++ 树约 124 kLOC 口径，检索于 2026-10） | 轻 |
| 生态锚 | 约 375M 总下载、~161 直接依赖包（第三方统计，检索于 2026-10） | ~1.3k stars、~37.5M 下载（同上） | ~4.4k stars；Rust 绑定较新（~22.4k 下载口径） | 最近 crate 发版 0.10.0（2022-11-04，低频信号，待复核） |
| 缺口 | 无 DWARF/PDB 语义、无写 PE 深度修改 | 写弱、歧义 sniff、eager 物化（可选 lazy_parse） | 构建重、写回有非幂等先例（须 check_layout 校验） | 仅 PE；不统一 32/64（设计取舍） |

**关键取舍锚**（检索口径，2026-10）：
- LIEF 0.17 线补了 **PE delayed imports 重做、可重算 RichHeader、computed_checksum、LoadConfiguration 重构**；Mach-O 补 **LC_FILESET_ENTRY**（内核缓存）与 FatBinary 按架构选择；ELF 修 PHDR/SHDR 对齐与 IAT 解析（0.17.6）——即"Windows 特有结构"在 LIEF 与 pelite 有明确能力，object 不承诺同等细度。
- goblin 的 16 字节前缀首发匹配在 APE 等歧义文件上会把 PE 判出来（"parser differential"），做安全判定时必须用**双解析器交叉或 magic+结构双重校验**。
- LIEF 的 Builder 是 opt-out 式（config_t 逐结构布尔），存在非幂等写回的历史坑 ⇒ 写回后必须过 `check_layout()` 类校验；ADV 若用，只做"读深挖 + 生成新文件"两段，不做原地改。

**adv-bin 选型基线**：`object::FileKind::parse` 嗅探 → 主解析 object；PE 附加 pelite（Rich header/base relocs/资源等）；WASM 走 wasmparser；只要涉及"重建/编辑/DEX 系/共享缓存"再降级到 LIEF 子进程。

### ② PDB/DWARF 消费：Windows（MSVC 产物）与 Linux（GNU 产物）的工程路径与坑

**Windows/MSVC 路径（PDB）**
1. PE → 调试目录（Debug Directory，IMAGE_DIRECTORY_ENTRY_DEBUG）→ **CodeView RSDS 记录**：GUID(16B) + Age(4B) + UTF-8 PDB 路径。这是"二进制→PDB"的唯一权威键。
2. 定位 PDB：本地路径 → `_NT_SYMBOL_PATH`/msdl（按 GUID+Age，见 ④）→ srcsrv（源文件回源，见 R4-043）。
3. 消费实现两条路：
   - **纯 Rust**：`pdb` crate（pdb2 0.10.0，2025-09-22；fork 自 willglynn/pdb）按需 on-disk 解析 MSF 容器与 PDB/TPI/DBI/IPI 流，无 DIA/Windows 依赖；`pdb-addr2line` 直出"地址→文件/行/内联帧"；`symbolic` 额外给跨格式统一与 SymCache。
   - **官方路径**：`dbghelp.dll`（SymInitialize/SymLoadModuleEx/SymFromAddr，天然接 symsrv.dll）+ DIA SDK（msdia140.dll，可查类型/源行）；经 windows-rs 可调用，但**专有组件、语义偏"调试器辅助"，不适合做批量离线扫描内核**。
4. 坑（工程口径）：
   - **匹配校验必须做**：PDB 与二进制可能不配对，必须比对 CodeView 里 GUID+Age 才可采信；
   - **源码路径是编译机路径**：行号映射可得，源文件内容需 srcsrv/SourceLink 才能回源；
   - **内联帧**需要 FrameTable/InlineSite 支持（pdb crate 有对应类型）；
   - 老格式覆盖不全（pdb2 自述不支持 16-bit/COBOL 用户定义类型等），覆盖率要按样本实测；
   - **MSVC 的"剥离"就是不在二进制里**：发布常只发 PE 不发 PDB ⇒ 恢复路径=符号服务器（④），否则只剩导出表/字符串级线索。

**Linux/GNU 路径（DWARF）**
1. ELF → `.debug_*`（DWARF 4/5 并存）→ **gimli**（zero-copy lazy 读）+ **addr2line**（find_frames/内联栈）。
2. split debuginfo 三种定位链（按优先级实现）：`.gnu_debuglink`（文件名+CRC32 → `<dir>/.debug/`、`/usr/lib/debug/<path>`）；`.note.gnu.build-id`（→ `/usr/lib/debug/.build-id/xx/yyyy.debug`）；**debuginfod**（build-id 键 HTTP，见 R4-044）。
3. addr2line 对 split 的接口是**续延式**：`LookupResult::Load(SplitDwarfLoad{ dwo_id, comp_dir, path })` → 调用方补齐文件（`.dwo` 或 `.dwp`）→ 续延继续；扫描器要自己实现"debuglink→build-id→debuginfod"三层查找回调，DWP 包与 Mach-O dSYM 都有内置 loader。
4. 坑：
   - DWARF 5 行程序 file index 0 基 vs 旧版 1 基（addr2line 源码明确指出 `version <= 4 may not have 0th index`）——自研行表时最容易翻车处；
   - `.debug_str_offsets`/`.debug_addr`/`.debug_line_str`（DWARF 5 索引化）需要正确解析 base，否则字符串/地址全错位；
   - 压缩调试段（SHF_COMPRESSED + zlib/zstd）**先解压再解析**（R4-023）；
   - GCC 与 Clang 对 DWARF 5 的扩展差异（如 `-gpubnames` 风格、DW_LNCT_MD5）需双编译器语料回归；
   - Rust 产物 debug info 体积大：`split-debuginfo` 三档决定你拿到的是整段、`.dwp` 还是散件（R4-021）。

**Rust 侧交叉**：Windows 默认 `packed`（→ PDB）、Linux 默认 `off`——所以"Linux 上拿到的 release Rust 二进制常常没有任何调试信息"，这是策略问题不是解析问题；adv-bin 的符号能力必须按"有 PDB / 有 DWARF / 有 .sym / 全无"四档降级设计。

### ③ cargo-auditable 机制拆解（可复用性；对 adv-sca 的二进制成分分析路线）

**机制链（官方 README 口径，检索于 2026-10）**：依赖树 → **JSON**（有 schema：cargo-auditable.schema.json）→ **zlib 压缩** → 注入专用链接器节 **`.dep-v0`**（ELF/PE/Mach-O 通用；wasm 自 0.6.3 支持）。设计要点：
- **可复现构建友好**：JSON 有序、无时间戳，同源两次构建输出一致；
- **体积小**：400+ 依赖 <4KB（约为二进制的 1/1000–1/10000）；
- **消费极简**：README 称 5 行 Python 可解；官方 `rust-audit-info` 直接吐 JSON；已形成生态：cargo-audit 0.17.3+、trivy 0.31.0+、grype 0.83.0+、osv-scanner 2.0.1+、syft 1.15.0+ 均可消费；
- **rustc 版本**：Rust 1.73 起编译器自身把 rustc 版本写进产物（更早只能从 debug info 字符串猜）；
- **上游化**：nightly 已可 `CARGO_BUILD_SBOM=true cargo +nightly auditable build -Z sbom`（Cargo 原生 SBOM 前身）；RFC rust-lang/rfcs#2801 在做；采用方含 Microsoft 内部与 Alpine/NixOS/openSUSE/Void/Chimera/Wolfi 等发行版（Ubuntu 26.04 定向 opt-in，单源待复核）。

**可复用到其他语言吗**：机制本身（**JSON+zlib+专用 linker section**）语言无关——任何能注入自定义节的工具链都能做（Rust 用 `#[used] static` + link_section；C/C++ 可用 linker 脚本/objcopy）。
- **Go 已经原生有**（`.go.buildinfo`，R4-028）——所以"二进制内嵌依赖清单"已是两大语言的事实标准；
- .NET/Java/Python 目前**没有等价标准**（.NET 仅发布目录的 deps.json；jar/wheel 的元数据随包不随独立二进制）——对 ADV 是机会也是缺口；
- 观察：nightly Cargo 原生 SBOM 若落地，`.dep-v0` 可能被取代 ⇒ 读侧应同时支持 `.dep-v0` 与未来原生节（探测式，别写死）。

**adv-sca 二进制路线（建议分级）**：
1. **L1 元数据层（本域，先做）**：`.dep-v0`（Rust）、`.go.buildinfo`（Go）、.NET AssemblyRef 表、jar pom.properties —— 官方/内嵌数据，精确、无外部依赖；
2. **L2 符号/字符串层**：导出表、demangle 后符号（R4-026 v0 mangling）、版本字符串启发式（syft binary-classifier 类兜底，高误报，必须标置信度）；
3. **L3 函数语义层（后续域）**：剥离符号后走函数相似性/二进制→源码匹配（R4-035 的 2025 研究路线），成本最高、作为研究位。

**对 ADV 自身**：ADV 产出物应构建时嵌入 `.dep-v0`（cargo-auditable 一行接入），保证自身可被 sbom 工具审计——"吃狗粮"且对下游免费。

### ④ 符号服务器协议消费的最小实现（MS Symbol Server / symsrv）

**协议要点（MS Learn + 检索口径）**：
- 符号路径语法：`srv*<下游缓存>*<上游存储>`（完整形式 `symsrv*symsrv.dll*...`，可多级 `*` 串联）；公共上游 `https://msdl.microsoft.com/download/symbols`；HTTP 存储**只读**——要当"下游缓存"必须架 SymProxy（IIS ISAPI 过滤器）。
- **检索路径规则**：`<文件名>/<签名>/<文件名>`；签名 = **GUID 32 位十六进制 + Age 十六进制**（无分隔）。实例：`dcomp.pdb/648B8DD0780A4E22FA7FA89B84633C231/dcomp.pdb`（末位 `1` 为 age）。
- 压缩件：以 `_` 结尾（`.pd_`），由 compress.exe 产物（MSZIP/CAB 族），使用前需解压。
- 索引器 SymStore 建库、AgeStore 清理、srcsrv 管源文件——都围绕同一路径规则。
- **数据来源键**：Windows 侧 = PE 调试目录里 CodeView RSDS 的 GUID+Age+PDB 名（②已述）。

**最小实现（adv-bin 可落地步骤）**：
1. 解析目标 PE 的调试目录，取 RSDS：GUID/Age/PDB 文件名；
2. 组装相对路径 `<pdb名>/<GUID32+AgeHex>/<pdb名>`；
3. 三级查找：本地缓存目录 → 自建/局域网存储 → `msdl.microsoft.com`（HTTP GET）；
4. 命中后按 PDB 内的 GUID/Age 与请求自校验，再交 PDB 解析器（R4-013）；
5. 缓存进下游目录，二次命中零网络。
- Rust 先行件：`symsrv` crate（0.2/0.3 口径）：解析 `_NT_SYMBOL_PATH`（分号分段、星号分层）、async 下载 + tokio::fs 缓存，`SymbolCache::get_file(相对路径)` 即得内容——**MVP 直接用它，不要自研协议栈**；待复核其维护活跃度。
- 红线：按需取用（一个二进制只取自己需要的 PDB），不批量镜像；网络不可用时整体降级为"无符号"模式（本地优先原则）。

### ⑤ adv-bin「格式解析」子模块接口设计建议（一次解析缓存成什么结构）

**设计原则**：*一次 mmap、分层解析、零拷贝视图、按需深挖*（学 object 的 facade + gimli/pdb 的 lazy 风格）。

```rust
// 第一层：嗅探（纯字节，不建结构）
probe(bytes) -> FileKind          // Elf32/64, Pe32/64(含 COFF), MachO(含 fat),
                                  // Wasm, Archive, Dotnet(PE+CLR 头双判), 未知

// 第二层：浅解析（一次完成，结果缓存）——Immutable "ImageFacts"
parse(bytes, &Budget) -> ImageFacts {
  identity:   { kind, arch, os_abi, entry, image_base, endian,
                ids: { elf_build_id | pe_pdb(GUID+age+name) | macho_uuid },
                sha256 }
  layout:     [{ name, va, vsize, file_off, size, flags, entropy }]
  linkage:    { imports: [...] (PE 含 delay_imports), exports: [...],
                elf:  { soname, needed[], rpath[] },
                macho: { dylibs[], rpaths[] },
                pe:   { exports_ordinals, bound_imports } }
  debug_refs: { dw: [{type, size, rva, ptr}],        // PE Debug Directory 全条目
                cv: Option<CodeViewRsds>,            // GUID/age/path
                debuglink: Option<{name, crc32}>,    // ELF
                build_id: Option<[u8;20]>,
                dSYM_uuid: Option<[u8;16]> }
  toolchain:  { rustc: Option<String>,                // Rust>=1.73 内嵌
                dep_v0: Option<Range<usize>>,         // cargo-auditable 原始节区
                go_buildinfo: Option<Range<usize>>,
                msvc_rich_header: Option<[u32;4]>, linker_ver: Option<String> }
  security:   { pe_dllchars (ASLR/CFG/CET/SEH), elf: {pie, relro, nx, canary},
                macho_flags, authenticode_present }
  code_meta:  { pe_pdata: Option<Range<usize>>,       // 异常表 .pdata（自研解析）
                eh_frame: Option<Range<usize>> }      // 只记位置，不展开
}

// 第三层：按需深挖（不默认执行；有预算/超时护栏）
deep::dwarf(&ImageFacts) -> DwarfIndex      // gimli+addr2line，split 链: debuglink→build-id→debuginfod
deep::pdb(&ImageFacts, SymbolSource) -> PdbDb
deep::sbom(&ImageFacts) -> DepList          // .dep-v0 / .go.buildinfo / .NET AssemblyRef / jar
deep::wasm(&ImageFacts) -> WasmInfo         // wasmparser 流式事件 + 验证（含 MAX_* 上限）
deep::unwind(&ImageFacts) -> FuncBounds     // PE .pdata / .eh_frame → 函数边界
```

**要点**：
- **缓存键**：`(canonical_path, size, mtime, 首 4KB sha256)` → `facts.bin`（postcard/bincode 序列化，参 X3 域）；`ImageFacts` 必须可序列化且不含指针（自相对偏移表示）。
- **恶意输入防护**：预算是 first-class——段数/符号数/流数/单段解压上限、LEB128/长度字段溢出检查、递归深度上限；直接照抄 wasmparser 的"验证上限"工程化思路（MAX_WASM_TYPES=1,000,000 / MAX_WASM_FUNCTION_LOCALS=50,000 等，检索于 2026-10）与 goblin"不要信任声明长度"的教训。
- **sniff 不做安全边界**：格式判定用双条件（magic + 结构校验），歧义（PE vs APE/NET）显式返回多候选，深挖时再消歧。
- **符号化输出与 addr2line `find_frames` 对齐**（地址→帧列表，含内联栈），上层展示不感知来源（PDB/DWARF/.sym 统一为一个 `Symbolizer` trait；对 R4-014 的 SymCache 只抄"扁平化预索引"思想）。
- **不默认做的事**：不解析外部文件（PDB/dwp 只在符号化阶段按需定位）、不执行任何 PDB 内命令（srcsrv 红线）、不写回原文件。

---

## 2. 对象明细

### A. 格式解析库

### R4-001 object（crate）
- 定位：Rust 生态首选读层；被 addr2line/gimli/backtrace 依赖，事实标准。
- 可抄机制：1) 统一 facade：`FileKind::parse` 嗅探 + `read::Object` trait 把 ELF/PE/COFF/Mach-O/XCOFF 拉平为 sections/symbols/relocations；2) 读+写双能力（可重定位目标 + ELF/PE 可执行），同一 crate 内完成"解析→改写→产出"；3) 生态定位学——多格式统一但不包办语义（DWARF/PDB 交给专库），是"薄 facade"的样本。
- 档位：吸收；integration：adv-parse（+adv-bin）。
- 第一步动作：以 object 为 adv-bin `ImageFacts` 的唯一主解析器建原型；用 10 个真实样本（Linux ELF/MSVC PE/macOS Mach-O/Rust/Go/C++）做字段覆盖回归。
- 许可证/成熟度：MIT/Apache-2.0；维护中（0.38.x；第三方统计约 161 直接依赖包，检索于 2026-10）。
- 链接：https://github.com/gimli-rs/object

### R4-002 goblin
- 定位：轻量零拷贝解析（no_std 可），安全工具/加载器常用。
- 可抄机制：1) 借用式零拷贝（`Elf<'a>`/`Strtab<'a>` 不复制）；2) 前缀 sniff 设计及其**反面教训**：16 字节首发匹配在歧义文件（APE→PE）会误判——提示我们 sniff 必须双条件；3) 默认 eager 物化（soname/libraries/rpaths 解析时展开）+ `lazy_parse` 开关的取舍。
- 档位：有界（快速嗅探备选/对照实现，不作安全边界）；integration：adv-parse。
- 第一步动作：在测试夹具里加"goblin vs object 判定分歧"的护栏用例（歧义文件集）。
- 许可证/成熟度：MIT；维护中（0.10.x；~1.3k stars 口径，检索于 2026-10）。
- 链接：https://github.com/m4b/goblin

### R4-003 LIEF（+lief-rs 官方绑定）
- 定位：格式覆盖最广的解析/修改库（C++ 核 + 多语言绑定），深挖与重建通道。
- 可抄机制：1) 格式无关 `Binary` 立面（sections/symbols/entrypoint/patch 抽象，接受有损）；2) `Builder::config_t` 逐结构 opt-out + `check_layout()` 事后校验的"可重建但需验证"工程模式；3) 能力超集：DWARF **Editor**、PDB reader、LLVM 反汇编/汇编器、DEX/OAT/VDEX/ART、dyld 共享缓存 dylib 提取、Mach-O LC_FILESET_ENTRY、PE delayed imports/RichHeader 重算。
- 档位：有界（深挖子进程/可选 feature；写回有非幂等先例，只"读深挖+产新文件"）；integration：adv-bin。
- 第一步动作：做一个 `lief` CLI 对照台（同一 PE 上对比 object/pelite/LIEF 三方字段差异报告），评估把哪些字段补进 ImageFacts。
- 许可证/成熟度：Apache-2.0；维护中（0.17.x 活跃发版：0.17.4/0.17.6 changelog 口径）。
- 链接：https://github.com/lief-project/LIEF

### R4-004 pelite
- 定位：Windows PE 细分结构 + 零分配视图解析（pe32/pe64 双模块设计）。
- 可抄机制：1) 刻意**不统一** PE32/PE32+（静态分派、无运行时匹配开销——与 object 的统一 trait 是两种工程取舍）；2) 模块化 PE 细分：imports/exports/base_relocs/**rich_structure**（Rich header）/resources/TLS 等；3) 同一套代码支持 disk/in-memory 两种视图（对"加载后镜像"分析直接可用）。
- 档位：吸收（Windows PE 补位层，object 之上的细节增强）；integration：adv-bin。
- 第一步动作：把 Rich header、base relocs、TLS 目录接入 `ImageFacts`（object 不细拆的部分）。
- 许可证/成熟度：MIT；维护中（低频：crate 最近发版 0.10.0 @2022-11-04，用前复核仓库近期提交，待复核）。
- 链接：https://github.com/CasualX/pelite

### R4-005 pe-parser (iscc)
- 定位：面向安全工具的极简 PE 解析（无依赖、可审计）。
- 可抄机制：最小集原则——只为工具所需字段建模（头/节/导入），体量小便于 diff 审计；适合作为"多解析器交叉验证"的第三方参照。
- 档位：reference；integration：reference。
- 第一步动作：纳入交叉验证夹具（同 PE 喂 object/pelite/pe-parser，比对字段一致性）。
- 许可证/成熟度：MIT（待复核）；成熟度待复核（用前查仓库活跃度）。
- 链接：https://github.com/iscc/pe-parser

### R4-006 windows-rs（windows / windows-metadata）
- 定位：Windows 官方 API 的 Rust 入口——含 dbghelp/DbgEng/DIA COM 绑定（符号消费的官方路径），及 windows-metadata（WinMD=ECMA-335 子集）解析。
- 可抄机制：1) COM 接口生成器治理（windows crate 的元数据投影机制）是"声明式绑定生成"的成熟样本；2) 对 ADV：用其调用 dbghelp 做 PDB 符号化的**兜底路径**（纯 Rust 路径失败时）。
- 档位：有界（仅兜底/官方对照，不做主解析器）；integration：adv-bin。
- 第一步动作：写一个最小 `SymFromAddr` 样例（windows-rs 调 dbghelp + msdl）作为符号化对照基准。
- 许可证/成熟度：MIT/Apache-2.0；维护中。
- 链接：https://github.com/microsoft/windows-rs

### R4-007 Go 标准库 debug/pe、debug/macho、debug/elf、debug/buildinfo（对照）
- 定位：Go 侧格式解析的原生实现；buildinfo 读取（.go.buildinfo 跨格式定位）。
- 可抄机制：1) 每格式一包 + 共享接口的小而全结构；2) buildinfo 的"按 magic 定位节区"实现可作为 adv-sca Go 线的参考实现；3) `go version -m <bin>` 是天然 ground truth 工具（回归对账用）。
- 档位：有界（对照/对账，不内联）；integration：reference。
- 第一步动作：把 `go version -m` 与自研 buildinfo 解析对拍进测试夹具。
- 许可证/成熟度：BSD-3-Clause；维护中（随 Go 发行版）。
- 链接：https://pkg.go.dev/debug/buildinfo

### R4-008 Kaitai Struct
- 定位：声明式二进制格式 DSL（.ksy）→ 多语言解析器生成。
- 可抄机制：把格式规格变成可执行 schema（含 PE/ELF 社区 schema）——"规格即测试"的思路；对 ADV 的启发：格式知识用声明式描述，测试夹具从 schema 派生。
- 档位：reference；integration：reference。
- 第一步动作：用 .ksy 版的 PE/ELF 描述对照人工解析器语义差异（作为规格教研工具）。
- 许可证/成熟度：编译器 GPL-3.0 / 运行时 MIT（待复核）；维护中。
- 链接：https://kaitai.io/

### R4-009 wasmparser / wasm-tools（Bytecode Alliance）
- 定位：WebAssembly 二元格式与组件模型的权威 Rust 解析+验证栈。
- 可抄机制：1) 事件驱动流式解析：`BinaryReader`（LEB128 变长整数 + 精确错误偏移）→ `Parser` 逐 section 发 `Payload` 事件，内存占用恒定；2) `OperatorValidator` 用操作数栈+控制流栈做逐函数验证；3) **验证上限工程化**（MAX_WASM_TYPES=1,000,000、MAX_WASM_FUNCTION_SIZE=7,654,321B、MAX_WASM_FUNCTION_LOCALS=50,000 等，检索于 2026-10）——防资源耗尽的范本，adv-bin 预算设计直接借鉴。
- 档位：吸收（WASM 格式线唯一主选）；integration：adv-bin。
- 第一步动作：`deep::wasm` 最小实现：sections/imports/exports 提取 + 验证开关（validate feature）。
- 许可证/成熟度：Apache-2.0 with LLVM exception；维护中（wasmparser 0.218.0 / wasm-tools 1.0.28 口径；月下载约 1,660 万，检索于 2026-10）。
- 链接：https://github.com/bytecodealliance/wasm-tools

### B. 符号与调试信息

### R4-010 DWARF 5（规范）
- 定位：Linux/macOS 调试信息的事实标准（第 5 版特性面）。
- 可抄机制：1) 索引化设计：`.debug_str_offsets`/`.debug_addr`/`.debug_line_str` + `DW_FORM_strx/line_strx`——减少重定位、加速解析；2) 行程序（line program）状态机与 file 表（v5 file index 0 基）；3) split DWARF（.dwo/.dwp 骨架单元）与 `.debug_names` 索引。
- 档位：吸收（规范面，落成解析器必须遵守的行为清单）；integration：adv-parse。
- 第一步动作：写"DWARF 版本兼容矩阵"（2–5，GCC/Clang 双列）作为 gimli 封装的回归依据。
- 许可证/成熟度：规范自由分发（dwarfstd.org）；稳定。
- 链接：https://dwarfstd.org/

### R4-011 gimli
- 定位：纯 Rust DWARF 读写层（zero-copy、lazy），与 object 组合为事实标准。
- 可抄机制：1) `Reader` 抽象 + `EndianSlice`（零拷贝）/`EndianRcSlice` 可换后端；2) 按需解析（只解析被请求的单元/条目）——对应 adv-bin"分层解析"哲学；3) 同一库含 write 面（`write` feature，编译器生态在用）。
- 档位：吸收；integration：adv-bin。（与 R3-025 的登记互补：此处记其结构与版本面。）
- 第一步动作：封装 `DwarfIndex`：单元表 + 惰性 DIE 遍历 + 行表缓存（对 addr2line 之上只做编排）。
- 许可证/成熟度：MIT/Apache-2.0；维护中（检索口径 0.31.x、提交至 2025-06，最新发版待复核）。
- 链接：https://github.com/gimli-rs/gimli

### R4-012 addr2line
- 定位：地址→文件/行/内联帧，Rust 生态标准实现（backtrace 等下游依赖）。
- 可抄机制：1) **续延式 split DWARF**：`LookupResult::Load(SplitDwarfLoad{dwo_id, comp_dir, path})` + `LookupContinuation` 非阻塞补齐文件，适配任意 IO 模型；2) DWP 包与 Mach-O dSYM 内置 loader；3) `find_frames` 输出内联栈 + `preload_units` 预热优化。
- 档位：吸收；integration：adv-bin。
- 第一步动作：实现 debuglink→build-id→debuginfod 三层 `SplitDwarfLoad` 解析回调，接 ② 的查找链。
- 许可证/成熟度：MIT/Apache-2.0；维护中（检索口径 0.26.0；最新发版待复核）。
- 链接：https://github.com/gimli-rs/addr2line

### R4-013 pdb crate（willglynn 原版 → pdb2 fork）
- 定位：纯 Rust、无 DIA/Windows 依赖的 PDB 按需解析；Windows 侧符号化主选。
- 可抄机制：1) "disk 上尽可能久"的 lazy 设计（对齐 gimli 哲学）；2) 流模型：MSF 容器（块+流目录/MFT）→ PDB/TPI/DBI/IPI 流；类型记录 DAG（TPI TypeIndexBegin 典型 0x1000）；3) 直出能力：全局符号/帧表/内联站点/行程序（`pdb-addr2line` 组装为地址→位置）。
- 档位：吸收；integration：adv-bin。
- 第一步动作：用本机 MSVC 产物 + 其 PDB 做端到端符号化样例，校验与 dbghelp 结果一致（对照 oracle）。
- 许可证/成熟度：MIT/Apache-2.0（待复核）；维护中（pdb2 0.10.0，2025-09-22 发版）。
- 链接：https://github.com/camden-smallwood/pdb

### R4-014 symbolic（Sentry）
- 定位：跨格式符号化统一层（PDB/.NET portable PDB/DWARF/Breakpad），Sentinel 规模验证。
- 可抄机制：1) **SymCache**：把符号预加工成扁平二进制缓存（地址区间→文件/行），查询 O(log n) 且零解析——adv-bin 符号缓存直接抄此设计（不抄实现）；2) 多格式统一 facade（与 object 同思路，值得对照）；3) source bundle（源码上下文打包）是"符号化 = 行号 + 上下文"的完整定义。
- 档位：有界（抄 SymCache 设计；crate 可作跨格式测试 oracle）；integration：adv-bin。
- 第一步动作：定义 `SymCache` 文件布局（查表友好、可 mmap、版本字段）并写 round-trip 测试。
- 许可证/成熟度：MIT；维护中（docs 8.3.2、MSRV 1.85 口径）。
- 链接：https://github.com/getsentry/symbolic

### R4-015 microsoft-pdb（mspdb 源码）
- 定位：PDB 格式的"源头参考"（dbicommon.h 等内部头文件），LLVM PDB 文档大半由此反推。
- 可抄机制：作为格式交叉验证的最终参照（与 LLVM PDB 文档、pdb crate 三方对拍）。
- 档位：reference；integration：reference。
- 第一步动作：把 LLVM PDB 文档 + 本仓库差异点记录进测试用例注释（不确定字段的出处）。
- 许可证/成熟度：专有（公开参考，不可内联代码）；冻结参考（研究用）。
- 链接：https://github.com/microsoft/microsoft-pdb

### R4-016 dbghelp / DbgEng 符号 API + DIA SDK
- 定位：Windows 官方符号消费路径（SymInitialize/SymLoadModuleEx/SymFromAddr、DIA 类型查询）。
- 可抄机制：1) `SymSrv`/`SymFindFileInPath` 与符号服务器天然集成（协议消费的官方实现，可当"答案对照"）；2) DIA 的会话模型（数据源→会话→符号）是 COM 化的 PDB 读接口。
- 档位：reference（专有系统组件；仅对照/兜底）；integration：watch。与 R3-028（DbgEng）互补：此处只记"符号检索 API 面"。
- 第一步动作：document 对照表：dbghelp 查得的符号 vs pdb crate 查得（一致性 oracle）。
- 许可证/成熟度：专有（系统组件/SDK，分发受限）；维护中。
- 链接：https://learn.microsoft.com/en-us/windows/win32/debug/dbghelp-functions

### R4-017 Breakpad .sym
- 定位：跨平台文本符号格式（崩溃栈符号化的通用交换格式），dump_syms（含 PDB→.sym）与 minidump_stackwalk 是配套工具。
- 可抄机制：1) 文本格式：首行 `MODULE os arch id name`；`FILE/FUNC addr size param name/PUBLIC/STACK CFI/INFO` 指令族；2) **id 规则**：Windows = PDB GUID+Age；其他平台 = 二进制片段哈希——这是"从 PE 到符号文件"的又一条键；3) 目录布局 `{SYMBOLS_ROOT}/{module}/{id}/{module}.sym`（与符号服务器规则同构，可复用缓存层）。
- 档位：吸收（格式，作为第四类符号源；PDB→.sym 转换让 Linux 侧工具链可消费 Windows 符号）；integration：adv-bin。
- 第一步动作：实现 .sym 解析器（行式，攻击面小）+ 与结构化符号源统一进 `Symbolizer`。
- 许可证/成熟度：BSD-3-Clause；维护中（低频；Crashpad 为后继）。
- 链接：https://github.com/google/breakpad

### R4-018 Crashpad
- 定位：Breakpad 后继（进程外崩溃捕获 + minidump 写出，Chromium 栈）。
- 可抄机制：只有捕获侧是它的强项（handler 独立进程、异常端口）；对 ADV 价值主要在"minidump 生成器的行为定义"（消费侧归 R4-019）。
- 档位：watch（捕获不在 ADV 目标；需要时用系统/external handler）；integration：watch。
- 第一步动作：无（若用户提供 .dmp 则走 R4-019 消费）。
- 许可证/成熟度：Apache-2.0；维护中。
- 链接：https://github.com/google/crashpad

### R4-019 minidump + rust-minidump
- 定位：minidump（MDMP）格式与纯 Rust 解析/栈走查栈。
- 可抄机制：1) MDMP 容器 = 流目录（ThreadList/ModuleList〔含每模块 CodeView 记录：base/size/GUID/age/PDB 名——即"转储→符号"的键"〕/MemoryList/ExceptionStream）；2) rust-minidump 的 processor 用 .sym/CFI 或 frame pointer 做离线栈走查——"无 PDB 也能出调用栈"的降级路径。
- 档位：吸收；integration：adv-bin。
- 第一步动作：`adv-bin crash` 最小命令：读 .dmp → 模块清单（含 GUID/age 键）→ 提示可用符号源。
- 许可证/成熟度：MIT（rust-minidump）；维护中。
- 链接：https://github.com/rust-minidump/rust-minidump

### R4-020 dSYM 包
- 定位：macOS 调试信息载体（.dSYM bundle 内为 DWARF Mach-O），UUID 与主二进制配对。
- 可抄机制：1) `dsymutil` 把目标文件 DWARF 链接成 dSYM——"链接期收集"而非编译期直出，消费方按 UUID 匹配；2) addr2line 已内置 dSYM loader（复用即可，不自研）。
- 档位：有界（借 addr2line loader；仅 UUID 匹配与路径发现自研）；integration：adv-bin。
- 第一步动作：UUID 发现链（同目录 → Spotlight/`mdfind` 备选）接入符号查找回调。
- 许可证/成熟度：格式属 Apple（工具链 Apache-2.0 with LLVM exception）；维护中。
- 链接：https://llvm.org/docs/CommandGuide/dsymutil.html

### R4-021 rustc split-debuginfo（off / packed / unpacked）
- 定位：Rust 构建的调试信息分布策略——决定 adv-bin 在 Rust 产物上"能拿到什么"。
- 可抄机制（rustc 官方口径，检索于 2026-10）：1) 平台默认：**Linux/其他 Unix = off；Windows = packed（*.pdb）；macOS = packed（*.dSYM）**；2) `packed`：非 Windows/macOS 的 Unix 上等同 `-Zsplit-dwarf=single` → `*.dwp`；3) `unpacked`：调试信息散在目标目录（本地开发最快；任何平台都非默认；Windows 不支持）——注意与"构建 rustc 自身时 Apple 默认 unpacked"的差异（工具链自身口径，别混用）。
- 档位：吸收（策略清单，直接决定符号能力矩阵的输入）；integration：adv-bin。
- 第一步动作：在支持矩阵里显式列出"Rust 产物四档：PDB / DWARF 整合 / .dwp / 无"的判定与提示。
- 许可证/成熟度：MIT/Apache-2.0（rustc）；维护中。
- 链接：https://doc.rust-lang.org/rustc/codegen-options/index.html#split-debuginfo

### R4-022 .gnu_debuglink + build-id
- 定位：Linux 分离调试信息的两条经典定位链（文件级 + ID 级）。
- 可抄机制：1) `objcopy --only-keep-debug`（抽 .debug）→ `--strip-debug`（瘦身）→ `--add-gnu-debuglink`（嵌入文件名+**CRC32**）；GDB 在 `<dir>/.debug/` 与 `/usr/lib/debug/<path>` 查找并校验 CRC32；2) `.note.gnu.build-id` 作为全局唯一 ID：debuginfod 的键、崩溃归因、`/usr/lib/debug/.build-id/xx/yyyy.debug` 布局。
- 档位：吸收（adv-bin 符号查找链的第一/二层）；integration：adv-parse。
- 第一步动作：实现"debuglink→build-id→debuginfod"查找器 + CRC32 校验失败的可观测日志。
- 许可证/成熟度：格式公开（binutils 工具 GPL-3.0——只用其产物约定，不内联代码）；维护中。
- 链接：https://sourceware.org/gdb/onlinedocs/gdb/Separate-Debug-Files.html

### R4-023 压缩调试段（SHF_COMPRESSED / zlib / zstd）
- 定位：ELF 段级压缩（`objcopy --compress-debug-sections=zlib|zstd`；历史遗留 `.zdebug`）。
- 可抄机制：1) 段头声明算法（现代 SHF_COMPRESSED + ch_type；旧式 `.zdebug_*` 名称约定）——解析器先查压缩标志再解压，顺序错了必崩；2) 扫描器只解压自己需要的段（预算控制点：解压后大小上限）。
- 档位：有界（读侧支持；识别不到就显式降级）；integration：adv-parse。
- 第一步动作：语料里加 zlib/zstd 压缩段样本，验证 gimli 路径全通。
- 许可证/成熟度：格式（zlib/zstd 开源许可）；维护中。
- 链接：https://sourceware.org/binutils/docs/binutils/objcopy.html

### R4-024 GSYM
- 定位：LLVM 的"DWARF→扁平地址索引"格式（llvm-gsymutil 生成），lldb 消费。
- 可抄机制：把大型 DWARF 的查询前置为 mmap 友好索引（地址→函数/行），避免每次解析 DAG——与 symbolic SymCache 同族，二选一抄思想（ADV 倾向自研缓存格式，吸收其"段化布局+校验"要点）。
- 档位：watch（不引入 LLVM 工具链；生产链用自研 SymCache）；integration：watch。
- 第一步动作：记录其文件布局要点进 SymCache 设计笔记（字段级参考）。
- 许可证/成熟度：Apache-2.0 with LLVM exception；维护中。
- 链接：https://llvm.org/docs/CommandGuide/llvm-gsymutil.html

### R4-025 blazesym
- 定位：Rust 符号化库（ELF+DWARF、GSYM、kallsyms、进程映射组合），libbpf 生态（细节待复核）。
- 可抄机制：1) 符号源自动选择链（ELF→GSYM 等）与"按 PID/映射"的在线符号化 API 面；2) 与 ADV 场景差异：偏 Linux/perf 生态、无 PDB。
- 档位：watch（与 addr2line 栈重叠，不重复造；观察其 GSYM/内核符号处理）；integration：watch。
- 第一步动作：无（登记备查；若做 Linux 在线剖析再接）。
- 许可证/成熟度：BSD-2-Clause（待复核）；维护中。
- 链接：https://github.com/libbpf/blazesym

### R4-026 Rust v0 mangling + rustc-demangle
- 定位：Rust 符号命名/还原（demangle）——adv-bin 符号视图的必需品，且 2025–2026 正处切换期。
- 可抄机制：1) 时间线锚：**nightly-2025-11-21 起 v0 为默认**（Rust 官方博客 2025-11-20）；stable 默认生效版本检索到 1.97（2026-07-09，单源待复核）；旧 Itanium 兼容格式退场；2) `rustc-demangle` crate 程序化 demangle（Rust/C 两实现），`rustfilt` CLI；3) 迁移教训（Dioxus #5005 类案例）：依赖旧 mangling 字符串的热补丁/匹配逻辑必须改走 demangle 库。
- 档位：吸收；integration：adv-parse。
- 第一步动作：符号管线统一走 rustc-demangle（同时接受 legacy/v0 输入），并把"v0 优先"写进测试语料。
- 许可证/成熟度：MIT/Apache-2.0；维护中。
- 链接：https://github.com/rust-lang/rustc-demangle

### C. 供应链二进制分析

### R4-027 cargo-auditable
- 定位：Rust 官方生态的"二进制内嵌依赖清单"机制——adv-sca 二进制线的 L1 首选。
- 可抄机制：1) 三段链：依赖树→JSON（schema 化）→zlib→**`.dep-v0` 链接器节**（跨 ELF/PE/Mach-O，wasm 自 0.6.3）；2) 工程约束：有序无时间戳（可复现构建友好）、400+ 依赖 <4KB；3) 生态互操作已成事实标准（cargo-audit/trivy/grype/osv-scanner/syft 版本线见 ③）。
- 档位：吸收（adv-sca L1 读取 + ADV 自身构建启用）；integration：adv-sca。
- 第一步动作：实现 `.dep-v0` 读取器（JSON+zlib，约 50 行）+ OSV 对拍用例；同时给 ADV 发布物接入 cargo-auditable。
- 许可证/成熟度：MIT/Apache-2.0；维护中（nightly 原生 SBOM 前身 CARGO_BUILD_SBOM 在推进；RFC 2801）。
- 链接：https://github.com/rust-secure-code/cargo-auditable

### R4-028 Go buildinfo
- 定位：Go 产物内嵌模块清单（与 .dep-v0 并列的"语言级内嵌 SBOM"）。
- 可抄机制：1) `.go.buildinfo` 节跨 ELF/PE/Mach-O/XCOFF 可读（`debug/buildinfo`）；内容：模块 path/version/sum + build settings（`-ldflags`、`vcs.revision`、GOEXPERIMENT）+ Go 版本；2) 消费方工程细节：主模块 `(devel)` 时的三级回退（LDFlags 解析 → 内容扫描（默认关）→ VCS revision）（syft 口径）；3) syft 1.39.0 支持 UPX 先解压、LZMA 压缩 buildinfo（对抗混淆的现实工程点）。
- 档位：吸收；integration：adv-sca。
- 第一步动作：读取器 + `go version -m` 对拍（R4-007）。
- 许可证/成熟度：BSD-3-Clause（Go）；维护中。
- 链接：https://pkg.go.dev/runtime/debug#ReadBuildInfo

### R4-029 syft（binary cataloger 框架）
- 定位：二进制成分编目的工程基线（cataloger 架构 + 各生态实现）。
- 可抄机制：1) 分层：文件分类器 → 格式/生态 cataloger（Go/Rust `.dep-v0`/Java/.NET/Python…）→ 兜底 binary-classifier（pattern 匹配版本串，**误报高，仅兜底**）；2) 边界意识："no packages" ≠ 无依赖，只是"未识别到已知元数据"——adv-sca 输出必须同样诚实分级；3) 版本锚：消费 cargo-auditable 自 1.15.0、UPX/LZMA 处理自 1.39.0。
- 档位：吸收（框架思想 + 可作对照 oracle）；integration：adv-sca。
- 第一步动作：同一样本集跑 syft，与自研 L1 结果比对差异矩阵（覆盖率/误报）。
- 许可证/成熟度：Apache-2.0；维护中。
- 链接：https://github.com/anchore/syft

### R4-030 blint（OWASP dep-scan）
- 定位：LIEF 驱动的二进制 lint + SBOM 生成器（多生态）。
- 可抄机制：1) 能力标注（YAML 规则→函数/符号打点，如网络/文件/驱动操作）；2) SBOM（v2 起）：NuGet/PyPI/Cargo/Go/Maven/RubyGems/npm，`--deep` 激进采集；3) WASM 组件 SBOM（`--wasm-sbom`）与 `--suggest-fuzzable`（给 fuzz 选靶）。
- 档位：有界（参考实现/能力标注思路；直接依赖 apt 式生态重）；integration：adv-sca。
- 第一步动作：抽取其 YAML 能力规则格式做对照设计（adv-rules 后续可用）。
- 许可证/成熟度：Apache-2.0（v2 口径；早版本曾标 MIT，待复核）；维护中。
- 链接：https://github.com/owasp-dep-scan/blint

### R4-031 dnfile / dnfile-rs / System.Reflection.Metadata / Mono.Cecil（.NET 元数据）
- 定位：.NET 二进制（PE+CLR）成分分析的四件套（Python/Rust/C# 各一路）。
- 可抄机制：1) 元数据模型：CLR 头 → `#~`/`#-` 流 + `#Strings/#US/#GUID/#Blob` 堆 → ~50 张 ECMA-335 表；2) **依赖提取表**：`Assembly`/`AssemblyRef`/`ModuleRef`/`File`/`ExportedType`（+TypeRef/MemberRef 追外部类型使用）；3) coded index 解析（TypeDefOrRef 等）是正确读表的前提；capa 静态分析即用 dnfile（dnfile-rs 供 capa-rs）。
- 档位：吸收（adv-sca .NET 线）；integration：adv-sca。
- 第一步动作：选 dnfile-rs（Rust 一致性）实现 AssemblyRef 提取；配 MSVC 产 .NET 夹具。
- 许可证/成熟度：dnfile MIT（待复核）/dnfile-rs MIT/Apache-2.0（待复核）/SRM MIT/Cecil MIT；维护中。
- 链接：https://github.com/malwarefrank/dnfile

### R4-032 Java class/jar 解析
- 定位：JVM class 与 jar 的供应链元数据面。
- 可抄机制：1) classfile：CAFEBABE + 常量池（分 tag）+ 属性（SourceFile 等），版本号在头；2) **jar = zip**：`META-INF/MANIFEST.MF`（Implementation-Version 等）与 Maven `pom.properties`（groupId/artifactId/version 坐标）是 L1 元数据；3) 交叉点：jar 内 native（JNI .so/.dll）需要再走 ELF/PE 解析——"容器内再容器"的递归设计点。
- 档位：有界（读取器自研成本低，不必引重库）；integration：adv-sca。
- 第一步动作：最小 jar 读取器（zip 清单 + pom.properties）+ 内嵌 native 文件的递归解析钩子。
- 许可证/成熟度：格式规范开放（JVMS）；Rust 侧小库许可待验证。
- 链接：https://docs.oracle.com/javase/specs/jvms/se21/html/jvms-4.html

### R4-033 Python wheel / native 组件
- 定位：Python 分发的二进制面（wheel=zip；原生扩展是真正的二进制）。
- 可抄机制：1) wheel 规范（PEP 566 METADATA：Name/Version/Requires-Dist；RECORD 文件哈希；平台 tag manylinux/musllinux/macosx）；2) 原生 `.so/.pyd` 内嵌需转 ELF/PE 解析（与 R4-031 同构的递归点）；3) 冻结/打包产物（PyInstaller 类）需专门识别（元数据被藏）。
- 档位：有界；integration：adv-sca。
- 第一步动作：`.dist-info/METADATA` 读取 + native 组件递归钩子（复用 adv-parse）。
- 许可证/成熟度：规范（PSF 生态，开放）；维护中。
- 链接：https://packaging.python.org/en/latest/specifications/binary-distribution-format/

### R4-034 SPDX / CycloneDX（SBOM 载体）
- 定位：二进制 SBOM 的输出标准（adv-sca 产物格式）。
- 可抄机制：1) CycloneDX 的 components+evidence 模型（证据字段可指向二进制内的定位）；2) SPDX 3.0 的 profile 化模型（2024 起）；3) 与 attestation（Docker SBOM attestation + cargo-auditable）组合的链路——"构建期嵌入、分发期证明"。
- 档位：有界（输出格式对齐，核心自研）；integration：adv-sca。
- 第一步动作：确定主输出为 CycloneDX（JSON）+ 导出 SPDX 的映射表（字段级）。
- 许可证/成熟度：规范（SPDX：CC-BY-3.0；CycloneDX：Apache-2.0，待复核）；维护中。
- 链接：https://cyclonedx.org/

### R4-035 二进制→源码 SCA 研究（2025 群）
- 定位：无元数据/剥离符号场景的成分分析前沿（adv-sca L3 路线判据）。
- 可抄机制/数字锚：1) B2SCOM（NTU 学位论文 2025）：二进制组件→源码映射，报告 94.12% 精度 / 97.39% 召回（超基线；纯字符串/纯函数名法在剥离场景很差）；2) Lund 学位论文 2025：Ghidra **p-code**（跨架构）做函数相似性 + 传统/ML 算法评分；3) 跨语言 BM25 路线（IJIS 2025，DOI 10.1007/s10207-025-01148-3）；4) SBOMproof（arXiv 2025）提出"SBOM 混淆漏洞"（工具间不一致导致漏报）+ sbomvert 转换；5) UniBOM（IoT 2025）：binwalk+syft+CCScanner 三路融合。
- 档位：吸收（路线判据：L1 元数据 → L2 符号/字符串 → L3 函数语义；L3 登记为后续研究位）；integration：adv-sca。
- 第一步动作：L3 只写一页"接口预留"（函数指纹源=反汇编器；与 R1/R6 的接口对齐），不立项。
- 许可证/成熟度：论文/学位论文（引用为主）；前沿。
- 链接：https://doi.org/10.1007/s10207-025-01148-3

### D. 差分/补丁（格式角度）

### R4-036 BinDiff（google/bindiff）
- 定位：图基二进制差分的经典工具（补丁分析/符号移植）。
- 可抄机制：1) 相似度=调用图 + CFG（手册口径：只考虑基本块/边/助记符）→ 匹配 → 别名传播（函数名/注释移植）；2) 架构事实：输入不是 PE，而是 **BinExport**（需 IDA/Binary Ninja/Ghidra 前端）——"反汇编前端/差分后端解耦"的关键设计；3) PDB 非构建依赖、但符号显著提升匹配（与 ② 呼应：符号是差分的质量乘数）；4) 时间锚：copyright 2011–2026（仓库口径），BinExport 12 / protobuf 3.14 / SQLite3 构建依赖。
- 档位：有界（算法思想 + 输出格式；不直接内联 C++ 工具）；integration：adv-bin。
- 第一步动作：用两版同一 PE（自建夹具）跑 BinDiff 端到端，观察"无 PDB/有 PDB"匹配质量差（量化符号价值）。
- 许可证/成熟度：Apache-2.0；维护中。
- 链接：https://github.com/google/bindiff

### R4-037 BinExport
- 定位：反汇编器→差分工具的 protobuf 导出（函数/基本块/边/调用图）。
- 可抄机制：中立中间格式把"前端（反汇编器）×后端（差分/索引）"解耦——adv-bin 若做差分，应照此定义自己的中间层（与 adv-parse IR 复用）。
- 档位：有界（格式设计参考）；integration：adv-bin。
- 第一步动作：把 BinExport 的实体粒度（function/basic_block/edge/instruction + call graph）抄进 adv-bin 中间层草案。
- 许可证/成熟度：Apache-2.0；维护中。
- 链接：https://github.com/google/binexport

### R4-038 Diaphora
- 定位：IDA/Binary Ninja 插件式差分（SQLite 特征库 + 启发式匹配）。
- 可抄机制：伪代码 AST/常数/哈希多特征存库、穷举+启发式两阶段匹配——特征工程的丰富样本。
- 档位：reference（GPL 插件、绑定商业 GUI，只抄特征思想）；integration：watch。
- 第一步动作：无（特征清单摘录入设计笔记）。
- 许可证/成熟度：GPL-3.0（待复核）；维护中。
- 链接：https://github.com/joxeankoret/diaphora

### R4-039 QBinDiff
- 定位：把二进制差分建模为图对齐/信念传播的问题（Quarkslab，2024）。
- 可抄机制：网络对齐（network alignment）视角 + belief propagation 求解——与启发式图匹配互补的"可微/概率化"路线。
- 档位：reference（研究参考）；integration：watch。
- 第一步动作：无（在差分选型笔记中登记为备选算法族）。
- 许可证/成熟度：LGPL-3.0（待复核）；维护中。
- 链接：https://github.com/quarkslab/QBinDiff

### R4-040 DeepDiff（2025）
- 定位：2025 年新一代二进制差分研究（面向漏洞/补丁精确检测，arXiv 2509.23970）。
- 可抄机制：论文定位为 BinDiff 的下一代对照；关注其"补丁场景评测口径"（对 ADV 的 patch diffing 实验设计有参照价值）。
- 档位：watch（论文，待其代码/复现）；integration：watch。
- 第一步动作：纳入 P6 研究域清单（交叉引用）。
- 许可证/成熟度：论文（arXiv）；前沿。
- 链接：https://arxiv.org/abs/2509.23970

### R4-041 Patch diffing（技术）
- 定位：厂商补丁→修复点定位的通用工作流（防御性：自证修复、可达性判断）。
- 可抄机制：流程 = 前后版本取回 → 符号对齐（PDB/.sym 可用则优先）→ 差分（BinDiff 族）→ 变更函数聚类 → 结合公告/CVE 推断修复语义；约束：无符号时匹配质量显著下降（R4-036 量化实验直接服务此判据）。
- 档位：有界（实验 + 报告能力，不做自动化利用）；integration：adv-bin。
- 第一步动作：定义"补丁对"夹具规范（两版二进制 + 可选 PDB）并登记首个实验（如某开源项目相邻版本）。
- 许可证/成熟度：技术方法（工具各计）；—。
- 链接：https://github.com/google/bindiff

### E. 符号服务器与调试文件分发

### R4-042 Microsoft 符号服务器协议 + symsrv crate
- 定位：Windows 符号分发的公共基础设施及其可复用客户端。
- 可抄机制：1) 路径规则：`srv*[下游]*[上游]`；检索 `<file>/<GUID32+AgeHex>/<file>`（实例 `dcomp.pdb/648B8DD0780A4E22FA7FA89B84633C231/dcomp.pdb`）；2) 压缩件尾 `_`（compress.exe 产物）与 SymProxy（HTTP 不能直接当下游）的工程约束；3) Rust 客户端 `symsrv`（0.2/0.3）：解析 `_NT_SYMBOL_PATH` + async 下载缓存，`SymbolCache::get_file()` 直接可用。
- 档位：吸收（adv-bin 符号查找链的第三层；MVP 用 symsrv crate）；integration：adv-bin。
- 第一步动作：msdl 端到端拉一个自有二进制的 PDB（校验 GUID/Age 一致）后离线缓存复放。
- 许可证/成熟度：协议文档公开（MS Learn）；crate MIT/Apache-2.0（待复核，活跃度待复核）；维护中。
- 链接：https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/symbol-stores-and-symbol-servers

### R4-043 srcsrv（Source Server）
- 定位：PDB 内嵌的"源码回源指令"数据流（调试器按需取源文件）。
- 可抄机制：1) INI 式变量 + 每文件命令（如 `cmd` 展开为版本控制打印或 HTTP 下载）；`pdbstr` 写、`srctool` 读；2) **安全红线**：srcsrv 的 cmd 可执行任意命令——adv-bin 只做**只读列出**（源文件清单/URL 映射），绝不执行其中命令。
- 档位：有界（只读消费）；integration：adv-bin。
- 第一步动作：srcsrv 流解析器（只读）+ 单元测试含"恶意 cmd"样本（断言不执行）。
- 许可证/成熟度：格式（微软工具链）；Rust crate `srcsrv` 0.2.3，MIT（待验证），成熟度实验。
- 链接：https://crates.io/crates/srcsrv

### R4-044 debuginfod（elfutils）
- 定位：Linux 的"符号服务器"等价物（build-id 键 HTTP 分发调试文件/源码）。
- 可抄机制：1) API：`/buildid/<id>/debuginfo`、`/executable`、`/source/<path>`；客户端生态广（GDB/binutils/LLDB/perf/delve；WinDbg 经 `DebugInfoD*` 标记也可接）；2) 自托管一行：`debuginfod -F <dir>` / `-R <rpm>` / `-U <deb>`；联邦服务器（elfutils.org 聚合 fedora/arch/debian/opensuse 等，服务页 2025-01 更新口径）；3) 完整性：0.192+ 支持 IMA 每文件签名校验；4) 2025 前沿：split DWARF 的 `.dwo/.dwp` 服务化在讨论（build-id vs dwo_id 键冲突；提案 `/buildid/<id>/dwp`；LLDB 已有 DWP 测试路径）。
- 档位：吸收（Linux 侧符号查找链第三层；作为外部服务/客户端协议消费，不内联 GPL 代码）；integration：adv-bin。
- 第一步动作：`DEBUGINFOD_URLS` 客户端最小实现（fetch debuginfo→gimli）并接入 ② 查找链。
- 许可证/成熟度：服务端 GPL-3.0（协议公开可自实现；以进程/HTTP 边界使用不触发传染）；维护中。
- 链接：https://sourceware.org/elfutils/Debuginfod.html

---

## 3. 2025–2026 前沿信号（时间锚）

1. **Rust v0 mangling 默认切换**：nightly-2025-11-21 起（官方博客 2025-11-20）；stable 默认检索到 1.97（2026-07-09，单源待复核）⇒ 符号工具两年内必须完成"双格式 demangle"迁移。
2. **PDB 纯 Rust 栈持续活跃**：pdb2 0.10.0（2025-09-22）；symbolic MSRV 1.85（8.3.x 线）。
3. **二进制 SBOM 的"元数据 → 语义"迁移**：2025 多篇学位论文/论文（B2SCOM 94.12%P/97.39%R；Ghidra p-code 跨架构；SBOMproof 的 SBOM 混淆问题）——adv-sca 路线分层的学术依据。
4. **syft 1.39.0**：UPX 解压 + LZMA buildinfo（对抗打包/压缩的现实工程）；cargo-auditable 消费线成型（cargo-audit 0.17.3+/trivy 0.31.0+/grype 0.83.0+/osv-scanner 2.0.1+）。
5. **cargo-auditable 进入发行版与厂商**：Microsoft 内部、多家发行版默认化；Ubuntu 26.04 定向 opt-in（待复核）；nightly `CARGO_BUILD_SBOM=true`（Cargo 原生 SBOM 前身、RFC 2801）。
6. **debuginfod split DWARF 服务化讨论**（2025 LLVM Discourse）：`/buildid/<id>/dwp` 提案 + LLDB DWP 支持——分离调试信息的"按需分发"继续深化；IMA 签名（0.192+）补上信任面。
7. **LIEF 0.17.x 稳定迭代**（PE delayed imports/RichHeader、Mach-O LC_FILESET_ENTRY、ELF 对齐修复）⇒ Rust 侧深挖/重建能力继续增强，但仍是"重依赖"定位。
8. **差分研究活跃**：QBinDiff（2024）与 DeepDiff（arXiv 2509.23970, 2025）在 BinDiff 之外探索对齐/精确检测路线。

---

## 4. Top-3（与第一步动作）

1. **解析层三件套：object（主）+ pelite（PE 细节）+ gimli/addr2line/pdb（符号）**——全部 MIT/Apache 系、纯 Rust、零 FFI；直接落成 adv-bin `ImageFacts` + `Symbolizer` 两个核心结构（见 ⑤ 设计）。第一步：以 object 搭 `probe→parse` 原型并用 10 样本做字段回归。
2. **cargo-auditable + Go buildinfo 双读取器（adv-sca L1）**——官方内嵌数据、零猜测、两天可交付；同时给 ADV 自身产物接 cargo-auditable（吃狗粮 + 下游可审计）。第一步：`.dep-v0` 读取器 + OSV 对拍。
3. **符号服务器消费层：msdl(symsrv crate) + debuginfod 客户端**——Windows/Linux 两侧"自动取符号"的能力闭环，复用 ② 的查找链设计。第一步：msdl 拉一个自有 PDB 并离线缓存复放（端到端验证 GUID/Age 键规则）。

## 5. 交叉引用与不吸收清单

- **不吸收（红线）**：srcsrv 命令执行（只读列出）；TTD .run/.idx 解析（R3 已定）；专有 DIA/DbgEng 深度绑定（仅对照 oracle）；LIEF 原地写回（只读深挖 + 产新文件）；微软符号服务器的批量镜像（按需取用）。
- **交叉**：R1（反汇编前端 → BinExport 类中间层）、R2（符号执行 → 解析层喂输入）、R3（调试器/ETW 消费 PDB 的运行时路径）、R5（YARA/capa 用 dnfile/PE 解析成果）、R6（缓解位读取即 `ImageFacts.security`）、X3（facts.bin 序列化格式）、X14（wasm 与组件模型）。
- **缺口登记**：PE `.pdata` 异常表解析在主流 Rust 库中非默认能力（object 无专门 API，待复核/需自研或 LIEF）；Symsrv 压缩格式（compress.exe 的准确变体）无正式规范（按 MSZIP 实测为准，待复核）；Rust stable 1.97 v0 默认（单源待复核）。
