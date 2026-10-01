"""handoff_gate.py —— 会话交接卡门（S200）。

**为什么**：并行长会话靠「继续」链接续，任务上下文只活在 session sqlite/artifacts 里
——实测重建一次意图要翻 423 条消息 + mimosa 报告考古。本门把交接变成**仓内账**：

1. **形状**：`spec/handoff.json` 必备 round/as_of/branch/next/open；next 非空；
2. **轮次对账**：round 必须已在 spec/ROUNDLOG.md 记账（ghost 轮次=完整性红，
   `--allow-stale` 不豁免——同 audit-ledger 的 S199 口径）；且落后 ROUNDLOG 最新轮
   ≤ MAX_BEHIND（默认 1）——ROUNDLOG 推进而卡不更新即红（节奏类，可显式放行）；
3. **锚可达**：next/open 里带路径分隔符的 `x.y/z.(py|md|json|yml|toml|rs)` 锚必须
   在仓内真实存在——ghost 锚与 ghost commit 同罪，不可放行（"下一步"指向被改名/
   删除的文件，等于交接卡自己在撒谎）。

用法：python -X utf8 scripts/handoff_gate.py [--allow-stale]
env：UNIFIED_RX_HANDOFF（卡路径）/ UNIFIED_RX_HANDOFF_LOG / UNIFIED_RX_HANDOFF_MAX_BEHIND
——金丝雀靠 env 自带输入（同 audit-ledger 实测教训：靠仓内状态的金丝雀会假绿）。
"""
from __future__ import annotations

import datetime
import json
import os
import pathlib
import re
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
CARD_KEYS = ("round", "as_of", "branch", "next", "open")
ANCHOR_RE = re.compile(r"[A-Za-z0-9_.\\/-]+\.(?:py|md|json|yml|yaml|toml|rs)(?![A-Za-z0-9_.\\/-])")


def _load_card(path: pathlib.Path, problems: list[str]) -> dict[str, Any]:
    if not path.is_file():
        problems.append(f"交接卡缺失：{path}（门被清空即失效）")
        return {}
    try:
        card: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        problems.append(f"交接卡坏 JSON：{e}")
        return {}
    problems.extend(f"交接卡缺字段 {key}" for key in CARD_KEYS if key not in card)
    nxt = card.get("next") or []
    if not isinstance(nxt, list) or not [x for x in nxt if str(x).strip()]:
        problems.append("next 为空——「下一步锚」是本卡存在的意义")
    return card


def _rounds(log: pathlib.Path) -> list[int]:
    if not log.is_file():
        return []
    return [int(m) for m in re.findall(r"^## S(\d+)\b", log.read_text(encoding="utf-8"),
                                       re.MULTILINE)]


def check_card(card: dict[str, Any], log: pathlib.Path, problems: list[str]) -> None:
    rounds = _rounds(log)
    if not rounds:
        problems.append(f"ROUNDLOG 提取不到轮次（账目失联）：{log.name}")
        return
    raw = str(card.get("round", ""))
    m = re.fullmatch(r"S(\d+)", raw)
    if not m:
        problems.append(f"round『{raw}』非 S<数字> 形状")
        return
    r = int(m.group(1))
    if r not in rounds:
        problems.append(f"round『{raw}』未在 ROUNDLOG 记账（ghost 轮次——先记 ROUNDLOG 再推卡；"
                        "--allow-stale 不豁免）")
    try:
        datetime.date.fromisoformat(str(card.get("as_of", "")))
    except ValueError:
        problems.append(f"as_of 非 ISO 日期：{card.get('as_of')}")
    max_behind = int(os.environ.get("UNIFIED_RX_HANDOFF_MAX_BEHIND", "1"))
    lag = max(rounds) - r
    if lag > max_behind:
        problems.append(f"[LAG] 卡停在 {raw}，ROUNDLOG 已到 S{max(rounds)}（滞后 {lag} > {max_behind}）")


def check_anchors(card: dict[str, Any], root: pathlib.Path, problems: list[str]) -> None:
    lines = [str(x) for x in (card.get("next") or [])] + [str(x) for x in (card.get("open") or [])]
    for line in lines:
        for tok in ANCHOR_RE.findall(line):
            if "/" not in tok and "\\" not in tok:
                continue  # 裸文件名不是锚——带路径分隔符才按仓内相对路径要求
            if not (root / tok.replace("\\", "/")).is_file():
                problems.append(f"[GHOST-ANCHOR] {tok.replace(chr(92), '/')} 不在仓内"
                                "（锚指向历史/错名——--allow-stale 不豁免）")


def main(argv: list[str]) -> int:
    root = pathlib.Path(os.environ.get("UNIFIED_RX_HANDOFF_ROOT", str(ROOT)))
    card_path = pathlib.Path(os.environ.get("UNIFIED_RX_HANDOFF", str(root / "spec" / "handoff.json")))
    log = pathlib.Path(os.environ.get("UNIFIED_RX_HANDOFF_LOG", str(root / "spec" / "ROUNDLOG.md")))
    problems: list[str] = []
    card = _load_card(card_path, problems)
    if card:
        check_card(card, log, problems)
        check_anchors(card, root, problems)
    hard = [p for p in problems if not p.startswith("[LAG]")]
    lags = [p for p in problems if p.startswith("[LAG]")]
    for p in hard:
        print(f"  ✗ {p}")
    for p in lags:
        print(f"  ⚠ {p}")
    if hard or (lags and "--allow-stale" not in argv):
        print(f"HANDOFF-GATE FAIL 原因={len(problems)}")
        return 1
    if lags:
        print("HANDOFF-GATE WARN-STALE（explicitly allowed）")
        return 0
    print(f"HANDOFF-GATE OK round={card.get('round')} 卡={card_path.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
