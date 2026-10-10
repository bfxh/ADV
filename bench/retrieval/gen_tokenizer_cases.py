"""重录 tokenizer 对拍语料：期望一律由冻结的 `protocol.py` 产出，源料取 `corpus.pin` 的真实文本。

为什么单独成脚本（M4-2，变异门催出来的）：`crates/adv-index/tests/tokenizer_protocol.rs` 钉的是
"Rust 切词 == Python 口径"，期望**不能由被测实现自己生成**，否则等于把实现错误一起钉成"正确"。
所以生成器只依赖 `protocol.py`（评测集自家那把尺）与钉版语料。

覆盖面两档，都是第二轮变异门教出来的（详见 eval/FP-LEDGER-m2s4.md）：
  · **标识符全收**，不抽样——抽样 1/9 时 `upper_end - start > 1` 这类边界变异照样存活；
  · **原始文本跨度**——`tokenize` 的入口是任意文本，`== b'_'` 与 `!= b'_'` 只在
    "数字/标点后面还跟着词"的文本上才可区分，只喂标识符等于没测这条边界。

行格式：`K\t<单行文本>\t<逗号连接的期望 token>`（文本里的 TAB/换行折叠成空格，故可安全按 TAB 分列）。

用法（改切词口径或换 pin 后要显式重录，并重跑 `cargo test -p adv-index`）：
  python -X utf8 bench/retrieval/gen_tokenizer_cases.py > crates/adv-index/tests/data/tokenizer_cases.txt
"""
from __future__ import annotations

import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bm25  # noqa: E402  —— 复用它的冻结语料入口：语料漂移即判不了，不在漂过的语料上生成
import protocol  # noqa: E402

#: 变异门点名过的形状，必须条条进语料（尾大写 / 数字段 / 下划线混合 / 单字母）。
BOUNDARIES = (
    "HTTP", "ABCd", "aB", "x1y2z3", "_lead", "trail_", "A1_", "ID3Tag",
    "loadWorkspaceFile", "v2beta", "UTF8", "parse_HTTPServerName", "n", "s2", "__x__y__",
)
#: 文本跨度：每隔几篇取一次文首若干字符（全收太大，跨度的作用是覆盖分隔符形状）。
SPAN_EVERY = 3
SPAN_CHARS = 160


def one_line(text: str) -> str:
    return " ".join(text.split())


def main() -> int:
    docs, drift = bm25.floor.load_corpus()
    if drift:
        print(f"TOKENIZER-CASES INCONCLUSIVE：语料漂移 {len(drift)} 处（先显式重录评测集）", file=sys.stderr)
        return 3

    idents: set[str] = set()
    spans: set[str] = set()
    for index, (_doc_id, text) in enumerate(docs):
        idents.update(protocol._IDENT.findall(text))
        if index % SPAN_EVERY == 0:
            head = one_line(text)[:SPAN_CHARS]
            if head:
                spans.add(head)
    idents.update(BOUNDARIES)

    # 冻结查询本身就是要检索的文本，全收。
    if bm25.QUERIES.is_file():
        for line in bm25.QUERIES.read_text(encoding="utf-8").splitlines():
            if line.strip():
                spans.add(one_line(json.loads(line)["text"]))

    rows = sorted(idents) + sorted(spans)
    for row in rows:
        print(f"K\t{row}\t{','.join(protocol.tokenize(row))}")
    print(
        f"源料：标识符 {len(idents)} 条（全收不抽样）+ 文本跨度 {len(spans)} 条 = 落盘 {len(rows)} 行",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
