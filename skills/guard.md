# guard 域（capability_manifest / hallucination_guard / breaker_status / breaker_reset）
- hallucination_guard：文件/符号声明 vs 真值校验（路径存在性、行号界内）
- S24 H2 口径：1379 条声明一致率 100%——但那是文件判定强项，语义维度只出 unverifiable
- **声明核查口径（S195 修复 + S196 设计修正）**：
  - 工具名：**只按在册闭集判**——精确在册 verified；近似拼写（difflib≥0.8，
    改名/笔误类幻觉）refuted 并点名最近在册工具。形态规则（多段 snake_case
    即疑似声明）**已退役**：328 份真实双臂答案实测它误杀 `build_terrain_collider`
    等代码标识符 712 次、真幻觉命中 0。其余小写词落「未核查·非声明小写词」
    （不静默、不冤判）。已知边界：近邻半径外的凭空造名不判
  - 文件：扩展名白名单扩至代码+文档+配置+头文件；白名单外的 `x.ext:NN` 引用落
    「未核查」不静默；行号须 1..N（`:0` 证伪）；Windows 盘符绝对路径可识别
  - 符号：限定扫描（限时/限数/跳垃圾目录，走沙盒钳制）——命中判 verified，
    语义是「字符串在本仓**出现**」（提及≠定义）；缺席落 unverifiable 附扫描范围
  - 输出带「根目录」回显：跨仓调用漏传 root 时能当场看出来，不再静默整片误判
  - **判定形状由金丝雀门钉死**（S195）：门链 `guard-gate` 步 + `spec/guard-corpus.json`
    逐条复算，多判/少判都红；语料期望锚外部观测（fixture 结构、真实答案分布）
    而非规则作者断言——教训在案：语料与代码同一只手写，规则级错误会连语料一起错
  - **路由名字有账**（S197）：`_INTENTS`/`_CAPABILITIES`/README 工具面三张镜像面的
    工具名经门链 `name-ledger` 步对在册闭集复算，不在册且无豁免即红（豁免须填 why）
  - H2 评测口径补「refuted 精确率」并在报告里记录沙盒态（受限沙盒下大量声明
    落 unverifiable，一致率含义不同）
- **breaker_status（S122，S131 自 meta 域归位）**：工具熔断状态——窗口内重复调用计数、
  已熔断的 key、阈值与旁路开关。同一（工具+参数）在窗口内调用超过 limit 次即熔断
- **breaker_reset（S122，S131 自 meta 域归位）**：复位熔断（给 `tool` 只复位该工具；
  缺省全清）

## 工具熔断（S122）速查

| 项 | 值 |
|---|---|
| 触发 | 同一（工具 + 规范化参数 + cursor）在 `WINDOW_S` 内调用 **> LIMIT** 次 |
| 空转 | 同一 key 连续返回**逐字节相同**的结果达 LIMIT 次 → 提前熔断 |
| 默认 | LIMIT=10 / WINDOW_S=300 / COOLDOWN_S=120（env `UNIFIED_RX_BREAKER_*` 覆盖） |
| 旁路 | `UNIFIED_RX_BREAKER=off` |
| 恢复 | 冷却到期自动恢复；**改参数/换目标即刻恢复**（key 变了）；`breaker_reset` 复位 |
| 豁免 | `breaker_status` / `breaker_reset` 自身永不熔断（救火的人不能被火拦住） |

定位：**循环刹车，不是安全边界**——拦重复，不拦"每次换参数的穷举"。宿主级同规则
插件见 `plugins/urx-marketplace/zcode-breaker`（ZCode Hook，覆盖所有工具）；
设计取舍见 `spec/BREAKER.md`。
