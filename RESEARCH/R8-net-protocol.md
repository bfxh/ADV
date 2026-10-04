# R8｜网络扫描与协议分析工具（机制级调研）

> 调研日期：2026-10-03 ｜ 方法：WebSearch 15 组（优先 2025–2026 资料）+ 既有工程知识库
> 口径说明：① 许可证/版本/成熟度标注为「调研时点公开信息」，未逐项复核仓内 LICENSE 的按「以官方仓/官网最新口径为准」处理；② 吞吐/性能数字一律带出处锚（README 自述、论文口径、厂商口径），单机单窗口测量不升格为普遍结论；③ 本文只写「能说清机制」的对象；④ 数字与版本随发布线变动，引用时先核对。
> 对象总数：35（机制级 deep 25 + 概览级 scan 10）
> 关联域：perf-core（批量调度）、adv-sandbox（网络能力边界）、adv-rules（规则/解析契约）、ci-ops（自托管网络）、watch/reference（跟踪/对照）。与 C1（CLI 加速）/X6 的 Rust 运行时话题不重复：本域只到「扫描器场景的适用性判据」。

---

## 0. 速览表（35 对象）

| id | 对象 | 档位 | 集成 | 机制摘要 |
|---|---|---|---|---|
| R8-001 | nmap | 有界 | adv-rules | 探测/重传/拥塞自适应 + service-probes DB + NSE(Lua) |
| R8-002 | masscan | 吸收(思想) | perf-core | 无状态 SYN-cookie 匹配 + BlackRock 置换 + TX/RX 解耦 |
| R8-003 | ZMap | 吸收(思想) | perf-core | 无状态收发 + mod p 随机排列 + 同族响应校验 |
| R8-004 | rustscan | 吸收 | perf-core | ulimit 驱动的批量自适应 + 交 nmap 做服务识别 |
| R8-005 | naabu | 有界 | adv-sandbox | SYN/CONNECT + 速率护栏 + 许可隔离集成 |
| R8-006 | zgrab2 | 吸收(模式) | adv-rules | 模块化 grab 管道 + 扫描/抓取两段式 |
| R8-007 | libpcap | 吸收 | perf-core | 内核 BPF 过滤 + pcapng 容器 |
| R8-008 | npcap | watch | adv-sandbox | Windows 抓包；许可是部署约束 |
| R8-009 | AF_PACKET/XDP/PF_RING/DPDK | 吸收(有界) | perf-core | ring + 零拷贝 + 批量 syscall + 亲和 |
| R8-010 | libpnet | 有界 | adv-sandbox | Rust 数据链路层抽象 + 分层包模型 |
| R8-011 | etherparse | 吸收 | adv-sandbox | 无分配零拷贝解析 + 边界检查 |
| R8-012 | pcap-parser | 有界 | adv-sandbox | 纯 Rust 离线 pcap/pcapng 解析 |
| R8-013 | scapy | reference | reference | 分层包 DSL（性能对照） |
| R8-014 | Wireshark dissector 核心 | 吸收 | adv-rules | 注册表三档匹配 + tvbuff + 字段契约 |
| R8-015 | Wireshark Lua/C 扩展 | 吸收 | adv-sandbox | Lua 稳定层 / C ABI 教训 / 挂载语义 |
| R8-016 | tshark 字段契约 | 吸收 | adv-rules | 稳定字段命名空间 + 离线输出 |
| R8-017 | Zeek 事件引擎 | 吸收 | adv-rules | 三层事件化 + 单线程 worker + 集群 |
| R8-018 | Zeek 脚本/日志框架 | 吸收 | adv-rules | 策略全在脚本层 + 日志/Notice 框架 |
| R8-019 | Suricata 架构 | 有界 | adv-rules | runmode + 流表 + MPM 预过滤 + EVE |
| R8-020 | Suricata Rust 迁移 | 吸收 | adv-sandbox | 渐进替换 + ABI 边界 + fuzz |
| R8-021 | Snort 3 | reference | reference | 规则引擎对照 |
| R8-022 | JA3/JA4 | watch | adv-rules | TLS 指纹 + 许可分层工程 |
| R8-023 | p0f | reference | reference | 被动指纹方法论（停滞） |
| R8-024 | mitmproxy | 吸收 | adv-sandbox | addon 生命周期 + wireguard 模式 |
| R8-025 | Envoy | reference | reference | filter chain / typed config 对照 |
| R8-026 | WireGuard | 吸收 | ci-ops | Noise_IK + cryptokey routing |
| R8-027 | Tailscale/Headscale | 吸收 | ci-ops | 自托管控制面 + ACL + 入网自动化 |
| R8-028 | frp | 有界 | ci-ops | 反向隧道 + STCP/XTCP |
| R8-029 | rathole | 有界 | ci-ops | 轻量 Rust 隧道 + token/Noise |
| R8-030 | SSH 隧道工程 | 有界 | ci-ops | ProxyJump/ControlMaster 最小私网 |
| R8-031 | step-ca | 吸收 | ci-ops | ACME + 短寿命证书自动化 |
| R8-032 | SPIRE/SPIFFE | watch | ci-ops | workload identity（按需） |
| R8-033 | boringtun | watch | watch | Rust WG 实现维护警讯 |
| R8-034 | tokio | 有界 | perf-core | async 边界判据（控制面 vs 数据面） |
| R8-035 | quinn | 有界 | adv-sandbox | QUIC 探测面 |

---

## 1. 高速扫描

### R8-001 nmap — 档位【有界】｜adv-rules
- **定位**：端口/服务/OS 探测的参考实现；扫描引擎的调度模型与「探测数据库+正则」式服务识别是整个领域的教科书。
- **可抄机制**：
  1. 探测/重传/拥塞自适应：timing 模板 T0–T5 把 超时/重试/并行度 打包成档位，引擎依据丢包反馈调并行度（丢包→放慢，稳定→加快），是「探测速率由反馈闭环控制」的最早工程化之一。
  2. 服务识别 = 数据驱动：nmap-service-probes 数据库（probe/rarity/intensity/match 正则/soft match）——识别能力全部外置为可编辑数据，而不是写死在代码。
  3. NSE 脚本引擎：Lua 脚本按 hostrule/portrule 门控 + 分类（safe/intrusive/vuln…）+ Nsock 事件循环做协作式并发；「规则门控 + 事件循环」对 adv-rules 的脚本化直接可参考。
- **档位**：【有界】——机制吸收，代码/数据不直接搬（NPSL 许可 + 体量）。
- **第一步动作**：把 nmap-service-probes 的「probe 强度/稀有度」两维参数表抄成 adv-rules 探测器的调度维度样例（成本感知调度的最小实现）。
- **许可证/成熟度**：NPSL（Nmap Public Source License，非 OSI 认证，再分发受限）／维护中（7.9x 发布线，2024–2025 有版本）。
- **链接**：https://nmap.org/ ｜ https://nmap.org/book/nse.html

