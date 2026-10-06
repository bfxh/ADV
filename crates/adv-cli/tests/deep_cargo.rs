//! 片B2 验收：深轨吃**crate 形状**的输入（`--deep-via-cargo` 把边车挂进目标 crate 的
//! cargo 构建）。这份夹具是合成的多文件 crate（`src/lib.rs` + `mod inner`），因为：
//! - 单文件档硬拼 `--crate-type=lib` 且没有依赖/模块解析 ⇒ 扫真实仓库代码必 exit=101、
//!   0 发现（片A2「适用边界」实测）；
//! - 只有走 cargo 才能扫到**非根模块**里的流，也才会执行到 `mir_body()` 的
//!   `mir_for_ctfe` 分支（夹具里的 `pub const OFFSET` 就是为它放的，片B1 自记过那条臂
//!   没有覆盖）。
//!
//! 期望值锚到夹具里的 `// adv-expect: hit|miss` 标记，不是把跑出来的数字抄一遍。
//! 金样冻结：`ADV_UPDATE_GOLDEN=1 cargo test -p adv-cli`（行序不是契约 ⇒ 排序后比）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("adv-cli 在 crates/ 下")
        .to_path_buf()
}

fn fixture_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("taint-crate")
}

fn scan(target: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(target)
        .arg("--rules")
        .arg(repo_root().join("rules"))
        .arg("--engine")
        .arg("both")
        .arg("--deep-via-cargo")
        .args(extra)
        .output()
        .expect("启动 adv 失败")
}

/// 深轨 stdout 的 JSONL 行（排序后）。
fn finding_lines(out: &std::process::Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    lines.sort();
    lines
}

/// 夹具里标了 `adv-expect: hit` 的 `(crate 相对路径, 行号)`。
fn expected_hits() -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for rel in ["src/lib.rs", "src/inner.rs"] {
        let path = fixture_crate().join(rel);
        let text = std::fs::read_to_string(&path).expect("读夹具");
        for (i, line) in text.lines().enumerate() {
            if line.contains("adv-expect: hit") {
                out.push((rel.to_string(), i + 1));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn cargo_mode_reports_cross_module_flows_and_respects_sanitizer() {
    let out = scan(&fixture_crate(), &[]);
    assert!(
        out.status.success(),
        "cargo 档退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = finding_lines(&out);
    let want = expected_hits();
    assert!(
        !want.is_empty(),
        "夹具的 hit 标记丢了——这份验收就退化成\"跑通即绿\""
    );
    assert_eq!(
        lines.len(),
        want.len(),
        "条数与夹具标记不符，实得：{lines:?}"
    );

    let mut got: Vec<(String, usize)> = Vec::new();
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("stdout 应为 JSONL");
        assert_eq!(v["engine"], serde_json::json!("mir"));
        assert_eq!(v["rule"], serde_json::json!("RS-TAINT-COMMAND"));
        got.push((
            v["file"].as_str().expect("file 字段").to_string(),
            v["start_line"].as_u64().expect("start_line 字段") as usize,
        ));
    }
    got.sort();
    assert_eq!(
        got, want,
        "深轨 cargo 档的发现与夹具标记不一致（文件归属或行号漂移）"
    );
    // 跨模块是真跨模块：不能全落在 lib.rs
    assert!(
        got.iter().any(|(f, _)| f == "src/inner.rs"),
        "非根模块没被扫到（单文件档的局限没有真的被绕开）：{got:?}"
    );
    // sanitizer 那条（`len` 之后）不许出现在账里；三方账的深轨桶必须等于标记数
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let ledger = format!("三方账：两边都报 0 / 仅快轨 0 / 仅深轨 {}", want.len());
    assert!(
        stderr.contains(&ledger),
        "三方账没按夹具标记出账（期望 {ledger}）：{stderr}"
    );
}

#[test]
fn cargo_mode_golden_freezes_the_lines() {
    let golden_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("deep-cargo.jsonl");
    let lines = finding_lines(&scan(&fixture_crate(), &[]));
    if std::env::var("ADV_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_file.parent().expect("parent")).expect("建 golden 目录");
        std::fs::write(&golden_file, format!("{}\n", lines.join("\n"))).expect("写 golden");
        eprintln!("golden 已更新（{} 行）", lines.len());
        return;
    }
    let raw = std::fs::read_to_string(&golden_file)
        .expect("golden 文件应存在（ADV_UPDATE_GOLDEN=1 重录并逐行披露）");
    let mut want: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    want.sort();
    assert_eq!(
        lines, want,
        "深轨 cargo 档的行契约与金标准漂移——改动必须是有意识重录"
    );
}

/// 指到一个不是 crate 的目录必须红：不能"没扫到东西"就当通过。
#[test]
fn cargo_mode_on_non_crate_dir_is_red() {
    let not_a_crate = repo_root().join("rules");
    let out = scan(&not_a_crate, &[]);
    assert!(
        !out.status.success(),
        "非 crate 目录却判成通过：{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(out.status.code(), Some(2), "执行失败该红，不该 0");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        err.contains("Cargo.toml") || err.contains("cargo 档"),
        "报错要说清是 cargo 档跑不动：{err}"
    );
}
