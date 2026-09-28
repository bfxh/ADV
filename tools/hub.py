"""tools/hub.py —— 平台域（S176，spec/HUB.md §一/§六）：4 工具。

本质：平台能力只实现一次（hub_core/hub_runner），MCP 工具面是智能体的调用入口，
server_web.py 是团队的第二张脸——两条通道走同一份实现与同一本账。

- hub_status    平台状态（实例/版本/用户数/账本链/降级原因）——**不报绿带病**；
- hub_pipelines 管线清单（含非法条目如实列出，不静默跳过）；
- hub_runs      运行账本（判定/封条/指纹/how_to_verify 独立复核命令）；
- hub_run       触发运行（需授权；独占护栏内顺序执行，日志落盘+哈希入账）。
"""
import os

import hub_auth
import hub_core
import hub_propose as hub_propose_mod
import hub_runner
from registry import tool


def instance_name() -> str:
    return (os.environ.get("UNIFIED_RX_HUB_INSTANCE") or "dev").strip().lower()


def _degraded_reasons(invalid: dict[str, str], chain: dict) -> list[str]:
    reasons: list[str] = []
    if not chain.get("ok"):
        reasons.append(f"账本链校验失败：{chain.get('reason')}（index={chain.get('broken_index')}）")
    reasons.extend(f"manifest 非法 {k}: {v}" for k, v in sorted(invalid.items()))
    return reasons


@tool("hub_status",
      "平台状态：实例/版本/用户数/账本链校验/降级原因（degraded 时不带病报绿）",
      "hub", {"type": "object", "properties": {}, "required": []})
def hub_status():
    pipes, invalid = hub_core.load_pipelines()
    chain = hub_core.verify_chain()
    reasons = _degraded_reasons(invalid, chain)
    return {
        "ok": True, "instance": instance_name(), "version": hub_core.server_version(),
        "pipelines": len(pipes), "invalid_manifests": invalid,
        "users": hub_auth.count_users(), "chain": chain,
        "degraded": bool(reasons), "degraded_reasons": reasons,
        "data_root": str(hub_core.runs_path().parent),
        "log_dir": str(hub_core.logs_dir()),
        "max_shared": hub_core.max_shared(),
        "active": hub_core.read_active(),
    }


def _pipeline_row(m: dict) -> dict:
    return {"id": m["id"], "title": m["title"], "when": m["when"],
            "resource_class": m["resource_class"], "on": m["on"],
            "steps": [st["step"] for st in m["steps"]],
            "fingerprint": hub_core.manifest_fingerprint(m)}


@tool("hub_pipelines",
      "管线清单：manifest（id/标题/触发/资源级/步骤数）与非法条目如实列出",
      "hub", {"type": "object", "properties": {}, "required": []})
def hub_pipelines():
    pipes, invalid = hub_core.load_pipelines()
    return {"ok": True, "pipelines": list(map(_pipeline_row, pipes.values())),
            "invalid": invalid}


@tool("hub_runs",
      "运行账本尾 N 条：判定/封条/指纹/独立复核命令（how_to_verify）；含 SKIP 的只算 green_with_skips",
      "hub", {"type": "object",
              "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 200}},
              "required": []})
def hub_runs(limit=10):
    try:
        n = int(limit)
    except (TypeError, ValueError):
        n = 10
    rows = hub_core.latest_runs(max(1, min(n, 200)))
    return {"ok": True, "count": len(rows), "runs": rows,
            "chain": hub_core.verify_chain()}


@tool("hub_run",
      "触发管线运行（需授权）：独占护栏内顺序执行，原始日志落盘+哈希入账，返回判定与 how_to_verify",
      "hub", {"type": "object",
              "properties": {"pipeline": {"type": "string", "description": "管线 id（见 hub_pipelines）"}},
              "required": ["pipeline"]},
      requires_auth=True)
def hub_run(pipeline):
    pipes, invalid = hub_core.load_pipelines()
    if pipeline not in pipes:
        return {"error": f"未知管线 {pipeline!r}", "available": sorted(pipes),
                "invalid": invalid}
    return hub_runner.run_pipeline(pipes[pipeline], actor="mcp", trigger="manual")


@tool("hub_propose",
      "修复提案（需授权，G1）：隔离树里跑白名单机械修复（ruff --fix / ruff format / cargo fmt），"
      "只出 patch 与落地建议——**永不改主树、永不提交/合并**",
      "hub", {"type": "object",
              "properties": {
                  "checks": {"type": "array", "items": {"type": "string"},
                             "description": "从 FIX_CHECKS 里选名字，如 [\"ruff-fix\"]"},
                  "timeout_s": {"type": "integer", "minimum": 30, "maximum": 3600}},
              "required": ["checks"]},
      requires_auth=True)
def hub_propose(checks, timeout_s=600):
    if not isinstance(checks, list) or not all(isinstance(c, str) for c in checks):
        return {"error": "checks 必须是字符串数组", "available": sorted(hub_propose_mod.FIX_CHECKS)}
    try:
        ts = int(timeout_s)
    except (TypeError, ValueError):
        ts = 600
    return hub_propose_mod.propose(checks, max(30, min(ts, 3600)))
