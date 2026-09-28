# hub 域 —— 平台层（S176，设计档 [../spec/HUB.md](../spec/HUB.md)）

> 域轴：**平台层**（管线编排 + 运行账本 + 触发面）。与"工具箱"定位的关系：能力仍然只有
> 一份（`hub_core`/`hub_runner`），**MCP 工具面是智能体的入口，网页是团队的第二张脸**——
> 两条通道共用授权门/沙盒/熔断/审计，不各写一套。

## 工具契约（4 件）

| 工具 | 说明 |
|---|---|
| `hub_status` | 实例（stable/dev）· 版本 · 用户数 · 账本链校验 · **降级原因**（链坏/非法 manifest）。**不带病报绿**：`degraded=true` 时必须先处理原因 |
| `hub_pipelines` | 管线清单（id/标题/触发/资源级/步骤/指纹）；**非法 manifest 如实列出**，不静默跳过 |
| `hub_runs` | 账本尾 N 条（判定/封条/指纹/独立复核指引）。`verdict`：`green` / `green_with_skips` / `red` |
| `hub_run` | **需 `__authorized`**：触发运行；返回判定、封印、`verify_plan`（机器可执行复核计划）与 `how_to_verify`（人读指引） |

## 里子（机制与纪律）

- **manifest 严格校验**：未知字段即拒；id 形如 `域.场景` 且与文件名一致；步骤 1..16；
  `timeout_s` 1..7200；**命令首段白名单**（python/python3/py/cargo/git/node）——平台不提供
  任意命令面（与 `local_run` 白名单同精神）；manifest 本体住受版本控制的 `pipelines/`。
- **绿的定义收紧**：`green` = 全部必跑步 exit 0 **且 skipped_lines == 0**；任何 SKIP 行
  只算 `green_with_skips`（诚实降级，**不是绿**）。退出码取自 OS，不经中间解释层。
- **账本**：`runs.jsonl` append-only + sha256 链（`prev_hash`/`hash`，篡改即链断）；
  每条 final 行带**判定封印** `seal`（同输入同封印）；**原始日志落盘 + sha256 入账**。
- **资源级准入**：`resource_class` 真生效——`exclusive` 与**任何**活跃运行互斥；`shared`
  之间可并行（上限 `UNIFIED_RX_HUB_MAX_SHARED`，默认 2），且不得与 exclusive 并存。
  护栏 = 活跃目录"一运行一文件"（O_CREAT|O_EXCL + pid 存活探测 + 陈旧清理），
  **检查与创建在同一临界区内**（目录锁，防 TOCTOU 超卖）。被拒返回 `busy` + 原因 +
  当前活跃集（可诊断）；**阻塞式排队未做**（先量后改，有真实需求再说）。
- **防骗判据**（`python -X utf8 scripts/hub_gate.py`，进本地门 + CI）：平台跑↔直跑 parity
  （退出码 + stdout 逐字节一致）· 金丝雀三态（必绿/必红/含 SKIP 不算绿）· 篡改注入必红 ·
  指纹八项齐 · 复核计划真执行（日志哈希重算 + 直跑）· 授权矩阵（坏令牌/角色/MCP 无授权）。
  独立复核入口：`--verify-chain` / `--seal <run_id>`。

## 网页面与身份

- `python -X utf8 server_web.py`（缺省端口：stable 7741 / dev 7742；只绑 `127.0.0.1`）。
  读面开放（回环）；**写面（POST /api/runs）要求 `Authorization: Bearer <user>:<token>`**
  且角色 ∈ {operator, admin}。首跑自举 `admin`，令牌**只在 stderr 打印一次**（表内只存
  pbkdf2 哈希；坏表 fail closed，不重建）。
- 触发一次运行记 `actor`（`mcp` / `web:<user>` / `cli` / `gate`），与 S172 的智能体归因同账。

## 何时不要用

- 想要"随便跑条命令"→ 用 `local_run`（白名单模板），或先加 manifest（要走受控管线）。
- 想要看代码质量/安全结论 → 那些是 scan/attack/appaudit 域；hub 只负责"把已有门跑起来 +
  把账记清"，**不解释证据**（裁决在智能体）。
