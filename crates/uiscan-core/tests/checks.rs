//! 判据的判决性测试。期望值全部来自 2026-10-16 的两场实测：
//! ①Chromium 150（Qoder 内置 webview）在 `tests/fixtures/i18n-pseudo.html` 上跑
//!   `getComputedStyle(el,'::before'/'::after'/'::marker').content` 的原样返回；
//! ②UIA（x86_64-pc-windows-gnu）枚举顶层窗口拿到的矩形。
//! 不靠"看起来合理"写期望——浏览器给什么，这里钉什么。

use uiscan_core::{
    Rect, channel_gap, css_content_to_text, looks_translatable, non_ascii, unresolved_generator,
    usable_rect,
};

#[test]
fn content_before_and_after_come_back_unquoted_and_are_passed_through() {
    // 实测：`before: "字面量-前"`（JSON 里没有内层引号）
    assert_eq!(
        css_content_to_text("字面量-前").as_deref(),
        Some("字面量-前")
    );
    assert_eq!(
        css_content_to_text("字面量-后").as_deref(),
        Some("字面量-后")
    );
}

#[test]
fn marker_content_keeps_its_quotes_so_it_must_be_stripped() {
    // 实测：`cnt_li_marker: "\"标记-\""` —— marker 与 before/after **不对称**，带引号
    assert_eq!(css_content_to_text("\"标记-\"").as_deref(), Some("标记-"));
    assert_eq!(css_content_to_text("'单引号'").as_deref(), Some("单引号"));
}

#[test]
fn multi_segment_concatenation_is_joined_in_order() {
    // 分段形式：拼完不许留引号或分隔空格（`"A" " · " "B"` → `A · B`）
    assert_eq!(
        css_content_to_text("\"A\" \" · \" \"B\"").as_deref(),
        Some("A · B")
    );
    // 实测 Chromium 已经把多段展平成不带引号的 `A · B`，这条也得原样通过
    assert_eq!(css_content_to_text("A · B").as_deref(), Some("A · B"));
}

#[test]
fn resolved_and_unresolved_generators_are_told_apart_by_measurement_not_by_guess() {
    // 实测已解析：attr() 与 var() 返回的是真值 ⇒ 不能当噪音丢
    assert_eq!(
        css_content_to_text("attr-文案").as_deref(),
        Some("attr-文案")
    );
    assert_eq!(
        css_content_to_text("变量-文案").as_deref(),
        Some("变量-文案")
    );
    // 实测未解析：counter 返回令牌原文 `counter(i) ". "` ⇒ 必须判成"不是文案"
    assert_eq!(css_content_to_text("counter(i) \". \""), None);
    assert!(unresolved_generator("counter(i) \". \""));
    assert!(!unresolved_generator("attr-文案"));
}

#[test]
fn none_and_normal_are_no_content() {
    assert_eq!(css_content_to_text("none"), None);
    assert_eq!(css_content_to_text("normal"), None);
    assert_eq!(css_content_to_text("   "), None);
    // 实测：装饰性 `content:""` 回来是空串，不能进结果集
    assert_eq!(css_content_to_text("\"\"").as_deref(), Some(""));
    assert!(!looks_translatable(""), "空文案不许当待翻译条目");
}

#[test]
fn unicode_escapes_arrive_decoded() {
    // 实测：`content:"\7eaf\5b57"` 回来是 `纯字`
    assert_eq!(css_content_to_text("纯字").as_deref(), Some("纯字"));
}

#[test]
fn offscreen_and_zero_rects_are_not_visible() {
    // 实测（UIA 顶层窗口）：`(-9999,-9999)-(-9998,-9998)` 与 `(0,0)-(0,0)`
    assert!(
        !usable_rect(Rect {
            x: -9999,
            y: -9999,
            w: 1,
            h: 1
        }),
        "屏外哨兵矩形不许算可见"
    );
    assert!(
        !usable_rect(Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0
        }),
        "零尺寸不许算可见"
    );
    assert!(usable_rect(Rect {
        x: 106,
        y: 236,
        w: 1579,
        h: 417
    }));
}

#[test]
fn symbols_stay_in_the_candidate_gate_for_coverage_checks() {
    // 豆腐块检查需要码点，符号也算；但"像文案"的判据只认字母
    assert!(looks_translatable("错误：网络超时"));
    assert!(!looks_translatable("→ ← ✓ ✗"), "纯符号不进漏翻比对");
    let cps = non_ascii(vec!["→ ← ✓".to_string()]);
    assert_eq!(cps.len(), 3, "符号仍要被收进码点集合，走覆盖检查那条路");
}

#[test]
fn channel_gap_reports_both_directions() {
    let dom = vec!["宿主".to_string(), "只在DOM".to_string()];
    let ax = vec!["宿主".to_string(), "字面量-前".to_string()];
    assert_eq!(
        channel_gap(&dom, &ax),
        (vec!["只在DOM".to_string()], vec!["字面量-前".to_string()]),
        "两个方向都要点名，单向报就是半个判据"
    );
    let same = vec!["x".to_string()];
    assert_eq!(channel_gap(&same, &same), (vec![], vec![]));
}
