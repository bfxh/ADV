# X10 域【密钥与内存卫生】调研（2026-10-04）

范围：ADV 自身持凭据（自托管 CI token、本机凭据存储）+ 扫描器接触 secret 值（adv-secrets）后的内存/日志/转储/报告卫生。本域不研究"如何检测 secret"（02 域已覆盖），只研究"检测到之后与自身凭据"的处置。调研方式：WebSearch（6 检索，3 条首轮并发失败后重试命中），日期 2026-10-04。

---

## 一、五个重点回答（结论先行）

### ① ADV 本机凭据存储定案：Windows Credential Manager（DPAPI 底座）为主，三级降级

- **定案**：ADV 自身凭据（CI runner token、API key）默认存 **Windows 凭据管理器**，经 `CredWrite/CredRead`（advapi32）写入；静态安全由 **DPAPI** 兜底——凭据管理器底层即 DPAPI，用当前 Windows 用户登录凭据派生包裹密钥（机制链见条目 X10-006，锚 syfuhs.net 对 DPAPI 密钥层级的拆解）。ADV 自己**不写加密代码**，只调 API。
- **落地封装**：首选 `keyring` v3（其 `windows-native-keyring-store` 1.1.0 封装了原生凭据管理器，要求 Windows Vista+，多 service 用命名格式模拟，锚 crates.io/docs.rs）；macOS/Linux 换 keyring 其余 backend，代码不动（条目 X10-010/012）。
- **降级路径**：
  - L1（默认，交互式本机会话）：Credential Manager。
  - L2（服务会话/凭据管理器被组策略停用）：`CryptProtectData`（带 `CRYPTPROTECT_UI_FORBIDDEN`）加密 blob 存 ADV 配置文件——同为 DPAPI，只是存储位置从系统库换成自己的文件；机器级范围需要额外熵（additional entropy），注意机器级 DPAPI 等于"同机任意用户可解"，默认用用户级。
  - L3（CI/无 DPAPI 可用）：环境变量注入，仅进程内存在、禁止落盘，且必须配合 ③ 的 env 白名单（否则等于泄漏给所有子进程）。
  - **明文配置文件：不采纳**为任何默认档；仅当用户显式 opt-in 时允许并打运行警告。
- 判据（何时触发降级）：keyring 条目创建/读取返回平台错误（如凭据库锁定）→ 自动降 L2 并记录一次告警日志（不重试风暴）。

### ② zeroize 适用判据：按"数据生命周期 × 可达性"二维度收，不做全量 zeroize

- **真需要**（两类）：
  1. ADV **自身长期驻留**的凭据：CI token、DPAPI 解密后的明文密钥——生命周期横跨整个进程，Drop 一定发生，收益明确。
  2. **扫描器读出的 secret 原文**（adv-secrets 命中值）：从匹配到写进红显报告之间存在于内存，用 `SecretBox<str>` 包住，出报告即析构清零。
- **不需要/过度**：一次性短命缓冲（解析中间产物，转瞬即逝且难以全路径追踪副本）；已写入管道/子进程的数据（离开进程内存，zeroize 无对象）；不含 secret 的 IR/索引数据。
- **成本口径**：zeroize 的 `Zeroize` 实现是逐字节 `write_volatile` + `compiler_fence(SeqCst)`（条目 X10-001），volatile 语义禁用 SIMD 向量化与合并，比 `memset` 慢一个常数倍——对 KB 级 secret 值不可测，对 MB 级缓冲才成问题；所以判据按"值小但敏感"划，不按"扫过所有缓冲"划。
- **两个必须写进代码规范的坑**（条目 X10-002）：
  - **Vec realloc**：`Zeroizing<Vec<u8>>` 扩容时旧缓冲的副本不被清零（docs.rs zeroize 有 "About buffer reallocations" 明文告诫）——secret 缓冲一律 `Vec::with_capacity` 预分配，或用定长 `SecretBox`。
  - **panic=abort**：abort 路径不走 unwind，Drop 不执行，zeroize-on-drop 整体失效——release profile 若选 abort（体积/速度动机），内存卫生对崩溃场景就没有承诺，防线移交给 ⑤ 的转储配置。这两条合起来：**zeroize 是"正常退出路径"的卫生，不是"异常路径"的安全边界**。

### ③ 子进程环境白名单：从 GIT_* 教训泛化为"默认空环境 + 显式注入"

