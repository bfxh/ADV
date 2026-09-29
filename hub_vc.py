"""hub_vc.py —— 构建凭证形状（S187，spec/FRONTIER-CI.md 方向⑤，判定：**有界**）。

本仓的"可验证"内核已经在：判定封印 `seal`（同输入同封印）+ 账本 sha256 链（篡改即断链）
+ `verify_plan`/`how_to_verify`（可独立重算）。缺的是**非对称签名**——纯 stdlib 无 Ed25519，
`cryptography` 属第三方（违零依赖红线）⇒ 本档**不假装**：`proof.cryptosuite` 如实写
`unsigned`，并写明"内容寻址 ≠ 签名"（能验「内容未被改」，不能验「谁签的」）。

凭证是**账本最终行的纯函数**（同输入同凭证）：谁拿到 runs.jsonl 都能独立重算，不依赖平台、
不依赖运行期内存 —— `python -X utf8 scripts/hub_gate.py --credential <run_id>`。

形状（vc-shape/1）：签发者 / 签发时刻 / 源码哈希 / 环境哈希 / 轨迹 / 封条 / 账本锚 / 证明档。
刻意**不套** W3C VC 的 `@context`/`type` 规范值：那是另一套语义，照抄会让人误以为本凭证能
在 VC 生态里验证——我们不假装。

**凭证的信任边界**（写在这里免得被误读）：它只能证明"账本里这条记录一字未改"。账本本身是
本机的 append-only 文件，签发者身份**未经任何密码学绑定**——所以它挡的是"事后改记录"，
不挡"一开始就伪造记录"。
"""
from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

import hub_core
import hub_runner

CRED_TYPE = "ADVBuildCredential"
CRED_SPEC = "vc-shape/1"
# 轨迹字段必须与**封印载荷**同一口径（`hub_runner.run_pipeline` 的 payload）：两边一旦
# 分叉，"凭证里的读数"与"封条盖住的读数"就不是同一份东西了（S187 实锤过一次：门里的
# 复核入口曾用 S177 前的旧键 `log_sha256`，于是所有现行运行的封条复核都假红）。
TRACE_KEYS = ("step", "exit", "out_sha256", "err_sha256")
_SHAPE_KEYS = ("type", "spec", "issuer", "issued_at", "subject", "seal", "ledger", "proof")
_GATE_SCRIPT = pathlib.Path(__file__).resolve().parent / "scripts" / "hub_gate.py"

PROOF = {
    "type": "none",
    "cryptosuite": "unsigned",
    "reason": "纯 stdlib 无 Ed25519（零依赖红线）：本凭证是内容寻址 + 链条锚定，不是签名"
              "——能验「记录未被改」，不能验「谁签的」",
}


def digest(cred: dict) -> str:
    """凭证摘要 = 规范 JSON 的 sha256；复用封印口径（同一把尺，不另起一套哈希）。"""
    return hub_runner.seal_of({k: v for k, v in cred.items() if k != "sha256"})


def seal_payload(row: dict) -> dict:
    """封条载荷，按**记录代次**取步骤字段（与 `TRACE_KEYS` 同住 = 凭证轨迹与封条同口径）。

    S177 把单流日志拆成 stdout/stderr 两路 ⇒ 步骤字段从 `log_sha256` 改名成
    `out_sha256`+`err_sha256`。封条是**当时的**载荷算出来的，所以复核必须按行里**实际有**
    的键取：S187 修掉的缺陷就是一律用旧键，导致**所有现行运行**的封条复核假红。
    """
    steps = row.get("steps") or []
    keys = ("step", "exit", "log_sha256") if (steps and "log_sha256" in steps[0]) \
        else TRACE_KEYS
    fp = row.get("fingerprint") or {}
    return {"pipeline": row.get("pipeline"),
            "manifest_sha256": fp.get("manifest_sha256"),
            "git_head": fp.get("git_head"), "git_dirty": fp.get("git_dirty"),
            "verdict": row.get("verdict"),
            "steps": [{k: s.get(k) for k in keys} for s in steps]}


def build(row: dict) -> dict:
    """账本最终行 → 凭证（**纯函数**：同输入同凭证，跨进程/跨机可重算）。"""
    fp = row.get("fingerprint") or {}
    cred = {
        "type": CRED_TYPE, "spec": CRED_SPEC,
        "issuer": {"actor": row.get("actor") or "", "trigger": row.get("trigger") or ""},
        "issued_at": int(row.get("ts") or 0),
        "subject": {
            "run_id": row.get("id"), "pipeline": row.get("pipeline"),
            "verdict": row.get("verdict"),
            "source": {"git_head": fp.get("git_head"), "git_dirty": fp.get("git_dirty"),
                       "manifest_sha256": fp.get("manifest_sha256")},
            "environment": {"python": fp.get("python"), "cargo": fp.get("cargo"),
                            "env_keys_hash": fp.get("env_keys_hash"), "cwd": fp.get("cwd")},
            "trace": [{k: s.get(k) for k in TRACE_KEYS} for s in row.get("steps") or []],
        },
        "seal": row.get("seal"),
        "ledger": {"hash": row.get("hash"), "prev_hash": row.get("prev_hash")},
        "proof": dict(PROOF),
    }
    cred["sha256"] = digest(cred)
    return cred


