//! `adv sca --snapshot` 的 CLI 边缘契约：命中判 1、快照读不出判 3、参数缺值判 2。
//!
//! 与锁面共用同一套三态；两面的记账不一致也算判不了（见 `adv_sca::matcher`）。

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
fn a_lock_with_a_known_vulnerable_dependency_exits_one() {
    let lock = data("osv_lock")
        .join("Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let snap = data("osv").to_string_lossy().into_owned();
    let (code, stdout, stderr) = adv(&["sca", &lock, "--snapshot", &snap]);
    assert_eq!(code, 1, "命中必须判红：{stderr}");
    let line = stdout
        .lines()
        .find(|l| l.contains("\"engine\":\"osv\""))
        .unwrap_or_else(|| panic!("stdout 里没有 osv 行：{stdout}"));
    let v: serde_json::Value = serde_json::from_str(line).expect("JSONL 行必须合法");
    assert_eq!(v["code"], "SCA-ADV-HIT");
    assert_eq!(v["id"], "RUSTSEC-2020-0071");
    assert_eq!(v["package"], "time");
    assert_eq!(v["version"], "0.1.44");
    assert_eq!(v["red"], true);
    assert!(stderr.contains("advisory 命中 1 条"), "{stderr}");
}

#[test]
fn an_empty_snapshot_directory_exits_three_not_zero() {
    let lock = data("osv_lock")
        .join("Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let empty = data("osv_empty");
    let (code, _out, stderr) = adv(&["sca", &lock, "--snapshot", &empty.to_string_lossy()]);
    assert_eq!(code, 3, "快照一份 advisory 都没有 ⇒ 判不了：{stderr}");
    assert!(stderr.contains("不折算为无发现"), "{stderr}");
}

#[test]
fn a_missing_value_for_the_snapshot_flag_is_a_usage_error() {
    let lock = data("osv_lock")
        .join("Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let (code, _out, stderr) = adv(&["sca", &lock, "--snapshot"]);
    assert_eq!(code, 2, "用法错要判 2：{stderr}");
}
