//! 片A2 验收：三条合成夹具的污点到达结论。判据走真路径——起子进程跑驱动，
//! 不在测试里重实现分析（否则测的是测试自己）。

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

fn driver() -> Command {
    let mut path = OsString::from(env!("ADV_RUSTC_BIN_DIR"));
    path.push(if cfg!(windows) { ";" } else { ":" });
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_adv-ast-rust-driver"));
    cmd.env("PATH", path);
    cmd
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate 在工作区根的 crates/ 下")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// 跑污点档，返回解析后的 finding 列表。
fn findings(fixture_name: &str) -> Vec<serde_json::Value> {
    let out = driver()
        .arg("--taint")
        .arg(repo_root().join("rules"))
        .arg(fixture(fixture_name))
        .output()
        .expect("启动 adv-ast-rust-driver 失败");
    assert!(
        out.status.success(),
        "驱动退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("finding 不是 JSON：{line}（{e}）"))
        })
        .collect()
}

#[test]
fn direct_flow_is_found() {
    let hits = findings("taint_direct.rs");
    assert_eq!(hits.len(), 1, "直接流应恰报 1 条，实得 {hits:?}");
    assert_eq!(hits[0]["rule"], serde_json::json!("RS-TAINT-COMMAND"));
    assert_eq!(hits[0]["function"], serde_json::json!("direct"));
    assert_eq!(hits[0]["engine"], serde_json::json!("mir"));
}

#[test]
fn sanitized_input_reports_nothing() {
    let hits = findings("taint_sanitized.rs");
    assert!(
        hits.is_empty(),
        "过了 len 净化工却仍报发现（假阳性）：{hits:?}"
    );
}

#[test]
fn taint_crosses_basic_blocks() {
    let hits = findings("taint_branch.rs");
    assert_eq!(
        hits.len(),
        1,
        "污点在 then 块产生、合流点后消费，应报 1 条，实得 {hits:?}"
    );
    assert_eq!(hits[0]["function"], serde_json::json!("branch"));
}
