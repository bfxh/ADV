//! BM25 分数金样：把评分公式的形状钉成数字，而不是只钉"谁排第一"。
//!
//! 为什么存在（变异门 2026-10-10）：`SearchIndex::search` 一轮点出 **26 条存活变异**，
//! `build` 的 avgdl 除法也活着——因为原有测试只断言排序首名与非空，对
//! `idf`/`length_norm`/`tf` 三处算术几乎不敏感。金样的期望由
//! `bench/retrieval/gen_bm25_cases.py` 里**独立转写的发表式**算出（Elastic Practical BM25 part 3），
//! 不由被测 Rust 生成；切词由 `tokenizer_protocol.rs` 那 1,331 条对拍钉住与 `protocol.py` 一致。
//!
//! 重录：`python -X utf8 bench/retrieval/gen_bm25_cases.py > crates/adv-index/tests/data/bm25_scores.tsv`

use adv_index::bm25::{B, K1, SearchIndex};
use std::collections::BTreeMap;
use std::path::Path;

fn fixture_text() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/bm25_scores.tsv");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到分数金样 {}：{error}", path.display()))
}

/// 一条用例：文档集 + 查询 + 期望的有序 (doc_id, score)。
#[derive(Default)]
struct Case {
    docs: Vec<(String, String)>,
    query: String,
    top_k: usize,
    want: Vec<(String, f64)>,
}

fn cases() -> BTreeMap<String, Case> {
    let body = fixture_text();
    let mut out: BTreeMap<String, Case> = BTreeMap::new();
    for line in body.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let kind = parts.next().unwrap_or("");
        let id = parts.next().unwrap_or("").to_string();
        let case = out.entry(id).or_default();
        match kind {
            "D" => case.docs.push((
                parts.next().unwrap_or("").to_string(),
                parts.next().unwrap_or("").to_string(),
            )),
            "Q" => {
                case.query = parts.next().unwrap_or("").to_string();
                case.top_k = parts
                    .next()
                    .unwrap_or("10")
                    .parse()
                    .expect("top_k 应是整数");
            }
            "W" => {
                let blob = parts.next().unwrap_or("");
                case.want = blob
                    .split(',')
                    .filter(|pair| !pair.is_empty())
                    .map(|pair| {
                        let (doc, score) = pair.split_once(':').expect("期望形状应是 doc:score");
                        (doc.to_string(), score.parse().expect("score 应是浮点"))
                    })
                    .collect();
            }
            other => panic!("金样里出现未知行类型 {other:?}：{line:?}"),
        }
    }
    out
}

#[test]
fn scores_match_the_published_bm25_formula() {
    let cases = cases();
    assert!(
        cases.len() >= 6,
        "金样用例数掉到 {}，先查生成器",
        cases.len()
    );
    let mut bad: Vec<String> = Vec::new();
    for (id, case) in &cases {
        let index = SearchIndex::build(case.docs.clone());
        let got = index.search(&case.query, case.top_k);
        let got_ids: Vec<&str> = got.iter().map(|(d, _)| d.as_str()).collect();
        let want_ids: Vec<&str> = case.want.iter().map(|(d, _)| d.as_str()).collect();
        if got_ids != want_ids {
            bad.push(format!(
                "{id}: 排序不一致，实得 {got_ids:?}，期望 {want_ids:?}"
            ));
            continue;
        }
        for ((doc, got_score), (_, want_score)) in got.iter().zip(case.want.iter()) {
            if (got_score - want_score).abs() > 1e-9 {
                bad.push(format!(
                    "{id}/{doc}: 分数 {got_score:.12} != 期望 {want_score:.12}"
                ));
            }
        }
    }
    assert!(bad.is_empty(), "与发表式对不上：\n{}", bad.join("\n"));
}

#[test]
fn exact_ties_keep_insertion_order_in_the_golden() {
    // 并列用例的价值在于：它把"分数完全相等"这件事本身钉住——
    // 任何改动 idf/length_norm 的变异都会让两个 0.447138587823 不再相等。
    let all = cases();
    let case = all.get("exact_tie").expect("金样缺 exact_tie 用例");
    let index = SearchIndex::build(case.docs.clone());
    let got = index.search(&case.query, case.top_k);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, "a.rs");
    assert_eq!(got[1].0, "c.rs");
    assert!(
        (got[0].1 - got[1].1).abs() < 1e-15,
        "同文两篇的分数必须完全相等，实得 {} 与 {}",
        got[0].1,
        got[1].1
    );
}

#[test]
fn golden_declares_the_same_parameters_as_the_implementation() {
    // 常数漂了要让金样**指名道姓地**红，而不是甩一堆对不上的分数。
    let body = fixture_text();
    let header = body
        .lines()
        .find(|line| line.starts_with("# params"))
        .expect("金样缺 # params 头");
    let declared = |key: &str| -> f64 {
        header
            .split_whitespace()
            .find_map(|token| token.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("金样头缺 {key}="))
            .parse()
            .unwrap_or_else(|_| panic!("{key} 不是浮点"))
    };
    assert_eq!(declared("k1"), K1, "金样 k1 与实现不一致，先重录金样");
    assert_eq!(
        declared("b"),
        B,
        "金样 b 与实现不一致：改常数必须同步生成器并重录"
    );
}

#[test]
fn golden_covers_the_shapes_mutation_gate_flagged() {
    // 生成器一改就把形状丢掉的话，这里当场红：每条形状都必须有期望或明确为空。
    let cases = cases();
    for id in [
        "length_vs_tf",
        "df_weight",
        "exact_tie",
        "single_letter_dropped",
        "no_match",
        "multi_term_sum",
        "avg_len_across_three",
    ] {
        let case = cases.get(id).unwrap_or_else(|| panic!("金样缺用例 {id}"));
        assert!(!case.docs.is_empty(), "{id} 没有文档");
        assert!(!case.query.is_empty(), "{id} 没有查询");
        if id == "no_match" {
            assert!(case.want.is_empty(), "no_match 的期望必须是空");
        } else {
            assert!(!case.want.is_empty(), "{id} 的期望空了，等于没钉");
        }
    }
}
