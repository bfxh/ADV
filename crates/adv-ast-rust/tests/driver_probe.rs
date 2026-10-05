//! 金丝雀：rustc-dev 在位判据（`driver_probe`）必须能判红，也能在真工件名上判绿。
//!
//! 期望值的锚：**不是**规则作者的同假设自证，而是 2026-10-05 在本机
//! 1.99.0-gnu sysroot 里 `ls` 到的真实文件名（`librustc_driver-1a029222508e0d37.dll.a`
//! 落在 `lib/rustlib/<host>/lib/`，`rustc_driver-1a029222508e0d37.dll` 同时落 `bin/` 与该 lib 目录）。
//! msvc 侧的 `.lib` 导入库形态在此**未经观测**（本机未装 msvc 的 rustc-dev，C 盘 99% 满），
//! 由 CI 的 msvc 档坐实——见 eval/FP-LEDGER-m2s4.md。

use adv_ast_rust::driver_probe::{
    artifact_dirs, find_driver_artifact, is_driver_artifact, missing_message,
};
use std::path::{Path, PathBuf};

fn temp_case(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adv-probe-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn accepts_real_gnu_artifact_names() {
    for name in [
        "librustc_driver-1a029222508e0d37.dll.a",
        "rustc_driver-1a029222508e0d37.dll",
    ] {
        assert!(
            is_driver_artifact(name),
            "实测存在的工件名被判不在位：{name}"
        );
    }
}

#[test]
fn rejects_non_artifacts() {
    for name in [
        "rustc_driver",                   // 无版本短横、无扩展
        "rustc_driver.exe",               // rustc 驱动可执行，不是库
        "rustc_driver-notes.txt",         // 前缀像但非库
        "librustc_driver_macros.rlib",    // 扩展不在认的三种里
        "libstd-c255629c10cd2379.dll",    // 别的 dll
        "libcore-50c831dc956e1673.rmeta", // 元数据
    ] {
        assert!(!is_driver_artifact(name), "非工件被判在位：{name}");
    }
}

#[test]
fn finds_artifact_in_dir_and_stays_red_when_absent() {
    let hit = temp_case("hit");
    std::fs::write(hit.join("librustc_driver-deadbeef.dll.a"), b"").expect("写假工件");
    assert_eq!(
        find_driver_artifact(std::slice::from_ref(&hit)),
        Some(hit.join("librustc_driver-deadbeef.dll.a")),
        "目录里有真名工件却判不在位"
    );

    let miss = temp_case("miss");
    std::fs::write(miss.join("libstd-1.dll"), b"").expect("写干扰文件");
    assert_eq!(
        find_driver_artifact(std::slice::from_ref(&miss)),
        None,
        "金丝雀失败：无 rustc_driver 工件时判据没有报红"
    );

    assert_eq!(
        find_driver_artifact(&[PathBuf::from("Z:/definitely-not-here/adv")]),
        None,
        "目录不存在时应判不在位（fail-closed），不是崩"
    );
    let _ = std::fs::remove_dir_all(&hit);
}

#[test]
fn artifact_dirs_includes_msvc_nested_bin_only_when_present() {
    let root = temp_case("dirs");
    let lib = root.join("lib").join("rustlib").join("H").join("lib");
    std::fs::create_dir_all(&lib).expect("建 lib");
    assert_eq!(
        artifact_dirs(&root, "H"),
        vec![lib.clone()],
        "无 bin/ 时不该凭空多一个搜索目录"
    );
    std::fs::create_dir_all(lib.join("bin")).expect("建 lib/bin");
    assert_eq!(
        artifact_dirs(&root, "H").len(),
        2,
        "msvc 布局（导入库在 lib/bin）应被纳入搜索面"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_message_carries_the_one_off_fix() {
    let msg = missing_message(Path::new("SYS/lib/rustlib/H/lib"), "x86_64-pc-windows-gnu");
    assert!(msg.contains("rustc-dev"), "文案要点名组件：{msg}");
    assert!(
        msg.contains("rustup component add rustc-dev --toolchain x86_64-pc-windows-gnu"),
        "文案要给一次性修法：{msg}"
    );
}
