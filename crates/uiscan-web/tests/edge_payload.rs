//! 回归钉：语料是 **2026-10-16 真 headless Edge 155 跑出来的载荷原文**
//! （`tests/fixtures/payload-edge155.json`，viewport `[1256,808,1256]`）。
//! 解析层 + 判据的期望值直接取自那次运行的控制台读数，不重新"推导"。

use uiscan_core::{Channel, Harvest, Item};
use uiscan_web::harvest_payload_to_items;

const PAYLOAD: &str = include_str!("fixtures/payload-edge155.json");

fn harvest() -> Harvest {
    let v: serde_json::Value = serde_json::from_str(PAYLOAD).expect("语料是合法 JSON");
    Harvest {
        surface: "web:edge155".into(),
        items: harvest_payload_to_items(&v),
    }
}

fn cand_texts(h: &Harvest) -> Vec<(String, Channel)> {
    h.candidates()
        .into_iter()
        .map(|i| (i.text.clone(), i.channel))
        .collect()
}

#[test]
fn viewport_is_real_this_time() {
    // 上一次（Electron webview）viewport 是 [0,0,0]，几何不可信；这一趟是真窗口。
    let v: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
    assert_eq!(
        v["viewport"][0].as_i64(),
        Some(1256),
        "窗口尺寸要真的量到，不然截断/可见性判据全是空转"
    );
}

#[test]
fn all_twelve_planted_strings_are_harvested_from_the_live_page() {
    let h = harvest();
    let got = cand_texts(&h);
    for want in [
        ("字面量-前", Channel::PseudoBefore),
        ("字面量-后", Channel::PseudoAfter),
        ("attr-文案", Channel::PseudoBefore),
        ("A · B", Channel::PseudoAfter),
        ("纯字", Channel::PseudoBefore),
        ("变量-文案", Channel::PseudoAfter),
        ("标记-", Channel::Marker),
        ("占位-文案", Channel::PlaceholderAttr),
        ("title-文案", Channel::TitleAttr),
    ] {
        assert!(
            got.contains(&(want.0.to_string(), want.1)),
            "应采到 {:?}（通道 {:?}），实得：{got:?}",
            want.0,
            want.1
        );
    }
}

#[test]
fn hidden_host_is_collected_but_kept_out_of_candidates() {
    // 这一条是"几何判据在干活"的证据：条目在 items 里，但 visible=false ⇒ 不进候选。
    let h = harvest();
    let hidden: Vec<&Item> = h.items.iter().filter(|i| i.text == "隐藏-前").collect();
    assert_eq!(
        hidden.len(),
        1,
        "隐藏元素上的伪元素文案要被采到：{:?}",
        h.items.len()
    );
    assert!(!hidden[0].visible, "display:none ⇒ w/h 为 0 ⇒ 不许算可见");
    assert!(
        !cand_texts(&h).iter().any(|(t, _)| t == "隐藏-前"),
        "不可见的文案不许进漏翻比对"
    );
}

#[test]
fn counter_token_is_dropped_and_not_counted_as_text() {
    let h = harvest();
    assert!(
        !cand_texts(&h).iter().any(|(t, _)| t.contains("counter(")),
        "未解析令牌出现在候选里就是假文案"
    );
}

#[test]
fn empty_style_block_text_and_normal_markers_do_not_leak_in() {
    let h = harvest();
    let all: Vec<String> = h.items.iter().map(|i| i.text.clone()).collect();
    assert!(
        !all.iter().any(|t| t == "normal" || t == "none"),
        "none/normal 不是文案"
    );
    assert!(
        all.iter().any(|t| t.contains(".lit::before { content:")),
        "先把事实记下来：<style> 的 CSS 源文本确实在 items 里（它就是 DOM 文本）"
    );
    assert!(
        !cand_texts(&h)
            .iter()
            .any(|(t, _)| t.contains(".lit::before { content:")),
        "但那个节点 h=0 ⇒ 不可见，不许进候选——判据是'不进候选'，不是'不出现在采集结果里'"
    );
}

#[test]
fn candidate_count_and_codepoints_are_pinned_to_this_run() {
    let h = harvest();
    assert_eq!(
        cand_texts(&h).len(),
        20,
        "这一份冻结语料的候选数就是 20 条（多一条少一条都说明判据被改了）"
    );
    let cps = h.rendered_codepoints();
    assert_eq!(cps.len(), 22, "非 ASCII 码点数应与控制台读数一致：{cps:?}");
    for c in ['占', '标', '记', '·'] {
        assert!(cps.contains(&c), "码点集合缺 {c}");
    }
    assert!(
        !cps.contains(&'保'),
        "`保` 只在 TUI 语料里，不该出现在这份 Web 载荷里"
    );
}
