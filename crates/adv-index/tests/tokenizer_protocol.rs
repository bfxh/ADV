//! 切词口径对拍：Rust `tokenize` 必须逐字复现冻结的 `bench/retrieval/protocol.py`。
//!
//! 为什么存在（变异门 2026-10-10 两轮催出来的）：第一轮点出 15 条切词族存活，补了 1,331 条
//! **标识符**对拍之后第二轮仍剩 6 条——因为 `tokenize` 的入口是任意文本，而纯标识符里
//! 没有"数字/标点后紧跟词"这种形状，`== b'_'` 与 `!= b'_'` 在只喂标识符时不可区分。
//! 所以语料现在两档全收：标识符**不抽样**（11k+ 条）+ 原始文本跨度（文首 160 字 + 全部冻结查询）。
//! 期望一律由 Python 侧算出，不由被测 Rust 生成——否则等于把实现错误一起钉成"正确"。
//!
//! 语料重录：`python -X utf8 bench/retrieval/gen_tokenizer_cases.py`

use adv_index::bm25::tokenize;
use std::path::Path;

fn cases() -> Vec<(String, Vec<String>)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tokenizer_cases.txt");
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到对拍语料 {}：{error}", path.display()));
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.split('\t');
            let kind = parts.next().unwrap_or("");
            assert_eq!(kind, "K", "语料行形状应是 K\\t文本\\ttoken：{line:?}");
            let input = parts.next().unwrap_or("").to_string();
            let want = parts
                .next()
                .unwrap_or("")
                .split(',')
                .filter(|token| !token.is_empty())
                .map(str::to_string)
                .collect();
            (input, want)
        })
        .collect()
}

#[test]
fn tokenizer_matches_frozen_python_protocol() {
    let cases = cases();
    assert!(
        cases.len() > 8000,
        "对拍语料只剩 {} 条，先查生成器是不是又改成抽样了",
        cases.len()
    );

    let bad: Vec<String> = cases
        .iter()
        .filter(|(input, want)| tokenize(input) != *want)
        .map(|(input, want)| format!("{input} -> {:?}，protocol.py 给 {want:?}", tokenize(input)))
        .take(8)
        .collect();
    assert!(
        bad.is_empty(),
        "与冻结口径不一致（前 8 条）：\n{}",
        bad.join("\n")
    );
}

#[test]
fn corpus_covers_both_shapes_the_mutants_hid_in() {
    // 两轮门各暴露一个缺口：标识符要全收（边界变异藏在少数形状里），
    // 而且必须有带分隔符的原始文本（下划线判据的变异在纯标识符上不可区分）。
    let cases = cases();
    let raw = cases
        .iter()
        .filter(|(input, _)| {
            input.contains(' ') || !input.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .count();
    assert!(
        raw > 200,
        "带分隔符的原始文本只有 {raw} 条，跨度那一档没进来（查生成器 SPAN_EVERY）"
    );
    for shape in ["HTTP", "ABCd", "ID3Tag", "__x__y__", "A1_", "n"] {
        assert!(
            cases.iter().any(|(input, _)| input == shape),
            "边界样本 {shape} 不在语料里，生成器的保形条款被改坏了"
        );
    }
}
