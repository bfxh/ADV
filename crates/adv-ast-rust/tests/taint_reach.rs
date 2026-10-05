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

/// 夹具里 `after_needle` 之后首次出现 `needle` 的行号（1 起）。
fn line_after(fixture_name: &str, after_needle: &str, needle: &str) -> usize {
    let text = std::fs::read_to_string(fixture(fixture_name)).expect("读夹具");
    let mut seen = false;
    for (idx, line) in text.lines().enumerate() {
        if seen && line.contains(needle) {
            return idx + 1;
        }
        if line.contains(after_needle) {
            seen = true;
        }
    }
    panic!("夹具 {fixture_name} 里 {after_needle} 之后找不到 {needle}");
}

/// 跑污点档（产品规则集 rules/），返回解析后的 finding 列表。
fn findings(fixture_name: &str) -> Vec<serde_json::Value> {
    findings_at(&fixture(fixture_name), &repo_root().join("rules"))
}

/// 跑污点档，规则目录由调用方给（夹具专用规则放 tests/fixtures/rules、tests/dual，
/// 都不进产品规则集，也不进 adv-cli 测试所扫的语料）。
fn findings_at(
    fixture_path: &std::path::Path,
    rules_dir: &std::path::Path,
) -> Vec<serde_json::Value> {
    let out = driver()
        .arg("--taint")
        .arg(rules_dir)
        .arg(fixture_path)
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

/// 在夹具源码里找 `needle` 所在行（1 起）。期望值锚到夹具文本，
/// 不是按"我认为分析会报在哪"手写——这是仓里 ruleid 逐行一致口径的最小实现。
fn line_of(fixture_name: &str, needle: &str) -> usize {
    lines_of(fixture_name, needle)[0]
}

/// 夹具里出现 `needle` 的所有行号（1 起）——多条汇点的夹具要逐行对账。
fn lines_of(fixture_name: &str, needle: &str) -> Vec<usize> {
    let text = std::fs::read_to_string(fixture(fixture_name)).expect("读夹具");
    let out: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(idx, _)| idx + 1)
        .collect();
    assert!(!out.is_empty(), "夹具 {fixture_name} 里没有 {needle}");
    out
}

#[test]
fn direct_flow_is_found() {
    let hits = findings("taint_direct.rs");
    assert_eq!(hits.len(), 1, "直接流应恰报 1 条，实得 {hits:?}");
    assert_eq!(hits[0]["rule"], serde_json::json!("RS-TAINT-COMMAND"));
    assert_eq!(hits[0]["function"], serde_json::json!("direct"));
    assert_eq!(hits[0]["engine"], serde_json::json!("mir"));
    assert_eq!(
        hits[0]["start_line"],
        serde_json::json!(line_of("taint_direct.rs", "Command::new")),
        "发现没锚在汇点那一行"
    );
}

