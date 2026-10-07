//! DD-0002 的端到端两半：三方账的「仅快轨」与「两边都报」都必须能被真扫描填上。
//!
//! 为什么这条值得钉成测试：片A6 之后分桶逻辑只有单测（喂合成 Finding 列表），端到端
//! 三个桶在任何真实扫描里都是 0 ⇒ "桶写错了"不会让任何东西变红。2026-10-06 实测：
//! `--engine both` 扫 `tests/data/three-way-corpus/` 得到 `仅快轨 2`，深轨 0 条——
//! 因为 RS-UNWRAP-USE / RS-PANIC-USE 只有 AST 侧实现（MIR 侧只做污点边）。
//!
//! 2026-10-07 订正：「两边都报」并不像旧登记写的"结构上凑不出来"。快轨的 Rust 污点引擎
//! 一直在（`adv-rules/src/taint.rs`），缺的是**链式调用的源点匹配**；片M 补掉它之后
//! `taint_direct.rs` 那一类流程两轨在同一行各报一条 ⇒ 本文件第二条测试把它钉成非空。

use std::collections::HashSet;
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

/// 深轨自有污点夹具（快轨与深轨共用同一批 .rs，才谈得上"两边都报"）。
fn deep_fixtures() -> PathBuf {
    repo_root()
        .join("crates")
        .join("adv-ast-rust")
        .join("tests")
        .join("fixtures")
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

/// 跑一次 `--engine both`，返回 (stdout, stderr)。
fn scan_both(path: &Path) -> (String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(path)
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
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// stdout 的 JSON 行按引擎分成 `(规则, 文件, 行)` 集合。
fn keys_by_engine(stdout: &str, engine: &str) -> HashSet<(String, String, usize)> {
    stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .filter(|j| j.get("engine").and_then(|v| v.as_str()) == Some(engine))
        .map(|j| {
            (
                j["rule"].as_str().unwrap_or_default().to_string(),
                j["file"].as_str().unwrap_or_default().to_string(),
                j["start_line"].as_u64().unwrap_or_default() as usize,
            )
        })
        .collect()
}

#[test]
fn three_way_ledger_is_non_empty_end_to_end() {
    let fx = corpus().join("only_fast.rs");
    let unwrap_line = line_of(&fx, "v.unwrap()");
    let panic_line = line_of(&fx, "panic!(");

    let (stdout, stderr) = scan_both(&corpus());

    // 桶计数：两条只归快轨。
    assert!(
        stderr.contains("三方账：两边都报 0 / 仅快轨 2 / 仅深轨 0"),
        "三方账的桶计数不对：{stderr}"
    );
    // 明细行要带规则码与锚得回夹具的行号。
    assert!(
        stderr.contains("仅快轨 RS-UNWRAP-USE") && stderr.contains(&format!(":{unwrap_line}")),
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

#[test]
fn both_bucket_is_non_empty_on_the_chained_rust_taint_flow() {
    // DD-0002 的另一半。期望行号取自夹具文本：`std::env::var(...).unwrap_or_default()`
    // 是源点+声明过的 propagator，`Command::new(raw)` 那行才是汇点——这个先后关系是夹具
    // 自己声明的判据（片A2 就写着"必须报 1 条"），不是按门跑出来的数回填的。
    let direct = deep_fixtures().join("taint_direct.rs");
    let source_line = line_of(&direct, "unwrap_or_default");
    let sink_line = line_of(&direct, "std::process::Command::new(raw)");
    assert!(
        source_line < sink_line,
        "夹具形状变了：源点行 {source_line} 不该在汇点行 {sink_line} 之后"
    );

    let (stdout, stderr) = scan_both(&deep_fixtures());
    let fast = keys_by_engine(&stdout, "tree-sitter");
    let deep = keys_by_engine(&stdout, "mir");
    // 按夹具名取键：`file` 的形状跟着调用者给的路径走（绝对/相对都可能），
    // 这里要钉的是"两引擎用的是同一个键"，不是字符串长什么样。
    let on_direct = |set: &HashSet<(String, String, usize)>| -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = set
            .iter()
            .filter(|(r, f, _)| r == "RS-TAINT-COMMAND" && f.ends_with("taint_direct.rs"))
            .map(|(_, f, l)| (f.clone(), *l))
            .collect();
        v.sort();
        v
    };
    let fh = on_direct(&fast);
    let dh = on_direct(&deep);
    assert!(
        !fh.is_empty(),
        "快轨在这份链式夹具上 0 条 ⇒ 片M 的修复没生效：{stdout}"
    );
    assert!(
        fh.iter().any(|(_, l)| *l == sink_line),
        "快轨没在链式源点的汇点行 {sink_line} 报（片M 的漏报回归）：{fh:?}"
    );
    assert_eq!(
        fh, dh,
        "同一条链式流程两键不同源 ⇒ 三方账会把它拆成两桶（见 DD-0013）：fast={fh:?} deep={dh:?}"
    );
    // 桶本身：同键的那几条必须进「两边都报」，而不是被拆成两桶双计。
    let shared = fast.intersection(&deep).count();
    assert!(
        shared >= 1,
        "两引擎没有任何同键发现 ⇒ 「两边都报」又被结构填空了：{stdout}"
    );
    // 三个桶的计数要和两侧集合自洽（门不许自己数一套、集合另一套）。
    assert_eq!(
        parse_bucket(&stderr, "两边都报"),
        shared,
        "「两边都报」计数 ≠ 复算交集（fast={} deep={shared}共享）：{stderr}",
        fast.len()
    );
    assert_eq!(
        parse_bucket(&stderr, "仅快轨"),
        fast.len() - shared,
        "「仅快轨」计数不自洽：{stderr}"
    );
    assert_eq!(
        parse_bucket(&stderr, "仅深轨"),
        deep.len() - shared,
        "「仅深轨」计数不自洽：{stderr}"
    );
}

/// 从 `三方账：两边都报 a / 仅快轨 b / 仅深轨 c` 里取某个桶的计数。
fn parse_bucket(stderr: &str, label: &str) -> usize {
    let seg = stderr
        .split("三方账：")
        .nth(1)
        .unwrap_or_else(|| panic!("没打到三方账行：{stderr}"));
    let mut parts = seg.split('/').map(str::trim);
    for _ in 0..3 {
        let item = parts.next().unwrap_or_default();
        if let Some(rest) = item.strip_prefix(label) {
            return rest
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("桶 {label} 没数字：{item}"));
        }
    }
    panic!("三方账行里没有桶 {label}：{seg}")
}