### R8-002 masscan — 档位【吸收(思想)，代码不吸收】｜perf-core
- **定位**：单机极高吞吐 SYN 扫描的工程经典；「无状态」四要素的完整示范。
- **可抄机制**：
  1. 无状态响应匹配：不建 per-target 状态表——把请求要素经 SipHash 取 32-bit 写入 SYN 的序号字段（syncookie 同族思路），回包重算比对即可判定归属；状态开销从 O(进行中目标数) 降到 O(1)。
  2. 随机化调度：BlackRock 密码（有限域置换，Feistel 构造，源自 Black–Rogaway「Ciphers with Arbitrary Finite Domains」）把目标集 [0..N) 双射打乱——顺序扫描会让路由/邻居缓存集中到少数条目形成热点（README 与工程分析口径），随机序把流量摊开；副产物是不可预测（防御视角：同机制也提高检测难度，衍生于对抗）。
  3. 工程三件套：自有用户态 TCP 栈 + raw socket/PF_RING 旁路内核 TCP；TX/RX 独立线程（sender 按 `--rate` pps 发、receiver 无状态匹配）；断点续扫（paused.conf）把「长任务可中断」做成一等公民。内核 RST 干扰用 `--adapter-ip/--adapter-port` + 防火墙隔离源端口缓解（README 口径）。
- **档位**：【吸收(思想)】——AGPL 代码不进入 ADV 产品树；机制以独立实现方式复刻。
- **第一步动作**：在 perf-core 做一个「无状态完成证据」原型：任务下发带 keyed token（任务 id+参数 HMAC 取短前缀），结果包携带该前缀即视为有效完成，不落 per-task 状态表。
- **许可证/成熟度**：AGPL-3.0／维护中（低节奏，README 自述「全互联网单端口扫描 <6 分钟、10 Mpps」为自述口径，非独立复现；实测吞吐依网卡/驱动/主机）。
- **链接**：https://github.com/robertdavidgraham/masscan

### R8-003 ZMap — 档位【吸收(思想)】｜perf-core
- **定位**：学术界+工程界的无状态扫描正典（USENIX Security 2013, Durumeric/Wustrow/Halderman）；后续 Censys 等互联网测量体系的方法源头。
- **可抄机制**：
  1. 数学化随机排列：在乘法群 mod p（p = 2³²+15 = 4,294,967,311，为大于 2³² 的最小素数）上迭代，构造覆盖全地址空间的随机置换，只需 3 个整数状态（本原根、起点、当前值）——比通用置换更省状态。
  2. 收发解耦 + 最小共享状态：发送/接收是两个独立连续线程，接收侧对发送侧完全无状态；探测包可变字段（源端口/初始序号）编码密钥，回包可重算校验（与 masscan 同族，论文口径为「encode secrets」）。
  3. 单探针 + 周期重扫：默认不重传，靠扫描周期重复提高覆盖率；块名单用 radix 树（性能 + opt-out 合规）。IMC 2024「Ten Years of ZMap」给出命中率工程数字：单包覆盖 ~97.9% 存活主机、两包 98.8%、三包 99.4%；带 TCP 选项（SA/TS/WS/MSS）命中率 +1.5–2.0%，MSS 单选项可覆盖 TCP/80 上 >99.99% 服务且不超最小以太帧。
- **档位**：【吸收(思想)】。
- **第一步动作**：把「随机排列 + 周期全量重扫」写进 perf-core 调度器的两行约束：任务入队前做一次域内置换；失败项不重试，统一进下一轮全量。
- **许可证/成熟度**：Apache-2.0／维护中（4.x 线；2024 起默认随机 per-probe IP ID，来源：IMC 2024 论文）。
- **链接**：https://github.com/zmap/zmap ｜ 论文：https://www.usenix.org/conference/usenixsecurity13/technical-sessions/presentation/durumeric

### R8-004 rustscan — 档位【吸收】｜perf-core
- **定位**：Rust 端口发现器，职责克制——只做「快速发现」，服务识别交 nmap。
- **可抄机制**：
  1. 批量自适应：`infer_batch_size` 依据 ulimit（fd 上限）等资源约束推导并发批量（AVERAGE_BATCH_SIZE 3000/4500 随版本口径不同），并在探测结果异常（全闭）时主动告警建议调参——「批量=资源上限函数」而非拍脑袋常数。
  2. 分层组合：端口发现（自研高速）→ 服务识别（nmap）→ 用户感知一个工具；分工边界清晰，各自可独立替换。
- **档位**：【吸收】——批量推导逻辑是可移植的数学，非代码规模。
- **第一步动作**：把「并发批量 = f(资源上限, 观测成功率)」公式（README/文档口径的 infer_batch_size 逻辑）落到 perf-core 批处理器的参数推导函数。
- **许可证/成熟度**：GPL-3.0（以仓内 LICENSE 为准；本轮在线检索未直接复核）／维护中（2.x 线）；「65,535 端口约 3 秒」为 README 声称的最优条件口径（单机/低延迟网络），默认 timeout 1500ms、tries=1。
- **链接**：https://github.com/RustScan/RustScan

### R8-005 naabu — 档位【有界】｜adv-sandbox
- **定位**：ProjectDiscovery 生态的 Go 端口扫描器，攻击面发现流水线的一环。
- **可抄机制**：
  1. 速率/并发双旋钮：`-rate`（默认 1000 pps）与 `-c`（默认 25 线程）分离——吞吐与资源各自独立控制；`-timeout` 1000ms / `-retries` 3 / `-verify` 二次确认把「准确率 vs 速率」显式参数化。
  2. 许可隔离的集成模式：`-sV` 需要 nmap-service-probes（NPSL），naabu 不打包该数据、只读用户本地 nmap 安装——「MIT 工具 + 受限数据分离」的合规做法，对 ADV 集成 NPSL/GPL 数据直接可抄。
  3. 被动通道：Shodan InternetDB 被动端口枚举与主动扫描并存（主动+被动融合的输出面）。
- **档位**：【有界】——参数化思想吸收，工具本体作为沙箱可选组件。
- **第一步动作**：在 adv-sandbox 的探测插件规范里定义「速率/并发/超时/重试/验证」五参数为标准旋钮（与 naabu 对齐），并禁止打包受限数据库。
- **许可证/成熟度**：MIT／维护中（v2.4 线，2025 有版本）。
- **链接**：https://github.com/projectdiscovery/naabu

### R8-006 zgrab2 — 档位【吸收(模式)】｜adv-rules
- **定位**：ZMap 团队的 L7 banner 抓取器；「扫描→抓取」两段式的第二段。
- **可抄机制**：
  1. 模块化 grab 管道：每协议一个模块（banner/TLS/SSH/HTTP/SMB/MQTT/Modbus…近 30 个），统一「连接→协议握手→解析→结构化 JSON」流水线；握手全量 transcript 输出供离线复核（一次在线、多次离线分析）。
  2. 职责与策略约束：项目明确不接受「利用漏洞/爆破凭据」的模块（仓库贡献政策）——安全工具的能力边界写进项目治理，这对 ADV 的能力白名单是治理样本。
- **档位**：【吸收(模式)】。
- **第一步动作**：把「一次采集→transcript 落地→离线解析」写为 ADV 网络采集器的数据流约定（网络结果 = 原始 materials + 离线解析器）。
- **许可证/成熟度**：Apache-2.0 + ISC／维护中（第三方包分析显示 2025-12 仍有更新）。
- **链接**：https://github.com/zmap/zgrab2

