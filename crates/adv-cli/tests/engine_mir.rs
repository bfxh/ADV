//! 片A3 验收：`adv scan --engine mir|both` 的行契约对齐与三方账。
//! 判据走真路径——起 `adv.exe`，由它再起边车驱动子进程；测试不重实现映射逻辑。

use std::path::PathBuf;
use std::process::Command;

/// 快轨 Finding 的行契约字段集（少一个键或多一个键都算契约漂移）。
const CONTRACT_KEYS: &[&str] = &[
    "rule",
    "severity",
    "message",
    "file",
    "start_line",
    "start_col",
    "end_line",
    "end_col",
    "engine",
];

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("adv-cli 在 crates/ 下")
        .to_path_buf()
}

fn fixtures_dir() -> PathBuf {
    crates_dir()
        .join("adv-ast-rust")
        .join("tests")
        .join("fixtures")
}

fn repo_root() -> PathBuf {
    crates_dir()
        .parent()
        .expect("crates 的父目录是仓根")
        .to_path_buf()
}

fn scan(engine: &str, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(fixtures_dir())
        .arg("--rules")
        .arg(repo_root().join("rules"))
        .arg("--engine")
        .arg(engine)
        .args(extra)
        .output()
        .expect("启动 adv 失败")
}

fn lines_of(stdout: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("stdout 不是 JSONL：{line}（{e}）"))
        })
        .collect()
}

/// 夹具里 `after` 之后首次出现 `needle` 的行号（1 起）——期望值锚到夹具文本。
fn line_after(path: &PathBuf, after: &str, needle: &str) -> usize {
    let text = std::fs::read_to_string(path).expect("读夹具");
    let mut seen = false;
    for (idx, l) in text.lines().enumerate() {
        if seen && l.contains(needle) {
            return idx + 1;
        }
        if l.contains(after) {
            seen = true;
        }
    }
    panic!("{} 里 {after} 之后找不到 {needle}", path.display());
}

#[test]
fn mir_lines_satisfy_the_fast_track_contract() {
    let out = scan("mir", &[]);
    assert!(
        out.status.success(),
        "mir 档退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let items = lines_of(&out.stdout);
    assert_eq!(items.len(), 4, "深轨在该语料应出 4 条，实得 {items:?}");
    for item in &items {
        let obj = item.as_object().expect("finding 应是 JSON 对象");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort();
        let mut want = CONTRACT_KEYS.to_vec();
        want.sort();
        assert_eq!(keys, want, "行契约字段集漂移：{item}");
        assert_eq!(item["engine"], serde_json::json!("mir"));
        assert_eq!(item["rule"], serde_json::json!("RS-TAINT-COMMAND"));
    }
    // 行号逐条锚回夹具文本（不是把上面的数字抄一遍）
    let fx = fixtures_dir();
    let expect_direct = line_after(&fx.join("taint_direct.rs"), "pub fn direct", "Command::new");
    let got_direct: Vec<usize> = items
        .iter()
        .filter(|i| {
            i["file"]
                .as_str()
                .is_some_and(|f| f.ends_with("taint_direct.rs"))
        })
        .map(|i| i["start_line"].as_u64().unwrap() as usize)
        .collect();
    assert_eq!(
        got_direct,
        vec![expect_direct],
        "taint_direct 的发现没锚在汇点行"
    );
    let expect_ops = line_after(
        &fx.join("taint_operators.rs"),
        "let raw = std::env::var",
        "Command::new",
    );
    let got_ops: Vec<usize> = items
        .iter()
        .filter(|i| {
            i["file"]
                .as_str()
                .is_some_and(|f| f.ends_with("taint_operators.rs"))
        })
        .map(|i| i["start_line"].as_u64().unwrap() as usize)
        .collect();
    assert_eq!(
        got_ops,
        vec![expect_ops],
        "taint_operators 在产品规则集下应只报经 env::var 的那条流（源点是 source_int 的整型链不在产品规则里）"
    );
}

#[test]
fn both_reports_three_way_tally_on_stderr() {
    let out = scan("both", &[]);
    assert!(
        out.status.success(),
        "stderr：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let items = lines_of(&out.stdout);
    assert_eq!(
        items.len(),
        4,
        "并集应仍是这 4 条（快轨在该 rust 语料 0 条）"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("三方账：两边都报 0 / 仅快轨 0 / 仅深轨 4"),
        "三方账没按实测出账：{stderr}"
    );
}

#[test]
fn bad_driver_path_fails_loud() {
    let out = scan("mir", &["--driver", "Z:/nope/adv-ast-rust-driver.exe"]);
    assert!(
        !out.status.success(),
        "驱动找不到时必须红（不能退 0 装作扫过）"
    );
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(stderr.contains("深轨不可用"), "报错要点名深轨：{stderr}");
}

#[test]
fn unknown_engine_is_rejected() {
    let out = scan("jit", &[]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "未知档位应拒绝：{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
