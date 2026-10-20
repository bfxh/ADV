//! `clip` 子命令的端到端契约：不需要浏览器，直接用冻结的原始载荷。

use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../uiscan-web/tests/fixtures/trunc-payload.json"
    ))
}

#[test]
fn clip_reports_the_four_truncated_shapes_and_exits_1() {
    let out = Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .args(["clip", "--json"])
        .arg(fixture())
        .output()
        .expect("跑 uiscan");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "有裁切必须退 1：{stdout}");
    assert!(stdout.contains("截断=4"), "要汇总条数：{stdout}");
    for want in ["p.clip", "p.ell", "p.vclip", "p.clamp"] {
        assert!(stdout.contains(want), "要点名 {want}：{stdout}");
    }
    assert!(!stdout.contains("p.fits"), "放得下的不该出现：{stdout}");
}

#[test]
fn clip_on_a_clean_payload_exits_0() {
    let p = std::env::temp_dir().join(format!("uiscan-clean-{}.json", std::process::id()));
    std::fs::write(
        &p,
        r#"{"title":"clean","items":[{"loc":"x","ch":"DomText","txt":"短","rect":{"x":0,"y":0,"w":10,"h":20},"sw":10,"cw":10,"sh":20,"chh":20,"ovx":"visible","ovy":"visible","ell":"clip"}]}"#,
    )
    .expect("写夹具");
    let out = Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .args(["clip", "--json"])
        .arg(&p)
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&p);
    assert_eq!(out.status.code(), Some(0), "干净载荷要退 0：{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("截断=0"));
}

#[test]
fn payload_without_geometry_fails_closed() {
    // 旧载荷（没有 sw/cw）如果安静地返回"截断=0"，就是把"没在看"当成"干净"。
    let p = std::env::temp_dir().join(format!("uiscan-old-{}.json", std::process::id()));
    std::fs::write(
        &p,
        r#"{"title":"old","items":[{"loc":"x","ch":"DomText","txt":"长文案","rect":{"x":0,"y":0,"w":10,"h":20}}]}"#,
    )
    .expect("写夹具");
    let out = Command::new(env!("CARGO_BIN_EXE_uiscan"))
        .args(["clip", "--json"])
        .arg(&p)
        .output()
        .expect("跑 uiscan");
    let _ = std::fs::remove_file(&p);
    assert_eq!(out.status.code(), Some(2), "缺几何字段必须退 2：{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("几何字段"), "要点名缺什么：{err}");
}
