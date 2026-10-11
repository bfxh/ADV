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
    decode_doc_lines(stdin.lock().lines())
}

/// [`read_docs`] 的解析面（纯函数）：逐行判型，非 `doc` 的 `type` 与坏 JSON 一律判错。
/// 拆出来是因为 `read_docs` 本体绑死 stdin（单测改不动进程输入），而"什么算一条合法文档"
/// 才是这层真正的判据。
fn decode_doc_lines(
    lines: impl Iterator<Item = io::Result<String>>,
) -> io::Result<Vec<(String, String)>> {
    let mut docs = Vec::new();
    for line in lines {
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

/// 自检判据①（纯函数）：两轮回放后的索引必须与**全量重建**逐字段相等。
/// 写在 `run` 里时它只在 stdout 的 JSON 里可见（单测截不到），于是 `==`→`!=` 的突变
/// 会把"复用产物已经跑偏"报成相等 —— 抽出来才有观察面。
fn parity_of(store: &IndexStore, docs: &[(String, String)]) -> bool {
    store.index() == SearchIndex::build(docs.to_vec())
}

/// 自检判据②（纯函数）：既要 parity，又要热轮没有再大面积重切词，两个条件**都**成立才放行。
/// `&&` 写成 `||` 会让"热轮仍在重切词"这一半单独决定通过 ⇒ 这条测量结论本身就是假的。
fn self_check_passed(parity: bool, tokens: u64, expected_tokens: usize) -> bool {
    parity && tokens == expected_tokens as u64
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
    let parity = parity_of(&store, &docs);
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
    if self_check_passed(parity, tokens, expected_tokens) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn docs2() -> Vec<(String, String)> {
        vec![
            ("a".to_string(), "alpha beta gamma".to_string()),
            ("b".to_string(), "beta delta".to_string()),
        ]
    }

    /// `timed` 的重复次数与样本数必须一致——采样数写错，整份测量的中位就建立在错误样本量上。
    #[test]
    fn timed_calls_the_body_exactly_reps_times_and_returns_that_many_samples() {
        let mut n = 0usize;
        let runs = timed(3, || n += 1);
        assert_eq!(runs.len(), 3, "样本数必须等于 reps");
        assert_eq!(n, 3, "被测体必须恰好跑 reps 次（少跑=把成本藏起来）");

        let mut m = 0usize;
        assert_eq!(
            timed(0, || m += 1).len(),
            0,
            "reps=0 ⇒ 零样本，不许偷偷跑一次"
        );
        assert_eq!(m, 0);
    }

    /// `median_ms` 的两个除/乘与单位换算。中位数取的是**排序后**的中间项，
    /// 所以既要钉奇数长度，也要钉偶数长度（`len/2` 写成 `len%2` 只在偶数上露馅）。
    #[test]
    fn median_ms_picks_the_middle_sample_and_reports_milliseconds() {
        let ms = |v: u64| Duration::from_millis(v);
        assert_eq!(median_ms(vec![ms(9), ms(1), ms(5)]), 5.0, "奇数个取正中");
        assert_eq!(
            median_ms(vec![ms(1), ms(2), ms(3), ms(4)]),
            3.0,
            "偶数个取上中位（len/2）"
        );
        assert_eq!(median_ms(vec![ms(7)]), 7.0, "单个就是它自己");
        // 单位：秒 → 毫秒必须乘 1000（写成加法或除法都会在这里差几个数量级）
        assert_eq!(median_ms(vec![Duration::from_secs_f64(2.5)]), 2500.0);
    }

    /// 输入行的判型：`doc` 才收，别的 `type` 与坏 JSON 一律判错（fail-closed，不静默跳过）。
    #[test]
    fn decode_doc_lines_accepts_docs_and_refuses_everything_else() {
        let ok = decode_doc_lines(
            vec![
                Ok(r#"{"type":"doc","doc_id":"a","text":"x y"}"#.to_string()),
                Ok(r#"{"type":"doc","doc_id":"b","text":"z"}"#.to_string()),
            ]
            .into_iter(),
        )
        .expect("合法文档行不该判错");
        assert_eq!(
            ok,
            vec![
                ("a".to_string(), "x y".to_string()),
                ("b".to_string(), "z".to_string())
            ],
            "doc_id 与 text 的映射必须按字段来，不许错位"
        );

        assert!(
            decode_doc_lines(vec![Ok(r#"{"type":"query","doc_id":"a"}"#.to_string())].into_iter())
                .is_err(),
            "非 doc 的 type 必须判错，不能被当成文档或静默丢掉"
        );
        assert!(
            decode_doc_lines(vec![Ok("{ not json".to_string())].into_iter()).is_err(),
            "坏 JSON 必须判错"
        );
        assert!(
            decode_doc_lines(Vec::<io::Result<String>>::new().into_iter())
                .expect("空输入不是错误")
                .is_empty(),
            "空输入 ⇒ 空文档集"
        );
    }

    /// `apply_round` 说成功了就必须真的把文档折进索引：空店与建好的索引不该相等，
    /// 回放一轮之后该相等（这条同时钉住 `parity_of` 的两个方向）。
    #[test]
    fn apply_round_folds_docs_in_and_parity_of_agrees_with_a_full_rebuild() {
        let docs = docs2();
        let mut store = IndexStore::new();
        assert!(
            !parity_of(&store, &docs),
            "空店的索引不该被判成与全量重建相等（`==`→`!=` 的突变会在这里翻）"
        );

        apply_round(&docs, &mut store).expect("回放不该报错");
        assert!(
            parity_of(&store, &docs),
            "apply_round 报成功却没折进索引 ⇒ 复用产物与全量重建不等"
        );

        // 反向：只喂了部分文档的店，对全量文档必须判不等（否则自检永远为真）
        let mut half = IndexStore::new();
        apply_round(&docs[..1], &mut half).expect("半份回放不该报错");
        assert!(
            !parity_of(&half, &docs),
            "少喂文档却仍判 parity ⇒ 这条自检失去了分辨力"
        );
    }

    /// 自检放行判据是**两个条件都成立**：`&&` 写成 `||` 会让"热轮仍在大面积重切词"
    /// 这一半单独决定通过，测出来的"增量复用有效"就成了假的。
    #[test]
    fn self_check_requires_both_parity_and_no_retokokenizing() {
        assert!(self_check_passed(true, 5, 5), "两半都成立 ⇒ 放行");
        assert!(
            !self_check_passed(false, 5, 5),
            "parity 不成立必须拦住（`&&`→`||` 会在这里放行）"
        );
        assert!(
            !self_check_passed(true, 6, 5),
            "热轮还在重切词必须拦住（`==`→`!=` 会反过来放行）"
        );
        assert!(!self_check_passed(false, 6, 5), "两半都不成立时更要拦");
    }
}
