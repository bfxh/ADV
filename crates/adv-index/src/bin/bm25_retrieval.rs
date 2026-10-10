//! BM25 检索评测的 Rust 侧入口（M4-2）：stdin 收 JSONL 请求，stdout 回每查询的排序结果。
//!
//! 协议由 `bench/retrieval/bm25.py` 消费；`query_id` 用冻结件的 `qid`，并列分数按插入序打破。

use adv_index::bm25::SearchIndex;
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Doc {
        doc_id: String,
        text: String,
    },
    Query {
        query_id: String,
        text: String,
        top_k: usize,
    },
}

fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut docs = Vec::new();
    let mut queries = Vec::new();
    for line in stdin.lock().lines() {
        let request: Request = serde_json::from_str(&line?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        match request {
            Request::Doc { doc_id, text } => docs.push((doc_id, text)),
            Request::Query {
                query_id,
                text,
                top_k,
            } => queries.push((query_id, text, top_k)),
        }
    }

    let index = SearchIndex::build(docs);
    let mut out = io::BufWriter::new(stdout.lock());
    for (query_id, text, top_k) in queries {
        let results = index
            .search(&text, top_k)
            .into_iter()
            .map(|(doc_id, score)| json!({"doc_id": doc_id, "score": score}))
            .collect::<Vec<_>>();
        serde_json::to_writer(&mut out, &json!({"query_id": query_id, "results": results}))?;
        out.write_all(b"\n")?;
    }
    out.flush()
}

fn main() -> io::Result<()> {
    run()
}