---

## 2. 抓包与数据面

### R8-007 libpcap — 档位【吸收】｜perf-core
- **定位**：抓包面的事实标准 API/格式。
- **可抄机制**：
  1. pcap_compile → 内核 BPF：过滤表达式编译进内核，用户态零成本丢弃无关包——「过滤尽量下沉」。
  2. pcapng 容器：多接口/多块/注释的抓包容器格式，作为采集交换格式（与 pcap-parser 对接）。
- **档位**：【吸收】（格式与 API 语义；Windows 侧走 npcap，见下）。
- **第一步动作**：ADV 采集产物统一定为 pcapng；BPF 表达式作为过滤参数的一等输入。
- **许可证/成熟度**：BSD-3-Clause／维护中。
- **链接**：https://www.tcpdump.org/

### R8-008 npcap — 档位【watch】｜adv-sandbox
- **定位**：Windows 上的 libpcap API 兼容实现（NDIS 6 轻量过滤驱动）。
- **可抄机制**：无（机制与 libpcap 同构）；价值在**部署约束**：非商业免费，再分发/OEM 需授权——adv-sandbox 在 Windows 分发含抓包能力时要么「不捆绑、提示用户自装」，要么走商业授权。
- **档位**：【watch】——不吸收，只记录许可边界。
- **第一步动作**：在 adv-sandbox 的 Windows 打包清单加一条：抓包依赖「用户自装 npcap」，镜像/安装包不捆绑。
- **许可证/成熟度**：专有（Npcap License，免费仅限非商业/评估）／维护中。
- **链接**：https://npcap.com/

### R8-009 高速抓包路径：AF_PACKET tpacket_v3 / AF_XDP / PF_RING / DPDK — 档位【吸收(有界)】｜perf-core
- **定位**：从「内核拷贝」到「零拷贝/旁路」的吞吐阶梯；扫描器/IDS 的数据面底座。
- **可抄机制**：
  1. 工程共识四件套：ring（PACKET_MMAP/tpacket_v3 的块+帧两级缓冲、批量提交）＋ 零拷贝（AF_XDP UMEM；DPDK 大页+轮询）＋ 批量 syscall（sendmmsg/recvmmsg 摊薄单包开销）＋ CPU 亲和/隔离（专用核、NUMA 就近网卡——Suricata 8 已把「自动 pin 到 NUMA/NIC」做进产品）。
  2. 过滤下沉：XDP 在驱动最早点丢弃，BPF 在套接字层过滤（libpcap 同源思想）。
  3. 定量锚（均为特定测试环境口径，不升格为普遍结论）：10GbE 64B 线速理论 14.88 Mpps；Intel XL710 40GbE 上有 AF_XDP vs AF_PACKETv3 的公开对比；二手资料称传统 AF_PACKET ~1 Mpps vs AF_XDP >10 Mpps（依赖批处理与部署）——作为「路径选择量级差」参考，落地必须本机自测。
- **档位**：【吸收(有界)】——按 ADV 实际带宽需求选档：100Mbps 级（本工具默认档）根本不需要旁路，先做批量 syscall + 亲和即可。
- **第一步动作**：perf-core 网络吞吐基准改用「批量 syscall + CPU 亲和」基线（不引入 DPDK），记录 10GbE 64B 理论线 14.88 Mpps 作为对照分母。
- **许可证/成熟度**：混合——内核 AF_PACKET/AF_XDP（GPLv2 内核；用户态 API 无碍）、PF_RING（LGPL-2.1 及商业版）、DPDK（BSD-3-Clause，部分例外按组件）／均在维护。
- **链接**：https://docs.kernel.org/networking/packet_mmap.html ｜ https://www.kernel.org/doc/html/latest/networking/af_xdp.html

### R8-010 libpnet — 档位【有界】｜adv-sandbox
- **定位**：Rust 原生数据链路层/包构造解析库。
- **可抄机制**：平台 datalink 后端抽象（Linux AF_PACKET / macOS BPF / Windows winpcap 后端）；分层包模型（Ethernet/IP/TCP/UDP×Mutable/Immutable 两态）——构造与解析共用类型，「发送用可变/接收用不可变」的类型级约束。
- **档位**：【有界】——沙箱内做包构造/解析的候选库之一；不如 etherparse 轻（pnet 带 I/O 面）。
- **第一步动作**：adv-sandbox 的「包构造」插件以 pnet 类型为内部 API 样例，评估与 etherparse 的分工（构造用 pnet / 解析用 etherparse）。
- **许可证/成熟度**：MIT OR Apache-2.0（双许可）／维护中（节奏平缓）。
- **链接**：https://github.com/libpnet/libpnet

### R8-011 etherparse — 档位【吸收】｜adv-sandbox
- **定位**：纯 Rust、无分配的网络报文解析库（L2–L4 + 部分扩展头）。
- **可抄机制**：零拷贝分层解析——输入 `&[u8]` 返回切片引用（不复制 payload）；每层带边界检查，超短输入返回可处理的错误而非 panic；builder 结构与解析结构对称。对「解析不可信输入」的沙箱场景是安全默认值示范。
- **档位**：【吸收】。
- **第一步动作**：adv-sandbox 的网络解析插件默认依赖 etherparse，补充 fuzz 目标（对照 R8-020 的迁移模式）。
- **许可证/成熟度**：MIT OR Apache-2.0／维护中（0.15/0.16 发布线）。
- **链接**：https://github.com/JulianSchmid/etherparse

### R8-012 pcap-parser — 档位【有界】｜adv-sandbox
- **定位**：纯 Rust 的 pcap/pcapng 离线解析 crate。
- **可抄机制**：离线抓包文件当作一等「测试语料」——解析器边界用例可用真实 pcapng 语料驱动（与 R8-007 的交换格式闭环）。
- **档位**：【有界】。
- **第一步动作**：ADV 解析器测试集增加 pcapng 语料通道（坏块/截断/超大注释等边界样本）。
- **许可证/成熟度**：MIT（以 crates.io/仓标注为准）／维护中。
- **链接**：https://github.com/courvoif/pcap-parser

### R8-013 scapy — 档位【reference】｜reference
- **定位**：Python 包构造/解析的通用 DSL。
- **可抄机制**：分层字段 + overlay 组合模型，任意协议可快速建模（一次性脚本与教学价值）；性能非目标——与 R8-009/R8-011 形成「建模灵活性 vs 吞吐」对照轴。
- **档位**：【reference】——不进产品路径。
- **第一步动作**：无（仅作为协议建模的学习参照）。
- **许可证/成熟度**：GPL-2.0／维护中。
- **链接**：https://scapy.net/

---

## 3. 协议解析与事件引擎

