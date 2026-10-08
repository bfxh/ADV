//! adv-cli 集成测试的共享帮手：**深轨驱动的自足解析/构建**（DD-0016 的修法）。
//!
//! 为什么必须有它：`deep_cargo.rs` / `engine_mir.rs` / `three_way_ledger.rs` 三条路径都要深轨驱动，
//! 而驱动是**兄弟包 `adv-ast-rust` 的 bin**——变异门的增量档只为受变异影响的包建测试、不建它，
//! 于是未变异基线当场红、门报"判不了"（实测两次）。靠"target 里碰巧有"就是靠环境；
//! 这里显式解析/构建，并把路径交给 `--driver`。
//!
//! 两个细节都是踩出来的：
//! ① **独立 target 目录**（`CARGO_TARGET_DIR` 指到缓存下）：外层 `cargo test` 正握着主 target 的锁，
//!    在它里面再起 cargo 会死等；
//! ② **缓存键 = 驱动源码内容哈希**：变异轮里每个变异都有自己的 scratch，只有真正改了
//!    `crates/adv-ast-rust` 源码的变异才会 miss（其余复用同一份），否则每个 adv-cli 变异都要重编驱动。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 仓根（`crates/adv-cli` 上两级）。
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("adv-cli 在 crates/ 下")
        .to_path_buf()
}

/// 深轨驱动 exe 的路径：没有就构建一次（缓存进 `TMP/adv-driver-cache/<源码哈希>/`）。
pub fn driver_path() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("adv-driver-cache")
        .join(driver_source_hash());
    let exe = dir.join("debug").join(format!(
        "adv-ast-rust-driver{}",
        std::env::consts::EXE_SUFFIX
    ));
    if exe.is_file() {
        return exe;
    }
    let status = Command::new("cargo")
        .current_dir(repo_root())
        .env("CARGO_TARGET_DIR", &dir)
        .args([
            "build",
            "-p",
            "adv-ast-rust",
            "--bin",
            "adv-ast-rust-driver",
        ])
        .status()
        .expect("起 cargo 构建深轨驱动失败");
    assert!(status.success(), "构建深轨驱动失败（DD-0016 的前置）");
    assert!(exe.is_file(), "驱动没落到 {}", exe.display());
    exe
}

/// `--driver <路径>` 两个参数，供各测试文件的 `scan()` 直接 `.args(...)`。
pub fn driver_args() -> [String; 2] {
    [
        "--driver".to_string(),
        driver_path().to_string_lossy().into_owned(),
    ]
}

/// 驱动源码（`crates/adv-ast-rust/**/*.rs`）的内容哈希：路径排序后逐个喂 DefaultHasher。
fn driver_source_hash() -> String {
    use std::hash::{Hash, Hasher};
    let mut files = walk_rs(&repo_root().join("crates").join("adv-ast-rust"));
    files.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in files {
        f.to_string_lossy().hash(&mut h);
        std::fs::read(&f).unwrap_or_default().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

fn walk_rs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_rs(&p));
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out
}
