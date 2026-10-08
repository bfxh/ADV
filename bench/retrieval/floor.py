"""检索评测的**地板基线**（文件名 `floor.py`：不叫 `eval.py` —— 那个名字与内建 `eval` 打架，mypy 直接解析不到）
（M4-1）：纯 Python 词袋 + tf-idf 余弦，零依赖。

为什么先写这个：M4-2 要接 Tantivy（+40 个直接依赖）。没有基线，就没人能回答"它到底有没有更好"——
所有增益主张必须过自家评测集（RESEARCH 09 的原话）。这条地板基线的职责就是把"下限"钉死：
如果 Tantivy 的召回打不过它，那这不是索引栈的胜利，是我们在自欺。

口径：
  · 语料先按 `corpus.sha256` 逐文件核 sha256，**漂移即判不了**（exit 3，要求显式重录）；
  · 度量 Recall@1/5/10、MRR、nDCG@10（每查询单条金标 ⇒ nDCG = 1/log2(rank+1)）；
  · `--check` 与冻结读数逐字段相等才算过（确定性：无随机、无时间、无并发）。

用法：
  python -X utf8 bench/retrieval/eval.py                 # 跑一遍，打印表
  python -X utf8 bench/retrieval/eval.py --check          # 与 baseline.json 对账
  python -X utf8 bench/retrieval/eval.py --write-baseline
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gen_queries  # noqa: E402
import protocol  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parent
BASELINE = HERE / "baseline.json"
KS = (1, 5, 10)


def load_corpus() -> tuple[list[tuple[str, str]], list[str]]:
    """读语料 + 核 sha256。返回 (docs, 漂移清单)。"""
    docs = gen_queries.enumerate_docs()
    manifest = {}
    if gen_queries.CORPUS_MANIFEST.is_file():
        for line in gen_queries.CORPUS_MANIFEST.read_text(encoding="utf-8").splitlines():
            if line.strip():
                digest, doc_id = line.split("  ", 1)
                manifest[doc_id] = digest
    drift = []
    for doc_id, text in docs:
        want = manifest.get(doc_id)
        got = hashlib.sha256(text.encode("utf-8")).hexdigest()
        if want is None or want != got:
            drift.append(doc_id)
    for doc_id in manifest:
        if doc_id not in {d for d, _ in docs}:
            drift.append(f"{doc_id}（清单有、语料不在）")
    return docs, drift


def index(docs: list[tuple[str, str]]) -> tuple[list[dict[str, float]], dict[str, float]]:
    """tf-idf 向量（l2 归一）+ idf 表。"""
    tokenized = [protocol.tokenize(t) for _id, t in docs]
    df: dict[str, int] = {}
    for toks in tokenized:
        for word in set(toks):
            df[word] = df.get(word, 0) + 1
    n = max(1, len(docs))
    idf = {w: math.log((1 + n) / (1 + c)) + 1.0 for w, c in df.items()}
    vectors: list[dict[str, float]] = []
    for toks in tokenized:
        tf: dict[str, int] = {}
        for word in toks:
            tf[word] = tf.get(word, 0) + 1
        vec = {w: (1 + math.log(c)) * idf[w] for w, c in tf.items()}
        norm = math.sqrt(sum(v * v for v in vec.values())) or 1.0
        vectors.append({w: v / norm for w, v in vec.items()})
    return vectors, idf


def score(query: str, vectors: list[dict[str, float]], idf: dict[str, float]) -> list[int]:
    """→ 文档下标按得分降序（并列时按原序，保证确定性）。"""
    toks = protocol.tokenize(query)
    tf: dict[str, int] = {}
    for word in toks:
        tf[word] = tf.get(word, 0) + 1
    qvec = {w: (1 + math.log(c)) * idf.get(w, math.log(1 + len(vectors)) + 1.0)
            for w, c in tf.items()}
    norm = math.sqrt(sum(v * v for v in qvec.values())) or 1.0
    qvec = {w: v / norm for w, v in qvec.items()}
    scored = [(sum(qvec.get(w, 0.0) * dv.get(w, 0.0) for w in qvec), -i, i) for i, dv in enumerate(vectors)]
    scored.sort(key=lambda t: (-t[0], t[1]))
    return [i for _s, _neg, i in scored]


def metrics(queries: list[dict], docs: list[tuple[str, str]], vectors, idf) -> dict:
    idx_of = {doc_id: i for i, (doc_id, _t) in enumerate(docs)}
    hits = {f"recall@{k}": 0 for k in KS}
    rr_sum = 0.0
    ndcg_sum = 0.0
    for q in queries:
        order = score(q["text"], vectors, idf)
        rel = {idx_of[r] for r in q["relevant"] if r in idx_of}
        first = next((rank for rank, i in enumerate(order, start=1) if i in rel), 0)
        for k in KS:
            if first and first <= k:
                hits[f"recall@{k}"] += 1
        rr_sum += (1.0 / first) if first else 0.0
        ndcg_sum += (1.0 / math.log2(first + 1)) if first and first <= 10 else 0.0
    n = max(1, len(queries))
    out = {k: round(v / n, 4) for k, v in hits.items()}
    out["mrr"] = round(rr_sum / n, 4)
    out["ndcg@10"] = round(ndcg_sum / n, 4)
    out["queries"] = len(queries)
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--write-baseline", action="store_true")
    a = ap.parse_args(argv)

    docs, drift = load_corpus()
    if drift:
        print(f"RETRIEVAL-EVAL INCONCLUSIVE：语料漂移 {len(drift)} 处（需显式重录）：{drift[:5]}")
        return 3
    if not gen_queries.QUERIES.is_file():
        print("RETRIEVAL-EVAL INCONCLUSIVE：queries.jsonl 不在（先跑 gen_queries.py --write）")
        return 3
    queries = [json.loads(l) for l in gen_queries.QUERIES.read_text(encoding="utf-8").splitlines() if l.strip()]
    vectors, idf = index(docs)
    reading = metrics(queries, docs, vectors, idf)
    print("RETRIEVAL-EVAL（地板基线 tf-idf）" + json.dumps(reading, ensure_ascii=False, sort_keys=True))
    if a.write_baseline:
        BASELINE.write_text(json.dumps(reading, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
                            encoding="utf-8", newline="\n")
        print("RETRIEVAL-EVAL 基线已重录")
        return 0
    if a.check:
        want = json.loads(BASELINE.read_text(encoding="utf-8")) if BASELINE.is_file() else {}
        if want != reading:
            print(f"RETRIEVAL-EVAL FAIL：与冻结读数不一致（改动必须显式 --write-baseline）\n  冻结 {want}\n  实测 {reading}")
            return 1
        print("RETRIEVAL-EVAL OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
