"""重录 tokenizer 对拍语料：期望一律由冻结的 `protocol.py` 产出，源料取 `corpus.pin` 的真实标识符。

为什么单独成脚本（M4-2，变异门催出来的）：`crates/adv-index/tests/tokenizer_protocol.rs` 钉的是
"Rust 切词 == Python 口径"，期望**不能由被测实现自己生成**，否则等于把实现错误一起钉成"正确"。
所以生成器只依赖 `protocol.py`（评测集自家那把尺）与钉版语料。

用法（改切词口径或换 pin 后要显式重录，并重跑 `cargo test -p adv-index`）：
  python -X utf8 bench/retrieval/gen_tokenizer_cases.py > crates/adv-index/tests/data/tokenizer_cases.txt
"""
from __future__ import annotations

import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bm25  # noqa: E402  —— 复用它的 corpus()：语料漂移即判不了，不在漂过的语料上生成
import protocol  # noqa: E402

#: 变异门点名过的形状，必须条条进语料（尾大写 / 数字段 / 下划线混合 / 单字母）。
BOUNDARIES = (
    "HTTP", "ABCd", "aB", "x1y2z3", "_lead", "trail_", "A1_", "ID3Tag",
    "loadWorkspaceFile", "v2beta", "UTF8", "parse_HTTPServerName", "n", "s2", "__x__y__",
)
#: 抽样目标行数：够覆盖各分支边界，又不把 cargo test 拖慢。
TARGET_LINES = 1200


def main() -> int:
    docs, drift = bm25.floor.load_corpus()
    if drift:
        print(f"TOKENIZER-CASES INCONCLUSIVE：语料漂移 {len(drift)} 处（先显式重录评测集）", file=sys.stderr)
        return 3
    idents: set[str] = set()
    for _doc_id, text in docs:
        idents.update(protocol._IDENT.findall(text))
    idents.update(BOUNDARIES)

    picked = sorted(idents)
    step = max(1, len(picked) // TARGET_LINES)
    rows = picked[::step]
    for shape in BOUNDARIES:
        if shape not in rows:
            rows.append(shape)
    rows.sort()

    for ident in rows:
        print(f"{ident}\t{','.join(protocol.tokenize(ident))}")
    print(f"源料 {len(idents)} 个标识符 → 落盘 {len(rows)} 行（步长 {step}，边界全保）", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