### R8-014 Wireshark dissector 核心架构 — 档位【吸收】｜adv-rules
- **定位**：解析器插件化体系的天花板级参考：注册、匹配、字段、重组四大机制全公开。
- **可抄机制**：
  1. 注册表三档匹配：`register_dissector` + `dissector_add_uint("tcp.port", …)`（端口/名字表，O(1)）→ `heur_dissector_add` 启发式链（按序试到识别，返回 false 即让位）→ `Decode As` 用户覆盖（运行时可改）。三档有明确的优先级与 fallback，新协议只需注册、不改核心。
  2. tvbuff 数据视图：越界访问抛异常（不静默截断）；支持 subset（子区间）与 composite（多段拼合）——对 mmap 的大文件可零拷贝分层视图。ADV 对应物：解析结果用 (file, offset, len) 引用原文件，不复制字节。
  3. 字段契约：proto_tree 的每个字段有注册的 hf id 与稳定 filter 名（`http.request.method` 级命名空间）；tshark 的 `-T fields -e` 与显示层共用同一命名空间——「一次性定义，规则/脚本/UI 共用」。
  4. 重组协议：解析器通过 `desegment_offset/desegment_len` 声明「数据不够」，由 reassembly 框架缓冲——跨包解析责任在框架不在单个解析器。
- **档位**：【吸收】——机制借鉴，GPL 代码不直接进树（ADV 为 Rust，天然重写）。
- **第一步动作**：在 adv-sandbox 定义「dissector 注册三键」（扩展名/魔数/启发式回调）+ fallback 优先级表；把现有解析器输出迁移到字段名契约。
- **许可证/成熟度**：GPL-2.0／维护中（4.x 线，2025 有版本）。
- **链接**：https://www.wireshark.org/docs/wsdg_html_chunked/ChapterDissection.html

### R8-015 Wireshark Lua/C 双扩展层 — 档位【吸收】｜adv-sandbox
- **定位**：插件稳定层设计的正反教材。
- **可抄机制**：
  1. 双轨扩展：Lua API（Proto.new / ProtoField / DissectorTable.get / add_for_decode_as / register_heuristic / postdissector）提供**稳定的脚本层**；C 插件走 ABI——官方不承诺 ABI 稳定，插件需随版本重编（社区教训）。稳定层（脚本）与高性能层（C）的边界由此清晰。
  2. 挂载语义陷阱：官方文档明确解析顺序「先顺序、后随机」——dissector 禁止依赖静态变量。ADV 的插件规范应显式声明「同一输入允许任意顺序调用/多线程调用」。
- **档位**：【吸收】。
- **第一步动作**：adv-sandbox 插件规范写入两条硬约束：a) 插件不得依赖调用顺序；b) 插件 I/O 通过宿主 API，不私建全局状态。
- **许可证/成熟度**：GPL-2.0（随主体）／维护中。
- **链接**：https://www.wireshark.org/docs/wsdg_html_chunked/lua_module_Proto.html

### R8-016 tshark 字段输出契约 — 档位【吸收】｜adv-rules
- **定位**：解析结果对机器（规则/脚本）的稳定出口。
- **可抄机制**：`-T fields -e <field>` / JSON / PDML 三种机读出口共用同一字段命名空间；`-2` 两遍分析（第一遍收集状态、第二遍定论）；`-z` 统计——「字段名 = 对规则作者的公共 API」的模式，正是 adv-rules 需要的契约面。
- **档位**：【吸收】。
- **第一步动作**：adv-rules 的规则语言引用解析字段时，绑定 tshark 风格的点分命名空间（域.字段），并写进版本化 schema。
- **许可证/成熟度**：GPL-2.0／维护中。
- **链接**：https://tshark.dev/

### R8-017 Zeek 事件引擎 — 档位【吸收】｜adv-rules
- **定位**：网络安全监控（NSM）的「解析→事件→策略」三层架构正典。
- **可抄机制**：
  1. 三层分离：包源 → 事件引擎（C++：协议 analyzer + DPD 两级识别（签名/端口）→ 连接表 + 定时器 → 有序事件队列）→ 脚本层订阅。引擎只产出「状态变化（new_connection/signature_match）」与「数据可得（http_entity_data/file_sniff）」两类事件，不做判定。
  2. 单线程 worker 免锁 + 集群水平扩展：Worker（抓包分析）/ Manager（全局视图）/ Logger / Proxy 四角色；负载均衡在抓包层做。2025 线：7.1 引入可插拔集群后端，7.2 Storage Framework（SQLite/Redis/NATS 原型），8.0（2025-08）ZeroMQ 后端生产可用、Broker 收窄为 pub/sub——「集群总线可替换」写进产品路线。
  3. 连接表 + 定时器：把「会话状态」和「时间驱动清理」作为引擎一等公民（ADV 的扫描会话/长任务可映射）。
- **档位**：【吸收】——架构模式全量借鉴（BSD 许可无扩散顾虑）。
- **第一步动作**：adv-rules 试用「规则=事件订阅者」模型：解析核心只 emit 类型化事件，检测/审计/统计各为独立订阅者（先用伪事件集跑通端到端）。
- **许可证/成熟度**：BSD-3-Clause／活跃（8.0 2025-08）。
- **链接**：https://docs.zeek.org/ ｜ https://blog.zeek.org/

### R8-018 Zeek 脚本语言与日志框架 — 档位【吸收】｜adv-rules
- **定位**：策略表达层与结构化日志层。
- **可抄机制**：
  1. 引擎最小化：Zeek 的默认行为（默认日志、常见检测）全部由 base/policy 脚本实现——核心代码只留机制，策略在脚本层演进。ADV 对应：核心只做解析与调度，规则/策略全部外置。
  2. 脚本语言选型要素：静态类型 + 网络域类型（addr/port/subnet/interval）+ 事件声明 + `when` 异步；对 adv-rules 的脚本面（如需 DSL）是近邻样本。
  3. 日志/Notice 框架：统一 TSV/JSON 日志 + Notice（告警对象）+ Intel（情报匹配）+ SumStats（聚合）；「告警是一等结构化对象」而非字符串。
- **档位**：【吸收】。
- **第一步动作**：ADV 的输出模型增加 Notice 等价物（结构化告警对象：类型/证据/引用/时间），日志层对齐 JSON Lines。
- **许可证/成熟度**：BSD-3-Clause／活跃。
- **链接**：https://docs.zeek.org/en/master/scripting/index.html

### R8-019 Suricata 多线程与规则引擎 — 档位【有界】｜adv-rules
- **定位**：多线程 IDS/IPS 的工程实现；2025-07 发布 8.0（支持线到 2028-07）。
- **可抄机制**：
  1. 线程模型谱系：runmode（single / autofp / workers）× 抓包后端（AF_PACKET tpacket-v3 默认推荐 / PCAP / PF_RING 已模块化为插件）；流表 + 流亲和；8.0 新增 worker 自动 pin 到 NUMA/NIC——「线程数、亲和、抓包路径」三件套显式化。
  2. 分流前预过滤：MPM（多模式匹配预过滤，Aho-Corasick/HS 族）+ 引擎排序 + 缓存（8.0 MPM caching），把「绝大多数不命中规则」在廉价阶段拒掉，与 ADV 规则引擎的 fast-reject 结构同构。
  3. 统一事件出口：EVE JSON（含 stats/flow/alert 全类型）成为 SIEM 事实标准——「一个 JSON 事件流=所有消费者」。
