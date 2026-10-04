# X4 域调研：正则与多模式匹配引擎（ADV 规则匹配层）

- 调研日期：2026-10-04；范围：Rust regex 生态 / 多模式匹配 / 工业引擎对标 / 匹配专项
- 数字锚点均标来源；许可证/成熟度以 crates.io / 仓库现值为准（核查日 2026-10-04）
- 档位口径：吸收（直接进 ADV）| 有界（条件吸收，画清边界）| 不吸收 | watch（观察）| reference（只读对标）

---

## Top-3（直接决定 ADV 匹配层骨架）

1. **regex-automata 元引擎分层**——"literal prefilter → 反向预过滤 → lazy DFA → onepass → bounded backtracker → PikeVM" 的选档树就是 ADV 规则匹配层的预算模型原型（机制见条 2/15/16/17/18）。吸收。
2. **literal 预抽取管线（regex-syntax Extractor → Aho-Corasick/memmem/Teddy prefilter）**——把"大部分规则先退化为字符串查找"工程化，ADV 规则编译器直接复用同一条管线（机制见条 3/18）。吸收。
3. **aho-corasick 的预算换速度档位**——NoncontiguousNFA/ContiguousNFA/DFA 三档 + MatchKind，官方文档给了实测量级：contiguous NFA ~21MB/构建 275ms vs DFA ~1.6GB/构建 1.88s（docs.rs `AhoCorasick` 文档示例语料，核查日版本），这是 ADV 选档的现成标尺（条 7）。吸收。

---

## 一、Rust regex 生态

### 1. regex crate（1.x，元引擎门面）
- 定位：ADV 规则正则的首选门面；1.9（2023-06）起内部改用 regex-automata 元引擎，对外仍是零配置 API。
- 可抄机制：①UTF-8 语义贯穿——匹配 span 是 UTF-8 字节偏移，`&str` 输入保证匹配不切码点；②线性时间保证：任何输入 O(n·m) 上界，永不指数回溯；③`RegexBuilder::size_limit`（默认 10MiB，编译产物）与 `dfa_size_limit`（默认 2MiB，lazy DFA 缓存）两个预算旋钮。
- 档位：**吸收**
- 第一步动作：规则正则一律走 `RegexBuilder`，显式设 `size_limit` 并记录编译耗时基线。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（rust-lang 组织仓）。
- 链接：https://docs.rs/regex ；https://github.com/rust-lang/regex

### 2. regex-automata（元自动机 + 各引擎拆件库）
- 定位：regex 1.9+ 的发动机仓，把每个引擎单独暴露——ADV 做"规则分级匹配"时绕不开。
- 可抄机制：①meta::Builder 逐档开关（`prefilter`、`ascii_case_insensitive`、`dfa_size_limit`、`onepass`、`backtrack`、`hybrid`）；②`meta::Regex` 对外抹平各引擎怪癖，内部按"成本递增"自动选档；③`strategy.rs`（meta/strategy.rs，rust-lang/regex master）就是选档树源码，可当设计文档读。
- 档位：**吸收**
- 第一步动作：读 docs.rs meta 模块页 + strategy.rs，画出 ADV 自己的选档决策表（见下文"五问"①）。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://docs.rs/regex-automata/latest/regex_automata/meta/index.html ；https://github.com/rust-lang/regex/blob/master/regex-automata/src/meta/strategy.rs

### 3. regex-syntax（AST → HIR → literal Extractor）
- 定位：规则编译的中间层价值所在：把正则解析成 HIR（高层 IR），ADV 规则的静态分析（字面量提取、危险模式审计、复杂度估算）都在 HIR 上做。
- 可抄机制：①`Hir` 结构保留原始 span 与字面段，`hir::literal::Extractor` 从 HIR 抽前缀/内缀/后缀/必需字面量集合，带 limit_class/limit_repeat/limit_literal_len/limit_total 等预算旋钮（量级 ~10²，具体默认值以 regex-syntax 0.8 文档为准——本轮抓取被限流未逐项核实）；②"字面量必须完整"不变式：语言有限时提取集要覆盖全部解（对应 rust-lang/regex#1046 的正确性讨论）；③HIR 可检"嵌套量词/巨型类"做规则作者 lint。
- 档位：**吸收**
- 第一步动作：写 demo：对每条 ADV 规则跑 `Extractor`，统计"可退化为纯字面量"的规则占比。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://docs.rs/regex-syntax/latest/regex_syntax/hir/literal/struct.Extractor.html

