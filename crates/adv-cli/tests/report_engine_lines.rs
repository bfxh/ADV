//! DD-0018 分诊（adv-cli `main.rs::report` 格）：摘要里**按档位分叉的那两行**此前没人看。
//!
//! 由来（`cargo mutants --list` 全量 6 条）：`engine != Ast` 那条"深轨：N 个文件跑失败"的留痕、
//! 与 `engine == Both` 才出的三方账，都只在**非 ast 档**被真跑过、从没在 **ast 档**断言过"不该出现"
//! ⇒ 换向变异（`!=`→`==`）把两行的出现条件整体反过来了，两档的测试各自都还是绿的。
//! 本文件把两侧一起钉住：ast 档不许出现这两行，mir/both 档必须出现。
//!
//! 判据走真路径（起 `adv.exe`，深轨由它再起边车），测试不重实现报告逻辑。

mod common;

use common::driver_args;
use std::path::PathBuf;
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("adv-cli 在 crates/ 下")
        .parent()
        .expect("crates 的父目录是仓根")
        .to_path_buf()
}

/// 深轨自有 rust 夹具（mir 档跑得动、且快轨对它们也有命中，两行的判据才都拿得到）。
fn deep_fixtures() -> PathBuf {
    repo_root()
        .join("crates")
        .join("adv-ast-rust")
        .join("tests")
        .join("fixtures")
}

fn scan(engine: &str, target: &PathBuf) -> (String, String) {
    let driver = driver_args();
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(target)
        .arg("--rules")
        .arg(repo_root().join("rules"))
        .arg("--engine")
        .arg(engine)
        .args(&driver)
        .output()
        .expect("启动 adv 失败");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// 摘要行本身两档都有（`adv scan：N 个文件…`），分叉的是这两行。
const DEEP_FAIL_NOTE: &str = "个文件跑失败";
const THREE_WAY_NOTE: &str = "三方账：";

#[test]
fn ast_engine_prints_neither_the_deep_failure_nor_the_three_way_note() {
    let (_stdout, stderr) = scan("ast", &deep_fixtures());
    assert!(
        stderr.contains("adv scan："),
        "摘要行本身要在（否则下面两条是空转）：{stderr}"
    );
    assert!(
        !stderr.contains(DEEP_FAIL_NOTE),
        "ast 档不跑深轨，不该出现深轨失败留痕（`engine != Ast` 一反向就会在这里出现）：{stderr}"
    );
    assert!(
        !stderr.contains(THREE_WAY_NOTE),
        "只有 both 档出三方账（`engine == Both` 一反向 ast 档也会出）：{stderr}"
    );
}

#[test]
fn both_engine_prints_both_notes() {
    let (_stdout, stderr) = scan("both", &deep_fixtures());
    assert!(
        stderr.contains(DEEP_FAIL_NOTE),
        "both 档必须留痕深轨失败面（0 次也要留，0 是量出来的不是猜的）：{stderr}"
    );
    assert!(
        stderr.contains(THREE_WAY_NOTE),
        "both 档必须出三方账：{stderr}"
    );
    assert!(
        stderr.contains("档 Both"),
        "摘要里的档位标要跟着引擎走：{stderr}"
    );
}
