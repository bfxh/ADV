# RESEARCH/registry 外部抽审报告

- 审计日期：2026-10-04；对象：`RESEARCH/registry/*.jsonl`（66 文件，2178 条，validate.py 基线全绿）
- 审计人：独立智能体（未参与 registry 产出）；工具：python 机械检查 + curl 三轮（HEAD→GET→`--ssl-no-revoke` GET，2026-10-04）+ WebSearch 存在性核验
- 本次**未修改任何 jsonl**：够格的机械错误（空 name / 明显乱码 / 同域重复）未发现（明细见 §3/§4）；链接错与 venue 错按纪律只记档，留补扫回填。

## 1. 抽样方法与样本数

分层（seed=42，可复现）：

| 层 | 定义 | 抽取 |
|---|---|---|
| L1 | verdict ∈ {P0,P1} 全权 | 244（P0 56 全 + P1 188 全） |
| L1' | verdict=吸收，超配额随机抽 | 35 / 561 |
| L2 | 每文件抽 2（其余条目内） | 132 |
| L3 | 剩余池随机 | 30 |

**样本合计 441 条**（P0/P1 全权后总量超出预估的 80–100，未再压缩）。其中 http(s) 链接 406 条（去重 370 URL）、`待验证` 27 条、RESEARCH 文档锚 8 条。verdict 分布（全量锚）：watch 300 / 吸收 561 / 有界 367 / 不吸收 78 / reference 303 / P0 56 / P1 188 / P2 325。

## 2. 链接核验结果

**条目级（406 条 http 链接）：**

| 类别 | 条数 | 占比 |
|---|---|---|
| 可核验（2xx/3xx） | **303** | 74.6% |
| 真错链（404 且对象/路径证实不对） | **5** | 1.2% |
| 无法核验（反爬 403 / 本环境 000 / crates.io 假 404） | **98** | 24.1% |

按档位：P0/P1 抽中 225 条链接，可核验 150，404 真错链 1，其余为 ACM/DOI 反爬；吸收档抽中 41 条链接，可核验 38，真错链 0。无法核验的构成：doi.org/dl.acm.org 反爬 403 占大头（ACM 对无头浏览器一律 403），另 crates.io 对 curl 返回假 404（经 WebSearch/docs.rs 证实 dunce、tantivy 等 9 条对象均存在）、4 条本环境 TLS 连接失败（zeek docs、fuchsia.dev、scholar.google、viperproject.org，三种姿势重试均 000）。

**真错链清单（5 条，建议补扫回填，本次未改）：**

| 条目 | 档位 | 问题 | 证据 |
|---|---|---|---|
| P7-002 | P0 | link=usenix.org nsdi23/fioraldi 404；venue 归属错 | 真文为 "LIBAFL: A Framework to Build Modular and Reusable Fuzzers"，NDSS 2023（WebSearch 多源一致，非 NSDI） |
| X1-045 | watch | link=github.com/DataDog/sketches-rust 404 | 对象存在：crate sketches-rust 真仓库为 github.com/p82ma75/sketches-rust（docs.rs） |
| X2-027 | 有界 | link=github.com/rawrunprotected/crc32f 404 | 对象存在：真仓库为 github.com/rawrunprotected/crc（FreeBSD 源码树引用） |
| C3-026 | reference | link=learn.microsoft.com/windows/win32/seccauth/authenticode 404 | `seccauth` 非微软文档路径段，需重找正确页 |
| X13-023 | 有界 | link=git-scm.com/docs/packformat 404 | Git 文档已重组为 gitformat-pack(5)，需换锚 |

另注：`待验证` 标记 120/2178（5.5%），集中在 P5(30)/P6(41)/P7(34)——是 schema 合法的诚实标记，但这三个文件的链接面尚未补齐。

## 3. 灌水检测

