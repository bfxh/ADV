//! 增量档的**相等判据**（M4-4a）：增量产物必须与全量重建逐字段相等。
//!
//! 期望值全部取自被测实现之外：参照物是另一条路径（`SearchIndex::build` 全量重建），
//! token 数由测试自己调 `tokenize` 数出来 —— 这样"增量偷偷全量重切"或"复用漏了某篇"
//! 都会判红，而不是跟着实现一起自证。

use adv_index::bm25::{SearchIndex, tokenize};
use adv_index::incremental::{Delta, IndexStore};

fn a() -> (String, String) {
    (
        "a.rs".to_string(),
        "sanitize unrelated cache setting".to_string(),
    )
}
fn b() -> (String, String) {
    (
        "b.rs".to_string(),
        "untrusted input reaches command sink".to_string(),
    )
}
fn c() -> (String, String) {
    (
        "c.rs".to_string(),
        "cache setting for workspace load".to_string(),
    )
}
fn d() -> (String, String) {
    (
        "d.rs".to_string(),
        "command sink reached by load path".to_string(),
    )
}

const PROBES: [&str; 4] = ["untrusted command", "cache setting", "sink", "x"];

/// 断言当前缓存与"按同一顺序全量重建"相等：结构相等 + 探针结果相等。
fn assert_parity(store: &IndexStore, docs: &[(String, String)]) {
    let rebuilt = SearchIndex::build(docs.iter().cloned());
    let got = store.index();
    assert_eq!(got, rebuilt, "增量产物与全量重建结构不等");
    for probe in PROBES {
        assert_eq!(
            got.search(probe, 10),
            rebuilt.search(probe, 10),
            "探针 {probe:?} 的排序与全量重建不等"
        );
    }
}

#[test]
fn add_then_noop_then_change_add_drop_keeps_parity() {
    let mut store = IndexStore::new();

    let step1 = vec![a(), b(), c()];
    let got = store.apply(step1.clone()).expect("首轮 apply");
    assert_eq!(
        got,
        Delta {
            added: 3,
            changed: 0,
            reused: 0,
            dropped: 0
        },
        "首轮账应全是新增"
    );
    assert_parity(&store, &step1);

    let got = store.apply(step1.clone()).expect("noop apply");
    assert_eq!(
        got,
        Delta {
            added: 0,
            changed: 0,
            reused: 3,
            dropped: 0
        },
        "同一批字节没变应全部复用"
    );
    assert_parity(&store, &step1);

    let mut step3 = vec![a(), b(), d()];
    step3[1] = (
        "b.rs".to_string(),
        "b.rs rewritten: token token".to_string(),
    );
    let got = store.apply(step3.clone()).expect("变更轮 apply");
    assert_eq!(
        got,
        Delta {
            added: 1,
            changed: 1,
            reused: 1,
            dropped: 1
        },
        "变更轮应各记一笔（新增 d / 改 b / 复用 a / 掉 c）"
    );
    assert_parity(&store, &step3);

    let step4 = vec![step3[2].clone(), step3[0].clone(), step3[1].clone()];
    let got = store.apply(step4.clone()).expect("换序轮 apply");
    assert_eq!(
        got,
        Delta {
            added: 0,
            changed: 0,
            reused: 3,
            dropped: 0
        },
        "只换顺序不该重切任何一篇"
    );
    assert_parity(&store, &step4);
}

#[test]
fn reuse_actually_skips_tokenization() {
    let mut store = IndexStore::new();
    let docs = vec![a(), b(), c()];
    store.apply(docs.clone()).expect("首轮 apply");

    // 锚：首轮切掉的 token 数 = 三篇各自 tokenize 长度之和（测试自己数，不读实现内部）。
    let expected: u64 = docs
        .iter()
        .map(|(_id, text)| tokenize(text).len() as u64)
        .sum();
    assert_eq!(store.tokens_tokenized(), expected, "首轮应切满整批");

    store.apply(docs.clone()).expect("noop apply");
    assert_eq!(
        store.tokens_tokenized(),
        expected,
        "字节没变的批次复用后，喂进 tokenize 的 token 数不该上涨"
    );
    assert_eq!(store.retokenized(), 3, "复用轮不该再算切词篇数");

    let changed = vec![a(), d(), c()];
    store.apply(changed.clone()).expect("新增 d 的 apply");
    let expected_after = expected + tokenize(&d().1).len() as u64;
    assert_eq!(
        store.tokens_tokenized(),
        expected_after,
        "只该多出新增那一篇的 token 数（b→d 的账见 dropped/added 用例）"
    );
}

