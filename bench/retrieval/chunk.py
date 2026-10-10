"""检索单元的**启发式分块**（M4-3）：把"行级金标 vs 整文件压住"的粒度账拆到块粒度。

方法：
  · .jsonl：保持**行级**（金标本身就是 `源路径#行号`，不能再分）。
  · .rs / .py：按**顶层语法定义**切块（`fn` / `impl` / `struct` / `enum` / `trait` / `mod` / `const` / `static`），
    每块独立索引用块号区分，块内不截断定义。
  · 其他扩展：整文件一块（与当前"文件级检索单元"口径一致）。

块 ID 格式：`<仓相对路径>#<块号>`（行级金标保留原 ID `源#行号`，不重编）。

**产物 `blocks.jsonl` 是派生物、不进 git**（语料钉版 ⇒ 逐字节可复现）。账本 M4-3 同单位 2×2
引用的"3231 units / 中位块长 13 token"就是这张表；复现口径：
`python -X utf8 bench/retrieval/chunk.py --write`，再核对输出行数与中位块长。
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import floor as floor_mod  # 复用：语料 sha256 核校 + enumerate_docs

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "blocks.jsonl"
TOP_LEVEL_PATTERNS = {
    "rs": [
        r"^(?:pub\s+)?fn\s+",
        r"^(?:pub\s+)?(?:async\s+)?fn\s+",
        r"^impl\s+(?:const\s+)?[\w<>:,\\s&<>]+[\\s{]*",
        r"^struct\s+",
        r"^enum\s+",
        r"^trait\s+",
        r"^mod\s+",
        r"^const\s+",
        r"^static\s+",
        r"^#[derive\s*\(.*\)]",  # 保留 derive，由下一条语句收进同一块
    ],
    "py": [
        r"^async\s+def\s+",
        r"^def\s+",
        r"^class\s+",
        r"^if\s+__name__\s*==\s*[\"']__main__[\"']:",
    ],
}


def split_source(text: str, ext: str) -> list[tuple[str, str, int, str]]:
    """按顶层定义切块：返回 [(block_id, text, first_line, kind)]。"""
    ext = ext.lstrip(".")
    lines = text.splitlines()

    if ext in ("json", "jsonl"):
        out: list[tuple[str, str, int, str]] = []
        for ln, line in enumerate(lines, start=1):
            if not line.strip():
                continue
            out.append(("chunk", line, ln, "line"))
        return out

    blocks: list[tuple[str, str, int, str]] = []
    try:
        candidates = TOP_LEVEL_PATTERNS[ext]
    except KeyError:
        return [("chunk", text, 1, "file")] if text else []

    body_start = 0
    for ln, line in enumerate(lines, start=1):
        for pat in candidates:
            import re

            if re.match(pat, line):
                if body_start < ln:
                    blocks.append((str(len(blocks) + 1), "\n".join(lines[body_start:ln]), body_start + 1, "chunk"))
                body_start = ln
                break

    blocks.append((str(len(blocks) + 1), "\n".join(lines[body_start:]), body_start + 1, "chunk"))
    return blocks


def enumerate_blocks(docs: list[tuple[str, str]]) -> list[tuple[str, str]]:
    """[(block_id, text)]：行级/块级文档统一摊平，block_id = `source#块号`。

    doc_id 里可能已带`#行号`后缀（jsonl 行级金标），拆出纯净路径再判断扩展名。
    """
    out: list[tuple[str, str]] = []
    for doc_id, text in docs:
        if "#" in doc_id:
            source, _, suffix = doc_id.partition("#")
            if suffix.isdigit() and text.strip():
                # 行级金标：块 = 行本身，block_id 即金标 id（不动）。
                out.append((doc_id, text.strip()))
                continue
            ext = pathlib.Path(source).suffix.lower()
        else:
            source = doc_id
            ext = pathlib.Path(doc_id).suffix.lower()

        if ext in ("json", "jsonl"):
            for ln, line in enumerate(text.splitlines(), start=1):
                if not line.strip():
                    continue
                try:
                    json.loads(line)
                except json.JSONDecodeError:
                    continue
                out.append((f"{source}#{ln}", line))
        else:
            blocks = split_source(text, ext)
            if blocks:
                for bid, btext, _, _kind in blocks:
                    out.append((f"{source}#{bid}", btext))
            else:
                out.append((source, text))
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true",
                    help="写 blocks.jsonl（派生物；语料钉版 ⇒ 逐字节可复现）")
    a = ap.parse_args(argv)

    docs, drift = floor_mod.load_corpus()
    if drift:
        print(f"CHUNK INCONCLUSIVE：语料漂移 {len(drift)} 处：{drift[:5]}")
        return 3

    blocks = enumerate_blocks(docs)
    if a.write:
        lines = [json.dumps({"doc_id": d, "text": t}, ensure_ascii=False) for d, t in blocks]
        OUT.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        print(f"CHUNK WROTE: {len(blocks)} 个块（语料 {len(docs)} 篇），{OUT}")
        return 0
    print(f"CHUNK OK：语料 {len(docs)} 篇 → 块 {len(blocks)} 个（平均 {len(blocks)/len(docs):.2f} 块/篇）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