- **复制粘贴模板：0**。全量 2178 条 mechanism 完全重复串 = 0；mechanism==name 回声 = 0。
- **空洞度**：mechanism 长度 p10/p50/p90 = 18/62/133 字符。verdict=吸收 且 mechanism<40 字符共 22/561（3.9%），**逐条人工过目**：均为紧凑但有实义的描述（多为 00 域自研门脚本与标准库 crate，如 02-018 cargo-auditable「依赖清单 JSON 段嵌进编译产物，产物自带 SBOM」、D2-007 gofmt「-l 退出 0 是门契约反例」），**未发现只有定位没有机制的吸收判**。
- **错配候选：0 够格**。随机 8 条全字段目检（P4-061/P2-067/P7-001/E1-010/P1-031/P1-093/C2-013/C1-003）mechanism 均含版本号/量化数字/具体机制（如 C1-003 fzf：chunk 队列+0.71.0/8 线程 1.84x），与 verdict 相称。
- 保留意见：00 域（00-010~00-016）7 条 name 为文件路径而非对象名（如 name=`scripts/claim_gate.py`），信息无假但命名风格与「对象名」惯例不一致——记档不改。

## 4. 跨域一致性

- **同域重复：0**（name 归一后按域分组，无一域内重名）。
- **严重矛盾（吸收/有界 vs 不吸收跨域打架）：0**。跨 ≥3 域的共享对象 10 个，多数各域 verdict 一致（memchr 07/X2/X3/X4 全吸收；salsa、aho-corasick、rayon 同）。
- 有档位差但可由「不同域不同用法」解释、不判违规：semgrep（01=有界，C1=watch，D1=吸收）、sccache（06=吸收，D4=有界，D9=吸收）。建议补扫时顺带核这两组语境是否真按各自用法吸收/有界。
- 全量抽查外字段：date 全落 2026-10-02→10-04、无未来日期；depth deep 884 / sweep 1294；kind 分布正常（paper 708 / project 849 / tool 283 / technique 288 / product 32 / dataset 18）。

## 5. license=unknown 比例（分组报数，不算违规）

| 域组 | unknown/总数 | 比例 |
|---|---|---|
| P（论文域） | 205/658 | **31.2%** |
| 数字域 00–10 | 50/219 | **22.8%** |
| C/D/E/R/X | 0/1500 | 0% |
| **全量** | **255/2178** | **11.7%** |

注：P 域 license 大量用 `论文`(363)/`OA-USENIX`(40)/`ACM`(26) 等载体标签而非 SPDX 许可证名，口径混用是 unknown 比例之外的另一处一致性问题（记档）。

## 6. 结论：数据可信度分级（估计，样本锚 441/2178 = 20.2%）

- **高档（可信）≈ 74%**：链接可核验 74.6%，且该批内容机械检查全净（无模板、无回声、无重名、无矛盾），目检 mechanism 均具体。
- **中档（暂信、待补扫）≈ 24%**：对应无法核验的 24.1% 链接（多为反爬所致，非造假证据）+ 120 条 `待验证` + P 域 license 口径混用。
- **低档（已证有错）≈ 1.2%**：5 条真错链（其中 P7-002 一条 P0 的 venue 归属错，是本次唯一涉及 P0 的实质错误）。
- 一句话：内容面（mechanism/verdict/一致性）未发现灌水或矛盾证据；风险集中在**链接可达性与出处字段**，修复面小且集中（5 条错链 + P5–P7 的 `待验证` 回填）。

## 附：核验方法锚

- curl 三轮：`curl -I -L` → 失败者 `curl -L` → 再失败者 `curl --ssl-no-revoke -L`（schannel 吊销检查报 CRYPT_E_NO_REVOCATION_CHECK，见 2026-10-04 现场日志）；判定码 2xx/3xx=可核验。
- WebSearch 存在性核验用于：LIBAFL venue、dunce crate、DataDog/sketches-rust、rawrunprotected/crc32f（2026-10-04）。
- 本环境限制：WebFetch 未用（预期证书受限）；ACM/doi.org、crates.io、lib.rs、researchgate、vulkan.org 对脚本化请求反爬，其上 98 条「无法核验」不构成存在性反证。
