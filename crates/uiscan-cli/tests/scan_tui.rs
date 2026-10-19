//! `scan` 的端到端契约（只用 TUI 面：不需要浏览器/桌面）。
//!
//! 钉三件事：
//! 1. 没给字体栈 ⇒ **退 2 并点名**（工具不替应用猜字体）；
//! 2. 只给拉丁字体时，中文采集必须判出缺字 ⇒ **退 1**，且名单里有 U+4FDD；
//! 3. 给系统字体并集 ⇒ 同一份采集 **退 0**（证明缺字来自字体栈，不是判据乱报）。
//! 2/3 需要 Windows 系统字体；没有就显式跳过（打印原因，不静默通过）。

use std::process::Command;

fn uiscan() -> Command {
    Command::new(env!("CARGO_BIN_EXE_uiscan"))
}

fn capture_file(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("uiscan-scan-{tag}-{}.bin", std::process::id()));
    // 一段真实终端流：中英混排 + 颜色 + 绝对定位
    let s = "\x1b[2J\x1b[1;1H保存成功\x1b[2;1H\x1b[31m错误：\x1b[0m网络超时\x1b[3;1HEnglish";
    std::fs::write(&p, s.as_bytes()).expect("写夹具");
    p
}

fn windows_font(name: &str) -> Option<std::path::PathBuf> {
    let p =
        std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("Fonts")
            .join(name);
    p.is_file().then_some(p)
}

#[test]
fn scan_without_font_stack_is_a_usage_error() {
    let cap = capture_file("nofont");
    let out = uiscan()
        .args(["scan", "--surface", "tui", "--file"])
        .arg(&cap)
        .args(["--rows", "6", "--cols", "40"])
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&cap);
    assert_eq!(out.status.code(), Some(2), "没字体必须退 2：{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("没有可用字体"), "要点名原因：{err}");
}

#[test]
fn scan_with_latin_only_stack_flags_cjk_and_exits_1() {
    let Some(arial) = windows_font("arial.ttf") else {
        eprintln!("跳过：本机没有 arial.ttf");
        return;
    };
    let cap = capture_file("arial");
    let out = uiscan()
        .args(["scan", "--surface", "tui", "--file"])
        .arg(&cap)
        .args(["--rows", "6", "--cols", "40", "--font"])
        .arg(&arial)
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&cap);
    assert_eq!(out.status.code(), Some(1), "有缺字必须退 1：{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("U+4FDD"), "要报出 保(U+4FDD)：{stdout}");
    assert!(
        stdout.contains("缺字=19") || stdout.contains("缺字="),
        "要有汇总：{stdout}"
    );
}

#[test]
fn scan_with_system_fonts_is_green_for_the_same_capture() {
    if windows_font("msyh.ttc").is_none() {
        eprintln!("跳过：本机没有 msyh.ttc");
        return;
    }
    let cap = capture_file("system");
    let out = uiscan()
        .args(["scan", "--surface", "tui", "--file"])
        .arg(&cap)
        .args(["--rows", "6", "--cols", "40", "--system"])
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&cap);
    assert_eq!(
        out.status.code(),
        Some(0),
        "系统字体并集应全覆盖（同一份采集对照上一条）：{out:?}"
    );
}

#[test]
fn empty_harvest_is_not_a_green_result() {
    // 真实项目跑出来的教训：空页/被拦/选错面都会得到 0 条，若报"缺字=0、rc=0"就是假绿。
    let p = std::env::temp_dir().join(format!("uiscan-empty-{}.bin", std::process::id()));
    std::fs::write(&p, b"").expect("写空夹具");
    let out = uiscan()
        .args(["scan", "--surface", "tui", "--file"])
        .arg(&p)
        .args(["--rows", "6", "--cols", "40", "--system"])
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&p);
    assert_eq!(
        out.status.code(),
        Some(2),
        "0 条必须退 2（判不了）：{out:?}"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("采集到 0 条"), "要点名'采集到 0 条'：{err}");
}
