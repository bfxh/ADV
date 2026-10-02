"""溢出落盘（S161 模型适配 P0 ③）：超大回包不塞进上下文，写文件 + 回摘要与路径。

为什么单列一个模块：
- 落盘是**有副作用**的事，集中在能被单独审计/测试的一处，比散在协议层好；
- 文件名由 `tempfile.mkstemp` **由标准库生成**（不是我们拼字符串）⇒ 结构上不存在
  路径穿越面：调用方只能给前缀（且前缀先过 `[A-Za-z0-9_]` 白名单），目录先过三步校验。

落点：`<沙盒根>/.urx-spill/`——必须在沙盒内，否则模型的 `fs_read` 取不回来（fail-closed）。
保留最近 `KEEP` 份，防无限增长。
"""
import glob
import os
import re
import tempfile

KEEP = 50
SPILL_DIRNAME = ".urx-spill"


def _sandbox_roots():
    """沙盒根（`UNIFIED_RX_SANDBOX`，分号分隔）。空 ⇒ 无落点。"""
    return [r for r in (os.environ.get("UNIFIED_RX_SANDBOX") or "").split(";") if r.strip()]


def _safe_prefix(name: str) -> str:
    """文件名前缀白名单：只留 `[A-Za-z0-9_]`（不可能含分隔符或 `..`）。"""
    return re.sub(r"[^A-Za-z0-9_]", "", str(name))[:40] or "result"


def spill_dir():
    """返回**已验证**的落盘目录；不可用返回 None。

    三步校验（写前显式，不是"相信拼接"）：abspath+normpath 规范化 → 禁 `..` → 必须落在沙盒根内。
    """
    roots = _sandbox_roots()
    if not roots:
        return None
    base = os.path.abspath(roots[0])
    d = os.path.normpath(os.path.join(base, SPILL_DIRNAME))
    if ".." in d or not d.startswith(base + os.sep):
        return None
    try:
        os.makedirs(d, exist_ok=True)
    except OSError:
        return None
    return d


def _prune(d: str) -> None:
    try:
        for old in sorted(glob.glob(os.path.join(d, "*.json")), key=os.path.getmtime)[:-KEEP]:
            os.unlink(old)
    except OSError:
        pass


def spill(payload: str, name: str):
    """把 payload 落盘，返回路径；任何异常返回 None（落盘失败绝不拖垮调用）。"""
    d = spill_dir()
    if not d:
        return None
    try:
        fd, path = tempfile.mkstemp(prefix=_safe_prefix(name) + "-", suffix=".json", dir=d)
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(payload)
        _prune(d)
        return path
    except OSError:
        return None


# ---- S215（片 D-2）：分析类回包的「大列表长尾溢出」----
# 旧行为是整包 >48KB 才整体落盘（bug_scan 一次 ~46KB 恰好卡在阈值下 ⇒ 全量 issues 塞进上下文）。
# 这里只对**列表字段**动手：短列表原样、长列表保留首屏 head + 全量计数 + 分布，把**尾部**落盘，
# 附 fs_read 取用命令——信息不减（尾部可整份取回），却把单次回包与命中总数解耦。
_HEAD_DEFAULT = 30          # 首屏保留条目数
_LIST_KB_DEFAULT = 12       # 单列表字段序列化超此阈值(KB)才考虑切尾


def _json_bytes(x) -> int:
    import json
    return len(json.dumps(x, ensure_ascii=False).encode("utf-8"))


def compact_large_lists(data, name, head_n=_HEAD_DEFAULT, thresh_bytes=_LIST_KB_DEFAULT * 1024):
    """返回 (new_data, meta)：对 dict `data` 里超阈且长于 head_n 的列表字段切尾落盘。

    - 不改原 `data`（浅拷贝替换命中的列表键）。
    - `meta` 为落盘描述列表；无命中或**落盘失败** ⇒ meta=[] 且原样返回（绝不静默丢数据）。
    - 只处理顶层 dict 的列表值（分析类回包 issues 在顶层；嵌套留待后续）。
    """
    if not isinstance(data, dict):
        return data, []
    import json
    out = dict(data)
    metas = []
    for key, val in data.items():
        if not isinstance(val, list) or len(val) <= head_n:
            continue
        if _json_bytes(val) <= thresh_bytes:
            continue
        head, tail = val[:head_n], val[head_n:]
        path = spill(json.dumps(tail, ensure_ascii=False), f"{name}-{key}")
        if not path:
            continue  # 落盘不可用 ⇒ 这个字段保持全量内联（宁大勿丢）
        out[key] = head
        metas.append({"field": key, "kept": head_n, "total": len(val),
                      "tail_path": path,
                      "fetch": f'fs_read(path="{path}")'})
    return (out, metas) if metas else (data, [])

