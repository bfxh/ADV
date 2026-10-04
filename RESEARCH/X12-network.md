# X12 Rust 网络栈调研（ADV）

- 检索日期锚：2026-10-04（WebSearch 当日结果）；hyper/quinn 两条因搜索并发受限，标注「训练知识·未二次核验」。
- 关联结论：X6 裁决 = tokio 不进默认档 → 本域裁决（②号问题）以此为约束。
- 范围：ADV 的出网面只有三处——威胁数据快照更新、（未来）URL 校验类工具、（可选）自托管 CI 对外通信。本地分析主路径零网络依赖。

---

## Top-3（裁决）

**① ADV 网络栈定档：ureq 3.x（同步+小依赖）进默认档，reqwest 进门控档。**
判据：X6 已定 tokio 不进默认档，而 reqwest 对 tokio 是硬依赖（两端点名：reqwest 的异步传输层构建在 tokio 之上）→ 默认档带 reqwest 即变相推翻 X6。ureq 3.x 是无 runtime 依赖的阻塞客户端，TLS/HTTP2/代理全走 feature 门控，契合"本地优先零重依赖"。触发条件（何时开门）：需要 async 生态中间件（如并发批量抓取、http-cache-reqwest 类中间件）或 tonic（对 tokio+hyper 有依赖）时，才以独立 feature（如 `net-async`）引入 reqwest——依赖结构上让"联网"显式化（⑤号问题见 §A6/§A7）。

**② rustls 后端选型：默认档 ring，FIPS 需求出现才切 aws-lc-rs。**
判据（三轴）：a) Windows 构建摩擦——aws-lc-rs 需要 CMake+NASM（FIPS 静态构建还要 Go），ring 无此依赖；Fedora 曾因 aws-lc 依赖无法打包而把 rustls 默认改回 ring（2024， Fedora 打包记录）——若 ADV 目标是用户本机可编译，此轴决定性。b) FIPS——AWS-LC 已获 FIPS 140-3 验证且含 ML-KEM（截至 2024-12 唯一，postquantumfield 2026-08 引述）；但 aws-lc-fips-sys 3.x 部分版本"已提交未获证"（GitLab 讨论 2026-03）→ 用前必须核对所用版本证书状态。c) 许可证——ring 是 ISC 风格 + OpenSSL 衍生广告条款（cargo-deny 需配例外），aws-lc-rs Apache-2.0/ISC。注意：rustls 0.23（2024-02 起）默认 provider 已是 aws-lc-rs，选 ring 是显式 opt-in（`-F ring --no-default-features` 方向）。

**③ 威胁数据快照更新的 HTTP 契约：条件 GET + Range 续传 + 验签后原子落地。**
顺序：① 首次 GET 存 ETag + Last-Modified → ② 下次带 If-None-Match（304 则复用本地快照，RFC 9110 §13.1.2）→ ③ 大文件用 `Range` + `If-Range`（If-Range 只接受强 ETag，弱 ETag/日期在并发修改窗口有错配风险，RFC 9110 §13.1.5）→ ④ 下载写临时文件 → 验签 → 原子 rename；验签失败不落地。全程包在"联网功能"开关内，离线时该路径不编译（feature 门控）不执行。

---

## A. 机制级条目

格式：定位 → 可抄机制 → 档位 → 第一步动作 → 许可证/成熟度 → 链接

### A1. ureq 3.x — 默认档 HTTP 客户端
- **定位**：同步阻塞 HTTP 客户端，卖点是最小依赖树；3.x 是重写，模块化拆分（ureq-proto 等），无 async runtime 依赖。
- **可抄机制**：① 阻塞 API + Agent 连接复用，无 runtime——正是"X6 tokio 不进默认档"约束下的解。② TLS/HTTP2/socks-proxy 全 feature 门控（rustls/native-tls/platform-verifier 可选，3.x 的 `src/tls/mod.rs` 特性系统）。③ 超时/重定向策略在 Agent 层集中配置。
- **档位**：默认档。
- **第一步动作**：`cargo add ureq --no-default-features -F rustls` 跑通快照下载 + 过 cargo-deny；同时核对 3.x 当前默认 TLS 组合（3.x 线演进快，README 为准）。
- **许可证/成熟度**：MIT/Apache-2.0。3.x 线活跃：生态引用见 3.1.0→3.1.1（2026-03 依赖升级记录）、Debian sid rust-ureq 3.4.2；3.x 属重写后的新线，API 稳定性需锁定版本观察。
- **链接**：https://github.com/algesten/ureq