- 机制面（条目 X10-009）：`CreateProcess` 默认子进程继承父进程**整个**环境块；Git 生态的教训（GIT_ASKPASS/GIT_CONFIG 等 `GIT_*` 可被上游环境注入进子进程行为）泛化后规则是——** ADV 调任何外部工具（git、gitleaks 式引擎、CI runner）不继承父环境，从零构造**：`std::process::Command` 起步即 `env_clear()`，再 `envs(whitelist)` 白名单注入。
- ADV 基线白名单（按工具需求裁剪，非固定全集）：`PATH`、`HOME`/`USERPROFILE`、`TEMP`/`TMP`、`SYSTEMROOT`、`SYSTEMDRIVE`、`COMSPEC`、`APPDATA`/`LOCALAPPDATA`、`PROGRAMFILES`、`LANG`/`LC_ALL` + 工具特需项显式列举（git 类加 `GIT_TERMINAL_PROMPT=0`、`GIT_CONFIG_NOSYSTEM=1`，既防交互挂起又防系统级配置劫持）。
- **secret 一律不进环境变量**：走 stdin 或带 ACL 的临时文件句柄。补一句 Windows 侧现实（判据而非绝对化）：默认 ACL 下，同用户的其他进程可打开本进程内存读取其环境块（Process Explorer/Process Hacker 即此路径），所以"用 env 传 secret 只在本机同用户内也可读"要作为设计前提。

### ④ 报告 redaction 标准语义：GitHub 拆解 + ADV 掩码规范

- **GitHub Actions 日志掩码（条目 X10-012）拆解**：注册过的 secret（repo/env/org secrets + `::add-mask::` 运行时注册）在日志中出现即整体替换为固定 `***`。三个语义要点：**(a) 定长占位符——不泄漏长度**；**(b) 只匹配原值形态**——base64/变换后、逐字符打印、跨行拆分都绕过（官方语义即"防意外打印"而非防对抗）；**(c) 短值/常见词不可靠掩码**。secret scanning（push protection）是另一条线（提交时拦截 + alert），不等于日志掩码（条目 X10-014）。
- **ADV 掩码规范（提案，判据可调）**：
  - 默认：整体 `[REDACTED:<类型>]`，不带长度、不带部分字符——与 GitHub 语义对齐。
  - 部分掩码（仅当报告读者需要核对自己看到的值）：值长 ≥16 字符时显示前 4 字符；按字符集熵估算，若隐藏部分剩余熵 < 40 bits 则退回整体掩码（熵保底即"部分显示不得把可暴力空间压到可枚举"，40 bits 为提案档，需在实现时按字母表复算）。
  - 掩码同时作用于原值与其常见编码形态（hex/base64），吸收 GitHub 的"变换绕过"教训。
  - adv-secrets 报告中 secret 值字段类型即 ② 的 `SecretBox`，从类型系统上使"忘掉掩码"变成编译错误级别的不便。

### ⑤ 崩溃转储的密钥风险：WER LocalDumps 按档位收口

- 机制（条目 X10-011）：`HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps`（支持按应用子键）下 `DumpType=1` 是小转储、`DumpType=2` 是全内存转储（含堆——含 secret 的堆在内，锚 Telerik/CodeMachine 的值语义与 LLVM 源码中 DumpType→`MINIDUMP_TYPE` 标志映射）；默认落 `%LOCALAPPDATA%\CrashDumps`。
- ADV 判据：ADV 若写 LocalDumps 键，**默认 `DumpType=1`** 并设 `DumpCount` 上限；文档明确告知用户：`procdump -ma` 等全量抓取工具会连堆一起抓，等于绕过 ②——因为 panic=abort 下 zeroize-on-drop 不执行，**转储选项是独立防线，不依赖 zeroize**。进阶（不吸收，留 watch）：`MiniDumpWriteDump` 的 `MiniDumpCallback` 可在生成时逐区过滤内存，工程成本高。

---

## 二、条目（机制级 9 条 + 扫视 6 条 = 15 条）

### 机制级

