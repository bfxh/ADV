//! 截断判据的判决性测试：语料是 2026-10-20 用 Edge 155 headless 真跑
//! `tests/fixtures/truncation.html` 后冻结的原始载荷。期望值逐格对应夹具里的五种形状。

use uiscan_web::clip::{ClipKind, findings};

const PAYLOAD: &str = include_str!("fixtures/trunc-payload.json");

fn got() -> Vec<(String, ClipKind)> {
    let v: serde_json::Value = serde_json::from_str(PAYLOAD).expect("语料是 JSON");
    findings(&v).into_iter().map(|f| (f.loc, f.kind)).collect()
}

#[test]
fn the_five_planted_shapes_split_exactly_as_measured() {
    let got = got();
    assert_eq!(
        got,
        vec![
            ("p.clip".to_string(), ClipKind::Horizontal),
            ("p.ell".to_string(), ClipKind::Ellipsis),
            ("p.vclip".to_string(), ClipKind::Vertical),
            ("p.clamp".to_string(), ClipKind::Vertical),
        ],
        "放得下的不报、横向/省略号/纵向各归各的：{got:?}"
    );
}

#[test]
fn fits_visible_text_is_not_reported() {
    let got = got();
    assert!(
        !got.iter().any(|(loc, _)| loc == "p.fits"),
        "160/160 放得下，不该报"
    );
}

#[test]
fn invisible_css_and_title_nodes_are_excluded() {
    // title/style 的 rect 与几何都是 0：它们是 DOM 文本但不是"画出来的文案"。
    let got = got();
    assert!(!got.iter().any(|(loc, _)| loc == "title" || loc == "style"));
}

#[test]
fn visible_overflow_is_not_clipping() {
    // overflow:visible 的文字会画到外面，不算"被裁"——即便 scrollWidth 更大。
    let v: serde_json::Value = serde_json::from_str(
        r#"{"items":[{"loc":"x","ch":"DomText","txt":"长文案","rect":{"x":0,"y":0,"w":10,"h":20},
            "sw":300,"cw":100,"sh":20,"chh":20,"ovx":"visible","ovy":"visible","ell":"clip"}]}"#,
    )
    .unwrap();
    assert!(findings(&v).is_empty(), "overflow:visible 不许报裁切");
}

#[test]
fn exact_fit_with_hidden_overflow_is_not_clipping() {
    // 差 1px 以内是取整噪声：sw == cw 不许报。
    let v: serde_json::Value = serde_json::from_str(
        r#"{"items":[{"loc":"x","ch":"DomText","txt":"刚好","rect":{"x":0,"y":0,"w":10,"h":20},
            "sw":161,"cw":160,"sh":24,"chh":24,"ovx":"hidden","ovy":"hidden","ell":"clip"}]}"#,
    )
    .unwrap();
    assert!(findings(&v).is_empty(), "只差 1px 属于取整噪声，不报");
}
