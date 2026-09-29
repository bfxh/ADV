"""hub_resilience.py —— 方向⑥ 平台韧性判据体（S183 建立 · S187 续）。

**分工**（与 `hub_manifest.meta_facts` / `hub_vc.selfcheck_facts` 同一规矩）：
判据的**事实**（注入 + 读数）住这里，**判定口径**（全真才绿 + 逐项可读）由 `hub_gate`
统一负责——判定格式只实现一次。

**为什么另立模块**：每加一轮 ⑥ 判据就要给门加行，而 god 门棘轮对 `scripts/hub_gate.py`
只准"最长函数不涨 + 文件涨幅 ≤10%"。把判据**体**外置，门只留判据表。

两条，都是"存储坏了，平台必须如实说"：
- `write_fail_facts`（S183）：账本不可写 / 日志目录不可写 / 好路径对照；
- `truncation_facts`（S187 续）：**账本被写坏一半**——S183 修的是"**不可读**"，这条修的是
  "**读得出来但写坏了**"。
"""
from __future__ import annotations

import contextlib
import json
import os
import pathlib
import shutil
import sys
import tempfile

import hub_core
import hub_runner

_KEYS = ("UNIFIED_RX_HUB_ROOT", "UNIFIED_RX_HUB_PIPELINES")


def _manifest(code: str) -> dict:
    """判据夹具：一条 exclusive 管线（命令首段必须在 manifest 白名单内）。"""
    return {"id": "adv.ok", "title": "t", "when": "t", "resource_class": "exclusive",
            "on": ["manual"],
            "steps": [{"step": "s", "cmd": [sys.executable, "-X", "utf8", "-c", code],
                       "timeout_s": 60}]}


@contextlib.contextmanager
def _isolated(root: pathlib.Path):
    """把数据根/管线目录指向临时处，退出即还原——判据**绝不碰真实账本**。"""
    saved = {k: os.environ.get(k) for k in _KEYS}
    os.environ["UNIFIED_RX_HUB_ROOT"] = str(root)
    os.environ["UNIFIED_RX_HUB_PIPELINES"] = str(root.parent / "pipes")
    try:
        yield
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def write_fail_facts() -> dict[str, bool]:
    """S183 混沌：两条写故障注入 + 一条对照（好根仍绿——混沌不得把好路径一起判死）。"""
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-chaos-"))
    pipes = tmp / "pipes"
    pipes.mkdir(parents=True)
    (pipes / "adv.ok.json").write_text(json.dumps(_manifest("print('chaos')")),
                                       encoding="utf-8")
    out: dict[str, bool] = {}
    try:
        with _isolated(tmp / "r1"):                    # ① 账本不可写（位置被目录占位）
            (tmp / "r1" / "runs.jsonl").mkdir(parents=True)
            pm, _inv = hub_core.load_pipelines()
            r1 = hub_runner.run_pipeline(pm["adv.ok"], actor="gate", trigger="chaos")
            out["ledger_rejected"] = r1.get("ok") is False
            out["no_leak_1"] = hub_core.read_active() == []
            out["no_fake_chain_ok"] = hub_core.verify_chain()["ok"] is False
        with _isolated(tmp / "r2"):                    # ② 日志不可写（目录被文件占位）
            (tmp / "r2").mkdir(parents=True, exist_ok=True)
            (tmp / "r2" / "logs").write_text("occupied", encoding="utf-8")
            r2 = hub_runner.run_pipeline(pm["adv.ok"], actor="gate", trigger="chaos")
            out["logs_red"] = r2.get("ok") is False
            out["no_leak_2"] = hub_core.read_active() == []
        with _isolated(tmp / "r3"):                    # ③ 对照：好根仍绿
            r3 = hub_runner.run_pipeline(pm["adv.ok"], actor="gate", trigger="chaos")
            out["happy_green"] = r3.get("ok") is True and r3.get("verdict") == "green"
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return out


def truncation_facts() -> dict[str, bool]:
    """S187 续：**账本被写坏一半** ⇒ 链复核不许报 OK（含"恢复即回绿"对照）。

    缺口实录（修前）：末行截断时 `verify_chain` 报 **`ok=True`**——截断行被解析器丢掉，剩下
    的前缀是自洽的，于是"账本被写坏"与"那行从没写过"**不可区分**，且那条运行永远停在
    `start`（看着像还在跑，而不是"账本坏了"）。中间行整条丢失反而一直能被 `prev_hash` 抓到。

    注入方式 = **直接改账本文件**，不用 chmod 也不造盘满：Windows 上 chmod 不可靠、盘满本机
    无法诚实造出；而"盘满"在本代码里的**可观测后果正是写到一半** ⇒ 直接注入截断更严格。
    """
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="adv-trunc-"))
    (tmp / "pipes").mkdir()
    out: dict[str, bool] = {}
    try:
        with _isolated(tmp / "hub"):
            hub_core.append_row({"id": "r-g", "phase": "start"})
            hub_core.append_row({"id": "r-g", "phase": "final", "verdict": "green"})
            led = hub_core.runs_path()
            whole = led.read_text(encoding="utf-8")
            out["baseline_green"] = hub_core.verify_chain()["ok"] is True
            led.write_text(whole.rstrip("\n")[:-12], encoding="utf-8")        # ① 末行砍尾
            out["truncated_tail_red"] = hub_core.verify_chain()["ok"] is False
            led.write_text("\n".join(whole.rstrip("\n").split("\n")[1:]) + "\n",
                           encoding="utf-8")                                 # ② 中间行整条丢
            out["missing_middle_red"] = hub_core.verify_chain()["ok"] is False
            led.write_text(whole, encoding="utf-8")
            out["restored_green"] = hub_core.verify_chain()["ok"] is True     # ③ 仪器非恒红
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return out