**1. zeroize（零化原语）**
- 定位：Rust 侧"确保内存真被清零"的事实标准原语，RustCrypto/iqlusion 系。
- 可抄机制：① 逐字节 `core::ptr::write_volatile(0)`——volatile 存储不可被死存储消除（DSE）优化掉，即使内存之后再不被读；② 写后 `compiler_fence(SeqCst)` 阻止编译器重排/下沉清零写入；③ 纯 Rust 无 FFI/asm，stable 可用（锚 docs.rs/zeroize 与 CipherStash 反汇编验证文）。
- 档位：【吸收】
- 第一步动作：adv-secrets 与凭据加载路径引入 `zeroize` + `Zeroizing`/`SecretBox`，先覆盖 ② 中两类"真需要"数据。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（RustCrypto 生态高使用率）。
- 链接：https://docs.rs/zeroize/latest/zeroize/ · https://ddanilov.me/zeroize

**2. zeroize 误用模式（Vec realloc 残留 / panic=abort / 栈副本）**
- 定位：用了 zeroize ≠ 清了所有副本；这是本域最容易被"绿灯错觉"坑的点。
- 可抄机制：① Vec 扩容 realloc 时旧缓冲不被清零（docs.rs 明文告诫）→ `with_capacity` 预分配或定长缓冲；② `panic=abort` 不走 unwind → Drop/zeroize-on-drop 不执行；③ 编译器可能在寄存器/栈上留未追踪副本（volatile 写只管你指的那块）——判据是"zeroize 管正常路径的已知缓冲，不管副本与异常路径"。
- 档位：【吸收】（作为代码规范 + review 检查项）
- 第一步动作：把两条坑写成 adv-secrets 的 review 清单条目；release profile 决策（abort vs unwind）与 ⑤ 联动。
- 许可证/成熟度：同上（机制来自同仓库文档与社区分析，非独立项目）。
- 链接：https://docs.rs/zeroize/latest/zeroize/ · https://ddanilov.me/zeroize

**3. secrecy（类型系统防泄漏）**
- 定位：把"这值不能被顺手打印"编码进类型，是日志卫生的前置防线。
- 可抄机制：① `SecretBox<T>`（`SecretString = SecretBox<str>`）`Debug` 输出固定 `SecretBox([REDACTED ...])`；② **故意不实现 `Display`**——`{}` 打印直接编译错，逼出 `expose_secret()` 显式暴露点（"feature, not a bug"，锚 Zero to Production）；③ Drop 时经 zeroize 清零，Serialize 为 feature-gated。
- 档位：【吸收】
- 第一步动作：adv-secrets 的命中值类型与 ADV 自身凭据类型统一换 `SecretString`；grep 代码库 `expose_secret` 调用点作为暴露面审计清单。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（iqlusioninc/crates，0.10 系列活跃）。
- 链接：https://docs.rs/secrecy · https://github.com/iqlusioninc/crates

**4. Windows 凭据管理器 + DPAPI（本机凭据存储底座）**
- 定位：① 定案的 L1/L2 底座；Windows 上"不自己管密钥材料"的最小方案。
- 可抄机制：① `CredWrite/CredRead/CredDelete`（advapi32）读写按 (service, user) 索引的凭据条目；② 底层 DPAPI 用当前用户登录凭据派生密钥链（用户口令解锁包裹密钥，包裹密钥再包内容密钥——锚 syfuhs.net 拆解），同机其他用户/他机不可解（用户级范围下）；③ `CryptProtectData` 可独立用于 L2 文件 blob，`CRYPTPROTECT_UI_FORBIDDEN` 适配服务会话。
- 档位：【吸收】
- 第一步动作：adv-secrets 凭据模块先接 keyring（条目 10），keyring 不达需求处（如需要 CRED_PERSIST 粒度控制）再以 windows-rs 直调（条目 11）。
- 许可证/成熟度：Windows 系统组件（无独立 license）；Microsoft 官方 API，长期稳定。
- 链接：https://syfuhs.net/adventures-in-credential-manager-trivia

**5. VirtualLock 与工作集限制（锁页的现实约束）**
- 定位：评估"锁内存防换页"是否值得做——结论倾向不做，先知其边界。
- 可抄机制：① POSIX `mlock` / Windows `VirtualLock` 把页钉在物理内存，阻止换页落 swap/hiberfil；② Windows 上锁定量受**进程工作集**上下限约束（`Get/SetProcessWorkingSetSize`），超限锁定失败——所以"锁了大缓冲"在 Windows 不是设个标志的事；③ 锁页防的是"落盘换页"，防不了 ②③ 的副本与转储，收益面窄。
- 档位：【有界】（ADV 凭据值均 KB 级，换页落盘概率低；不默认上锁页）
- 第一步动作：不动；在威胁模型文档记录"若未来需缓存 MB 级敏感物再评估"。
- 许可证/成熟度：系统 API（无 license）；Windows Vista+ 行为稳定。
- 链接：https://crates.io/crates/shrouded （其对 mlock/VirtualLock 语义的汇总）

