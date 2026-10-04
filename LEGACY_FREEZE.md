# 旧版 ADV 停用说明（2026-08-24）

> ## ⚠️ 2026-10-04 磁盘复核：本文件的四条事实性陈述现已全部不可用（证据等级 S1，逐项实测）
>
> | 本文声称 | 现值 |
> |---|---|
> | v2 生效路径 `D:\开发\ADV` | **失效**。现址 `D:\KF\ADV`，分支 `adv-rewrite`（Rust `crates/` + Python 并存，见 `AGENTS-ADV.md`） |
> | v1 冻结路径 `D:\开发\ADV`（与上一行同路径，本自相矛盾） | **失效**。`D:\开发` 整树已不存在 |
> | E 盘旧副本 `E:\共享\51\ADV` | **失效**。E 盘可达，但该目录已不存在 |
> | `config.yaml` 已指向 `D:\开发\ADV\server.py` | **不可核**。仓内已无 `config.yaml`（全盘仅 `~/.qodersec/config.yaml`，与此无关） |
> | 旧库备份 `D:\开发\backups\ADV-20260824-040352.zip`（4836 文件/109MB） | **幸存但已改名、且位置危险**：现为 `D:\_数据恢复\其他\backups\unified-rx-mcp-20260824-040352.zip`（45.9 MB）。同目录另有 `unified-rx-v2-20260824-230022.zip`（0.5 MB） |
>
> 另有一份本文未提的较新备份：`D:\KF\backups\ADV-20260928-184809.zip`（75.7 MB，2026-09-28）。
>
> **建议（未执行）**：把 `_数据恢复\其他\backups\` 里这两份 zip 移到 `D:\KF\backups\`。`_数据恢复` 是恢复产物堆、处于会被清理的位置，而它们是目前**唯一的 08-24 冻结档**。下文"回滚"步骤照原文已无法执行。

## 现状

| 版本 | 路径 | 状态 |
|---|---|---|
| **v2（新）** | `D:\开发\ADV` | ✅ 生效（config.yaml 已指向） |
| v1（旧） | `D:\开发\ADV` | 🔒 冻结归档 |
| E 盘旧副本 | `E:\共享\51\ADV` | 🔒 冻结（不再同步） |

## 为什么停用旧版

1. **工具面爆炸**：183 工具（实际注入 200+）→ AI 上下文污染
2. **上帝文件**：server.py 7462 行，改一个工具动主文件
3. **环境自伤**：fs_write 授权剥离写不了文件、ide_edit_multi 0 应用、python 卡死
4. **功能重复**：5 套检索并存、文档 40+ 份

## 停用方式（已做）

- `config.yaml` 的 `mcp_servers.ADV` args 已指向 `D:\开发\ADV\server.py`
- 旧库已备份：`D:\开发\backups\ADV-20260824-040352.zip`（4836 文件/109MB）
- config 备份：`config.yaml.bak-v2-20260824-045453` / `config.yaml.bak-cg-20260824-051423`

## 重启 Hermes 后的预期

```
mcp_servers:
  ADV:   → v2（34 组合工具）
  codegraph:    → codegraph_explore（语义引擎）
```

## 回滚（如 v2 有问题）

1. 用备份恢复 config.yaml：`config.yaml.bak-v2-20260824-045453` → config.yaml
2. 重启 Hermes

## 旧版还有价值的东西（已提炼进 v2）

- 防幻觉闭环（hallucination_guard 三分级）→ v2 guard 域
- 多语言扫描（bug_scan/std_check/ui_check）→ v2 scan 域（增强）
- 教训引擎（lesson_recall）→ v2 learn 域
- 写完即验（dev_check 四连）→ v2 测试门槛
- codegraph（Yan Agent 内核）→ v2 engine 域 + 原生 MCP 接入