### 4. regex-lite（体积档）
- 定位：同语法、小体积档：只保留 PikeVM 单引擎（无 lazy DFA、无 prefilter、无 SIMD），换取编译产物大幅缩小；速度让位。
- 可抄机制：①"单引擎 + 保留线性时间"的裁剪思路——ADV 的 CLI 极简发行版可参照；②用 feature flag 在 regex/regex-lite 之间二选一的发行策略。
- 档位：有界（仅最小发行版考虑）
- 第一步动作：量一次 regex-lite 与 regex 的二进制差（ADV 现有规则集），入基线表再决定。
- 许可证/成熟度：MIT OR Apache-2.0；维护中（2023 起随 regex 生态发布）。
- 链接：https://docs.rs/regex-lite

### 5. RegexSet（多正则同扫）
- 定位：一次扫描判定"哪些规则可能命中"，不返回位置——ADV 的粗筛层（把数千条规则先收敛到几条候选）。
- 可抄机制：①多模式合一自动机，一次 O(n) 遍历输出命中规则号集合；②"粗筛 RegexSet + 精扫单 Regex"两段式，天然隔离规则间性能干扰。
- 档位：**吸收**
- 第一步动作：benchmark：1000 条规则规模下 RegexSet 粗筛 vs 逐条编译的吞吐差（自有语料）。
- 许可证/成熟度：随 regex crate；维护中。
- 链接：https://docs.rs/regex/latest/regex/struct.RegexSet.html

### 6. fancy-regex（回溯扩展的边界）
- 定位：在 regex 之上补 lookaround/反向引用的兼容层：模式无 fancy 特性时直接委托 regex crate（快路径），有则走自己的回溯 VM（慢路径，失去线性保证）。
- 可抄机制：①"委托 vs 自有引擎"的双路径结构——ADV 若必须支持少量 PCRE 语义规则，照此隔离，不让回溯引擎污染主路径；②可按需给回溯 VM 加步数上限（fancy-regex 长期讨论中的问题，集成时自查当前版本行为）。
- 档位：watch（除非规则作者强需求）
- 第一步动作：在 ADV 规则 DSL 里统计 lookaround/反向引用的真实请求数，为 0 则不做。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://docs.rs/fancy-regex

## 二、多模式匹配

### 7. aho-corasick crate（多模式主干）
- 定位：字面量集合匹配的工业标准；rg 与 regex 元引擎的 prefilter 底座。
- 可抄机制：①三档自动机：NoncontiguousNFA（默认、构造快）/ ContiguousNFA（单块分配、内存优）/ DFA（最快、内存最贵）——docs.rs 官方示例给出量级：~21MB/275ms（contiguous NFA）vs ~1.6GB/1.88s（DFA），同一语料（核查日版本）；②MatchKind::{Standard, LeftmostFirst, LeftmostLongest} 决定语义，LeftmostFirst 对"规则优先级"语义直接可用；③`premultiply`：状态 ID 预乘转移步长，搜索期省一次乘法（DESIGN.md）。
- 档位：**吸收**
- 第一步动作：ADV 字面量规则层默认 ContiguousNFA + LeftmostFirst，仅在实测证明 DFA 收益显著时按规则集白名单升档。
- 许可证/成熟度：MIT OR Unlicense；维护中。
- 链接：https://docs.rs/aho-corasick/latest/aho_corasick/ ；https://github.com/BurntSushi/aho-corasick（DESIGN.md）

### 8. memchr crate（SIMD 字节/子串底座）
- 定位：单字节/双字节/三 needle 与单子串搜索的 SIMD 实现（x86_64/aarch64/wasm32），一切 prefilter 的地基。
- 可抄机制：①"rare byte" 启发式：选语料中最罕见的字节先扫；②Packed Pair：取子串中两个字节做类 SIMD 预筛再确认（Simon Willison 2026-01 的 C 移植实测称对 Python 内建最高 ~28x 提速——单点口径，仅作量级参考）；③运行时 `is_target_feature_detected!` 分派 sse2/avx2。
- 档位：**吸收**
- 第一步动作：确认 ADV 依赖树里已统一走 memchr（避免各处手写朴素扫描）。
- 许可证/成熟度：MIT OR Unlicense；维护中（2.7.x 线）。
- 链接：https://docs.rs/memchr

