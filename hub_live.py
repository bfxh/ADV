"""hub_live.py —— 平台层实时日志总线（S177，spec/HUB.md §七 M1 第一片）。

**为什么总线是文件而不是内存**：MCP 侧 `hub_run` 跑在 server.py 进程、网页面读日志跑在
server_web.py 进程 —— 内存队列跨不了进程。日志文件本身就是被哈希的真相 ⇒ 直接按字节
offset 增量 tail 它，**看到的就是账上记的**，不需要第二份现实（少一份漂移源）。

- `read_from(path, offset)`：增量读；UTF-8 边界不完整的尾字节**留到下一轮**
  （逐次 `errors="ignore"` 会把多字节字符切坏，这里退回等待而不是替换）。
- `log_files(run_id)`：该 run 的日志文件（`{run}.{step}.{out,err}.log`）→ (path, step, stream)。
- `sse_frame(event, payload)`：SSE 帧（单行 data，JSON 编码，换行不破帧）。
"""
from __future__ import annotations

import json
import pathlib

import hub_core

STREAMS = ("out", "err")


def log_files(run_id: str) -> list[tuple[pathlib.Path, str, str]]:
    """(path, step, stream)；排序稳定，供 offset 跟踪。"""
    d = hub_core.logs_dir()
    if not d.is_dir():
        return []
    out: list[tuple[pathlib.Path, str, str]] = []
    prefix = f"{run_id}."
    for p in sorted(d.glob(f"{run_id}.*.log")):
        stem = p.name[len(prefix):-len(".log")]
        step, _, stream = stem.rpartition(".")
        if step and stream in STREAMS:
            out.append((p, step, stream))
    return out


def read_from(path: pathlib.Path, offset: int) -> tuple[str, int]:
    """从 offset 起增量读；返回 (文本, 新 offset)。尾随不完整多字节字符留待下一轮。"""
    try:
        size = path.stat().st_size
    except OSError:
        return "", offset
    if size <= offset:
        return "", offset
    try:
        with path.open("rb") as fh:
            fh.seek(offset)
            raw = fh.read()
    except OSError:
        return "", offset
    for back in range(4):                        # 最多 3 字节不完整尾码点：退回等待
        body = raw[: len(raw) - back] if back else raw
        try:
            return body.decode("utf-8"), offset + len(body)
        except UnicodeDecodeError:
            continue
    return raw.decode("utf-8", "replace"), offset + len(raw)


def sse_frame(event: str, payload: dict) -> bytes:
    """SSE 帧：event + 单行 data（JSON 编码 ⇒ 换行被转义，不破帧）。"""
    return f"event: {event}\ndata: {json.dumps(payload, ensure_ascii=False)}\n\n".encode()
