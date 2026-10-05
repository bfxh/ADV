//! 深轨边车的构建前置（docs/PLAN-deep-track.md §1）：定位 rustc-dev 落盘位置、
//! 缺失即 fail-closed 给可读报错，并把运行期取 DLL 的目录交给 main.rs。
//!
//! 实测（2026-10-05，1.99.0）：gnu 侧导入库在 `lib/rustlib/<host>/lib/librustc_driver-*.dll.a`，
//! msvc 侧在 `lib/rustlib/<host>/lib/bin/rustc_driver.lib`——两个 search path 都要，
//! 否则本机过、CI（msvc）不过。

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
            .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
            .unwrap_or_else(|| panic!("`{args:?}` 输出里没有 {key}：{text}")),
        None => text,
    }
}

/// 在候选目录里找 rustc_driver 的库工件（导入库或 dylib 前缀，跨 gnu/msvc）。
fn find_driver_artifact(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| {
        if !dir.is_dir() {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("rustc_driver-") || name.starts_with("librustc_driver-") {
                return Some(entry.path());
            }
        }
        None
    })
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let sysroot = PathBuf::from(rustc_print(&rustc, &["--print", "sysroot"], None));
    let host = rustc_print(&rustc, &["-vV"], Some("host:"));
    let lib_dir = sysroot.join("lib/rustlib").join(&host).join("lib");
    // msvc 侧 rustc-dev 把导入库单独放在 lib/bin（gnu 侧无此目录，跳过）。
    let msvc_bin = lib_dir.join("bin");

    let mut search = vec![lib_dir.clone()];
    if msvc_bin.is_dir() {
        search.push(msvc_bin.clone());
    }
    for dir in &search {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    if find_driver_artifact(&search).is_none() {
        panic!(
            "缺 rustc-dev 组件（在 {} / {} 下找不到 rustc_driver 库工件）。一次性前置：\n  \
             rustup component add rustc-dev --toolchain {host}",
            lib_dir.display(),
            msvc_bin.display()
        );
    }

    // 运行期 rustc_driver 的 DLL 在工具链 bin/（gnu 实测）；main.rs 据此补 PATH，
    // 使 `cargo run`/测试能在 PATH 未含工具链的情况下加载。
    let bin_dir = sysroot.join("bin");
    println!("cargo:rustc-env=ADV_RUSTC_BIN_DIR={}", bin_dir.display());
    println!("cargo:rustc-env=ADV_SYSROOT={}", sysroot.display());
    println!("cargo:rustc-env=ADV_RUSTC_HOST={host}");
}
