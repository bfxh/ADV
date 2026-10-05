//! 片A1 验收：驱动对给定 .rs 输出该 crate 的 MIR 函数名清单。
//! 运行前置由 build.rs 交出（`ADV_RUSTC_BIN_DIR`），测试自己补 PATH——
//! 否则加载器找不到 `rustc_driver` 的 DLL。

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// 夹具里四条有函数体的项，驱动必须全部报出。
const EXPECTED: &[&str] = &["source", "flow", "helper", "unused_unit"];

fn driver() -> Command {
    let mut path = OsString::from(env!("ADV_RUSTC_BIN_DIR"));
    path.push(if cfg!(windows) { ";" } else { ":" });
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_adv-ast-rust-driver"));
    cmd.env("PATH", path);
    cmd
}

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("m1_probe.rs")
}

#[test]
fn driver_lists_mir_functions_of_fixture() {
    let fixture = fixture_path();
    let out = driver()
        .arg(&fixture)
        .output()
        .expect("启动 adv-ast-rust-driver 失败");
    assert!(
        out.status.success(),
        "驱动退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let names: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    for expected in EXPECTED {
        assert!(
            names.contains(expected),
            "输出缺 {expected}，实得 {names:?}（夹具 {fixture:?}）"
        );
    }
}

#[test]
fn driver_fails_closed_without_input() {
    let out = driver().output().expect("启动 adv-ast-rust-driver 失败");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        !out.status.success(),
        "无输入应红（fail-closed），实得绿：{stderr}"
    );
    assert!(stderr.contains("用法"), "报错应给出用法，实得：{stderr}");
}
