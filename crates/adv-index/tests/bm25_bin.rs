//! bin 的 JSONL 协议走真路径：起真 `bm25_retrieval.exe`，喂 stdin、读 stdout。
//!
//! 为什么存在（变异门 2026-10-10）：`bin/bm25_retrieval.rs` 的 `main`/`run` 各有一条"整函数体
//! 换成 `Ok(())`"的变异存活——因为过去只有 Python 评测脚本走过这个入口，cargo 侧零断言。
//! 本测试判 exe 的输入输出形状，顺带钉住查询侧筛项（单字项不评分）与并列打破规则在协议层的可见结果。

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

const INPUT: &str = concat!(
    "{\"type\":\"doc\",\"doc_id\":\"a.rs\",\"text\":\"workspace index notes\"}\n",
    "{\"type\":\"doc\",\"doc_id\":\"b.rs\",\"text\":\"index only\"}\n",
    "{\"type\":\"doc\",\"doc_id\":\"c.rs\",\"text\":\"workspace index notes\"}\n",
    "{\"type\":\"query\",\"query_id\":\"q1\",\"text\":\"workspace x\",\"top_k\":5}\n",
    "{\"type\":\"query\",\"query_id\":\"q2\",\"text\":\"x\",\"top_k\":5}\n",
    "{\"type\":\"query\",\"query_id\":\"q3\",\"text\":\"index\",\"top_k\":1}\n",
);

fn run_bin(input: &str) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bm25_retrieval"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起不到 bm25_retrieval");
    child
        .stdin
        .as_mut()
        .expect("没有 stdin 管道")
        .write_all(input.as_bytes())
        .expect("喂不进请求");
    let out = child.wait_with_output().expect("bin 没退出");
    assert!(
        out.status.success(),
        "bin 非零退出：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = String::from_utf8(out.stdout).expect("回包不是 UTF-8");
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("回包不是逐行 JSON"))
        .collect()
}

fn doc_ids(row: &Value) -> Vec<String> {
    row["results"]
        .as_array()
        .expect("results 不是数组")
        .iter()
        .map(|hit| {
            hit["doc_id"]
                .as_str()
                .expect("doc_id 不是字符串")
                .to_string()
        })
        .collect()
}

fn only_row(rows: &[Value], query_id: &str) -> Value {
    let matched: Vec<&Value> = rows
        .iter()
        .filter(|row| row["query_id"] == query_id)
        .collect();
    assert_eq!(
        matched.len(),
        1,
        "{query_id} 应当且仅当回一行，实际 {rows:?}"
    );
    matched[0].clone()
}

#[test]
fn bin_answers_every_query_once_in_request_order() {
    let rows = run_bin(INPUT);
    let ids: Vec<&str> = rows
        .iter()
        .map(|row| row["query_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["q1", "q2", "q3"], "回包顺序必须跟请求顺序一致");
}

#[test]
fn real_terms_score_single_letters_do_not_and_ties_follow_insertion_order() {
    let rows = run_bin(INPUT);
    // q1 = "workspace x"：a.rs 与 c.rs 文本完全相同 ⇒ 分数并列，必须按插入序给出；
    // 单字 x 不参与评分，所以只含 x 的形状不该把 b.rs 带上榜。
    assert_eq!(doc_ids(&only_row(&rows, "q1")), ["a.rs", "c.rs"]);
    // q2 = "x"：纯单字查询 ⇒ 判不到就是空表，不拿噪声凑答案。
    assert_eq!(doc_ids(&only_row(&rows, "q2")), Vec::<String>::new());
    // q3 = "index" 且 top_k=1：三篇都含 index，limit 必须生效。
    assert_eq!(doc_ids(&only_row(&rows, "q3")).len(), 1);
}

#[test]
fn malformed_request_exits_nonzero_with_a_reason() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bm25_retrieval"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起不到 bm25_retrieval");
    child
        .stdin
        .as_mut()
        .expect("没有 stdin 管道")
        .write_all(b"not json\n")
        .expect("喂不进请求");
    let out = child.wait_with_output().expect("bin 没退出");
    assert!(!out.status.success(), "坏请求该非零退出，却成功了");
    assert!(
        !out.stderr.is_empty(),
        "非零退出必须把原因写到 stderr，不能静默"
    );
}