### 9. Teddy 机制（memchr packed/teddy128·teddy256）
- 定位：多子串 SIMD 加速器（源自 Hyperscan），在 memchr 内部实现：teddy128（SSE2/AVX2 双 128 位 lane）与 teddy256（AVX2 全 256 位）。
- 可抄机制：①nibble 桶掩码：按位低/高 nibble 查 16 项表，跨候选位 OR 后得到命中位图，再用 rare-byte 预筛确认（packed/teddy/README.md）；②模式集太大或 rare byte 不足时"预筛失效自动回退"——预算化降级的好范本；③模式长度上限（128 位档约 8 字节/chunk）以上靠回退确认。
- 档位：**吸收**（随 aho-corasick/memchr 间接获得，不自研）
- 第一步动作：在 ADV 规则集上验证 Teddy 路径触发条件（模式数/长度分布），写进选档注释。
- 许可证/成熟度：随 memchr；维护中。
- 链接：https://github.com/BurntSushi/memchr（src/arch/*/memmem/packed/teddy*/）

### 10. fst crate（FST 有序映射/集合）
- 定位：共享前缀的紧凑有序键值结构，整块 mmap 零反序列化——对 ADV 符号表（标识符/敏感函数名/规则标签）直用价值最高的一个。
- 可抄机制：①构建须按序输入（或外排合并），产物是自包含字节块，可 `mmap` 打开即查——符号表跨进程共享零成本；②`Stream` 支持范围/automaton 过滤的惰性迭代，stream 间 union/intersection/difference 都是自动机交并差；③`fst-regex` 可用正则定义键集查询，`fst-levenshtein` 做模糊键查询。
- 档位：**吸收**
- 第一步动作：把 ADV 索引域的符号表原型改成 fst Map，测构建（含乱序排序成本）与点查/范围查延迟。
- 许可证/成熟度：MIT OR Unlicense；维护中（0.4.x，更新慢但 API 稳定）。
- 链接：https://docs.rs/fst ；https://github.com/BurntSushi/fst

### 11. fst-levenshtein / levenshtein_automata（模糊符号查询）
- 定位：Unicode 感知的 Levenshtein 自动机，对 fst 做近似键查询；tantivy 全文引擎用它做模糊词条检索。
- 可抄机制：①把"编辑距离 ≤ d"编译成 DFA 再与 fst 的自动机求交——查询仍是流式、线性；②ADV"拼写近似 API 名告警"（如 `eval` vs `evil`）可直接组合这两件。
- 档位：有界（模糊查询按需启用）
- 第一步动作：以 ADV 符号表样例测 d=1/d=2 的查询成本，d=2 成本陡增则封顶 d=1。
- 许可证/成熟度：levenshtein_automata：MIT OR Apache-2.0；维护中（tantivy 生态在用）。
- 链接：https://docs.rs/fst-levenshtein ；https://crates.io/crates/levenshtein_automata

### 12. tantivy 对 fst 的用法（先例参照）
- 定位：Lucene 系全文引擎的 Rust 实现，term dictionary 用 fst + levenshtein_automata——"fst 做符号/词条表"的最大规模先例。
- 可抄机制：①字典与倒排解耦：fst 只存键到 offset 的有序映射；②增量构建：段内有序写、段间合并（ADV 规则表/符号表增量更新可仿）。
- 档位：reference
- 第一步动作：读 tantivy termdict 模块源码，摘合并策略进 ADV 索引域设计。
- 许可证/成熟度：MIT OR Apache-2.0；维护中。
- 链接：https://github.com/quickwit-oss/tantivy

## 三、工业引擎对标

### 13. Hyperscan（Intel，对标）
- 定位：网络级多模式正则引擎，块/流/向量三模式；流模式每条流要一块状态内存，块模式无跨调用状态（Hyperscan 5.4.1 官方文档）。
- 可抄机制：①编译期/运行期分离（compile 出 DB、运行仅查）——ADV 规则库"预编译分发"可借鉴概念；②大规模模式集下编译内存/时间可成为主要瓶颈，官方实践是拆库——对应 ADV"规则分组建库"。
- 档位：不吸收（x86 私有底座、编译期成本与 C 集成成本都不划算，仅作对标）
- 第一步动作：无（读 NSDI'19 论文补机制图）。
- 许可证/成熟度：BSD-3-Clause（Intel）；原仓低频更新，主线转 Vectorscan。
- 链接：https://intel.github.io/hyperscan/dev-reference/compilation.html ；https://www.usenix.org/conference/nsdi19（Hyperscan 论文）