- **档位**：【有界】——架构模式吸收（Go/Rust 重写），不集成 C 代码。
- **第一步动作**：adv-rules 引入「两级匹配」：先多模式预过滤（字面量/关键词）→ 再跑昂贵的语义规则；事件输出对齐 EVE 风格单流 JSON。
- **许可证/成熟度**：GPL-2.0／活跃（8.0，2025-07）。
- **链接**：https://suricata.io/ ｜ https://docs.suricata.io/

### R8-020 Suricata Rust 解析器迁移 — 档位【吸收】｜adv-sandbox
- **定位**：C 引擎渐进 Rust 化的产业级案例（8.0 后大致 C≈75% / Rust≈25% 的代码构成口径）。
- **可抄机制**：
  1. 渐进替换 + 严格 ABI 边界：libhtp（HTTP 解析库）全量 Rust 重写并入主仓，FTP/ENIP/MIME 逐协议转 Rust；Rust 模块与 C 核心通过明确 C-ABI 边界交换数据——每步可单独验证、可回退。
  2. fuzz 驱动安全：解析器 Rust 化伴随持续 fuzz（历史上找到多个解析类 CVE）——「重写不只为性能，也为内存安全 + 可模糊测试」。
- **档位**：【吸收】——ADV 全 Rust 不必迁移，但「解析器按协议边界独立 + 每个都有 fuzz harness」是可直接立的门。
- **第一步动作**：adv-sandbox 的每个解析器 crate 附一个 cargo-fuzz 目标（最小种子语料进仓）。
- **许可证/成熟度**：GPL-2.0／活跃。
- **链接**：https://github.com/OISF/suricata

### R8-021 Snort 3 — 档位【reference】｜reference
- **定位**：Cisco 的 IDS/IPS，与 Suricata 同代对照物（模块化 + Lua 插件 + 规则语法）。
- **可抄机制**：以对照为主——规则语法演化、Lua 策略层与 Suricata 路线的差异（同为「C 核心 + 脚本策略」的两种取舍）。
- **档位**：【reference】。
- **第一步动作**：无。
- **许可证/成熟度**：GPL-2.0／维护中（Cisco）。
- **链接**：https://www.snort.org/

### R8-022 JA3/JA4 TLS 指纹 — 档位【watch】｜adv-rules
- **定位**：TLS 客户端指纹的事实标准（JA3 → JA4 换代），adv-rules 的指纹类规则素材。
- **可抄机制**：
  1. 指纹 = 字段摘要：JA3（版本/密码套件/扩展/曲线/格式拼接后 MD5）；JA4 改为结构化可读格式（t13d1516h2_ 风格）+ 向后兼容——「指纹格式要可读可演进」的教训。
  2. **许可分层工程**（直接可抄）：JA4（TLS 客户端）为 BSD-3-Clause（与 JA3 同许可，专利放弃声明）；JA4+ 家族（JA4S/L/H/X/SSH）为 FoxIO License 1.1（专利待定；学术/内部可用，商业化需 OEM 授权）。Suricata 生态因此产生摩擦：ET Open/Pro 规则集未收录 JA4 规则；Rust 生态用 feature gate 做许可分层编译（flowscope/netring 默认只含 BSD 组，`ja4plus` 显式 opt-in）——这正是 ADV 需要的「许可感知功能开关」范式。
- **档位**：【watch】——JA3/JA4 本体可用；JA4+ 引入前先做许可评审。
- **第一步动作**：adv-rules 指纹模块按其许可分层：BSD 组默认开、FoxIO 1.1 组默认关且单列构建 feature。
- **许可证/成熟度**：JA4 为 BSD-3-Clause；JA4+ 为 FoxIO License 1.1（专利待定）／活跃（2026 仍有厂商集成动作）。
- **链接**：https://github.com/FoxIO-LLC/ja4

### R8-023 p0f — 档位【reference】｜reference
- **定位**：纯被动 OS 指纹（TCP/IP 特征匹配，不发送任何探测包）。
- **可抄机制**：方法论——「零发包的识别」在授权/隐蔽场景的价值；指纹库结构（参数组合匹配）。
- **档位**：【reference】——项目成熟度低，不建产品依赖。
- **第一步动作**：无（列为「被动优先」原则的出处）。
- **许可证/成熟度**：GPL-2.0 级／弃维护（自述停止开发，3.09b 后基本停滞）。
- **链接**：https://lcamtuf.coredump.cx/p0f3/

---

## 4. 代理/中间件

### R8-024 mitmproxy — 档位【吸收】｜adv-sandbox
- **定位**：可编程 HTTP(S) 代理的事实标准（MIT）；「代理即插件平台」的样板。
- **可抄机制**：
  1. addon 全功能化 + 事件钩子生命周期：拦截、改写、记录、UI 皆为 addon；钩子谱系齐全（load/running/done → client_connected → requestheaders/request/responseheaders/response → websocket_*/tcp_*）；AddonManager 用 safecall 隔离——单个插件崩溃不终止代理。
  2. 流式钩子：`requestheaders/responseheaders` 阶段可「边转发边改」，大 body 不整读——CI 代理/抓取场景的内存边界做法。
  3. 模式矩阵：regular / reverse / transparent / upstream / socks5 / dns / tun / **wireguard（默认 UDP 51820，服务端 10.0.0.1/24、客户端 10.0.0.2/32）**——wireguard 模式让「透明拦截」免去 iptables/pf 配置，是沙箱网络拦截的最省事路径（跨平台、移动端可扫二维码接入）。
- **档位**：【吸收】。
- **第一步动作**：adv-sandbox 的拦截插件规范照抄「钩子名 + safecall 隔离 + 流式阶段」三要素；透明拦截优先评估 wireguard 模式而非 iptables 透明代理。
- **许可证/成熟度**：MIT（含 mitmproxy_wireguard）／活跃（11/12 发布线，2025 有版本）。
- **链接**：https://mitmproxy.org/ ｜ https://docs.mitmproxy.org/stable/addons-overview/

### R8-025 Envoy — 档位【reference】｜reference
- **定位**：云原生 L4/L7 代理的配置模型天花板。
- **可抄机制**：filter chain 有序管道（每个过滤器显式声明处理阶段）+ typed config（protobuf 强类型）+ xDS 动态配置（配置面与数据面分离）+ admin `/config_dump` 可观测——对照项：ADV 若做「插件管线」，过滤器的排序/阶段/可观测三件事可借其形状。
- **档位**：【reference】——体量与运行时（C++）不匹配 ADV，只借配置模型。
- **第一步动作**：无（记录为管线设计对照）。
- **许可证/成熟度**：Apache-2.0／活跃（CNCF）。
- **链接**：https://www.envoyproxy.io/

---

## 5. 自托管组网 / 证书 / 隧道（ci-ops）

### R8-026 WireGuard — 档位【吸收】｜ci-ops
- **定位**：现代组网底座协议（内核主线实现）。
- **可抄机制**：
  1. Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s 握手：1-RTT 互认证、抗 DoS（cookie 机制）；协议面小（白皮书量级），实现可审计。
  2. cryptokey routing：公钥 ↔ allowed-IPs 绑定即「路由表 + ACL」，类 SSH authorized_keys 模型——网络可达性与身份合一，配置面极简。
  3. roaming：端点随已认证包自动更新（无需会话固定）。