### A2. rustls — TLS 层（含后端裁决落地）
- **定位**：Rust 主力 TLS（rustls 0.23+，2024-02 起默认 provider 为 aws-lc-rs，ring 为 opt-in feature）。
- **可抄机制**：① CryptoProvider 插件化——ring 与 aws-lc-rs 可并存，运行时/编译期二选一，ADV 把选择收敛为一个 feature。② 自定义 CertificateVerifier——内嵌企业 CA/自签校验的挂载点（自托管 CI 场景）。③ 与 rustls-platform-verifier 组合用系统验证器（见 A8）。
- **档位**：默认档 = rustls + ring（理由见 Top-3②）；FIPS 档 = aws-lc-rs `-F fips`，仅在合规需求触发。
- **第一步动作**：adv-server 内建 `tls` 模块抽象 `connect(host)`，provider 选择只暴露一个 Cargo feature，锁死其余组合。
- **许可证/成熟度**：rustls Apache-2.0/ISC/MIT 三选一；ring ISC 风格+OpenSSL 广告条款（cargo-deny 例外清单要登记）；aws-lc-rs Apache-2.0/ISC。rustls 是事实标准级成熟；ring 发布节奏慢（0.17 线多年，社区共识）。
- **链接**：https://github.com/rustls/rustls

### A3. reqwest — 门控档异步客户端（含超时矩阵）
- **定位**：高级 HTTP 客户端，生态最大；0.12 线持续维护（2025 内见 0.12.28，crates.io），0.13 已出现（renovate 升级记录），0.13 起 rustls-platform-verifier 成为默认验证路径（Ian Wagner 2025 述评）。
- **可抄机制**：① **超时矩阵三层分开**：`connect_timeout`（建连）/ `timeout`（整个请求）/ `read_timeout`（读间隔）——快照下载用"短 connect + 长 total + 中 read"组合。② **redirect policy 可编程**：`Policy::custom` 闭包逐跳检查——SSRF 防护挂载点（见 A6）。③ feature 门控体积：`default-tls`/`rustls-tls`/`http2`/`socks`/`system-proxy` 按需裁剪。
- **档位**：门控档（`net-async` feature 或独立子 crate），不进默认档。
- **第一步动作**：暂不引入；写一行 ADR 触发判据（"需要 async 中间件生态或 >N 路并发抓取时启用"，N 先不定死，实测定）。
- **许可证/成熟度**：MIT/Apache-2.0；Rust HTTP 客户端中成熟度最高一档。
- **链接**：https://github.com/seanmonstar/reqwest

### A4. hyper 1.x — 底座（不直接依赖，watch 升级路径）
- **定位**：低层 HTTP client/server，1.0 于 2023-11 稳定（训练知识·未二次核验）；reqwest 与 tonic 对 hyper 的共同依赖（两端点名）。
- **可抄机制**：① tower `Service` 抽象——中间件/连接复用统一挂载。② hyper-util 提供面向使用的 client/连接管理（legacy client）。③ http-body 1.x 帧流抽象——流式下载大快照的参考形态。
- **档位**：不直接依赖；作为 ureq/reqwest 传递依赖存在。
- **第一步动作**：无；watch reqwest 0.13 的升级节奏。
- **许可证/成熟度**：MIT；hyperium 生态核心，1.x 稳定线（训练知识·未二次核验）。
- **链接**：https://github.com/hyperium/hyper

