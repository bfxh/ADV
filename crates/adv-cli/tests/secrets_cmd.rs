//! `adv secrets` 的端到端（M3-1）：行契约与掩码纪律。
//!
//! 与 `adv scan` 同一套纪律：stdout 只出 JSONL、stderr 只出摘要、**原文永不出接口**。
//! 假值按 S123 运行时拼接，写进 Windows 可见的临时目录（别用 POSIX `/tmp`——Windows 程序看不见）。

use std::path::PathBuf;
use std::process::Command;

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adv-secrets-cmd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建 tmp_path");
    dir
}

#[test]
fn secrets_subcommand_emits_jsonl_with_masked_snippet() {
    let dir = scratch();
    let key = format!("{}{}", "AKIA", "DEADBEEFDEADBEEF");
    std::fs::write(dir.join("hit.txt"), format!("aws = {key}\n")).expect("写夹具");
    std::fs::write(dir.join("clean.txt"), "nothing here\n").expect("写干净样本");

    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("secrets")
        .arg(&dir)
        .output()
        .expect("启动 adv 失败");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "退出码应为 0：{:?}", out.status);

    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let lines: Vec<&str> = stdout.lines().filter(|l| l.starts_with('{')).collect();
    assert_eq!(lines.len(), 1, "只该有一条发现（干净文件不许报）：{stdout}");
    let v: serde_json::Value = serde_json::from_str(lines[0]).expect("stdout 应为 JSONL");
    assert_eq!(v["engine"], serde_json::json!("noseyparker"));
    assert_eq!(v["rule"], serde_json::json!("np.aws.1"));
    assert_eq!(v["start_line"], serde_json::json!(1));
    assert_eq!(v["snippet"], serde_json::json!("***"), "X10：只给定长掩码");
    assert!(
        v["file"].as_str().unwrap_or_default().ends_with("hit.txt"),
        "文件要指到命中那个：{v}"
    );
    assert!(
        !stdout.contains(&key) && !stderr.contains(&key),
        "原文不许出现在 stdout/stderr 的任何角落"
    );
    assert!(stderr.contains("1 条发现"), "摘要行要点数：{stderr}");
}