- **档位**：【吸收】（协议与治理模型；ADV 不自研协议实现）。
- **第一步动作**：ci-ops 现网先用「WireGuard 明文配置 + 仓库外密钥」跑通两台 runner 互联（POC 一步到位）。
- **许可证/成熟度**：内核实现 GPLv2；wireguard-go 为 MIT；工具集以仓为准／维护中（内核活跃，userland 低节奏）。
- **链接**：https://www.wireguard.com/ ｜ 白皮书：https://www.wireguard.com/papers/wireguard.pdf

### R8-027 Tailscale/Headscale — 档位【吸收】｜ci-ops
- **定位**：WireGuard 之上的「控制面」；Headscale 是其自托管控制服务器（BSD-3-Clause）。
- **可抄机制**：
  1. 控制面/数据面分离：控制面（密钥注册、ACL 评估、MagicDNS、DERP 中继发现）与数据面（WireGuard 直连/中继）解耦——自托管时元数据不经过第三方（数据驻留/合规诉求）。
  2. 入网自动化与授权模型：pre-auth key 支持无人值守入网；tag 授权 + ACL 策略做服务级隔离——CI runner 的「一次性/临时节点」模型（ephemeral 节点自动过期）直接可用。
  3. 拓扑兜底：直连失败走 DERP 中继，保证可达性优先。
- **档位**：【吸收】。
- **第一步动作**：一台公网小机跑 Headscale（SQLite 单容器）→ 生成 pre-auth key → 一台 CI runner 用官方客户端入网 → 用 ACL 只放开 runner↔repo 服务端口。
- **许可证/成熟度**：Headscale BSD-3-Clause／活跃（0.26–0.29 发行线，2025 进入 Alpine/Fedora/Debian 打包节奏；注意其定位为单 tailnet 规模）。
- **链接**：https://github.com/juanfont/headscale ｜ https://tailscale.com/kb/

### R8-028 frp — 档位【有界】｜ci-ops
- **定位**：Go 反向代理隧道（Apache-2.0），自建内网穿透的成熟选项。
- **可抄机制**：
  1. 反向隧道模型：frpc（内网侧）主动连 frps（公网侧），控制连接 + 多路复用承载多隧道——NAT 后服务暴露的最小形态。
  2. STCP：双端共享 secret 才能建立访问（对端口探测不暴露）；XTCP：P2P 打洞优先、失败回退中继。控制通道支持 token/TLS（`transport.tls.enable`）。
- **档位**：【有界】——若 Headscale 方案成型则不引 frp；作为「只有一台跳板机」场景的备选。
- **第一步动作**：记录决策点——需要 HTTP 层路由/面板/多服务时用 frp；纯 TCP 转发且资源受限时用 rathole（R8-029）。
- **许可证/成熟度**：Apache-2.0／活跃（月度发布节奏，v0.6x–0.7x 线）。
- **链接**：https://github.com/fatedier/frp

### R8-029 rathole — 档位【有界】｜ci-ops
- **定位**：Rust 轻量反向代理（Apache-2.0），frp 的资源受限替代。
- **可抄机制**：token 每服务鉴权 + 传输层 TLS/Noise 二选一 + 配置热重载；体积/内存显著小于 frp（厂商/社区口径：~574KiB 最小构建、~8MB 空闲内存——非独立复测，但量级可信）；TCP/UDP only，无 HTTP 层需自行配 Caddy/Traefik。注意「比 frp 快 3 倍」是 2021 年自测口径，早已过时，不引用。
- **档位**：【有界】。
- **第一步动作**：在 512MB 级跳板机上做 rathole POC（单条 TCP 隧道：SSH + 一个 CI 服务端口）。
- **许可证/成熟度**：Apache-2.0／维护中（低节奏：v0.5.0 为 2023-10 最后 tag，dev 分支 2025–2026 仍有活动——生产使用前评估发布节奏风险）。
- **链接**：https://github.com/rathole-org/rathole

### R8-030 SSH 隧道工程 — 档位【有界】｜ci-ops
- **定位**：零新增基础设施的「最小私网」。
- **可抄机制**：ProxyJump（-J）跳板链 + ControlMaster/ControlPath 连接复用（一次认证多条隧道）+ `ExitOnForwardFailure`/`ServerAliveInterval` 保活 + autossh/systemd 常驻——SSH 本身即隧道/证书/认证三合一，POC 阶段成本最低。
- **档位**：【有界】——不作为长期方案（可维护性），作 POC 与兜底。
- **第一步动作**：CI 服务器访问内网仓库先用 `-J` + ControlMaster 打通，验证后再评估升级 WireGuard/Headscale。
- **许可证/成熟度**：OpenSSH（BSD 系许可）／活跃。
- **链接**：https://www.openssh.com/

### R8-031 step-ca — 档位【吸收】｜ci-ops
- **定位**：自托管在线 CA（Apache-2.0，功能不阉割），mTLS/短寿命证书的自动化中枢。
- **可抄机制**：
  1. ACMEv2 服务器 + 短寿命证书：自动签发/续期（小时级 TTL），被动吊销（过期即失效）替代 CRL/OCSP 的依赖——「证书短到不需要吊销」。
  2. Provisioner 矩阵：OIDC/JWK 单次令牌/云实例身份/SCEP/SSHPOP——每个 provisioner 对应一类「怎么证明你是谁」，CI 用 JWK 或 OIDC 即可自动化。
  3. 一物两用：同一 step-ca 可兼作 SSH CA（主机/用户证书），减少一套 PKI。
- **档位**：【吸收】。
- **第一步动作**：一台机器起 step-ca 容器 → 用 JWK provisioner 给 CI 服务签发 24h 证书 → 服务间 mTLS 校验脚本化。
- **许可证/成熟度**：Apache-2.0（开源版无核心功能门控；主动吊销/托管 UI 等在企业版）／活跃。
- **链接**：https://smallstep.com/docs/step-ca/

### R8-032 SPIRE/SPIFFE — 档位【watch】｜ci-ops
- **定位**：workload identity 框架（SPIFFE ID/SVID + 节点与工作负载 attestation）。
- **可抄机制**：身份发放基于「证明」（节点 attestation + workload attestation），而非静态凭据分发——多云/多运行时零信任身份的标准路线；对 ADV 自托管 CI 属「以后需要再加」的重装备。
- **档位**：【watch】——先记概念（SVID 短寿命 + 证明式发放），不与 step-ca 并行引入。
- **第一步动作**：无（列入后续触发条件：跨云或 k8s 规模上来再评估）。
- **许可证/成熟度**：Apache-2.0／活跃（CNCF 毕业项目）。
- **链接**：https://spiffe.io/

### R8-033 boringtun — 档位【watch】｜watch
- **定位**：Cloudflare 的 Rust WireGuard 用户态实现（BSD-3）。
- **可抄机制**：无新增机制；价值是**选型警讯**：无专职维护者（Cloudflare 员工公开口径），Mullvad 等已转向 NepTUN/GotaTun——「组网底座选型看维护者，不只协议对不对」。
- **档位**：【watch】。
- **第一步动作**：无（如未来需要 Rust 内嵌 WG，先核对 NepTUN/GotaTun 的维护状态再选）。
- **许可证/成熟度**：BSD-3-Clause／弃维护倾向（无活跃专职维护者）。
- **链接**：https://github.com/cloudflare/boringtun