#[test]
fn changed_bytes_retokenize_that_doc_only() {
    let mut store = IndexStore::new();
    let docs = vec![a(), b()];
    store.apply(docs.clone()).expect("首轮 apply");
    let before = store.tokens_tokenized();

    let edited = vec![
        a(),
        (
            "b.rs".to_string(),
            "untrusted input reaches command sink twice".to_string(),
        ),
    ];
    let delta = store.apply(edited.clone()).expect("改 b 的 apply");
    assert_eq!(
        (delta.changed, delta.reused, delta.added),
        (1, 1, 0),
        "只该记一篇变更"
    );
    assert_eq!(
        store.tokens_tokenized(),
        before + tokenize(&edited[1].1).len() as u64,
        "变更后只重切那一篇"
    );
    assert_parity(&store, &edited);
}

#[test]
fn duplicate_id_is_rejected_without_touching_the_store() {
    let mut store = IndexStore::new();
    let docs = vec![a(), b()];
    store.apply(docs.clone()).expect("首轮 apply");
    let index_before = store.index();
    let tokens_before = store.tokens_tokenized();

    let bad = store.apply(vec![a(), a()]);
    assert!(bad.is_err(), "重复 id 必须判错，不能静默取第一条");

    assert_eq!(store.index(), index_before, "判错的输入不该改动索引");
    assert_eq!(
        store.tokens_tokenized(),
        tokens_before,
        "判错的输入不该切词"
    );
    assert_parity(&store, &docs);
}

#[test]
fn same_length_different_content_is_not_reused() {
    // 钉的是"复用判据是逐字节相等"这一条：若实现退化成只比长度（或只比指纹），
    // 这里会静默复用旧 tf 表 ⇒ 与全量重建不等 ⇒ 判红。
    let mut store = IndexStore::new();
    let before = vec![
        ("x.rs".to_string(), "sink alpha".to_string()),
        ("y.rs".to_string(), "cache beta".to_string()),
    ];
    store.apply(before).expect("首轮 apply");

    let after = vec![
        ("x.rs".to_string(), "sink gamma".to_string()), // 与 "sink alpha" 同为 10 字节
        ("y.rs".to_string(), "cache beta".to_string()),
    ];
    assert_eq!(
        after[0].1.len(),
        "sink alpha".len(),
        "用例前提：新内容必须与旧内容等长，否则这条判据什么也没测到"
    );
    let delta = store.apply(after.clone()).expect("等长变更 apply");
    assert_eq!(
        (delta.changed, delta.reused),
        (1, 1),
        "等长但不同字节 ⇒ 记变更，不许记复用"
    );
    assert_parity(&store, &after);
}

#[test]
fn identical_content_under_different_ids_is_not_confused() {
    // 两篇内容一样、id 不同：索引必须按 id 记账，不能按内容合并。
    let mut store = IndexStore::new();
    let text = "shared body text sink".to_string();
    let docs = vec![
        ("p.rs".to_string(), text.clone()),
        ("q.rs".to_string(), text),
    ];
    store.apply(docs.clone()).expect("首轮 apply");
    let delta = store.apply(docs.clone()).expect("第二轮 apply");
    assert_eq!(
        (delta.reused, delta.added, delta.changed),
        (2, 0, 0),
        "两篇都该独立复用"
    );
    assert_eq!(
        store.index().search("shared", 10).len(),
        2,
        "同内容两篇都要在结果里"
    );
    assert_parity(&store, &docs);
}