### A5. HTTP 条件请求与缓存 — 威胁数据快照契约（Top-3③ 展开）
- **定位**：ADV 威胁数据快照（CVE/OSV 类数据）的离线更新协议，纯 HTTP 语义、不绑定任何客户端。
- **可抄机制**：① 条件 GET：响应侧记录 `ETag`（强）+ `Last-Modified`，请求侧 `If-None-Match` / `If-Modified-Since`，304 复用本地（RFC 9110 §13.1.2/§13.1.3——服务端对两者的支持不一致，两者都存、双保险）。② 断点续传：`Range: bytes=N-` + `If-Range`（**只允许强 ETag**；并发修改窗口内弱比较器会拼出损坏文件）。③ 落地顺序：临时文件 → 验签 → 原子 rename；**验签先于信任**，网络层缓存/续传产物在验签通过前一律视为不可信。
- **档位**：默认档逻辑（代码在，执行与否由联网开关决定）。
- **第一步动作**：写 snapshot-fetch 契约一页 + 用 GitHub Releases / OSV API 实测其对 If-None-Match/If-Range 的支持面（先量后改）。
- **许可证/成熟度**：协议级，n/a。门控档可选 crate：http-cache / http-cache-reqwest（MIT，RFC 7234 缓存中间件——训练知识·未二次核验）。
- **链接**：https://httpwg.org/specs/rfc9110.html

### A6. SSRF 防护坑清单 — URL 校验类工具（若出）
- **定位**：ADV 若做 URL 分析/抓取工具，出网前的地址校验。
- **可抄机制（坑清单）**：① **DNS rebinding（TOCTOU）**：校验时解析的 IP ≠ 连接时解析的 IP——解法是"解析一次、校验该 IP、直连该 IP"（first-resolved-IP dialing，GHSA-j6r3-76f7-8jcv 教训：还要在 socket 层核对实际对端）。② **重定向**：关闭自动跟随或逐跳全量重校验（含 scheme 降级、跨 host、跨端口）。③ **IP 解析绕过**：IPv4-mapped IPv6（`::ffff:127.0.0.1`）、十/八/十六进制 IPv4 写法、`0.0.0.0`、**IPv6 zone id**（`fe80::1%eth0`、URL 编码 `%25` 形式）——解析器差异本身就是攻击面（不同 URL 解析库对同一串的 host 判定不同）。④ 黑名单范围：loopback、私网、链路本地（fe80::/10）、云元数据（169.254.169.254）。
- **档位**：门控档（URL 工具立项时）。
- **第一步动作**：本清单登记为 fuzz 目标（hostname/IP 校验器 fuzzing，2026-08 已有同类项目把 SSRF 列为最高风险类别并这么做）。
- **许可证/成熟度**：n/a。
- **链接**：GHSA-j6r3-76f7-8jcv；learn.secbyte.org SSRF bypass 合集。

### A7. 网络功能 feature 门控 — 离线默认档的依赖结构（⑤号问题）
- **定位**：把"联网功能显式开关"做成依赖结构而非纪律。
- **可抄机制**：① Cargo feature 纵向分层：默认 `[]` 零网络依赖 → `net-sync`（ureq+快照更新）→ `net-async`（tokio+reqwest）——`cargo tree` 上可直接审计默认档无网络边。② 门控内代码模式：`#[cfg(feature="net-sync")]` 模块 + 对外的 `NetworkError::Offline` 哨兵错误，调用方拿到的是结构化"未启用"而非运行时失败。③ CI 门：默认档构建跑 `cargo tree` 断言无 tokio/reqwest 边（绿=SKIP 不算绿——断言必须真执行比较结果）。
- **档位**：默认档（这条本身就是默认档的定义）。
- **第一步动作**：在 workspace Cargo.toml 定 feature 名并落 ADR；与 X6 的 tokio 结论对齐。
- **许可证/成熟度**：n/a（Cargo 原生机制）。
- **链接**：https://doc.rust-lang.org/cargo/reference/features.html

---