**6. memsec（页面对齐分配 + mlock/mprotect 封装）**
- 定位：libsodium 风格的安全内存原语集；是"真要做锁页/保护页"时的现成件。
- 可抄机制：① 页面对齐分配（Unix memalign 系 / Windows `VirtualAlloc` 本身页粒度）作为 mprotect/保护页的前提；② `mlock/munlock`、`mprotect/mprotect_unlock` 封装（Windows 走 VirtualLock / VirtualProtect）；③ guard page 模式（越界读写即崩，可作 secret 缓冲的越界哨兵）。
- 档位：【有界】（机制好，但 ADV 当前 secret 体量用不上；且其维护活跃度低于 zeroize 系）
- 第一步动作：watch 其 API 演进；若 ⑤ 的 MiniDumpCallback 过滤方案立项，页面粒度分配是前置件。
- 许可证/成熟度：MIT；quininer/memsec，低活动（多年稳定但更新少——按 2026-10 观察，非定论）。
- 链接：https://github.com/quininer/memsec · 对照 https://crates.io/crates/redoubt-buffer（页对齐+mlock+mprotect 缓冲）

**7. 子进程环境泄漏面与 env 白名单（GIT_* 教训泛化）**
- 定位：③ 的机制底座；ADV 调外部工具/CI runner 的进程边界卫生，归 adv-sandbox 联动。
- 可抄机制：① 子进程默认继承父**全量**环境块（CreateProcess/Rust Command 默认行为）；② 泄漏面三处：同用户进程可读环境块（默认 ACL 下）、Linux `/proc/<pid>/environ` 同用户可读、env 被"顺手"传进更深子进程（GIT_* 注入类教训）；③ 防法：`env_clear()` 起步 + 显式白名单 `envs()`，工具特需项（`GIT_TERMINAL_PROMPT=0` 等）逐项列名。
- 档位：【吸收】
- 第一步动作：在 ADV 的进程启动封装（唯一 spawn 出口）实现"默认空环境"，白名单作为参数由调用方显式给；CI/测试断言 secret 值不出现在子进程环境中。
- 许可证/成熟度：机制来自系统文档与 Git 生态既有实践（无单一 license 主体）。
- 链接：https://doc.rust-lang.org/std/process/struct.Command.html

**8. WER LocalDumps / MiniDumpWriteDump（崩溃转储密钥风险）**
- 定位：⑤ 的配置抓手；"转储含堆 = 全部 zeroize 白做"的边界。
- 可抄机制：① `HKLM\...\Windows Error Reporting\LocalDumps` 注册表（可按 exe 子键），`DumpType` 1=小转储 / 2=全内存含堆（值语义锚 Telerik/CodeMachine；LLVM 源码可见 DumpType→`MINIDUMP_TYPE` 标志映射）；② 默认目录 `%LOCALAPPDATA%\CrashDumps`；③ `MiniDumpWriteDump` 的 `MiniDumpCallback` 支持生成时逐区内存过滤（进阶，成本高）。
- 档位：【吸收】（配置判据 + 文档告知）
- 第一步动作：ADV 安装/文档中固定"若启用 LocalDumps 则 DumpType=1 + DumpCount 上限"；威胁模型中把"用户自开全量转储"列为接受的残余风险。
- 许可证/成熟度：Windows 系统组件；长期稳定。
- 链接：https://llvm.org/docs/ （Signals 实现）· Telerik/CodeMachine 对 DumpType 值语义的佐证

**9. GitHub Actions 日志掩码语义（redaction 标准语义）**
- 定位：④ 的对照标准；业界对"报告里 secret 怎么显"的主流语义。
- 可抄机制：① 注册 secret（repo/env/org + `::add-mask::` 运行时注册）在日志命中即整体替换固定 `***`——定长占位符不泄漏长度；② 已知局限作为 ADV 边界输入：base64/变换形态不匹配、逐字符/跨行打印绕过、短值与常见词不可靠——语义定位是"防意外打印"，非防对抗；③ secret scanning（push protection 拦提交、alert 不回显完整值）与日志掩码是两条独立线，别混同。
- 档位：【吸收】（语义抄，实现自研）
- 第一步动作：按 ④ 的 ADV 掩码规范实现 adv-secrets 报告渲染层，测试用例覆盖"base64 形态也掩"。
- 许可证/成熟度：GitHub 平台行为（无 license）；语义多年稳定，局限清单为 2025-2026 社区文章汇总口径。
- 链接：https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions

