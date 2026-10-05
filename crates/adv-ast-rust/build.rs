//! 深轨边车的构建前置（docs/PLAN-deep-track.md §1）：定位 rustc-dev 落盘位置、
//! 缺失即 fail-closed 给可读报错，并把运行期取 DLL 的目录交给 main.rs。
//!
//! 判据不在此重写——`#[path]` 直接纳入 `src/driver_probe.rs`，使构建期与
//! `cargo run -p xtask -- mir` 用的是同一把尺。

#[path = "src/driver_probe.rs"]
mod driver_probe;

use std::path::PathBuf;
use std::process::Command;

/// 取 rustc 打印值（`--print sysroot` / `-vV`）。失败即中止构建，不静默降级。
fn rustc_print(rustc: &str, args: &[&str], want: Option<&str>) -> String {
    let out = Command::new(rustc)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("调用 {rustc} {args:?} 失败：{e}"));
    if !out.status.success() {
        panic!(
            "{rustc} {args:?} 退出码非零：{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match want {
        Some(key) => text
            .lines()
            .find_map(|line| line.strip_prefix(key).map(|value| value.trim().to_string()))
            .unwrap_or_else(|| panic!("`{args:?}` 输出里没有 {key}：{text}")),
        None => text,
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/driver_probe.rs");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let sysroot = PathBuf::from(rustc_print(&rustc, &["--print", "sysroot"], None));
    let host = rustc_print(&rustc, &["-vV"], Some("host:"));
    let dirs = driver_probe::artifact_dirs(&sysroot, &host);
    for dir in &dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    if driver_probe::find_driver_artifact(&dirs).is_none() {
        panic!("{}", driver_probe::missing_message(&dirs[0], &host));
    }
    // 运行期 rustc_driver 的 DLL 在工具链 bin/（gnu 实测）；main.rs 与测试据此补 PATH，
    // 使 `cargo run`/测试能在 PATH 未含工具链的情况下加载。
    println!(
        "cargo:rustc-env=ADV_RUSTC_BIN_DIR={}",
        sysroot.join("bin").display()
    );
    println!("cargo:rustc-env=ADV_SYSROOT={}", sysroot.display());
    println!("cargo:rustc-env=ADV_RUSTC_HOST={host}");
}
