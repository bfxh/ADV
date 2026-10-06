//! DD-0002 的端到端一半：三方账的「仅快轨」桶必须能被真扫描填上。
//!
//! 为什么这条值得钉成测试：片A6 之后分桶逻辑只有单测（喂合成 Finding 列表），端到端
//! 三个桶在任何真实扫描里都是 0 ⇒ "桶写错了"不会让任何东西变红。2026-10-06 实测：
//! `--engine both` 扫 `tests/data/three-way-corpus/` 得到 `仅快轨 2`，深轨 0 条——
//! 因为 RS-UNWRAP-USE / RS-PANIC-USE 只有 AST 侧实现（MIR 侧只做污点边）。
//!
//! 「两边都报」这一桶**今天还填不上**，且不是分桶缺陷：要让同一条 `(规则, 文件, 行)`
//! 在两引擎同时成立，必须先有一条两引擎都实现的规则 —— 那是 DD-0003（快轨的 Rust
//! 污点能力）的内容，记在那里而不是在这里假装测过。

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus() -> PathBuf {
    manifest_dir()
        .join("tests")
        .join("data")
        .join("three-way-corpus")
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("adv-cli 在 crates/ 下")
        .parent()
        .expect("crates 的父目录是仓根")
        .to_path_buf()
}

/// 夹具里 `needle` 首次出现的行号（1 起）——期望值锚回文本，不抄跑出来的数。
fn line_of(path: &Path, needle: &str) -> usize {
    std::fs::read_to_string(path)
        .expect("读夹具")
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("{} 里没有 {needle}", path.display()))
        + 1
}

#[test]
fn three_way_ledger_is_non_empty_end_to_end() {
    let fx = corpus().join("only_fast.rs");
    let unwrap_line = line_of(&fx, "v.unwrap()");
    let panic_line = line_of(&fx, "panic!(");

    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(corpus())
        .arg("--rules")
        .arg(repo_root().join("rules"))
        .arg("--engine")
        .arg("both")
        .output()
        .expect("启动 adv 失败");
    assert!(
        out.status.success(),
        "both 档退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    // 桶计数：两条只归快轨。
    assert!(
        stderr.contains("三方账：两边都报 0 / 仅快轨 2 / 仅深轨 0"),
        "三方账的桶计数不对：{stderr}"
    );
    // 明细行要带规则码与锚得回夹具的行号。
    assert!(
        stderr.contains("仅快轨 RS-UNWRAP-USE")
            && stderr.contains(&format!(":{unwrap_line}")),
        "unwrap 那条没落在 {unwrap_line} 行：{stderr}"
    );
    assert!(
        stderr.contains("仅快轨 RS-PANIC-USE") && stderr.contains(&format!(":{panic_line}")),
        "panic 那条没落在 {panic_line} 行：{stderr}"
    );
    // stdout 的行契约：两条都是 tree-sitter 出的。
    let fast_lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("\"engine\":\"tree-sitter\""))
        .collect();
    assert_eq!(fast_lines.len(), 2, "stdout 应有 2 条快轨发现：{stdout}");
}
