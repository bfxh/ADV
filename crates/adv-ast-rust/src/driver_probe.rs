//! rustc-dev 在位判据（单一事实源）：`crates/adv-ast-rust/build.rs` 与 `xtask/src/mir.rs`
//! 共用这把尺，避免"构建期一套判据、门另一套判据"的分裂。
//!
//! 判据锚在实测落盘形态（2026-10-05 本机 gnu：`lib/rustlib/<host>/lib/librustc_driver-<hash>.dll.a`；msvc 导入库在 `.../lib/bin/rustc_driver-<hash>.lib`；两者旁另有 `.dll`）。
//! **linux 的 `.so` 是待坐实推断**（run 37754496350 只证明"该目录被读到、这几个名字全不匹配"），真名由 adv.yml coverage job 的探针步打印。

use std::path::{Path, PathBuf};
const ARTIFACT_EXTS: [&str; 4] = [".dll.a", ".lib", ".dll", ".so"];

/// 库工件名判定：前缀认 `rustc_driver-` / `librustc_driver-`，后缀认 `.dll.a` / `.lib` / `.dll` / `.so`。
pub fn is_driver_artifact(name: &str) -> bool {
    let stem_ok = name.starts_with("rustc_driver-") || name.starts_with("librustc_driver-");
    let ext_ok = ARTIFACT_EXTS.iter().any(|ext| name.ends_with(ext));
    stem_ok && ext_ok
}

/// 目录 = `<sysroot>/lib/rustlib/<host>/lib`；msvc 的导入库在其下的 `bin/`，故两址都查。
pub fn artifact_dirs(sysroot: &Path, host: &str) -> Vec<PathBuf> {
    let lib = sysroot.join("lib/rustlib").join(host).join("lib");
    let mut dirs = vec![lib.clone()];
    let nested = lib.join("bin");
    if nested.is_dir() {
        dirs.push(nested);
    }
    dirs
}

/// 缺组件时的一次性修法文案（出处唯一：构建期 panic 与 `xtask mir` 报红同字）。
pub fn missing_message(dir: &Path, host: &str) -> String {
    format!(
        "缺 rustc-dev 组件（{} 下无 rustc_driver 库工件）。一次性前置：rustup component add rustc-dev --toolchain {host}",
        dir.display()
    )
}

/// 在候选目录里找到任一 rustc_driver 库工件即返回其路径（找不到 = 组件不在位）。
pub fn find_driver_artifact(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_driver_artifact(&name) {
                return Some(entry.path());
            }
        }
        None
    })
}
