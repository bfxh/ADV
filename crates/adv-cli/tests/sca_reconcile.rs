//! `adv sca --reconcile` 的 CLI 边缘：差异判红进退出码、缺 `--snapshot` 时判用法错。

use std::path::{Path, PathBuf};
use std::process::Command;

fn adv(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .args(args)
        .output()
        .expect("adv 跑不起来");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../adv-sca/tests/data")
        .join(rel)
}

#[test]
fn a_rustsec_only_entry_exits_one_through_the_cli() {
    let lock = data("osv_lock")
        .join("Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let snap = data("reconcile").to_string_lossy().into_owned();
    let (code, stdout, stderr) = adv(&["sca", &lock, "--snapshot", &snap, "--reconcile"]);
    assert_eq!(code, 1, "「仅 RustSec 有」要经 CLI 判红：{stderr}");
    let line = stdout
        .lines()
        .find(|l| l.contains("SCA-RS-ONLY-RUSTSEC"))
        .unwrap_or_else(|| panic!("stdout 里没有那条判红的对账行：{stdout}"));
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["code"], "SCA-RS-ONLY-RUSTSEC");
    assert_eq!(v["id"], "RUSTSEC-2099-0003");
    assert_eq!(v["red"], true);
    assert!(stderr.contains("对账 OSV 2 / RustSec 2"), "{stderr}");
}

#[test]
fn reconcile_without_a_snapshot_is_a_usage_error() {
    let lock = data("osv_lock")
        .join("Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let (code, _out, stderr) = adv(&["sca", &lock, "--reconcile"]);
    assert_eq!(code, 2, "缺 --snapshot 要判用法错：{stderr}");
}
