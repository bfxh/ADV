//! 磁盘缓存（M4-4b）的判据：跨进程复用必须**零重切词**，且被改过的原文不许再被当成"没变"。
//!
//! 参照物仍然是全量重建（`SearchIndex::build`），不是本模块的另一段代码；
//! "零重切词"用的是 `IndexStore::tokens_tokenized()`——读回的 store 从 0 起计，
//! 所以这个数若不为 0，就是真切了词，糊不过去。

use adv_index::bm25::SearchIndex;
use adv_index::incremental::{CACHE_VERSION, Delta, IndexStore};

/// 取判错的原因文本（不用 `expect_err`：那会逼产品面为测试 ergonomics 去 derive `Debug`）。
fn reject(result: &Result<IndexStore, String>, what: &str) -> String {
    match result {
        Err(reason) => reason.clone(),
        Ok(_) => panic!("{what}：却读成功了"),
    }
}

fn docs() -> Vec<(String, String)> {
    vec![
        (
            "a.rs".to_string(),
            "sanitize unrelated cache setting".to_string(),
        ),
        (
            "b.rs".to_string(),
            "untrusted input reaches command sink".to_string(),
        ),
        (
            "c.rs".to_string(),
            "cache setting for workspace load".to_string(),
        ),
    ]
}

#[test]
fn export_is_byte_stable_and_restart_reuses_everything() {
    let input = docs();
    let mut store = IndexStore::new();
    store.apply(input.clone()).expect("首轮");
    let first = store.to_json().expect("导出");
    store.apply(input.clone()).expect("第二轮");
    assert_eq!(
        store.to_json().expect("再导出"),
        first,
        "同一批文档两次导出必须逐字节相同（否则缓存没法 diff、也没法钉哈希）"
    );

    // 模拟"进程重启"：只吃导出的字节，不喂原文之外的任何东西。
    let mut restored = IndexStore::from_json(&first).expect("读回");
    let delta = restored.apply(input.clone()).expect("重启后首轮 apply");
    assert_eq!(
        delta,
        Delta {
            added: 0,
            changed: 0,
            reused: 3,
            dropped: 0
        },
        "重启后同一批文档必须全部复用"
    );
    assert_eq!(
        restored.tokens_tokenized(),
        0,
        "复用了还切词 ⇒ 磁盘缓存白存（这条就是这个判据要抓的形状）"
    );
    assert_eq!(
        restored.index(),
        SearchIndex::build(input),
        "重启后的索引与全量重建不等"
    );
}

#[test]
fn tampered_text_equal_length_is_not_reused() {
    // 概率性指纹最容易在这种形状上出事：长度不变、内容变了。逐字节比对必须认出来。
    let input = docs();
    let mut store = IndexStore::new();
    store.apply(input.clone()).expect("首轮");
    let json = store.to_json().expect("导出");
    let mut parsed: serde_json::Value = serde_json::from_str(&json).expect("自产 JSON 必可解析");
    let original = parsed["docs"][0]["text"]
        .as_str()
        .expect("text 字段")
        .to_string();
    assert_eq!(original.len(), "sanitize unrelated cache settinX".len());
    parsed["docs"][0]["text"] =
        serde_json::Value::String("sanitize unrelated cache settinX".to_string());
    let tampered = serde_json::to_string(&parsed).expect("回写");

    let mut restored = IndexStore::from_json(&tampered).expect("读回被改的缓存");
    let delta = restored.apply(input.clone()).expect("apply 真原文");
    assert_eq!(
        (delta.reused, delta.changed),
        (2, 1),
        "被改过的那一篇不许记复用"
    );
    assert!(
        restored.tokens_tokenized() > 0,
        "该重切的那一篇必须真切了词"
    );
    assert_eq!(
        restored.index(),
        SearchIndex::build(input),
        "重切之后索引必须回到与全量重建相等"
    );
}

#[test]
fn unreadable_cache_is_rejected_not_guessed() {
    let mut store = IndexStore::new();
    store.apply(docs()).expect("首轮");
    let json = store.to_json().expect("导出");
    let mut parsed: serde_json::Value = serde_json::from_str(&json).expect("自产 JSON 必可解析");

    parsed["version"] = serde_json::Value::from(CACHE_VERSION + 1);
    let future = serde_json::to_string(&parsed).expect("回写");
    let why = reject(&IndexStore::from_json(&future), "更高的版本号必须判错");
    assert!(why.contains("版本"), "报错要点名是版本问题：{why}");

    parsed["version"] = serde_json::Value::from(CACHE_VERSION);
    parsed["order"] = serde_json::json!(["ghost.rs"]);
    let dangling = serde_json::to_string(&parsed).expect("回写");
    let why = reject(
        &IndexStore::from_json(&dangling),
        "order 引用不存在的 id 必须判错",
    );
    assert!(why.contains("ghost.rs"), "报错要指名是哪个 id：{why}");

    assert!(
        IndexStore::from_json("not json at all").is_err(),
        "非本格式的输入必须判错"
    );
}
