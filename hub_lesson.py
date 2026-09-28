"""hub_lesson.py —— 平台层失败记忆（S184，spec/FRONTIER-CI.md 方向⑧）。

**能力复用（不造第二份教训库）**：记录格式与 `tools/learn.py` **完全一致**
（`{id, text, ts, recall_count}`），读写直接复用它的 `_load_lessons/_save_lessons`，
默认路径也一致（`~/.ADV/lessons.jsonl`）——所以智能体用 `lesson(action="recall")`
就能搜到平台写的草稿，"集体记忆"天然跨项目共享（一机一库）。

**路径**：`UNIFIED_RX_LESSONS` 环境变量优先（测试/金丝雀用；不设即 learn 的默认路径）。

**去重**：错误签名 = sha256(`pipeline|step|exit|错误特征`)[:12] 写进记录文本；
同签名已存在 ⇒ `duplicate`（不重复写）——否则每次失败都在教训库里刷屏。

**边界**：写教训**绝不改变**运行结果（调用方容错）；本模块只写"草稿 + 怎么修的待补"，
真正的修复经验由人或智能体补写。
"""
from __future__ import annotations

import hashlib
import os
import pathlib
import time


def _default_lessons_path() -> pathlib.Path:
    """默认库：**数据根被显式指向时落在数据根内**（测试/金丝雀自动隔离——否则每次跑门
    都会往真教训库刷"金丝雀失败"），否则与 `tools/learn.py` **同库**
    （`~/.ADV/lessons.jsonl`）——生产永远只有一份教训库。"""
    raw_root = (os.environ.get("UNIFIED_RX_HUB_ROOT") or "").strip()
    if raw_root:
        return pathlib.Path(raw_root) / "lessons.jsonl"
    return pathlib.Path(os.path.expanduser("~")) / ".ADV" / "lessons.jsonl"


def lessons_path() -> pathlib.Path:
    """显式 `UNIFIED_RX_LESSONS` 覆盖 > 数据根内（隔离）> learn 默认库。"""
    raw = (os.environ.get("UNIFIED_RX_LESSONS") or "").strip()
    return pathlib.Path(raw) if raw else _default_lessons_path()


def _load(path: pathlib.Path) -> list[dict]:
    # 延迟导入：同库同格式（避免加载期耦合）
    from tools.learn import _load_lessons
    return _load_lessons(str(path))


def _save(rows: list[dict], path: pathlib.Path) -> None:
    # 同上：复用 learn 的写入口
    from tools.learn import _save_lessons
    path.parent.mkdir(parents=True, exist_ok=True)
    _save_lessons(rows, str(path))


def _signature(pipeline: str, failing: list[dict]) -> str:
    first = failing[0] if failing else {}
    err = " ".join((first.get("verdict_lines") or [])[:2])
    blob = f"{pipeline}|{first.get('step')}|{first.get('exit')}|{err}"
    return hashlib.sha256(blob.encode("utf-8", "replace")).hexdigest()[:12]


def draft_from_run(pipeline: str, run_id: str, steps: list[dict]) -> dict:
    """失败运行 ⇒ 一条草稿（账本互链 + 签名去重）；返回 {status: written|duplicate|skipped}。"""
    failing = [s for s in steps if not s.get("ok") and not s.get("optional")]
    if not failing:
        return {"status": "skipped", "reason": "无失败步骤"}
    sig = _signature(pipeline, failing)
    path = lessons_path()
    rows = _load(path)
    if any(f"sig:{sig}" in str(r.get("text", "")) for r in rows):
        return {"status": "duplicate", "signature": sig, "total": len(rows)}
    first = failing[0]
    lines = "; ".join((first.get("verdict_lines") or [])[:2]) or "(无判定行)"
    text = (f"[平台] 管线 {pipeline} 失败（red）：步骤 {first.get('step')} "
            f"exit={first.get('exit')}；特征：{lines}；run={run_id}；"
            f"复核: python -X utf8 scripts/hub_gate.py --seal {run_id} "
            f"[sig:{sig}]（平台自动草稿——「怎么修的」请人或智能体补写）")
    entry = {"id": f"L{int(time.time())}", "text": text, "ts": int(time.time()),
             "recall_count": 0}
    rows.append(entry)
    _save(rows, path)
    return {"status": "written", "id": entry["id"], "signature": sig, "total": len(rows)}


def recall_for(pipeline: str, limit: int = 3) -> list[dict]:
    """按管线名召回历史教训（关键词口径，复用 learn 的 `_kw_hits` 打分）。"""
    from tools.learn import _kw_hits
    rows = _load(lessons_path())
    hits = [(_kw_hits(pipeline, str(r.get("text", ""))), r) for r in rows]
    hits = [(s, r) for s, r in hits if s > 0]
    hits.sort(key=lambda x: -x[0])
    return [r for _s, r in hits[:max(1, limit)]]
