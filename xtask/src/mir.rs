//! 深轨边车前置断言（docs/PLAN-deep-track.md §1）：rustc-dev 组件在位则绿，
//! 缺失即红并给一次性修法——不做"编不出来才知道"的静默降级。
//! 判据与 `crates/adv-ast-rust/build.rs` 同尺：sysroot 的 host lib 目录里有 rustc_driver 库工件。

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

/// 取 rustc 打印值；调用失败/非零退出即报错（不降级为"当作在位"）。
fn rustc_print(args: &[&str], key: Option<&str>) -> Result<String> {
    let out = Command::new("rustc")
        .args(args)
        .output()
        .context("调用 rustc 失败（PATH 里没有 rustc？）")?;
    anyhow::ensure!(
        out.status.success(),
        "rustc {args:?} 退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match key {
        Some(prefix) => text
            .lines()
            .find_map(|line| {
                line.strip_prefix(prefix)
                    .map(|value| value.trim().to_string())
            })
            .ok_or_else(|| anyhow::anyhow!("`{args:?}` 输出里没有 {prefix}：{text}")),
        None => Ok(text),
    }
}

/// 断言 rustc-dev 在位，返回违规清单（空 = 绿）。
pub fn run() -> Result<Vec<String>> {
    let sysroot = PathBuf::from(rustc_print(&["--print", "sysroot"], None)?);
    let host = rustc_print(&["-vV"], Some("host:"))?;
    let lib_dir = sysroot.join("lib/rustlib").join(&host).join("lib");
    let candidates = [lib_dir.clone(), lib_dir.join("bin")];
    let found = candidates.iter().any(|dir| {
        PathBuf::from(dir)
            .read_dir()
            .map(|mut entries| {
                entries.any(|entry| match entry {
                    Ok(entry) => {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        name.starts_with("rustc_driver-") || name.starts_with("librustc_driver-")
                    }
                    Err(_) => false,
                })
            })
            .unwrap_or(false)
    });
    if found {
        return Ok(Vec::new());
    }
    Ok(vec![format!(
        "缺 rustc-dev 组件（{} 下无 rustc_driver 库工件）。一次性前置：rustup component add rustc-dev --toolchain {host}",
        lib_dir.display()
    )])
}
