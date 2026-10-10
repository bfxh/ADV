"""生成 BM25 分数金样：期望由**独立转写的发表式**算出，不由被测 Rust 实现生成。

为什么这么造（M4-2，2026-10-10）：变异门在 `SearchIndex::search` 一处就点了 26 条存活变异，
根因是原有测试只断言"谁排第一 / 非空"——对公式形状几乎不敏感。分数金样能把任何改变评分的
变异钉红；而金样的期望**必须来自被测实现之外**，这里用发表式：

    freq·(k1+1) / (freq + k1·((1 - b) + b·|d|/avgdl))，
    idf = ln(1 + (N - df + 0.5)/(df + 0.5))
    （Elastic Practical BM25 part 3；k1=1.2 / b=0.25 —— b 是本仓冻结集实测最优，见 bm25.rs 的常数注释）

切词口径仍取 `protocol.py`（与 Rust 侧由 `tokenizer_protocol.rs` 的 1,331 条对拍钉住一致）。

用法：
  python -X utf8 bench/retrieval/gen_bm25_cases.py > crates/adv-index/tests/data/bm25_scores.tsv
"""
from __future__ import annotations

import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import protocol  # noqa: E402

K1, B, TOP_K = 1.2, 0.25, 10

#: 每条用例都挑一个公式形状敏感的形状：长度差、df 差、并列、纯单字、零命中、limit 截断。
CASES: list[tuple[str, list[tuple[str, str]], str]] = [
    ("length_vs_tf", [("short.rs", "guard py"),
                      ("long.rs", "guard py " + "guard py " * 8 + "filler words here now more")],
     "guard py"),
    ("df_weight", [("rare.rs", "zephyr guard"),
                   ("common.rs", "guard py"),
                   ("noise.rs", "guard guard guard")],
     "zephyr guard"),
    ("exact_tie", [("a.rs", "workspace index notes"),
                   ("c.rs", "workspace index notes"),
                   ("b.rs", "index only")],
     "workspace"),
    ("single_letter_dropped", [("a.rs", "workspace index"),
                               ("b.rs", "x index")],
     "workspace x"),
    ("no_match", [("a.rs", "alpha beta")], "gamma"),
    ("multi_term_sum", [("a.rs", "alpha beta gamma"),
                        ("b.rs", "alpha alpha"),
                        ("c.rs", "beta gamma")],
     "alpha beta gamma"),
    ("avg_len_across_three", [("one.rs", "sink"),
                              ("two.rs", "sink source sink"),
                              ("three.rs", "sink source sink source sink source")],
     "sink source"),
]


def index(docs: list[tuple[str, str]]):
    toks = [(d, protocol.tokenize(t)) for d, t in docs]
    df: dict[str, int] = {}
    for _d, ts in toks:
        for word in set(ts):
            df[word] = df.get(word, 0) + 1
    n = len(toks)
    avg = (sum(len(ts) for _d, ts in toks) / n) if n else 0.0
    return toks, df, float(n), avg


def search(docs: list[tuple[str, str]], query: str, top_k: int = TOP_K) -> list[tuple[str, float]]:
    toks, df, n, avg = index(docs)
    scores: dict[str, float] = {}
    order = [d for d, _ts in toks]
    for term in [t for t in protocol.tokenize(query) if len(t) > 1]:
        if term not in df:
            continue
        idf = math.log(1.0 + (n - df[term] + 0.5) / (df[term] + 0.5))
        for doc_id, doc_toks in toks:
            freq = doc_toks.count(term)
            if not freq:
                continue
            norm = (1.0 - B) + B * (len(doc_toks) / avg if avg else 0.0)
            scores[doc_id] = scores.get(doc_id, 0.0) + idf * freq * (K1 + 1.0) / (freq + K1 * norm)
    hits = [(d, s) for d, s in scores.items() if s > 0.0]
    hits.sort(key=lambda kv: (-kv[1], order.index(kv[0])))
    return hits[:top_k]


def main() -> int:
    print(f"# params k1={K1} b={B} top_k={TOP_K}")
    print("# 期望=独立转写的发表式，切词=protocol.py；改 Rust 侧常数必须同步这里并重录")
    for case_id, docs, query in CASES:
        for doc_id, text in docs:
            print(f"D\t{case_id}\t{doc_id}\t{text}")
        print(f"Q\t{case_id}\t{query}\t{TOP_K}")
        want = ",".join(f"{d}:{s:.12f}" for d, s in search(docs, query))
        print(f"W\t{case_id}\t{want}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