## B. 扫视条目（机制一句 + 档位 + 链接）

8. **native-tls** — 包裹层：Windows 用 SChannel、Linux 用 OpenSSL，验证与根证书全交系统；Windows 上免额外构建依赖；对照项（ADV 不采用，理由：多后端行为差异 vs rustls 统一栈）；MIT/Apache-2.0，成熟。https://github.com/sfackler/rust-native-tls
9. **webpki-roots** — 把 Mozilla 根证书静态打进二进制：确定性、裸容器可跑，但根更新必须重发版（官方文档自述适合"能即时重编译部署"的场景）；门控档 fallback（platform-verifier 不可用时）；crate 许可证与证书数据许可需过 cargo-deny 核（CDLA-Permissive-2.0 待核）。https://github.com/rustls/webpki-roots
10. **rustls-platform-verifier** — 委托 OS 验证器（Windows 走系统 API），自动继承企业根与吊销状态；rustls 团队定位为"多数客户端首选"（2024-03 reqwest issue），reqwest 0.13 已设默认；默认档候选（ureq 的对应 feature）；Apache-2.0。https://github.com/rustls/rustls-platform-verifier
11. **quinn + h3（HTTP/3）** — quinn 是 QUIC 传输（0.11 线，训练知识·未二次核验），h3 crate 长期 0.0.x 实验线；HTTP/3 对本地优先场景无收益（无跨网络延迟问题）；watch；MIT/Apache-2.0。https://github.com/quinn-rs/quinn
12. **tonic（gRPC）** — hyper 1.x 底座上的 gRPC；0.14（2025）拆分 tonic-build/tonic-prost-build、TLS feature 改名（`tls-native-roots`/`tls-ring` 类，lib.rs 见 0.14.6 引用）；自托管 CI 若需跨语言 RPC 才评估；watch（ci-ops 触发）；MIT。https://github.com/hyperium/tonic
13. **tarpc** — trait 宏生成 RPC stub + serde 传输解耦，比 gRPC 轻；reference（adv-server 自研 JSON-RPC 的对照物，不进依赖）；MIT/Apache-2.0。https://github.com/google/tarpc
14. **自研 JSON-RPC over stdio/TCP（adv-server 衔接）** — LSP 式 `Content-Length` 分帧 + serde_json 载荷；机制要点：分帧层与序列化层分离，stdio 单工管道复用同一帧格式；默认档（沿用既有 adv-server 方向）；n/a。
15. **系统代理发现** — 环境变量 `HTTPS_PROXY`/`NO_PROXY`（事实标准，大小写两套都要读）+ Windows 注册表 `HKCU\...\Internet Settings`（ProxyEnable/ProxyServer；PAC 复杂度建议直接放弃支持）；门控档（企业环境出现才做）；reqwest 0.12.x 有 system-proxy 类 feature、ureq 手动 Proxy 对象。https://docs.rs/reqwest
16. **SOCKS/HTTP 代理支持** — ureq `socks-proxy` feature（socks5h 把域名解析交给代理端——SSRF 校验场景反而要求解析留在本地进程）；reqwest `socks` feature；门控档；认证覆盖面是已知短板区。https://github.com/algesten/ureq
17. **限速** — reqwest/ureq 均无内建限速；令牌桶模式（governor crate，MIT/Apache-2.0，训练知识·未二次核验）在门控档按调用方需求套；先量后改，不预置。

---

## 回答索引

- ① 定档裁决 → Top-3①（ureq 默认档 / reqwest 门控档 / 判据= X6 tokio 结论）
- ② rustls 后端 → Top-3② + A2（ring 默认，aws-lc-rs=FIPS 触发，三轴判据）
- ③ 快照 HTTP 契约 → Top-3③ + A5（ETag/If-Range 强比较/验签先于落地）
- ④ SSRF 坑清单 → A6（rebinding/重定向/解析绕过/zone id 四类）
- ⑤ 离线默认档结构 → A7（feature 纵向分层 + cargo tree 门禁断言）
