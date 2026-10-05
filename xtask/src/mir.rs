//! 深轨边车前置断言（docs/PLAN-deep-track.md §1）：rustc-dev 组件在位则绿，
//! 缺失即红并给一次性修法——不做"编不出来才知道"的静默降级。
//! 判据取自 `adv_ast_rust::driver_probe`（与 build.rs 同一把尺）；
//! 本模块只有 `rustc_print`/`run` 两个 IO 壳不可单测，判定逻辑在 driver_probe 里受断言。

use adv_ast_rust::driver_probe;
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
    let dirs = driver_probe::artifact_dirs(&sysroot, &host);
    if driver_probe::find_driver_artifact(&dirs).is_some() {
        return Ok(Vec::new());
    }
    Ok(vec![driver_probe::missing_message(&dirs[0], &host)])
}
