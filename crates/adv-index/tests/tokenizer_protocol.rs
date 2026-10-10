//! 切词口径对拍：Rust `tokenize` 必须逐字复现冻结的 `bench/retrieval/protocol.py`。
//!
//! 为什么存在（变异门 2026-10-10 催出来的）：`tokenize`/`split_identifier`/
//! `split_alphanumeric`/`push_token` 一轮跑出 15 条存活变异，根因是原有测试只钉了
//! 3 个手挑字符串。语料里的期望**全部由 Python 侧算出**（规则作者之外的观测），
//! 不是拿 Rust 实现自己生成——否则等于把可能的实现错误一起钉成"正确"。
//!
//! 语料重录：`python -X utf8 bench/retrieval/gen_tokenizer_cases.py`
//! （源料 = `corpus.pin` 那批文档里的 11,839 个标识符，抽样 1,331 条 + 边界样本全保）

use adv_index::bm25::tokenize;
use std::path::Path;

fn cases() -> Vec<(String, Vec<String>)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/tokenizer_cases.txt");
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不到对拍语料 {}：{error}", path.display()));
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (input, expected) = line
                .split_once('\t')
                .unwrap_or_else(|| panic!("语料行缺 TAB 分隔：{line:?}"));
            let want = expected
                .split(',')
                .filter(|token| !token.is_empty())
                .map(str::to_string)
                .collect();
            (input.to_string(), want)
        })
        .collect()
}

#[test]
fn tokenizer_matches_frozen_python_protocol() {
    let cases = cases();
    assert!(
        cases.len() > 1000,
        "对拍语料缩水到 {} 条，先查生成器",
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
fn boundary_shapes_are_pinned() {
    // 变异门点名过的形状：尾大写、数字段、下划线混合、单字母。任一处退化即红。
    let cases = cases();
    for shape in ["HTTP", "ABCd", "ID3Tag", "__x__y__", "A1_", "n"] {
        assert!(
            cases.iter().any(|(input, _)| input == shape),
            "边界样本 {shape} 不在语料里，生成器的保形条款被改坏了"
        );
    }
}
