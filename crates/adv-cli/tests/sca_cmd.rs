//! `adv sca` 的三态契约（绿/红/判不了）钉在 CLI 边缘，而不是只钉在库函数里。
//!
//! 为什么值得单列：退出码是门链与调用方唯一读的接口。库内 `scan_paths` 返回
//! `Err` 与返回"零条判红"在单测里分得很清，走到 CLI 若被折叠成同一个 0，
//! 就又是"判不了长得像干净"那次踩过的坑（secrets 档同一条纪律）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn adv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_adv"))
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/adv-cli ⇒ 仓根在上面两级。
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = adv().args(args).output().expect("adv 跑不起来");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn real_locks_exit_zero_and_every_line_is_json() {
    let root = repo_root();
    let mut args: Vec<String> = vec!["sca".to_string()];
    for l in [
        "Cargo.lock",
        "rust/Cargo.lock",
        "third_party/noseyparker/Cargo.lock",
    ] {
        args.push(root.join(l).to_string_lossy().into_owned());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, stdout, stderr) = run(&refs);
    assert_eq!(code, 0, "承重锁必须绿：{stderr}");
    assert!(stderr.contains("判过 3 把锁"), "{}", stderr);
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("JSONL 行必须合法");
        assert_eq!(v["engine"], "sca");
        assert_eq!(v["red"], false, "承重锁不许出判红：{line}");
    }
}

#[test]
fn drifted_corpus_exits_one_with_the_drift_code() {
    let lock = repo_root()
        .join("crates/adv-sca/tests/data/drift/Cargo.lock")
        .to_string_lossy()
        .into_owned();
    let (code, stdout, stderr) = run(&["sca", &lock]);
    assert_eq!(code, 1, "漂移语料必须判红：{stderr}");
    let line: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    assert_eq!(line["code"], "SCA-LC-MEMBER-DRIFT");
    assert_eq!(line["red"], true);
    assert_eq!(line["package"], "cli");
}

#[test]
fn a_tree_without_any_lock_is_not_clean() {
    let src = repo_root()
        .join("crates/adv-sca/src")
        .to_string_lossy()
        .into_owned();
    let (code, stdout, stderr) = run(&["sca", &src]);
    assert_eq!(code, 3, "没有可判定的锁 ⇒ 判不了（3），不许是 0：{stderr}");
    assert!(stdout.is_empty(), "判不了时不该有发现行：{stdout}");
    assert!(stderr.contains("不折算为无发现"), "{}", stderr);
}

#[test]
fn missing_path_argument_is_a_usage_error() {
    let (code, _out, stderr) = run(&["sca"]);
    assert_eq!(code, 2, "用法错是 2 不是 3：{stderr}");
}
