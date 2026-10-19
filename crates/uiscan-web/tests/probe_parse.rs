//! Web 解析层的判决性测试：语料是**真跑出来的 JSON**，不是手写的理想值。
//! 路径从 crate 目录看是 `../../tests/fixtures/`（仓库根的共享夹具目录）。

use uiscan_core::{Channel, Harvest, Item};
use uiscan_web::probe_rows_to_items;

const FIXTURE: &str = include_str!("fixtures/pseudo-chromium.json");

fn harvest_from(json: &str) -> Harvest {
    Harvest {
        surface: "web:chromium150-probe".to_string(),
        items: probe_rows_to_items(json).unwrap(),
    }
}

fn texts(h: &Harvest) -> Vec<String> {
    let mut v: Vec<String> = h.items.iter().map(|i| i.text.clone()).collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn the_six_planted_css_forms_all_came_back_as_text() {
    let h = harvest_from(FIXTURE);
    for t in [
        "字面量-前",
        "字面量-后",
        "attr-文案",
        "A · B",
        "纯字",
        "变量-文案",
    ] {
        assert!(
            h.items.iter().any(|i| i.text == t),
            "实测能拿到的串不该被丢掉：{t}"
        );
    }
    // ::marker 带引号，剥完引号才算同一条文案
    assert!(
        h.items
            .iter()
            .any(|i| i.text == "标记-" && i.channel == Channel::Marker)
    );
}

#[test]
fn unresolved_counter_tokens_and_empty_decoration_are_not_text() {
    let h = harvest_from(FIXTURE);
    assert!(
        !texts(&h).iter().any(|t| t.contains("counter(")),
        "未解析令牌混进结果集就是假文案：{:?}",
        texts(&h)
    );
    assert!(
        !texts(&h).iter().any(|t| t == "normal" || t.is_empty()),
        "`normal`/空 content 不许进结果"
    );
    assert!(
        !texts(&h).iter().any(|t| t.contains("attr(")),
        "attr() 已被浏览器解析，不该看到函数原文"
    );
}

#[test]
fn css_placeholder_is_not_a_text_channel() {
    // 实测：`::placeholder` 的 content 回来是 `normal`（值被忽略），所以这条通道一个条目都不该有
    let h = harvest_from(FIXTURE);
    assert!(
        !h.items
            .iter()
            .any(|i| i.loc == "pl" && i.text == "占位-文案"),
        "CSS 伪元素通道的占位文案在 Chromium 里拿不到，别假装有"
    );
}

#[test]
fn unusable_geometry_keeps_everything_out_of_the_candidate_gate() {
    // 实测 viewport = [0,0,0]（这台机器上那个 webview 的几何不可信）⇒ 候选必须为空。
    // 这条钉的是"宁可不说，也不报错文案"的方向。
    let h = harvest_from(FIXTURE);
    assert!(h.items.len() > 10, "通道本身要有产出：{}", h.items.len());
    assert_eq!(h.candidates().len(), 0, "几何不可信时不许产出候选条目");
    assert!(
        h.rendered_codepoints().is_empty(),
        "没有可见条目时码点也必须为空"
    );
}

#[test]
fn the_gate_is_not_vacuous_once_geometry_is_real() {
    // 同一批字符串，换成可用矩形 ⇒ 必须进候选（否则上一条测试是空转绿的）
    let json = r#"{"rows":[
      {"cls":"lit","before":"字面量-前","after":"字面量-后","w":500,"h":20,"x":100,"y":100},
      {"cls":"hide","before":"隐藏-前","w":0,"h":0,"x":0,"y":0}
    ]}"#;
    let items: Vec<Item> = probe_rows_to_items(json).unwrap();
    let h = Harvest {
        surface: "web:synthetic".to_string(),
        items,
    };
    let got = texts(&h);
    assert_eq!(
        got,
        vec!["字面量-前", "字面量-后", "隐藏-前"],
        "原始条目三条都在：{got:?}"
    );
    let cand: Vec<String> = h.candidates().into_iter().map(|i| i.text.clone()).collect();
    assert_eq!(
        cand,
        vec!["字面量-前".to_string(), "字面量-后".to_string()],
        "display:none 那条必须被几何判据挡住"
    );
}
