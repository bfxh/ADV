//! TUI 采集的判决性测试。字节流与期望值都取自 2026-10-16 在 `E:\tmp\uic-probe\vt`
//! 上真跑出来的输出（`cargo run` 原样贴回），不是按规范推的。

use uiscan_core::{Channel, Harvest, Item};
use uiscan_tui::harvest;

const STREAM: &str = concat!(
    "\x1b[?1049h",
    "\x1b[2J",
    "\x1b[1;1H保存成功",
    "\x1b[2;1H\x1b[31m错误：\x1b[0m网络超时",
    "\x1b[3;1HEnglish",
    "\x1b[3;10H混排 A 本",
    "\x1b[4;1HＡＢＣ",
    "\x1b[5;1H→ ← ✓ ✗",
);

fn texts(h: &Harvest) -> Vec<String> {
    h.items.iter().map(|i| i.text.clone()).collect()
}

#[test]
fn rows_harvest_exactly_as_measured() {
    let h = harvest(STREAM.as_bytes(), 6, 40);
    assert_eq!(
        texts(&h),
        vec![
            "保存成功",
            "错误：网络超时",
            "English",
            "混排 A 本",
            "ＡＢＣ",
            "→ ← ✓ ✗",
        ],
        "行内单空格不许断词、两处跳格要断段"
    );
    assert!(
        h.surface.starts_with("tui:6x40 alt=true"),
        "实测进的是 alt screen：{}",
        h.surface
    );
}

#[test]
fn locations_carry_the_cell_column_of_the_run_start() {
    let h = harvest(STREAM.as_bytes(), 6, 40);
    let by: Vec<&Item> = h
        .items
        .iter()
        .filter(|i| i.channel == Channel::TermCell)
        .collect();
    assert_eq!(by[0].loc, "r0c0");
    // `混排 A 本` 是光标跳到第 10 列（0 基 = 9）写的
    assert_eq!(by[3].loc, "r2c9", "起始列要落到真格子，不能一律记行首");
}

#[test]
fn wide_cjk_gives_one_cell_per_char_not_two() {
    // 实测：每个汉字 = 主格 + 续格；采集器跳过续格 ⇒ 4 个汉字是一段，不会被切成 4 段
    let h = harvest(STREAM.as_bytes(), 6, 40);
    assert_eq!(h.items[0].text, "保存成功");
}

#[test]
fn ambiguous_width_symbols_are_kept_as_symbols_and_stay_out_of_the_text_gate() {
    let h = harvest(STREAM.as_bytes(), 6, 40);
    let sym = h
        .items
        .iter()
        .find(|i| i.text.contains('→'))
        .expect("实测有这行");
    assert!(
        !uiscan_core::looks_translatable(&sym.text),
        "纯符号不该进漏翻候选"
    );
    assert!(
        h.rendered_codepoints().contains(&'→'),
        "但码点要留着，覆盖检查要用"
    );
    assert!(
        h.rendered_codepoints().contains(&'保'),
        "汉字码点要在集合里"
    );
}

#[test]
fn empty_stream_harvests_nothing_instead_of_crashing() {
    let h = harvest(b"", 6, 40);
    assert!(h.items.is_empty(), "空屏不许造出条目：{:?}", texts(&h));
    assert!(h.rendered_codepoints().is_empty());
}