### 扫视级

**10. keyring v3（跨平台凭据封装）**——v3 重构为平台 store crate：Windows 走 `windows-native-keyring-store`（1.1.0，封装 CredWrite/CredRead，Vista+，多 service 以命名格式模拟），另有 apple-native 与 secret-service 系 backend。机制：`Entry::new(service,user)`→set/get/delete。【吸收】（① 定案的落地件）· MIT OR Apache-2.0 · 维护中 · https://docs.rs/keyring

**11. windows-rs 直调 CredWriteW/CredReadW**——keyring 的降级实现路径：`windows` crate（Microsoft 官方绑定）直调 Win32 凭据 API，换取 CRED_PERSIST 档位与标志位细粒度控制。机制：`Windows.Win32.Security.Credentials` 模块。【有界】（仅 keyring 不够用时）· MIT OR Apache-2.0 · 维护中（Microsoft）· https://microsoft.github.io/windows-docs-rs/

**12. macOS Keychain / Linux secret-service（对照）**——keyring 对应 backend：Keychain（Security.framework，用户登录解锁）与 secret-service（DBus `org.freedesktop.secrets`，需 DBus 会话——headless/CI 常缺失，触发 ① 降级路径 L2/L3）。【reference】· 标准/各平台系统组件 · 稳定 · https://docs.rs/keyring

**13. 日志脱敏钩子（tracing 层）**——结构化日志防 secret 落盘：字段值类型用 secrecy 封装（Debug 即 `[REDACTED]`）+ tracing-subscriber 格式化层按字段名（token/password/key/secret 族）二次兜底替换。机制：自定义 `Visit` 实现。【吸收】· MIT · 维护中（tokio）· https://docs.rs/tracing-subscriber

**14. GitHub secret scanning（push protection / alert）**——与日志掩码并行的另一线：提交前拦截已匹配 secret 模式、alert 不回显完整值、`secret_scanning.yml` 配置旁路。机制：模式匹配 + 提交时门禁。【reference】（02 域检测侧的对端）· GitHub 平台行为 · 稳定 · https://docs.github.com

**15. shrouded（mlock+guard page+zeroize 一体化密钥容器）**——2026-03 出现的新 crate：RAM 锁定（mlock/VirtualLock）+ guard page + mprotect 再锁 + drop 清零的一体化 secret 类型，文档自带与 memsec/secstr 的对照表。【watch】（太新，成熟度未验证）· license 以 crates.io 页为准 · 实验 · https://crates.io/crates/shrouded

---

## 三、Top-3

1. **凭据存储定案（①+条目 4/10）**：keyring v3 → Windows 凭据管理器（DPAPI 底座），降级 L2=DPAPI 文件 blob、L3=env（仅 CI）；明文配置文件不采纳。第一步：adv-secrets 凭据模块接 keyring，错误自动降级 + 单次告警。
2. **zeroize 适用判据（②+条目 1/2/3）**：只 zeroize"长期驻留凭据 + 扫描命中 secret 原文"两类，用 `SecretBox` 收类型；写死两条规范——secret 缓冲预分配防 realloc 残留、panic=abort 下防线移交转储配置。
3. **外部输出的两道闸（③+⑤+④，条目 7/8/9/13）**：spawn 出口"默认空环境+白名单"、LocalDumps 默认 DumpType=1、报告掩码抄 GitHub 语义（定长 `***` + base64 形态同掩 + 部分掩码熵保底 40 bits 提案档）。

## 附：口径与未竟

- 数字锚：DumpType 1/2 语义（Telerik 论坛 2025-05、CodeMachine、LLVM Signals 源码）；keyring `windows-native-keyring-store` 1.1.0（2026，crates.io）；shrouded 2026-03；`***` 定长占位符与变换绕过局限（GitHub 官方语义 + 2025-2026 社区文章口径）。
- 40 bits 熵保底、env 白名单具体清单均为**提案档**，实现时需按实际字母表/工具需求复算，不是引用值。
- 未竟：memsec 的 Windows 工作集超限具体失败行为未做实验验证（机制描述以 VirtualLock 文档口径为准）；shrouded 未深查源码。
