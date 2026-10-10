"""BM25 首片评测（M4-2）：冻结查询 + Rust bm25_retrieval + 与 floor.py 同一指标口径。

为什么单独成片：M4-2 的目标不是“引入检索器”，而是先回答它是否打赢 tf-idf 地板。
本脚本复用 `floor.py` 的冻结语料与指标公式；差别只在排序器换成本仓自研 BM25。

用法：
  python -X utf8 bench/retrieval/bm25.py
  python -X utf8 bench/retrieval/bm25.py --check
  python -X utf8 bench/retrieval/bm25.py --write-baseline

退出码与地板同规：0 全绿 / 1 判红 / 3 判不了（CLI 未编、语料漂移、冻结件缺失）。
"""
from __future__ import annotations

import argparse
import json
import math
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import floor  # noqa: E402  —— 语料与 sha256 核对复用地板那把尺，不另起一套
BASELINE = HERE / "bm25.json"
CLI = ROOT / "target" / "debug" / "bm25_retrieval.exe"
QUERIES = HERE / "queries.jsonl"
KS = (1, 5, 10)
TOP_K = 10


def load_queries() -> list[dict]:
    if not QUERIES.is_file():
        print("RETRIEVAL-BM25 INCONCLUSIVE：queries.jsonl 不在（先跑 gen_queries.py --write）")
        raise SystemExit(3)
    return [json.loads(line) for line in QUERIES.read_text(encoding="utf-8").splitlines() if line.strip()]


def corpus() -> list[tuple[str, str]]:
    """语料走地板那把尺：内容逐文件核 sha256，漂移即判不了（exit 3）。

    不绕过它的原因：BM25 的读数要和 `baseline.json` 比高低，两边读的不是同一份语料就没有可比性。
    """
    docs, drift = floor.load_corpus()
    if drift:
        print(f"RETRIEVAL-BM25 INCONCLUSIVE：语料漂移 {len(drift)} 处（需显式重录）：{drift[:5]}")
        raise SystemExit(3)
    return docs


def build_payload(docs: list[tuple[str, str]], queries: list[dict]) -> str:
    """Rust `Request` 的形状：doc{doc_id,text} / query{query_id,text,top_k}。

    query_id 用冻结件的 `qid` 而不是 `source`——实测 120 条查询只有 83 个不同 source（33 个被
    2–3 条共用），按 source 收键会把后面的查询静默吃掉。
    """

    def request(kind: str, **fields):
        return json.dumps({"type": kind, **fields}, ensure_ascii=False, separators=(",", ":"))

    return "\n".join(
        [request("doc", doc_id=doc_id, text=text) for doc_id, text in docs]
        + [request("query", query_id=q["qid"], text=q["text"], top_k=TOP_K) for q in queries]
    ) + "\n"


def run_rust(docs, queries) -> dict[str, list[dict]]:
    if not CLI.is_file():
        print(f"RETRIEVAL-BM25 INCONCLUSIVE：CLI 不在（先 cargo build -p adv-index --bin bm25_retrieval）：{CLI}")
        raise SystemExit(3)

    proc = subprocess.run(
        [str(CLI)],
        input=build_payload(docs, queries).encode("utf-8"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(proc.stderr.decode("utf-8", "replace").strip())
    out: dict[str, list[dict]] = {}
    for line in proc.stdout.decode("utf-8").splitlines():
        if not line.strip():
            continue
        item = json.loads(line)
        if item["query_id"] in out:
            print(f"RETRIEVAL-BM25 INCONCLUSIVE：Rust 侧回包 query_id 重复：{item['query_id']}")
            raise SystemExit(3)
        out[item["query_id"]] = item["results"]
    return out


def rows_and_gold(queries: list[dict], results_by_id: dict[str, list[dict]]):
    """按冻结查询的顺序配对（query_id, 命中表）与金标集合；缺一条就 KeyError，不静默降样本。"""
    rows = [{"query_id": q["qid"], "results": results_by_id[q["qid"]]} for q in queries]
    return rows, {q["qid"]: set(q["relevant"]) for q in queries}


def metrics(results, relevant) -> dict:
    """与 `floor.py:metrics` 同一口径：金标是集合、取首个命中的排名、分母是查询数、round 4。"""
    hits = {f"recall@{k}": 0 for k in KS}
    rr_sum = 0.0
    ndcg_sum = 0.0
    for row in results:
        ranked = row["results"]
        first = next(
            (rank for rank, hit in enumerate(ranked, start=1) if hit["doc_id"] in relevant[row["query_id"]]),
            0,
        )
        for k in KS:
            if first and first <= k:
                hits[f"recall@{k}"] += 1
        rr_sum += 1.0 / first if first else 0.0
        ndcg_sum += 1.0 / math.log2(first + 1) if first and first <= TOP_K else 0.0
    count = len(results)
    denom = max(1, count)
    return {
        **{key: round(value / denom, 4) for key, value in hits.items()},
        "mrr": round(rr_sum / denom, 4),
        "ndcg@10": round(ndcg_sum / denom, 4),
        "queries": count,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("--baseline", default=str(BASELINE))
    args = parser.parse_args(argv)

    baseline_path = pathlib.Path(args.baseline)
    if args.check and not baseline_path.is_file():
        print(f"RETRIEVAL-BM25 INCONCLUSIVE：冻结基线不在（先 --write-baseline）：{baseline_path}")
        raise SystemExit(3)

    docs = corpus()
    queries = load_queries()
    results_by_id = run_rust(docs, queries)
    rows, relevant = rows_and_gold(queries, results_by_id)
    reading = metrics(rows, relevant)
    label = "RETRIEVAL-BM25"
    print(label + " " + json.dumps(reading, ensure_ascii=False, sort_keys=True))
    if args.write_baseline:
        baseline_path.write_text(
            json.dumps(reading, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        print(f"{label} 基线已重录")
        return 0
    if args.check:
        frozen = json.loads(baseline_path.read_text(encoding="utf-8"))
        if frozen != reading:
            print(f"{label} FAIL：与冻结读数不一致（改动必须显式 --write-baseline）\n  冻结 {frozen}\n  实测 {reading}")
            return 1
        print(f"{label} OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
