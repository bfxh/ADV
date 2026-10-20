//! `locale` 子命令的端到端契约（用真实的 TUI 采集 dump，不需要浏览器）。
//!
//! 钉两件事：① 语言包覆盖不全时，**逐条点名**找不到的文案并退 1；
//! ② 覆盖全时退 0；符号行（`→ ← ✓ ✗`）不是待翻译文案，不许混进缺失清单。

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run(locale: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .arg("locale")
        .arg("--json")
        .arg(fixture("tui-dump.json"))
        .arg("--locale")
        .arg(fixture(locale))
        .output()
        .expect("跑 uiscan")
}

#[test]
fn missing_translations_are_named_and_exit_1() {
    let out = run("locale-zh.json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "有缺失必须退 1：{stdout}");
    for want in ["English", "混排 A 本", "ＡＢＣ"] {
        assert!(stdout.contains(want), "要点名 {want}：{stdout}");
    }
    assert!(
        !stdout.contains("保存成功"),
        "语言包覆盖的文案不该出现在缺失里：{stdout}"
    );
    assert!(
        !stdout.contains("→ ← ✓ ✗"),
        "纯符号不是待翻译文案：{stdout}"
    );
}

#[test]
fn full_coverage_exits_0() {
    let out = run("locale-all.json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "全覆盖必须退 0：{stdout}");
    assert!(stdout.contains("找不到 0 条"), "要有汇总：{stdout}");
}
