//! `scan --surface web` 对本地 file:// 页面的端到端契约（需要 Edge；没有就显式跳过）。
//!
//! 钉的是真实项目跑出来的两个坑：
//! 1. Edge 起窗后先有一个 `about:blank` 的 page 目标，`readyState` 在它上面已是 complete
//!    ⇒ 必须等 `location.href` 真到目标才能采；
//! 2. 真实站点常常永不 `complete`（挂起请求）⇒ 就绪判据是"href 到目标 + 不是 loading"。

use std::path::PathBuf;
use std::process::Command;

const EDGE: &str = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";

fn edge() -> Option<PathBuf> {
    let p = PathBuf::from(EDGE);
    p.is_file().then_some(p)
}

#[test]
fn scan_web_waits_for_the_real_document_not_about_blank() {
    let Some(_edge) = edge() else {
        eprintln!("跳过：本机没有 Edge");
        return;
    };
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../uiscan-web/tests/fixtures/i18n-pseudo.html"
    );
    let url = format!("file:///{}", fixture.replace('\\', "/"));
    let out = Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .args([
            "scan",
            "--surface",
            "web",
            &url,
            "--wait-secs",
            "40",
            "--system",
        ])
        .output()
        .expect("跑 uiscan");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "本地夹具应采到并全覆盖：{stderr}"
    );
    assert!(
        stdout.contains("title="),
        "surface 行要带页面 title 供诊断：{stdout}"
    );
    assert!(
        !stdout.contains("href=String(\"about:blank\")"),
        "采到的应是真文档，不是 about:blank：{stdout}"
    );
    assert!(stdout.contains("缺字=0"), "系统字体应覆盖夹具：{stdout}");
}

#[test]
fn scan_web_reports_missing_glyphs_and_clipping_in_one_go() {
    let Some(_edge) = edge() else {
        eprintln!("跳过：本机没有 Edge");
        return;
    };
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../uiscan-web/tests/fixtures/truncation.html"
    );
    let url = format!("file:///{}", fixture.replace('\\', "/"));
    let out = Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .args([
            "scan",
            "--surface",
            "web",
            &url,
            "--wait-secs",
            "40",
            "--system",
        ])
        .output()
        .expect("跑 uiscan");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "有裁切必须退 1：{stdout}");
    assert!(stdout.contains("缺字=0"), "系统字体应覆盖夹具：{stdout}");
    assert!(
        stdout.contains("截断=4"),
        "五种形状里应报 4 条裁切：{stdout}"
    );
    for want in ["p.clip", "p.ell", "p.vclip", "p.clamp"] {
        assert!(stdout.contains(want), "要点名 {want}：{stdout}");
    }
}
