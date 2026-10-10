//! M4-4b 的**成本读数**入口：stdin 收 JSONL 的 `doc` 行，stdout 出一段 JSON 读数。
//!
//! 四段各跑 `REPS` 次取**中位**：全量重建 / 冷 `apply` / 热 `apply`（应全复用）/
//! 复用后重建倒排。**不是计时门**——墙钟随负载漂，这里不设阈值也不冻结数字，
//! 读数口径与复现命令见 `bench/retrieval/incremental_cost.py`。
//!
//! 自带两条自检，任一不过就非 0 退出（"快"必须是"对"的快）：
//!   · 复用后的索引与全量重建**相等**；
//!   · 冷+热两轮的累计切词量恰好等于语料的 token 总数（热轮没重切词）。

use adv_index::bm25::SearchIndex;
use adv_index::incremental::IndexStore;
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

/// 每段测量重复次数（取中位，不剔首轮——预热后只取稳态是把成本藏起来）。
const REPS: usize = 3;

/// 输入只接受 `{"type":"doc",…}` 行；其他 `type` 由 serde 直接判错（fail-closed）。
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Input {
    /// 参与测量的文档。
    Doc { doc_id: String, text: String },
}

fn timed(reps: usize, mut run: impl FnMut()) -> Vec<Duration> {
    (0..reps)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .collect()
}

fn median_ms(mut runs: Vec<Duration>) -> f64 {
    runs.sort_unstable();
    runs[runs.len() / 2].as_secs_f64() * 1000.0
}

fn read_docs() -> io::Result<Vec<(String, String)>> {
    let stdin = io::stdin();
    let mut docs = Vec::new();
    for line in stdin.lock().lines() {
        let input: Input = serde_json::from_str(&line?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        match input {
            Input::Doc { doc_id, text } => docs.push((doc_id, text)),
        }
    }
    Ok(docs)
}

fn apply_round(docs: &[(String, String)], store: &mut IndexStore) -> io::Result<()> {
    store.apply(docs.to_vec()).map_err(io::Error::other)?;
    Ok(())
}

fn run() -> io::Result<()> {
    let docs = read_docs()?;
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut rebuild = Vec::new();
    let mut tokens = 0_u64;
    for _ in 0..REPS {
        let mut store = IndexStore::new();
        cold.push({
            let start = Instant::now();
            apply_round(&docs, &mut store)?;
            start.elapsed()
        });
        warm.push({
            let start = Instant::now();
            apply_round(&docs, &mut store)?;
            start.elapsed()
        });
        rebuild.push({
            let start = Instant::now();
            std::hint::black_box(store.index());
            start.elapsed()
        });
        tokens = store.tokens_tokenized();
    }
    let full = timed(REPS, || {
        std::hint::black_box(SearchIndex::build(docs.clone()));
    });

    // 自检：热跑（全复用）之后的索引必须与全量重建逐字段相等。
    let mut store = IndexStore::new();
    apply_round(&docs, &mut store)?;
    apply_round(&docs, &mut store)?;
    let parity = store.index() == SearchIndex::build(docs.clone());
    let expected_tokens: usize = docs
        .iter()
        .map(|(_id, text)| adv_index::bm25::tokenize(text).len())
        .sum();

    let payload = json!({
        "docs": docs.len(),
        "reps": REPS,
        "median_ms": {
            "full_build": median_ms(full),
            "cold_apply": median_ms(cold),
            "warm_apply": median_ms(warm),
            "index_rebuild": median_ms(rebuild),
        },
        "tokens_corpus": expected_tokens,
        "tokens_tokenized_after_two_rounds": tokens,
        "warm_round_retokenized": tokens != expected_tokens as u64,
        "parity_with_full_build": parity,
    });
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &payload)?;
    out.write_all(b"\n")?;
    out.flush()?;
    if parity && tokens == expected_tokens as u64 {
        Ok(())
    } else {
        Err(io::Error::other(
            "自检不过：复用产物与全量重建不等，或热轮仍在大面积重切词",
        ))
    }
}

fn main() -> io::Result<()> {
    run()
}
