# hub 域 —— 平台层（S176，设计档 [../spec/HUB.md](../spec/HUB.md)）

> 域轴：**平台层**（管线编排 + 运行账本 + 触发面）。与"工具箱"定位的关系：能力仍然只有
> 一份（`hub_core`/`hub_runner`），**MCP 工具面是智能体的入口，网页是团队的第二张脸**——
> 两条通道共用授权门/沙盒/熔断/审计，不各写一套。

## 工具契约（4 件）

| 工具 | 说明 |
|---|---|
| `hub_status` | 实例（stable/dev）· 版本 · 用户数 · 账本链校验 · **存储可写性**（`storage_writable`）· **降级原因**（链坏/不可读/不可写/非法 manifest）。**不带病报绿**：`degraded=true` 时必须先处理原因 |
| `hub_pipelines` | 管线清单（id/标题/触发/资源级/步骤/指纹）；**非法 manifest 如实列出**，不静默跳过 |
| `hub_runs` | 账本尾 N 条（判定/封条/指纹/独立复核指引）。`verdict`：`green` / `green_with_skips` / `red` |
| `hub_run` | **需 `__authorized`**：触发运行；返回判定、封印、`verify_plan`（机器可执行复核计划）与 `how_to_verify`（人读指引） |
| `hub_propose` | **需 `__authorized`（G1）**：隔离工作树里跑白名单机械修复（`ruff-fix` / `ruff-format` / `cargo-fmt`），只出 `patch` + 落地建议——**永不改主树、永不提交/合并** |
| `hub_impact` | **只读**：变更（`git diff base..HEAD` ∪ 未提交）→ import **反向闭包** → 建议测试集（保守，宁多跑不误跳）；不可信即 `fallback=full` + 理由 |

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
- **鉴权分层（S179）**：开放 = 控制台外壳 + `/api/status` **摘要态**（未登录也看得见平台
  是否降级）+ 登录端点；**读面要会话**（`POST /api/login` → Cookie `hub_sid`，
  HttpOnly + SameSite=Strict）；`/api/users*` 要 admin；触发 = 会话（operator/admin）
  **或** `Authorization: Bearer <user>:<token>`（脚本/智能体兼容面）。
- **用户管理（admin）**：`GET/POST /api/users`、`POST /api/users/role`、
  `DELETE /api/users/<name>`；令牌**只此一次返回**（表内只存 pbkdf2 哈希）；
  **最后 admin 不可删/降**（防锁死）；管理动作入账本（`kind=admin`，链保护）。
- 首跑自举 `admin`，令牌**只在 stderr 打印一次**；坏用户表 fail closed（不重建）。
- 触发一次运行记 `actor`（`mcp` / `web:<user>` / `cli` / `gate`），与智能体归因同账。

## 修复提案（G1 的机器版本）

`hub_propose(checks)` → `git worktree add --detach` 造**隔离树** → 在白名单里跑机械修复 →
`git diff` 即提案 → 移除隔离树。返回值含 `patch` / `files` / `checks` / `branch_suggestion`
/ `apply_hint` / `unchanged_main`（**主树逐位不变**，防骗门 `propose-isolation` 判据守）。
`checks` **只接受白名单名字**（不提供任意命令面）；隔离树基于 **HEAD**（脏树上的未提交改动
不进提案）。落地永远由人/智能体走 `git apply` + 门禁 + 人审——平台不碰合并。

## 静态选测（方向①：语义 diff）

`hub_impact(base)` → 变更集 → **反向 import 闭包**（谁依赖我，传递）→ `tests/` 下受影响的
测试 + **图上无记录的测试一律纳入**（保守）。**与既有能力的分工**：`tia` 是动态口径（要"上次
跑过"才有依赖集、状态在进程内），`ide_impact` 是单符号级（LSP 三级），本件是**变更驱动的静态
口径**（不依赖历史、可跨进程复算）。不确定（非 git 仓 / 解析失败率过高）⇒ `fallback="full"`
+ 理由，**不假装选过**。

## 何时不要用

- 想要"随便跑条命令"→ 用 `local_run`（白名单模板），或先加 manifest（要走受控管线）。
- 想要看代码质量/安全结论 → 那些是 scan/attack/appaudit 域；hub 只负责"把已有门跑起来 +
  把账记清"，**不解释证据**（裁决在智能体）。
