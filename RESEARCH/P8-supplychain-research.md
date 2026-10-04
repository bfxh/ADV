# P8 供应链安全研究论文扫视（2023–2026）

> 域：恶意包检测 / typosquatting·依赖混淆 / 构建完整性与可复现构建 / SBOM 实证 / 包管理器攻击面 / 依赖图与风险传播 / signing·provenance / 模型供应链 / 历史事件技术分析。扫视日 2026-10-04。
> 方法：WebSearch 定向检索（USENIX/NDSS/S&P/CCS/ICSE/RAID 关键词 + arXiv）+ 既有 P1 四大会议清单交叉复用（已核验链接直接引用）。
> 结论默认 **sweep 级**（标题+摘要/检索摘要，未逐篇读全文）。收录 **44** 篇（P0 6 / P1 19 / P2 17 / watch 2）。
> 链接政策：仅收录真实链接；**已核验**=单篇页直接核对；「待验证」=检索摘要确认题目但单篇链接未二次核验，只给真实的 venue/库根链接。排除纯区块链、纯 DDoS。

## P0 清单（≤6，读了能直接改哪个设计决策）

- **MalGuard: Towards Real-Time, Accurate, and Actionable Detection of Malicious Packages in PyPI Ecosystem**（USENIX Security 2025）— 据此定 adv-sca 恶意包检测的"静态特征族 + 轻量模型实时管线 + 人读解释"主形态，并定告警出口要"可行动"的验收口径。 [链接](https://www.usenix.org/conference/usenixsecurity25/presentation/gao-xingan)（已核验）
- **Taxonomy of Attacks on Open-Source Software Supply Chains**（IEEE S&P 2023，Ladisa 等）— 把 110+ 真实攻击（含 SolarWinds/XZ 谱系）折成攻击树与防御映射；据此给 adv-sca 检测规则库做覆盖度对账表（哪类攻击面有规则、哪类没有）。 [链接](https://arxiv.org/abs/2204.04008)（已核验）
- **From Noise to Signal: Precisely Identify Affected Packages of Known Vulnerabilities in npm Ecosystem**（NDSS 2025）— 已知漏洞→受影响包要靠补丁/代码级归属验证而非版本区间推断；据此定 adv-sca 匹配层"清单分离后如何降噪"的判据。 [链接](https://www.ndss-symposium.org/ndss-paper/from-noise-to-signal-precisely-identify-affected-packages-of-known-vulnerabilities-in-npm-ecosystem)（已核验）
- **We Have a Package for You!（LLM 包幻觉大规模分析）**（USENIX Security 2025，Spracklen 等）— 代码生成 LLM 会幻觉出可被抢注的包名（slopsquatting）；据此给 adv-sca 增加"AI 产出依赖名单"的幻觉/抢注检查项。 [链接](https://www.usenix.org/conference/usenixsecurity25/technical-sessions)（单篇条目「待验证」，页真实）
- **An Empirical Study on Reproducible Packaging in Open-Source Ecosystems**（ICSE 2025）— 六生态可复现打包的真实可行性数据；据此校准 adv-sca D5 锁定完整性判据的档位（哪些生态可要求 bit 级复现、哪些只能要求元数据一致）。 [链接](https://wspr.csc.ncsu.edu)（实验室页，已核验）
- **Signing in Four Public Software Package Registries**（Schorlemmer 等，2024）— 四大 registry 签名采用率与影响因素实证；据此定 adv-sca 签名/来源验证模块的预期覆盖率（低采用⇒验签只能作加分信号不能作硬门）与失败模式。 [链接](https://www.computer.org)（「待验证」）

## T1 恶意包检测（静态特征 / 动态沙箱 / 元数据 / LLM）

- **[P0]** **MalGuard: Towards Real-Time, Accurate, and Actionable Detection of Malicious Packages in PyPI Ecosystem**（USENIX Security 2025）— PyPI 恶意包可在安装路径上实时检测，低误报、可解释、可行动。 可抄机制：静态特征族 + 轻量模型实时管线 + 人读解释输出。 模块：adv-sca/恶意包规则引擎。[链接](https://www.usenix.org/conference/usenixsecurity25/presentation/gao-xingan)
- **[P1]** **Cerebro: Malicious Package Detection in npm and PyPI using a Single Model of Malicious Behavior Sequence**（arXiv 2023，2309.02637）— 用统一"恶意行为序列"建模跨 npm/PyPI 检测。 可抄机制：行为序列抽象层使规则跨生态复用。 模块：adv-sca/规则库组织。[链接](https://arxiv.org/abs/2309.02637)
- **[P1]** **FV8: A Forced Execution JavaScript Engine for Detecting Evilly Behaved Packages**（CCS 2024）— 强制执行可逼出恶意包的逃逸型行为。 可抄机制：强制执行环境（喂假参数/假环境续跑）作动态校验通道。 模块：adv-sca/动态沙箱。[链接](https://dl.acm.org)「待验证」
- **[P1]** **PyFEX: Uncovering Evasive Python-based Threats via Forced Execution**（arXiv 2026）— FV8 思路移植到 PyPI：Python 侧强制执行暴露逃逸威胁。 可抄机制：Python 强制执行框架 + 逃逸覆盖指标。 模块：adv-sca/动态沙箱。[链接](https://arxiv.org)「待验证」
- **[P1]** **Shifting the Lens: Detecting Malware in npm Ecosystem**（arXiv 2024）— 换视角做 npm 恶意代码检测并实测 typosquatting（12 例）。 可抄机制：安装态视角特征 + typosquatting 实例提取流程。 模块：adv-sca/规则库。[链接](https://arxiv.org)「待验证」
- **[P1]** **Bad Snakes: Understanding and Improving Python Package Malware Scanning**（ACM，约 2023–2024）— 对 PyPI 官方扫描器的目标与部署做实证，量化扫描器盲区与漏报。 可抄机制：以"运营方视角"定义扫描器验收指标（漏报/扫描预算）。 模块：adv-sca/检测验收口径。[链接](https://dl.acm.org)「待验证」
- **[P1]** **A Knowledge-Driven Framework for Malicious Packages**（2026）— 知识驱动的可解释检测，部署于 PyPI 实际发现 54 个未报告恶意包。 可抄机制：知识库驱动规则生成 + 线上持续发现回路。 模块：adv-sca/规则库 + R9 规则源。[链接](https://www.researchgate.net)「待验证」
- **[P1]** **Are They a Silver Bullet? On the Ability of LLMs to Detect Malicious Packages**（2025）— 系统评测 LLM 检测恶意包的能力边界。 可抄机制：LLM 作为 adv-sca 辅助通道的评测协议（能力/成本/漏报分档）。 模块：adv-sca/LLM 辅助检测。[链接](https://arxiv.org)「待验证」
- **[P2]** **Guarding the npm Ecosystem with Semantic Malware Detection**（Gobbi 等，约 2024）— 代码语义级检测覆盖变形/混淆样本。 可抄机制：语义 embedding 作规则失配时的第二通道。 模块：adv-sca/规则库。[链接](https://arxiv.org)「待验证」
- **[P2]** **MalPacDetector: An LLM-Based Malicious NPM Package Detector**（IEEE 2025）— LLM 检测器实测发现 39 个新恶意包并获 npm 安全团队确认。 可抄机制：LLM 告警→人工确认→入册的运营闭环。 模块：adv-sca/LLM 辅助检测。[链接](https://ieeexplore.ieee.org)「待验证」
- **[P2]** **Mind the Gap: Evaluating LLMs for High-Level Malicious Package Detection**（arXiv 2026）— 13 个 LLM 跨 PyPI/npm 的系统评测，暴露高层推理与实际检测的差距。 可抄机制：多模型横评协议（含 typosquatting 类样本）。 模块：adv-sca/LLM 辅助检测。[链接](https://arxiv.org)「待验证」
- **[P2]** **Evaluating LLM-Based Detection of Malicious Package Updates in npm**（RAID 2025）— 更新场景下 LLM 检测与"对抗自适应"问题。 可抄机制：把"版本更新 diff"设为独立检测单元（不只扫新包）。 模块：adv-sca/更新监测。[链接](https://www.researchgate.net)「待验证」
- **[P2]** **Malicious Package Detection using Metadata Information**（Halder 等，2024）— 发布节奏/维护者历史等元数据信号即可检出相当比例恶意包。 可抄机制：零成本元数据信号作预筛层。 模块：adv-sca/清单阶段信号。[链接](https://www.semanticscholar.org)「待验证」
- **[P2]** **Towards Fast, Accurate, and Multilingual Detection of Malicious Code**（Li 等，2023）— npm/PyPI 双生态快速多语言检测。 可抄机制：速度/精度/语言覆盖三指标折衷口径。 模块：adv-sca/规则引擎性能档。[链接](https://www.semanticscholar.org)「待验证」

## T2 typosquatting / 依赖混淆 / 幻觉包名

- **[P0]** **We Have a Package for You! A Comprehensive Analysis of Package Hallucinations by Code Generating LLMs**（USENIX Security 2025，Spracklen 等）— 代码生成 LLM 大量幻觉不存在的包名，且相当比例可被注册（slopsquatting 成为新攻击面）。 可抄机制：对锁定文件/AI 产出依赖名单做"幻觉名+相似名"双查。 模块：adv-sca/typosquatting 规则。[链接](https://www.usenix.org/conference/usenixsecurity25/technical-sessions)「待验证」
- **[P1]** **Uncovering Similar but Different Packages in PyPI and Potential Security Threats**（约 ICSE/ASE 2024）— 大规模"相似但不同"包克隆检测，给出 PyPI 抄袭/仿冒的真实规模。 可抄机制：相似度检测算法（克隆+改名）作 typosquatting 候选生成器。 模块：adv-sca/typosquatting 规则。[链接](https://dl.acm.org)「待验证」
- **[watch]** **Exploring the Unchartered Space of Container Registry Typosquatting**（Liu 等，约 2022，越界窗口作旁证）— 容器 registry typosquatting 可行性系统研究，被后续工作广泛引用。 可抄机制：编辑距离+语义相似候选生成在非代码 registry 同样成立。 模块：adv-sca/watch（镜像/容器场景）。[链接](https://dl.acm.org)「待验证」

## T3 构建完整性与可复现构建

- **[P0]** **An Empirical Study on Reproducible Packaging in Open-Source Ecosystems**（ICSE 2025，NC State WSPR）— 六个生态的可复现打包大规模量化：可复现率与失败模式分布。 可抄机制：把"可复现"拆成分生态档位判据，直接校准 D5 锁定完整性。 模块：adv-sca/D5 + xtask-gates。[链接](https://wspr.csc.ncsu.edu)
- **[P1]** **An Evidence-driven Protocol for Trustworthy CI Pipelines**（arXiv）— 以确定性构建系统（DBS）为基础，给 CI 管线定义证据协议。 可抄机制：CI 证据链（构建声明+产物哈希）作为 D5 的上游输入。 模块：xtask-gates。[链接](https://arxiv.org)「待验证」
- **[P2]** **Automating re-Build Process for Open-Source Software**（约 2024–2025）— 实证分析可复现 vs 不可复现构建的差异并自动化重建流程。 可抄机制：重建差异归因（源/工具链/时间戳）清单。 模块：adv-sca/D5。[链接](https://www.semanticscholar.org)「待验证」
- **[watch]** **Fifty Years of Open Source Software Supply Chain Security**（ACM，约 2024）— 长周期综述，把确定性构建/可复现性放进供应链防御谱系。 可抄机制：防御谱系图用于 PROGRAM.md 定位。 模块：reference。[链接](https://www.researchgate.net)「待验证」

## T4 SBOM 实证研究（质量 / 消费方 / 准确性）

- **[P1]** **Empirical Analysis of SBOMs in Maven Central（演化与内容准确性）**（Gamage 等，ACM 2025）— Maven Central 大规模真实 SBOM 的演化与准确率量化：真实 SBOM 普遍不完整。 可抄机制：SBOM 完整性验收指标（缺失依赖率）用于 adv-sca 自产 SBOM 质检。 模块：adv-sca/SBOM 质量验收。[链接](https://dl.acm.org)「待验证」
- **[P1]** **A Landscape Study of Open Source and Proprietary Tools for SBOM**（2024）— SBOM 生成工具横向评测，生成器之间差异显著。 可抄机制：多生成器交叉比对作验收手段（≥2 生成器并集/差集）。 模块：adv-sca/清单阶段（Syft→Grype 范式的工具选型依据）。[链接](https://arxiv.org)「待验证」
- **[P2]** **If it's not SBOM, then what?（意大利从业者研究）**（Nocera 等）— 定性研究 SBOM 采纳障碍与消费方真实需求。 可抄机制：消费方视角的需求清单（告警可行动性>完整性）。 模块：reference（产品形态决策）。[链接](https://www.computer.org)「待验证」
- **[P2]** **sbom-unifier: Integration Framework for Heterogeneous SBOMs**（arXiv 2026）— 异构 SBOM 整合框架，直指多工具输出不一致这一核心痛点。 可抄机制：SBOM 归一层（字段映射+冲突消解）。 模块：adv-sca/清单阶段。[链接](https://arxiv.org)「待验证」

## T5 包管理器攻击面（install script / registry，防御视角）

- **[P1]** **iHunter: Hunting Privacy Violations at Scale in the Software Supply Chain on iOS**（USENIX Security 2024）— 包粒度污点摘要使供应链级隐私违规检测可规模化。 可抄机制：包级污点摘要 + 跨包组合分析（adv-sca 依赖图上的摘要节点）。 模块：adv-sca/风险传播。[链接](https://www.usenix.org/conference/usenixsecurity24/presentation/liu-dexin)
- **[P2]** **A Survey on Common Threats in npm and PyPI Registries（typosquatting/combosquatting/dependency confusion）**（Kaplan 等，arXiv 2023）— 双 registry 常见威胁分类+案例库。 可抄机制：威胁→检测器映射表，用于规则库覆盖度对账。 模块：adv-sca/规则库。[链接](https://www.semanticscholar.org)「待验证」
- **[P2]** **"I wasn't sure if this is indeed a security risk"（npm 生态安全风险实证）**（2025）— npm 安全风险与从业者判断的实证。 可抄机制：从业者误判模式→告警文案与分级依据。 模块：reference（告警出口设计）。[链接](https://www.semanticscholar.org)「待验证」
- 生态旁证（不入册）：pnpm/Bun 默认禁 post-install scripts 而 npm 不禁；npm 引入 minimumReleaseAge 延迟安装；约 94% 恶意 npm 包带 install scripts（检索摘要口径，待验证）→ 支撑 adv-sca 把 install script 存在性作为独立风险信号。

## T6 依赖图分析与风险传播（可达性实证 / 传递依赖审计）

- **[P0]** **From Noise to Signal: Precisely Identify Affected Packages of Known Vulnerabilities in npm Ecosystem**（NDSS 2025）— 版本区间推断误报大；补丁/代码级归属验证显著降噪。 可抄机制：补丁级归属验证（adv-sca 匹配引擎的"命中须过归属验证"层）。 模块：adv-sca/匹配引擎。[链接](https://www.ndss-symposium.org/ndss-paper/from-noise-to-signal-precisely-identify-affected-packages-of-known-vulnerabilities-in-npm-ecosystem)
- **[P1]** **VulSCA: A Community-Level SCA Approach for Accurate C/C++ Supply Chain Vulnerability Analysis**（NDSS 2025）— 无锁文件的 C/C++ 可用社区共现+代码证据做漏洞归属。 可抄机制：社区共现图 + 代码证据双通道（对应 ADV 场景里 vendor 进仓库的 C/C++）。 模块：adv-sca/匹配引擎。[链接](https://www.ndss-symposium.org/ndss-paper/vulsca-a-community-level-sca-approach-for-accurate-c-c-supply-chain-vulnerability-analysis)
- **[P1]** **Präzi: From Package-based to Call-based Dependency Networks**（ACM，约 2024）— 包平均只调用约 40% 已解析依赖 ⇒ 元数据级 SCA 系统性高估暴露面。 可抄机制：包级→调用级依赖转换作可达性预过滤。 模块：adv-sca/风险传播。[链接](https://dl.acm.org)「待验证」
- **[P1]** **Counterfactual analysis of vulnerability propagation**（Abdollahpour 等，IEEE）— 依赖分析粒度从包级到模块级可把 SCA 精度从约 35% 提到约 71%（检索摘要口径）。 可抄机制：粒度消融作 adv-sca 传播模块的验收实验设计。 模块：adv-sca/风险传播。[链接](https://ieeexplore.ieee.org)「待验证」
- **[P2]** **On the Discoverability of npm Vulnerabilities in Node.js Applications**（约 2025–2026）— 6,546 个 Node.js 应用的依赖漏洞可发现性大规模实证。 可抄机制："可达才报警"的分档输出。 模块：adv-sca/风险传播。[链接](https://www.researchgate.net)「待验证」
- **[P2]** **Cross-Ecosystem Vulnerability Analysis for Python（XECG 跨生态调用图）**（arXiv）— Python 跨生态调用图上的可达性分析。 可抄机制：跨生态（PyPI↔conda 等）依赖图合并的边界处理。 模块：adv-sca/风险传播。[链接](https://arxiv.org)「待验证」

## T7 signing 与 provenance（Sigstore / 透明日志 / SLSA）

- **[P0]** **Signing in Four Public Software Package Registries: 数量、质量与影响因素**（Schorlemmer 等，2024）— 四大 registry 签名采用实证：相关性证据表明采用受生态事件驱动且总体偏低。 可抄机制：验签结果分档（未签名≠恶意）+ 事件驱动的采用监控。 模块：adv-sca/签名与来源。[链接](https://www.computer.org)「待验证」
- **[P1]** **An Industry Interview Study of Software Signing for Supply Chain Security**（arXiv 2025-01）— 签名落地的工业访谈：工具/流程/激励障碍。 可抄机制：分阶段采纳路径（先日志后强验）。 模块：adv-sca/签名与来源。[链接](https://arxiv.org)「待验证」
- **[P2]** **Dual-Use Attestations: A Verifiable Disclosure Framework**（OpenReview）— 以 Sigstore Rekor 类透明日志承载可验证披露。 可抄机制：attestation 双向语义（声明+披露）设计。 模块：xtask-gates。[链接](https://openreview.net)「待验证」

## T8 模型供应链（模型文件完整性，watch）

- **[P1]** **Large Language Model Supply Chain: A Research Agenda**（arXiv 2024，2411.01604）— LLM 供应链 12 类风险分类。 可抄机制：风险分类作模型依赖登记字段（来源/哈希/许可证/风险档）。 模块：watch→未来 adv 模型清单。[链接](https://arxiv.org/abs/2411.01604)
- **[P1]** **A Large-Scale Exploit Instrumentation Study of AI/ML Supply Chain Attacks in Hugging Face Models**（ACM 2024）— HF 恶意模型实证：pickle 反序列化载荷为主，100+ 恶意模型被 JFrog 等发现。 可抄机制：模型文件（pickle/safetensors）静态载荷检查清单。 模块：watch（模型完整性）。[链接](https://dl.acm.org)「待验证」
- **[P2]** **Machine Learning systems are Bloated and Vulnerable**（arXiv 2024）— ML 系统依赖臃肿放大攻击面，SBOM 手段对 ML 件覆盖不足。 可抄机制：ML 件（权重/分词器/数据集）纳入清单的缺口清单。 模块：watch。[链接](https://arxiv.org)「待验证」

## T9 历史事件技术分析（SolarWinds / XZ / event-stream 规则提炼）

- **[P1]** **A Software Engineering Analysis of the XZ Utils Supply Chain Attack（CVE-2024-3094）**（约 2024–2025）— 从 SE 视角复盘 XZ：社会工程时间线+构建注入点。 可抄机制：时间线→可检测点列表（维护者行为突变、构建脚本差异）喂给 R9 规则。 模块：adv-sca/规则库（R9）。[链接](https://www.researchgate.net)「待验证」
- **[P2]** **How to reduce your risk of being SolarWinds, Log4j, or XZ**（2026）— 系统映射供应链框架与历史攻击，评估哪类防御接得住哪类事件。 可抄机制：攻击×防御映射矩阵（与 Ladisa 攻击树互补）。 模块：reference/xtask-gates。[链接](https://www.researchgate.net)「待验证」
- 事件旁证（不入册）：StepSecurity 对 XZ 被投毒构建过程的 Harden-Runner 逐步分析（build-to-release 管线注入点）；openSUSE 对 XZ 的"教训"归纳——两者都指向"构建/发布管线差异监控"这一 R9 规则族。

## T10 SoK / 综合分类

- **[P0]** **Taxonomy of Attacks on Open-Source Software Supply Chains**（IEEE S&P 2023，Ladisa/Plate/Martinez/Barais）— 110+ 真实攻击（含 SolarWinds 等）折成攻击树，攻击节点映射现有防御。 可抄机制：攻击树作 adv-sca 规则覆盖度对账表与威胁建模基线。 模块：reference + adv-sca/规则库规划。[链接](https://arxiv.org/abs/2204.04008)
- **[P2]** **Software supply chain: A taxonomy of attacks, mitigations and open problems**（Gokkaya 等，ACM 2026，被引 32+）— 更新的风险评估方法论综述。 可抄机制：供应链风险评估维度清单。 模块：reference。[链接](https://dl.acm.org)「待验证」

## 参考资源（数据集/行业报告，不入册但直接可用）

- **Datadog malicious-software-packages-dataset**（GitHub，28,623+ 恶意包样本，npm/PyPI 等）— adv-sca 规则回归语料首选。[https://github.com/DataDog/malicious-software-packages-dataset](https://github.com/DataDog/malicious-software-packages-dataset)
- **OpenSSF malicious-packages**（社区恶意包报告库，持续更新）— 增量语料+事件源。[https://github.com/ossf/malicious-packages](https://github.com/ossf/malicious-packages)
- **nesbitt.io package_management_papers**（包管理/供应链论文持续维护清单）— 本域后续补扫的起点。[https://nesbitt.io](https://nesbitt.io)
- Socket《The Landscape of Malicious Open Source Packages: 2025》（年度恶意包形态报告，行业口径）。

## 缺口（本次扫视没扫到的）

- 2023–2026 专门针对 **install/post-install scripts** 的顶会单篇实证未定位到强链接（现有支撑多为综述引用与生态旁证）→ 标「待验证」收录 0 篇，后续从 nesbitt.io 清单补扫。
- **Sigstore Rekor 透明日志的单篇大规模审计**（日志完整性/滥用）未检索到强链接；现有 T7 为采用率与访谈实证。
- CCS 2023–2025 供应链专场未逐一抓取（官网 JS 渲染），仅收录检索命中的 FV8 一篇。
- registry 中「待验证」条目的单篇链接需后续二核后才可升级为"已核验"。
