//! 聚合逻辑的判决性测试：不依赖任何字体文件，CI 上可跑。

use uiscan_font::missing_where;

#[test]
fn ascii_and_control_chars_are_not_candidates() {
    // 缺字形通常是"非 ASCII 文字/符号"的事；ASCII 是各字体必有的基线，控制符不渲染。
    let missing = missing_where("A1 \n\t保".chars(), |_| false);
    assert_eq!(
        missing,
        vec!['保'],
        "只有非 ASCII 非控制符才进缺字集合：{missing:?}"
    );
}

#[test]
fn missing_is_sorted_and_deduplicated() {
    let missing = missing_where("文保字文保".chars(), |_| false);
    assert_eq!(
        missing,
        vec!['保', '字', '文'],
        "要按码点升序且去重：{missing:?}"
    );
}

#[test]
fn covered_chars_are_not_reported() {
    let covered = |c: char| c == '保' || c == '护';
    assert_eq!(missing_where("保护文".chars(), covered), vec!['文']);
    assert!(
        missing_where("保护".chars(), covered).is_empty(),
        "全covered 必须空"
    );
}

#[test]
fn empty_input_is_empty_output() {
    assert!(missing_where(std::iter::empty(), |_| false).is_empty());
}