#[test]
fn sanitizer_kills_taint_and_control_still_reports() {
    // 同一文件两函数：sanitized 走 len（净化）、unsanitized 不走。
    // 期望恰 1 条且落在 unsanitized 的汇点行——多报说明 sanitizer 失效或被恒真污染，
    // 少报说明对照流断了。
    let hits = findings("taint_sanitized.rs");
    let want = line_after("taint_sanitized.rs", "pub fn unsanitized", "Command::new");
    let got: Vec<u64> = hits
        .iter()
        .map(|h| h["start_line"].as_u64().unwrap_or_default())
        .collect();
    assert_eq!(
        got,
        vec![want as u64],
        "sanitized 应零发现、unsanitized 应恰 1 条（汇点行 {want}），实得 {hits:?}"
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
    assert_eq!(
        hits[0]["start_line"],
        serde_json::json!(line_of("taint_branch.rs", "Command::new")),
        "跨块流没报在合流后的汇点行"
    );
}

#[test]
fn propagation_forms_are_covered() {
    // 夹具④：整型源点穿 BinaryOp + Cast、以及经 Ref 的两条汇点。
    // 变异门实测过：这三条 Rvalue arm 删掉都不影响前三条夹具的结论，故单独钉住。
    let rules = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rules");
    let hits = findings_at(&fixture("taint_operators.rs"), &rules);
    let got: Vec<u64> = hits
        .iter()
        .map(|h| h["start_line"].as_u64().unwrap())
        .collect();
    let mut want = lines_of("taint_operators.rs", "Command::new")
        .into_iter()
        .map(|l| l as u64)
        .collect::<Vec<u64>>();
    want.sort();
    let mut sorted_got = got.clone();
    sorted_got.sort();
    assert_eq!(
        sorted_got, want,
        "传播形态夹具的汇点行不符（实得 {got:?}，应为 {want:?}；完整 finding：{hits:?}）"
    );
}

#[test]
fn dump_calls_reports_real_callee_paths() {
    let out = driver()
        .arg("--dump-calls")
        .arg(fixture("taint_direct.rs"))
        .output()
        .expect("启动驱动失败");
    assert!(
        out.status.success(),
        "取证档退出码非零：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<Vec<String>> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| -> Vec<String> { line.split('\t').map(str::to_string).collect() })
        .filter(|f| f.len() == 4)
        .collect();
    let callees: Vec<&str> = lines.iter().map(|f| f[1].as_str()).collect();
    for expected in ["std::env::var", "std::process::Command::new"] {
        assert!(
            callees.contains(&expected),
            "取证档没列出 {expected}，实得 {callees:?}"
        );
    }
    // 目的局部与实参列必须是 `_N`/`const` 形态：换成空串或 "xyzzy" 都会在这里红。
    for f in &lines {
        assert!(
            f[2].starts_with('_') && f[2][1..].chars().all(|c| c.is_ascii_digit()),
            "目的局部列形态不符：{f:?}"
        );
        for token in f[3].split(',') {
            assert!(
                token.starts_with('_') || token == "const",
                "实参列形态不符：{f:?}"
            );
        }
    }
}

#[test]
fn product_rules_do_not_source_the_int_chain() {
    // 夹具④的整型源点 `source_int` 不在产品规则集里 ⇒ 产品规则下只该报 env::var 那条流。
    // 这条断言是专门用来在**本包内**杀 `tainted_operand -> true` 的：那个变异会让
    // BinaryOp/Cast 把无源点的整型也染上污点，命中数从 1 顶到 2。（此前它只有 adv-cli
    // 的跨包测试能杀，而 cargo-mutants 默认只跑变异所在包的测试，于是长期漏网。）
    let hits = findings("taint_operators.rs");
    let want = line_after(
        "taint_operators.rs",
        "let raw = std::env::var",
        "Command::new",
    );
    let got: Vec<u64> = hits
        .iter()
        .map(|h| h["start_line"].as_u64().unwrap_or_default())
        .collect();
    assert_eq!(
        got,
        vec![want as u64],
        "整型链在产品规则集下不该被当作有源点（应只报 {want} 行），实得 {hits:?}"
    );
}

#[test]
fn sanitizer_wins_over_propagator_for_same_name() {
    // 分支顺序 源点 → 净化工 → 传播子 → 未声明即杀；同名两列时净化工优先。
    // 这条钉住 all_sanitizers：它退化成空表/杂串时，此处从"零发现"翻成"报一条"。
    // 夹具与规则都放在 tests/dual/，**不进** adv-cli 测试所扫的 tests/fixtures 语料
    // （否则产品规则下这条流会多算一条，adv-cli 的条数断言会被我改动夹具这件事弄红）。
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("dual");
    let hits = findings_at(&dir.join("taint_dual_role.rs"), &dir);
    assert!(
        hits.is_empty(),
        "to_string 同时列为净化工与传播子时应零发现（净化工优先），实得 {hits:?}"
    );
}