def for_rows(rows: list[dict]) -> dict[str, dict]:
    """账本行 → {run_id: 凭证}；只认**已封印的 final 行**。

    构建失败**不静默丢**：回一个 `ok=False` 条目——"还没有凭证"（start 行）与"凭证构建坏了"
    必须看得出区别，否则坏凭证会被当成不存在。
    """
    out: dict[str, dict] = {}
    for row in rows:
        rid = row.get("id")
        if not isinstance(rid, str) or row.get("phase") != "final" or not row.get("seal"):
            continue
        try:
            out[rid] = build(row)
        except (TypeError, ValueError) as exc:
            out[rid] = {"ok": False, "error": f"凭证构建失败：{exc}"}
    return out


def final_row(run_id: str) -> dict:
    """按 run id 取**最终行**（独立复核入口用）；找不到回空表（调用方如实报 NOT_FOUND）。"""
    for row in hub_core.read_rows():
        if row.get("id") == run_id and row.get("phase") == "final":
            return row
    return {}


def credential_facts(row: dict) -> dict[str, bool]:
    """方向⑤ 的判据**事实**（纯函数；判定口径由门统一负责）。"""
    cred = build(row)
    subj = cred["subject"]
    return {
        "shape_complete": all(cred.get(k) for k in _SHAPE_KEYS) and bool(subj["trace"])
        and bool(cred["issuer"]["actor"]),
        "rows_surface": for_rows([row]).get(str(row.get("id")), {}) == cred,
        "trace_matches_seal": subj["trace"] == seal_payload(row)["steps"],
        "tamper_changes": build({**row, "verdict": "hacked"})["sha256"] != cred["sha256"],
        "honest_unsigned": cred["proof"] == PROOF,
    }


def _gate_cli(*args: str) -> subprocess.CompletedProcess:
    """**独立复核入口的真回放**：再跑一次本门脚本（子进程 ⇒ argv 接线、sys.path、
    导入副作用都在内）。S187 修的那个缺陷（复核入口用旧键 ⇒ 现行运行全假红）正是靠这条
    路径才被抓住——仓内单测当时用的是正确键，看不见。"""
    return subprocess.run([sys.executable, "-X", "utf8", str(_GATE_SCRIPT), *args],
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", cwd=str(_GATE_SCRIPT.parent.parent),
                          shell=False, timeout=120)


def selfcheck_facts() -> dict[str, bool]:
    """方向⑤ 的判据事实（**自给自足**：临时根里真跑一次绿灯，再回放两个复核入口）。

    为什么要临时根而不是借用门里的金丝雀：`tamper-detect` 判据会**篡改**金丝雀账本，
    借它=把自己的判据排在别人的篡改之后（S187 实锤：`--seal` 回归判据因此假红）。
    判据之间不许有这种隐式时序依赖。
    """
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-vc-"))
    pipes = tmp / "pipelines"
    pipes.mkdir(parents=True)
    (pipes / "adv.vc.json").write_text(json.dumps({
        "id": "adv.vc", "title": "t", "when": "t", "resource_class": "exclusive",
        "on": ["manual"],
        "steps": [{"step": "s", "cmd": [sys.executable, "-X", "utf8", "-c", "print('vc')"],
                   "timeout_s": 60}]}), encoding="utf-8")
    saved = {k: os.environ.get(k) for k in ("UNIFIED_RX_HUB_ROOT", "UNIFIED_RX_HUB_PIPELINES")}
    try:
        os.environ["UNIFIED_RX_HUB_ROOT"] = str(tmp / "hub")
        os.environ["UNIFIED_RX_HUB_PIPELINES"] = str(pipes)
        pm, _inv = hub_core.load_pipelines()
        res = hub_runner.run_pipeline(pm["adv.vc"], actor="gate", trigger="canary")
        rid = res["run_id"]
        out = {"run_green": res.get("verdict") == "green"}
        out.update(credential_facts(final_row(rid)))
        cli = _gate_cli("--credential", rid)
        out["credential_cli_agrees"] = cli.returncode == 0 \
            and build(final_row(rid))["sha256"] in cli.stdout
        out["seal_cli_ok"] = _gate_cli("--seal", rid).returncode == 0
        return out
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        shutil.rmtree(tmp, ignore_errors=True)