---

## 6. Rust 网络运行时（扫描器适用性）

### R8-034 tokio — 档位【有界】｜perf-core
- **定位**：Rust 异步运行时（MIT）。
- **可抄机制**：判据而非采用——每连接一个 task 的模型在「百万级短命探测」下调度开销与抖动显著（与 masscan/ZMap 的「专用线程 + 批量 syscall + 无状态匹配」对照）；工程共识路线：**控制面用 tokio（配置/编排/长连接），数据面用专用线程 + raw socket + 批量 syscall**。ADV 的验证导向：先量 task 创建/唤醒成本，再决定扫描热路径边界。
- **档位**：【有界】（在 perf-core 用测量裁决，不预设）。
- **第一步动作**：perf-core 加一个基准：N 个并发短连接任务在 tokio 与「固定 worker + 批量」两种模式下的吞吐/尾延迟对比。
- **许可证/成熟度**：MIT／活跃。
- **链接**：https://tokio.rs/

### R8-035 quinn — 档位【有界】｜adv-sandbox
- **定位**：Rust QUIC 实现（MIT OR Apache-2.0）。
- **可抄机制**：QUIC/HTTP3 服务探测与「UDP 面可达性验证」的构建块（对照 TCP 扫描面）；QUIC 的 0-RTT/连接迁移特性影响探测语义（无状态假设不成立，连接即状态——扫描设计需显式处理）。
- **档位**：【有界】。
- **第一步动作**：adv-sandbox 的「HTTP/3 探测」样例用 quinn 做一次 ALPN 探测 POC。
- **许可证/成熟度**：MIT OR Apache-2.0／活跃。
- **链接**：https://github.com/quinn-rs/quinn

---

## 7. 2025–2026 前沿信号

1. **Suricata 8.0（2025-07）深化 Rust 化**：libhtp 全量 Rust 重写并入主仓、FTP/ENIP/MIME 转 Rust、Rust 最低 1.75、C≈75%/Rust≈25%；新增 worker 自动 pin 到 NUMA/NIC、实验性 Firewall runmode、库模式（自带线程）；AF_PACKET tpacket-v3 为默认推荐。信号：「解析器 Rust 化 + fuzz + 线程亲和」是 2025 工程主线。
2. **Zeek 7.1→8.0（2025-01 → 2025-08）**：集群后端 Broker→ZeroMQ（8.0 生产可用）、Storage Framework（SQLite/Redis，NATS 原型）、telemetry 事件度量、analyzer.log/dpd.log 合并。信号：事件引擎的「后端插件化」。
3. **ZMap「Ten Years of ZMap」（IMC 2024）**：单探针覆盖 ~97.9%/两包 98.8%/三包 99.4%；TCP 选项（SA/TS/WS/MSS）+1.5–2.0% 命中率；MSS 单选项覆盖 TCP/80 >99.99% 服务且不超最小以太帧；2024 起默认随机 per-probe IP ID。信号：无状态扫描进入「命中率工程」精调期。
4. **Stratoshark（2025-01-22 FOSDEM 发布，Sysdig 捐赠给 Wireshark Foundation）**：复用 Wireshark 解析/过滤引擎分析 eBPF 系统调用（.scap 文件，Falco libscap/libsinsp 采集）。信号：解析引擎可跨数据源复用——解析内核与 UI/数据源分层是有回报的架构。
5. **JA4 许可分层定型**：JA4（TLS 客户端）BSD-3、专利放弃；JA4+ 家族 FoxIO License 1.1（专利待定，商业化需 OEM）；ET 规则集因许可模糊未收录 JA4 规则；Rust crate（flowscope/netring）用 feature gate 做许可分层编译。信号：**许可感知的功能开关**进入安全工具的标准做法。
6. **masscan 经典化 / 生态接棒**：README 仍自述 10 Mpps 与 <6 分钟全网口径，但发布线稀疏；高频维护由 rustscan/naabu/zgrab2 等接替。信号：经典工具作思想参照，集成选活跃生态。
7. **WireGuard 实现层洗牌**：boringtun 无专职维护者；Mullvad 转向 NepTUN/GotaTun。信号：协议稳定 ≠ 实现可依赖，选型盯维护者。
8. **Headscale 进入发行版打包节奏（2025）**：0.26–0.29 线，Alpine/Fedora/Debian ITP；自托管控制面成熟度上台阶。
9. **数据面共识固化**：AF_XDP/tpacket-v3/DPDK 对比资料持续更新（含 Intel XL710 40GbE 公开基准）；Suricata 8 默认推荐 tpacket-v3。信号：「ring + 批量 + 亲和」是抓包路径的默认工程解。
10. **无状态扫描的对抗面成为显学**：随机化既是反热点手段也提高检测难度（masscan/ZMap 设计文档自述）；防御侧（Suricata/Zeek 规则、指纹分层）与扫描侧同步演进——ADV 作防御/审计工具时需双向阅读这些机制。

---

## 8. Top-3（按对 ADV 的杠杆排序）

| # | 主题 | 吸收物 | 集成 | 第一步动作 |
|---|---|---|---|---|
| 1 | **无状态扫描范式**（masscan R8-002 + ZMap R8-003） | 随机排列调度；任务携带 keyed 完成证据（免状态表）；TX/RX 解耦；速率与并发双旋钮；单发+周期重扫替代逐项重试；断点续跑 | perf-core | 原型一个「token 式完成证据 + 置换入队」的调度实验（§9 清单前 3 条） |
| 2 | **插件化解析体系**（Wireshark R8-014/015/016 + Zeek R8-017/018） | dissector 注册三档匹配 + fallback；tvbuff 式 (file,offset,len) 引用；稳定字段名契约；引擎只发类型化事件、策略全在脚本层；单线程 worker + 可替换集群总线 | adv-rules / adv-sandbox | 定义 ADV 的「注册三键 + 字段命名空间 + 事件类型集」最小 schema，跑通一个端到端样例 |
| 3 | **自托管 CI 网络/证书最小方案**（WireGuard/Headscale R8-026/027 + step-ca R8-031，兜底 frp/rathole/SSH R8-028/029/030） | mesh + ACL + pre-auth 入网；ACMEv2 + 短寿命证书（被动吊销）；隧道只在「单跳板机」场景引入 | ci-ops | 一台机 headscale + step-ca 容器；一台 runner pre-auth 入网；服务证书 24h TTL 自动续期 |

---

## 9. 重点回答①：masscan/ZMap 无状态扫描拆解 + 迁移到 ADV 批量任务调度

