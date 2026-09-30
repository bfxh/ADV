# guard 域（capability_manifest / hallucination_guard / breaker_status / breaker_reset）
- hallucination_guard：文件/符号/工具名声明 vs 真值校验（存在性、行号 `1..N`、符号有界全仓检索）。
  **S195 起"疑似才判"**：反引号小写词只有在册 / 命名形态（下划线分段）/ 与在册名编辑距离 ≤2 时
  才判工具名，其余落 `skipped.not_a_tool_claim`（曾把 `` `git` `` 判成"工具不存在"并给整段文本扣
  "存在幻觉"）；未覆盖扩展名落 `unverifiable` + `skipped.unchecked_ext`（**不静默丢**）；
  符号**真查**（命中⇒verified / 扫完未命中⇒refuted / 达上限⇒unverifiable）。
  **`root` 讨论别的项目必须给**——缺省用服务进程 cwd，会给出一整批假 refuted
- **H2 口径警告（S195）**：历史一致率数字（S24 1379 条 100%；A 臂 1.0 / B 臂 0.9295）是在
  `UNIFIED_RX_SANDBOX=*` 下取得、且**只统计 file 分支**——生产 fail-closed（沙盒外声明一律
  unverifiable）与符号/工具名分支当时的缺陷都没进那个数字。补注见 spec/EVAL.md §7
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