### 14. Vectorscan（Hyperscan 便携 fork）
- 定位：VectorCamp 维护的便携化 Hyperscan（x86/ARM/POWER，SIMD 后端化），语义与模式限制继承上游。
- 可抄机制：①Rust 侧已有 WAF 项目将其作为可开关依赖（KrakenWaf：CLI 开关 + cargo feature 可选编译）——若 ADV 未来确需 PCRE 级表达力 + 流模式，这是唯一现成路径；②编译大库的成本管理（拆库、feature 门控）。
- 档位：watch（unsafe/C FFI 门槛高，先不进依赖树）
- 第一步动作：记录触发条件（如出现"必须支持流式扫描 + 万级模式"的硬需求）。
- 许可证/成熟度：BSD-3-Clause；维护中（社区驱动）。
- 链接：https://github.com/VectorCamp/vectorscan

### 15. RE2（线性时间哲学，对标）
- 定位：Google 的 C++ 线性时间正则引擎；与 Rust regex 同源思想（PikeVM + lazy DFA + bitstate backtracker + onepass NFA，无反向引用、基本无 lookaround）。
- 可抄机制：①"语义上阉割换复杂度保证"的产品化先例——ADV 规则文档可直接引用其取舍来教育规则作者；②Go regexp 同族（同样无反向引用）——大量真实代码库规则已在 RE2 语义下编写，ADV 兼容 RE2 语义即兼容最大规则存量。
- 档位：reference
- 第一步动作：把 ADV 规则语法声明为"RE2/regex 子集"，写进规则作者手册。
- 许可证/成熟度：BSD-3-Clause（Google）；维护中。
- 链接：https://github.com/google/re2

### 16. lazy DFA（hybrid DFA）机制
- 定位：regex-automata 的主力快引擎：按需惰性确定性化，缓存命中时每字节一次转移表查访。
- 可抄机制：①双预算旋钮：`dfa_size_limit`（缓存占用）与 `determinize_size_limit`（单次确定性化规模），超限自动降档到慢而线性的引擎，而非失败——"预算换速度、降级不降正确性"正是 ADV 匹配层要的模型；②锚定/反向匹配配反向 DFA（见条 18）。
- 档位：**吸收**
- 第一步动作：在 ADV 里对典型规则集测"缓存命中率 vs dfa_size_limit"曲线，定 ADV 默认值。
- 许可证/成熟度：随 regex-automata；维护中。
- 链接：https://docs.rs/regex-automata/latest/regex_automata/hybrid/index.html

### 17. onepass DFA + bounded backtracker + PikeVM（兜底层）
- 定位：三种"慢但保 captures/保线性"的引擎：onepass（无歧义 NFA，单线程存活/字节，能出捕获组）、bounded backtracker（(state,pos) 至多访一次，位图随 haystack 复位，仅短 haystack 划算）、PikeVM（万能兜底，O(n·m)，出全部捕获组）。
- 可抄机制：①onepass 的"歧义性检测即免确定性化"判据；②backtracker 的 visited 位图 = 把回溯的空间换时间限制成线性——ADV 自研任何匹配器时照抄该不变式；③meta 引擎对"短 haystack 才开 backtracker"的门控。
- 档位：**吸收**（直接用 regex-automata，不自研）
- 第一步动作：确认 ADV 用 `meta::Regex` 而非裸 PikeVM（避免把兜底当主力）。
- 许可证/成熟度：随 regex-automata；维护中。
- 链接：https://docs.rs/regex-automata/latest/regex_automata/nfa/thompson/backtrack/index.html 等

### 18. literal 预抽取 + prefilter 分层（rg / meta 引擎机制）
- 定位：ripgrep 快的核心：从 HIR 抽字面量 → 单字面量走 memmem、多字面量走 Aho-Corasick/Teddy → 只有候选命中才跑真引擎；BurntSushi《Regex engine internals as a library》（2023-07）系统化了这条管线。
- 可抄机制：①Extractor 四类产物（prefix/suffix/required/inner）对应四种 prefilter 接法（前缀扫、反向后缀扫、内缀锚定 + 双向 DFA 夹逼）；②"优化 literal sequence 的时间"本身要预算（同博客：去重→剪枝→排序各阶段）；③已知边界案例（rust-lang/regex#1046 字面量集合正确性、ripgrep#2884 大小写折叠下的 alternation 回归）——预过滤器必须与主引擎语义一致性测试。
- 档位：**吸收**
- 第一步动作：为 ADV 规则编译器加"字面量抽取报告"：每条规则输出提取到的字面量集合与预计 prefilter 类型。
- 许可证/成熟度：随 regex 家族；维护中。
- 链接：https://burntsushi.net/regex-internals/

## 四、专项