**「单机极高吞吐」要素清单（机制级）**：
1. **无 per-target 状态**：响应归属靠包内可变字段携带的密钥校验（masscan：SipHash→SYN 序号；ZMap：源端口/ISN 编码密钥，论文口径）。状态表 → 常数级。
2. **域内随机置换**：masscan 用 BlackRock（有限域 Feistel，Black–Rogaway）；ZMap 用 mod p=2^32+15 乘法群 + 3 整数状态。目的：打散路由/邻居缓存热点 + 不可预测（后者同时是检测侧课题）。
3. **收发解耦**：TX/RX 独立线程、共享状态最小化；发送端只认速率，接收端无状态匹配。
4. **速率是全局旋钮**：`--rate`（pps）独立于并发度；与 naabu 的 rate/threads 双旋钮同构。
5. **单发 + 周期重扫**：默认不重传，用「再扫一轮」覆盖失败——把重试从任务状态机降为批处理轮次。
6. **可中断/可续跑**：masscan paused.conf 检查点（长任务一等公民）。
7. **内核交互隔离**：自持源 IP/端口 + 防火墙隔断 RST 干扰（README 口径）；抓包旁路（PF_RING）按需。
8. **旁路哲学（知其边界）**：C10M 三旁路（驱动/TCP 栈/同步）只在公网级吞吐才值得；ADV 本地优先场景多数不需要。

**迁移到 ADV（perf-core 批量任务调度）**：
- A. **keyed 完成证据**：任务下发时嵌入 HMAC(id, 参数) 短前缀，结果回传该前缀即认定完成（对照 masscan 的思路），调度器不再维护 per-task 状态机。
- B. **置换入队**：把「按目录序/字母序处理」改为域内随机置换（平滑锁热点、对象存储分片热点、单仓库热点）——与 ADV 既有经验锚「邻域访存局部性」不冲突：置换作用于任务序，不破坏单文件内局部性。
- C. **重试降级为轮次**：幂等任务失败不进重试队列，统一进「下一轮全量」；把每任务重试的复杂度让位给轮扫描（代价=时延，收益=调度器简化）。
- D. **双旋钮**：吞吐（速率）与资源（并发）分开控制，呼应 naabu(rate/threads) 与 rustscan(ulimit 自适应) 两类参数化。
- E. **断点续跑文件**：处理进度落盘（已完成索引集合的紧凑表示），对齐 ADV「本地优先、可随时中断恢复」。

## 10. 重点回答②：Wireshark dissector 架构 → ADV 插件/解析体系借鉴

- **注册三档匹配 + fallback**：端口/名字表（O(1)）→ 启发式链（按序试探、false 让位）→ 用户 Decode As（运行时覆盖）。ADV 对应：扩展名/魔数/启发式三键注册 + 明确优先级 + 用户可覆盖配置。
- **数据视图零拷贝**：tvbuff 的 subset/composite 支持对大文件做分层视图；越界即异常（不静默截断）。ADV 对应：解析结果以 (file, offset, len) 引用原文件，解析器只能看「视图」。
- **字段契约即 API**：hf 字段的稳定 filter 名同时服务显示层与 `-T fields` 机读出口。ADV 对应：规则作者面 = 版本化的点分字段命名空间。
- **重组在框架不在解析器**：`desegment_offset/len` 声明式让框架缓冲。ADV 对应：跨块/流式解析的「数据不足」信号协议。
- **扩展层教训**：Lua（稳定）与 C 插件（ABI 不承诺）双轨；解析顺序「先顺序后随机」→ 插件禁静态状态。ADV 对应：插件规范显式声明调用顺序无关性与线程安全要求。

## 11. 重点回答③：Zeek 事件引擎拆解

- **三层**：包源 → 事件引擎（analyzer + DPD 两级识别[签名→端口] → 连接表 + 定时器 → **有序事件队列**）→ 脚本层（类型化事件订阅）。
- **事件两类**：状态变化（new_connection/signature_match）+ 数据可得（http_entity_data/file_sniff）——判定不在引擎。
- **策略全外置**：默认日志与检测都在 base/policy 脚本；核心只留机制。
- **并发模型**：单线程 worker（免锁）+ 集群四角色（Manager/Logger/Proxy/Worker）水平扩展；负载均衡在采集层；集群总线从 Broker 迁 ZeroMQ（8.0 生产可用）证明「总线可替换」。
- **ADV 映射**：解析/扫描核心 emit 类型化事件（file_parsed/secret_found/dep_resolved/scan_round_done）→ adv-rules 订阅判定 → 审计/统计各为订阅者；worker 按核分片、无共享态；日志 = 结构化 JSON Lines（Notice 一等对象）。

## 12. 重点回答④：自托管 CI 的网络/证书最小方案

- **判据先行**：需要「网络可达性」→ WireGuard/Headscale（R8-026/027）；需要「进程级身份与互认证」→ mTLS + step-ca 短寿命证书（R8-031）；只有一台公网跳板 → frp/rathole 隧道（R8-028/029）；POC/兜底 → SSH ProxyJump + ControlMaster（R8-030）。
- **推荐最小组合**（三件套，按需叠加）：Headscale（单 tailnet、ACL、pre-auth key 入网、DERP 兜底）+ step-ca（ACMEv2、24h 级 TTL、被动吊销、兼作 SSH CA）+ 运行在各节点的官方客户端。仅当无法部署 mesh 时才引隧道类工具。
- **风险标注**：Headscale 单 tailnet 定位 + 0.2x 快速演进（2025 打包节奏可作成熟度旁证）；rathole 发布节奏慢于 frp（选型时对照）；证书自动化优先「短寿命 + 自动续期」，避免自建 CRL/OCSP。
- **触发升级条件**：跨云/多 k8s 集群 → 再评估 SPIRE/SPIFFE（R8-032）。

## 13. 重点回答⑤：adv-sandbox 网络能力白名单（能力边界设计）

- **默认允许（无需提权）**：离线 pcap/pcapng 解析（R8-012）；本地回环范围内抓包/注入；对显式声明目标（127.0.0.0/8、用户登记域名）的 CONNECT 级探测；受控 resolver 的 DNS；TLS 指纹的 BSD 组（JA3/JA4，R8-022）。
- **需要显式授权（elevated 标签）**：raw socket / SYN 扫描（naabu -s s、masscan 类）；AF_PACKET/PF_RING 抓包（R8-009）；WireGuard 设备创建；mitmproxy 透明/WireGuard 拦截模式与 TLS 解密（R8-024）。
- **默认禁止**：无速率上限的高速扫描（护栏：速率/目标/时长三参数上限）；内核模块加载；npcap 捆绑分发（Windows 许可，R8-008）；JA4+ 默认关闭（FoxIO 1.1，R8-022）。
- **工程化建议**：能力=标签（capture/raw-socket/tunnel/cert-issuer），每个标签强制三参数（速率上限、目标白名单、时限）；网络事件全量落审计日志（对齐 Zeek 日志模型）；不打包 NPSL 等受限数据（学 naabu 读本地安装，R8-005）；策略约束写进项目治理（学 zgrab2 不接受利用类模块，R8-006）。

---

## 附：集成分布

- **perf-core**：R8-002/003/004/007/009/034
- **adv-sandbox**：R8-005/008/010/011/012/015/020/024/035
- **adv-rules**：R8-001/006/014/016/017/018/019/022
- **ci-ops**：R8-026/027/028/029/030/031/032
- **watch/reference**：R8-013/021/023/025/033
