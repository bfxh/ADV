"""hub_ledger.py —— 账本文本 ↔ 行的编解码（S187 续，为方向⑥ 的判据服务）。

**纯函数、零内部依赖**（路径/文本由调用方给）：这里是"**坏行必须被数出来**"这条纪律的
唯一实现。旧实现把解析失败静默 `continue`，于是"末行被写坏"与"那行从没写过"在链判据下
**不可区分**——实测：末行砍一半，`verify_chain` 报 `ok=True`（假绿），且那条运行永远停在
`start`。链的语义（prev_hash / 行哈希 / 逐代次字段）仍住在 `hub_core` / `hub_vc`。

自 `hub_core.py` 拆出：链的**语义**与账本的**编解码**是两件事，而 god 门棘轮对
`hub_core.py` 只准减不增；拆出来还让"坏行计数"可以脱离账本单独单测。
"""
from __future__ import annotations

import json
import pathlib


def read_text(path: pathlib.Path) -> str | None:
    """账本文本（None = **存在但不可读**；不存在等同空表）——读路径容错的唯一入口。"""
    if not path.exists():
        return ""
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def parse_rows(raw: str) -> tuple[list[dict], list[int]]:
    """解析账本文本 → (行, **坏行行号**)。坏行必须被数出来：跳过 ≠ 不存在。"""
    rows: list[dict] = []
    bad: list[int] = []
    for i, line in enumerate(raw.split("\n"), 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except ValueError:
            bad.append(i)
            continue
        if isinstance(row, dict):
            rows.append(row)
        else:
            bad.append(i)
    return rows, bad
