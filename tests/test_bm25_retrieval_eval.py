"""M4-2 BM25 评测通路的锚（口径一律锚 `floor.py`，不按本文件作者的手感写期望）。

为什么这几条存在：上一版把冻结查询按 `query_id`/`source` 读，而冻结件的字段是
`{kind, qid, relevant, source, text}`——当场 KeyError；同时 metrics 的期望把分母一会儿写成
查询数、一会儿写成命中数，是自相矛盾的断言。所以这里每条都指明锚在哪个既有观测上。
"""
import importlib.util
import json
import math
import pathlib

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "bench" / "retrieval" / "bm25.py"


def load_module():
    spec = importlib.util.spec_from_file_location("bm25_retrieval_eval", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_bm25_metrics_match_floor_formula():
    """同分母（查询数）+ 同精度（round 4）：锚 `floor.py:metrics` 与冻结 `baseline.json`。"""
    module = load_module()
    rows = [
        {
            "query_id": "q1",
            "results": [
                {"doc_id": "miss", "score": 2.0},
                {"doc_id": "gold", "score": 1.0},
            ],
        },
        {"query_id": "q2", "results": []},
    ]
    got = module.metrics(rows, {"q1": {"gold"}, "q2": {"nope"}})
    assert got == {
        "mrr": 0.25,
        "ndcg@10": round(1 / math.log2(3) / 2, 4),
        "queries": 2,
        "recall@1": 0.0,
        "recall@10": 0.5,
        "recall@5": 0.5,
    }


def test_relevant_is_a_set_like_floor():
    """金标是集合（`relevant` 在冻结件里就是数组）：任一成员上榜即算命中，取首个排名。"""
    module = load_module()
    rows = [
        {
            "query_id": "q1",
            "results": [{"doc_id": "a", "score": 2.0}, {"doc_id": "b", "score": 1.0}],
        }
    ]
    assert module.metrics(rows, {"q1": {"b"}})["recall@1"] == 0.0
    assert module.metrics(rows, {"q1": {"a", "b"}})["recall@1"] == 1.0


def test_build_payload_keys_query_by_qid_not_source():
    """协议锚 Rust 侧 `Request::Query{query_id,text,top_k}`；键必须是 `qid`。

    实测冻结件 120 条只有 83 个不同 `source`（33 个被 2–3 条共用）——按 source 收键会丢 37 条查询。
    """
    module = load_module()
    queries = [
        {"qid": "q042", "source": "a/b.rs", "relevant": ["a/b.rs"], "text": "guard py"},
        {"qid": "q043", "source": "a/b.rs", "relevant": ["c/d.rs"], "text": "ledger name"},
    ]
    body = module.build_payload([("a/b.rs", "guard python")], queries)
    parsed = [json.loads(line) for line in body.splitlines() if line.strip()]
    got = [item for item in parsed if item["type"] == "query"]
    assert got == [
        {"type": "query", "query_id": "q042", "text": "guard py", "top_k": 10},
        {"type": "query", "query_id": "q043", "text": "ledger name", "top_k": 10},
    ]


def test_rows_and_gold_keeps_queries_sharing_a_source():
    """共源查询各自成一条样本，分母仍是查询数；金标取自 `relevant` 而非 `source`。"""
    module = load_module()
    queries = [
        {"qid": "q1", "source": "same.rs", "relevant": ["same.rs"], "text": "a"},
        {"qid": "q2", "source": "same.rs", "relevant": ["other.rs"], "text": "b"},
    ]
    by_id = {"q1": [{"doc_id": "same.rs", "score": 1.0}], "q2": []}
    rows, relevant = module.rows_and_gold(queries, by_id)
    assert [row["query_id"] for row in rows] == ["q1", "q2"]
    assert relevant == {"q1": {"same.rs"}, "q2": {"other.rs"}}
    assert module.metrics(rows, relevant)["queries"] == 2


def test_rows_and_gold_fails_loudly_when_rust_skips_a_query():
    module = load_module()
    queries = [{"qid": "q1", "source": "s", "relevant": ["s"], "text": "a"}]
    with pytest.raises(KeyError):
        module.rows_and_gold(queries, {})


def test_corpus_fails_closed_on_sha_drift(monkeypatch):
    """语料漂移 ⇒ 判不了（exit 3），与地板同一处置；不许在漂过的语料上出可比读数。"""
    module = load_module()
    monkeypatch.setattr(module.floor, "load_corpus", lambda: ([("a", "b")], ["a"]))
    with pytest.raises(SystemExit) as exc:
        module.corpus()
    assert exc.value.code == 3


def test_check_requires_frozen_baseline_before_reading_corpus(monkeypatch, tmp_path, capsys):
    module = load_module()

    def boom():
        raise AssertionError("基线缺失时不该去读语料")

    monkeypatch.setattr(module, "corpus", boom)
    with pytest.raises(SystemExit) as exc:
        module.main(["--check", "--baseline", str(tmp_path / "missing.json")])
    assert exc.value.code == 3
    assert "冻结基线不在" in capsys.readouterr().out