### 19. Unicode 大小写折叠/归一化的匹配成本
- 定位：regex-syntax 的 case-insensitive 是简单 case folding（CaseFolding.txt 派生类扩展），非全折叠、无 Turkic 规则；不做 NFC 归一化——规则作者要自己保证文本已归一。
- 可抄机制：①折叠把字符类扩张（如 k/K/K<sup>Kelvin</sup> 同类），状态数与 prefilter 字面量候选随之变化——ADV 编译期对"开了 case-insensitive 的规则"单独审计字面量抽取是否退化；②ASCII-only 快路径（`ascii_case_insensitive`）在代码扫描场景通常足够且便宜得多。
- 档位：**吸收**（作为规则编译选项与 lint）
- 第一步动作：lint：默认建议 `(?i)` 规则改用 `ascii_case_insensitive`，除非规则显式声明 Unicode 语义。
- 许可证/成熟度：随 regex-syntax；维护中。
- 链接：https://docs.rs/regex-syntax/latest/regex_syntax/ （hir/case folding 相关模块）

### 20. UTF-16/UTF-8 偏移换算（Windows/编辑器接口层）
- 定位：regex 系输出 UTF-8 字节偏移；LSP 3.17 前的默认 position encoding 是 UTF-16 code unit，Windows/VSCode 生态默认 UTF-16——非 BMP 字符（emoji 等）下两者错位（示例：一个 emoji 4 UTF-8 字节 vs 2 UTF-16 单元，Phpactor 博客实例）。
- 可抄机制：①握手协商：`general.positionEncodings` 里优先声明 `utf-8`，谈不拢才换算（LSP 3.17 规范；客户端未声明时按 UTF-16 假设）；②行索引结构：rust-analyzer 的 line-index / Ropey 的 `char_to_utf16_cu()`/`line_to_utf16_cu()` 提供 O(log n) 换算，避免每次 O(n) 重扫；③换算层只放一处（ADV 的报告/编辑器适配层），规则引擎内部永远只说字节偏移。
- 档位：**吸收**
- 第一步动作：在 ADV 报告序列化层定死"内部=字节偏移，出口=按协商编码换算"，配非 BMP 回归用例。
- 许可证/成熟度：ropey：MIT OR Apache-2.0（维护中）；line-index（rust-analyzer）：MIT OR Apache-2.0（维护中）；LSP：规范 3.17。
- 链接：https://microsoft.github.io/language-server-protocol/ （positionEncodings）；https://docs.rs/ropey ；https://github.com/rust-analyzer/line-index

### 21. ReDoS 防御与超时/步数预算（无回溯引擎下的意义）
- 定位：线性引擎消灭了"恶意输入触发指数回溯"一类（CWE-1333）；反面教材：Cloudflare 2019-07-02 全球 27 分钟 502，一条 WAF 正则（嵌套量词如 `.*.*.*`）在 PCRE 回溯上打满 CPU（官方 postmortem：~82% HTTP 流量受挫）。
- 可抄机制：①无回溯≠零预算：剩余风险是 O(n·m) 常数与编译期膨胀——预算应放在"编译 size_limit + lazy DFA 缓存上限 + 单文件扫描墙钟预算"，而不是匹配步数；②ADV 作为安全工具，自身扫描器用线性引擎是"御人者先自御"——规则文件来自远端仓库时，编译期预算（防超大规则库卡死 CLI）比运行期更重要；③对 ADV 检出的 ReDoS 类 finding，判据是"目标是否回溯型引擎"（PCRE/JS/Python/Go 受影响面不同，Go 也是线性语义——不能一刀切报高危，要绑引擎档位）。
- 档位：**吸收**
- 第一步动作：定 ADV 三层预算常量：规则编译 size_limit、DFA 缓存、每文件墙钟；超预算动作=降档+记录，不静默吞。
- 许可证/成熟度：规范/实践条目。
- 链接：https://blog.cloudflare.com/details-of-the-cloudflare-outage-on-july-2-2019 ；https://cwe.mitre.org/data/definitions/1333.html

### 22. ripgrep 整体架构（rg 对标）
- 定位：本域最完整的"字面量 prefilter + 自动机 + 行级装配"参照系；《Regex engine internals as a library》《ripgrep is faster than …》（2016）两篇是主线文档。
- 可抄机制：①"确定该不该跑真引擎"与"跑哪个引擎"分离成两级决策；②大小写不敏感 + 字面量组合的坑（ripgrep#2884）有现成回归语料可借。
- 档位：reference
- 第一步动作：把 rg 的 literal 优化 issue 清单导入 ADV 回归库。
- 许可证/成熟度：MIT OR Unlicense；维护中。
- 链接：https://blog.burntsushi.net/ripgrep/

