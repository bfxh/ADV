//! 真桌面上的冒烟：默认 `#[ignore]`，本机手动跑（CI 没有交互式桌面）。
//!
//! 它**不是**"覆盖率"，是"这条路在本机还活着"的证据：
//! 跑 `cargo test -p uiscan-gui -- --ignored --nocapture`。

#[test]
#[ignore = "需要真实 Windows 桌面会话"]
fn live_uia_lists_windows_and_harvests_the_focused_one() {
    let wins = uiscan_gui::list_windows().expect("本机 UIA 应该能枚举顶层窗口");
    assert!(!wins.is_empty(), "真桌面上至少有一个顶层窗口");
    println!("顶层窗口 {} 个", wins.len());
    for w in wins.iter().take(5) {
        println!(
            "  {:?} pid={} rect={:?} offscreen={}",
            w.name, w.pid, w.rect, w.offscreen
        );
    }

    // 焦点窗口可能没有（无焦点时 UIA 返回错误），这条只要求"有的话能采出东西"
    match uiscan_gui::harvest_focused(8, 5000) {
        Ok(s) => {
            println!(
                "焦点窗口采到 {} 条（surface={}）",
                s.harvest.items.len(),
                s.harvest.surface
            );
            for it in s.harvest.candidates().iter().take(5) {
                println!("  {} {:?}", it.loc, it.text);
            }
            println!("TextPattern 对账发现 {} 条", s.text.len());
        }
        Err(e) => println!("焦点窗口没采到（可接受）：{e}"),
    }
}
