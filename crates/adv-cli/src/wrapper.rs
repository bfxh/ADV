//! 深轨 cargo 档（片B2）：把边车当 `RUSTC_WORKSPACE_WRAPPER` 挂进目标 crate 的
//! cargo 构建，读回它按 crate 落的 JSONL。
//!
//! 为什么单独成模块：Windows 加载器按 wrapper exe 所在目录找 `rustc_driver.dll`
//!，PATH 帮不上忙；这条平台边界有自己的复制/fail-closed 判据，不该压胖单文件档。

use super::mir::{child_path, parse_finding, toolchain_bin};
use adv_rules::{Finding, Rule};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Windows 上包装器 exe 需要同目录有 `rustc_driver.dll`。把驱动 exe 和 DLL 一起
/// 复制到临时目录，wrapper 指向副本，扫完一起清。
#[cfg(windows)]
fn wrapper_with_dlls(driver: &Path, bin_dir: &Path, scratch: &Path) -> Result<PathBuf, String> {
    let wrapper_dir = scratch.join("wrapper");
    std::fs::create_dir_all(&wrapper_dir)
        .map_err(|e| format!("建 wrapper 目录 {} 失败：{e}", wrapper_dir.display()))?;
    let wrapper_exe = wrapper_dir.join(driver.file_name().ok_or("驱动路径没有文件名")?);
    std::fs::copy(driver, &wrapper_exe).map_err(|e| {
        format!(
            "复制驱动 {} → {} 失败：{e}",
            driver.display(),
            wrapper_exe.display()
        )
    })?;
    let mut copied_dll = false;
    // 复制 rustc_driver DLL（名字带 hash，通配匹配）。
    for entry in
        std::fs::read_dir(bin_dir).map_err(|e| format!("读 {} 失败：{e}", bin_dir.display()))?
    {
        let entry = entry.map_err(|e| format!("读目录项失败：{e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("rustc_driver") && name.ends_with(".dll") {
            let dst = wrapper_dir.join(&name);
            std::fs::copy(entry.path(), &dst).map_err(|e| {
                format!(
                    "复制 DLL {} → {} 失败：{e}",
                    entry.path().display(),
                    dst.display()
                )
            })?;
            copied_dll = true;
        }
    }
    if !copied_dll {
        return Err(format!(
            "{} 里没有 rustc_driver*.dll：wrapper 副本会入口崩溃",
            bin_dir.display()
        ));
    }
    Ok(wrapper_exe)
}

/// 非 Windows：无需复制 DLL，直接返回原路径。
#[cfg(not(windows))]
fn wrapper_with_dlls(driver: &Path, _bin_dir: &Path, _scratch: &Path) -> Result<PathBuf, String> {
    Ok(driver.to_path_buf())
}

/// 整 crate 一档：挂 wrapper、起 cargo、读回发现。cargo 跑不动 = 执行失败，不是干净。
pub fn scan_crate_via_cargo(
    crate_dir: &Path,
    rules_dir: &Path,
    driver: &Path,
    rules: &[Rule],
) -> Result<Vec<Finding>, String> {
    let manifest = crate_dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "cargo 档要的是一个 crate 目录（含 Cargo.toml）：{}",
            crate_dir.display()
        ));
    }
    let bin_dir = toolchain_bin()?;
    // 规则目录必须传**绝对路径**：包装器进程的工作目录是被扫 crate 的根（cargo 在那儿
    // 起 rustc），相对路径会解析到不存在的目录（2026-10-06 实测：驱动报"读规则目录 rules"）。
    let rules_abs = rules_dir
        .canonicalize()
        .map_err(|e| format!("规则目录 {} 解析失败：{e}", rules_dir.display()))?;
    let scratch = std::env::temp_dir().join(format!("adv-mir-cargo-{}", std::process::id()));
    let mir_out = scratch.join("mir");
    let target_dir = scratch.join("target");
    std::fs::create_dir_all(&mir_out).map_err(|e| format!("建 {} 失败：{e}", mir_out.display()))?;
    let wrapper = wrapper_with_dlls(driver, &bin_dir, &scratch)?;
    let out = Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            manifest.to_str().ok_or("manifest 路径不是有效 UTF-8")?,
            "--target-dir",
            target_dir.to_str().ok_or("target 路径不是有效 UTF-8")?,
        ])
        .env("PATH", child_path(&bin_dir))
        .env("RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .env("ADV_MIR_WRAPPER", "1")
        .env("ADV_MIR_RULES", &rules_abs)
        .env("ADV_MIR_OUT", &mir_out)
        .output()
        .map_err(|e| format!("启动 cargo 失败：{e}"))?;
    // 先把账读回来再清临时目录：清晚了留残渣，读晚了拿不到内容。
    let collected = collect_crate_findings(&mir_out, rules);
    let _ = std::fs::remove_dir_all(&scratch);
    if !out.status.success() {
        return Err(format!(
            "cargo 构建失败（退出码 {:?}）：{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("")
        ));
    }
    collected
}

/// 读回 `<crate>.jsonl`。一个都没有 = 红（静默空就是假绿入口，与片A2 的边界口径一致）。
fn collect_crate_findings(mir_out: &Path, rules: &[Rule]) -> Result<Vec<Finding>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(mir_out)
        .map_err(|e| format!("读 {} 失败：{e}", mir_out.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "cargo 档深轨没落任何 crate 的账（{} 下没有 .jsonl）——不能折算为\"无发现\"",
            mir_out.display()
        ));
    }
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("读 {} 失败：{e}", file.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            found.push(parse_finding(line, None, rules)?);
        }
    }
    Ok(found)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// cargo 档的 wrapper 根因：副本缺 exe 或 DLL 就会回到入口崩溃（`-1073741511`）。
    #[test]
    fn wrapper_copy_contains_driver_and_driver_dll() {
        let bin_dir = toolchain_bin().expect("rustc-dev 工具链应可用");
        let scratch = std::env::temp_dir().join(format!("adv-wrapper-test-{}", std::process::id()));
        let driver = scratch.join("source").join("driver.exe");
        std::fs::create_dir_all(driver.parent().expect("driver parent")).expect("建驱动源目录");
        std::fs::write(&driver, b"driver").expect("写假驱动");
        let has_source_dll = std::fs::read_dir(&bin_dir)
            .expect("读 toolchain bin")
            .flatten()
            .any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("rustc_driver") && n.ends_with(".dll")
            });
        assert!(has_source_dll, "toolchain bin 应有 rustc_driver DLL");
        let wrapper = wrapper_with_dlls(&driver, &bin_dir, &scratch).expect("建 wrapper 副本");
        assert!(
            wrapper.is_file(),
            "wrapper exe 没复制：{}",
            wrapper.display()
        );
        let copied_dlls = std::fs::read_dir(wrapper.parent().expect("wrapper parent"))
            .expect("读 wrapper 目录")
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("rustc_driver") && n.ends_with(".dll")
            })
            .count();
        assert_eq!(copied_dlls, 1, "rustc_driver DLL 没进 wrapper 目录");
        assert_eq!(
            std::fs::read(&wrapper).expect("读 wrapper"),
            b"driver",
            "wrapper 副本不是驱动本体"
        );
        let empty_bin = scratch.join("empty-bin");
        std::fs::create_dir_all(&empty_bin).expect("建空 bin");
        let no_dll = wrapper_with_dlls(&driver, &empty_bin, &scratch)
            .expect_err("toolchain 没有 rustc_driver DLL 时必须红，不能让 wrapper 带病上岗");
        assert!(no_dll.contains("rustc_driver"), "{no_dll}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