### 23. Russ Cox 系列（Regular Expression Matching Can Be Simple And Fast）
- 定位：线性时间正则的原典（Thompson NFA/PikeVM 源流），regex/RE2/Go 三家共同的思想源头。
- 可抄机制：①"回溯是实现选择而非正则语义"的论证框架——写进 ADV 规则手册的理论背书；②NFA 模拟 vs 回溯的复杂度对照图示。
- 档位：reference
- 第一步动作：无（引用即可）。
- 许可证/成熟度：文章（2007 起系列，与 RE2 同源）。
- 链接：https://swtch.com/~rsc/regexp/

### 24. V8/irregexp 与线性化路线（watch 项）
- 定位：回溯型阵营的修补路线（2024 arXiv《Linear Matching of JavaScript Regular Expressions》等），说明"主流运行时保留回溯 + 旁路验证"的现实约束。
- 可抄机制：①对 JS/PCRE 目标做 ReDoS 判定时，注意引擎版本差异（V8 近年对部分模式线性化——报 finding 前先绑目标引擎与版本）；②ADV 检测规则的"引擎敏感度"标注。
- 档位：watch
- 第一步动作：ADV 规则模板加 `target_engine` 字段（回溯型才告警 ReDoS）。
- 许可证/成熟度：论文/工程实践。
- 链接：https://arxiv.org/abs/2402.07271 （Linear Matching of JavaScript Regular Expressions，2024）

---

## 五问回答（浓缩）

**① regex-automata 分层怎么选档（ADV 预算模型）**：选档树照 meta 引擎：字面量独占 → 只跑 prefilter；有必需字面量 → prefilter 门控后再进 DFA；尾部锚定 → 反向 DFA；需要捕获组且模式无歧义 → onepass；短 haystack → bounded backtracker；其余 PikeVM。预算三旋钮：编译 size_limit（防规则膨胀）、`dfa_size_limit`/`determinize_size_limit`（缓存换速度，超限自动降档不失败）、prefilter 开关。ADV 默认走 `meta::Regex` 全自动，只有热点规则才手动指定引擎。

**② fst 的直用价值**：符号表/规则标签表用 fst Map——有序键、共享前缀压缩、mmap 即开、流式 range/automaton 查询、stream 间 union/intersection；模糊查询配 fst-levenshtein（先封顶 d=1）。代价：构建要求键有序（增量更新需段式合并，参照 tantivy）。适合读多写少的 ADV 符号表；高频小改动场景收益要实测。

**③ literal 预抽取机制**：regex-syntax `hir::literal::Extractor` 在 HIR 上抽 prefix/suffix/required/inner 四类字面量集合（带 class/repeat/长度/总量预算旋钮）；单字面量→memmem，多字面量→aho-corasick（内部再可能走 Teddy）；有限语言时要求提取集"完整"（regex#1046）。ADV 规则编译器照此产出"prefilter 计划"，并配预过滤器与主引擎的一致性测试。

**④ UTF-16/UTF-8 偏移规范**：内部一律 UTF-8 字节偏移；出口（LSP/编辑器）在握手时优先协商 `utf-8`（LSP 3.17 positionEncodings），客户端只支持 UTF-16 时用行索引（line-index/ropey 内建 `char_to_utf16_cu` 等）做 O(log n) 换算；必须配非 BMP 字符回归用例（emoji 4 字节 vs 2 UTF-16 单元的经典错位）。

**⑤ 无回溯哲学对规则作者的影响**：写不出：反向引用（`\1`）、环视（lookaround）、以及一切"需数重复次数"的语义（如"重复单词"）。替代：反向引用→用多规则组合 + 捕获组后处理（在 ADV 引擎层做二次校验）；环视→锚定 `^`/`$`/`\b` + 字面量门控 + 引擎层后验条件；负向需求拆成"匹配 A 且不匹配 B"两条规则取差。规则手册要明说：ADV 规则语义 = RE2/regex 子集（条 15），fancy-regex 路线（条 6）仅在有实证需求时按有界方式引入。

---

*条目数：24。核查口径：WebSearch/WebFetch 于 2026-10-04（docs.rs、GitHub、Cloudflare postmortem、Hyperscan 官方文档、LSP 3.17 规范）；个别默认数值未逐项核实处已就地标注"以文档为准"。*
