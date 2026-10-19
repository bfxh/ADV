//! 真字体上的冒烟：默认 `#[ignore]`（CI 上没有这些 Windows 字体）。
//! 跑：`cargo test -p uiscan-font -- --ignored --nocapture`
//!
//! 这条测的不是"聚合逻辑"，而是**判据在真字体上给不给得出已知答案**：
//! - 拉丁字体（arial）对汉字 `保` 没有字形 ⇒ 必缺；
//! - 中文字体（msyh）对 `保` 有字形 ⇒ 不缺；
//! - 未分配码点 U+10FFFF 在任何字体里都不该有 ⇒ 必缺。
//! 这三条一起，才能区分"判据坏了"和"字体真的缺"。

use std::path::PathBuf;

fn windows_font(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
        .join("Fonts")
        .join(name);
    p.is_file().then_some(p)
}

#[test]
#[ignore = "需要 Windows 系统字体（arial/msyh）"]
fn latin_font_lacks_cjk_and_cjk_font_has_it() {
    let Some(arial) = windows_font("arial.ttf") else {
        panic!("这台机器没有 arial.ttf，跑不了这条真字体冒烟");
    };
    let Some(msyh) = windows_font("msyh.ttc") else {
        panic!("这台机器没有 msyh.ttc，跑不了这条真字体冒烟");
    };

    let mut latin = uiscan_font::FontSet::new();
    latin.add_file(&arial).expect("读 arial");
    println!("arial 面数={}", latin.face_count());
    assert!(latin.covers('A'), "拉丁字体必须有 A");
    assert!(
        !latin.covers('保'),
        "arial 不该有汉字字形（有的话这条要重写）"
    );
    // 实测（2026-10-16）：arial 覆盖 `→`(U+2192)，不覆盖汉字与 ✓/✗ ⇒ 这里只报 保
    assert_eq!(
        latin.missing("保A→".chars()),
        vec!['保'],
        "缺字要只报非 ASCII 里没覆盖的"
    );

    let mut cjk = uiscan_font::FontSet::new();
    cjk.add_file(&msyh).expect("读 msyh");
    println!("msyh 面数={}", cjk.face_count());
    assert!(cjk.covers('保'), "msyh 必须有常用汉字");
    assert!(
        !cjk.missing("保护保存成功错误网络超时".chars())
            .contains(&'保'),
        "中文字体覆盖的汉字不该被报缺"
    );

    // 未分配码点：任何字体都不该有
    assert!(
        !cjk.covers('\u{10FFFF}'),
        "U+10FFFF 是未分配码点，不该被覆盖"
    );
    assert_eq!(
        cjk.missing(std::iter::once('\u{10FFFF}')),
        vec!['\u{10FFFF}']
    );
}
