//! 片C 的端到端对账：旧引擎（Python 污点，规则编在 `rust/` 内部）vs 新快轨（tree-sitter，
//! 规则在 `rules/*.yaml`）在同一批语料上的旧/新账。
//!
//! **这不是强弱对比**：两边规则集不同源、判定形态也不同（旧=污点流，带 source_line/flow；
//! 新=模式规则为主，`PY-EVAL-USE` 见到 `eval(` 就报）。本文件钉的是"哪些同、哪些不同、差在哪"。
//!
//! 旧侧是**冻结金样**（`tests/golden/py-old-new.old.json`），测试里不现跑旧引擎——那要 cargo 套
//! cargo（测试进程再起 `--manifest-path rust/Cargo.toml`），又慢又把这条测试绑在旧仓构建上。
//! 重录（人工、有意为之，与 `deep-cargo.jsonl` 同款纪律）：
//!
//! ```text
//! UNIFIED_RX_SANDBOX='*' cargo run -q --manifest-path rust/Cargo.toml --bin rx-taint -- \
//!   crates/adv-cli/tests/data/py-old-new | tail -1 \
//!   > crates/adv-cli/tests/golden/py-old-new.old.json
//! ```
//!
//! 新侧现跑（那是我们自己的二进制），并以**仓根为 cwd + 相对路径**跑——与生成金样时同口径，
//! 免得金样里嵌进本机绝对路径。期望行号一律锚回语料文本。

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

/// 金样与扫描共用的相对口径（相对仓根）。
const CORPUS_REL: &str = "crates/adv-cli/tests/data/py-old-new";

fn corpus_abs() -> PathBuf {
    repo_root().join(CORPUS_REL)
}

/// 语料里 `needle` 首次出现的行号（1 起）——期望值锚回文本，不抄跑出来的数。
fn line_of(name: &str, needle: &str) -> usize {
    let p = corpus_abs().join(name);
    std::fs::read_to_string(&p)
        .expect("读语料")
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("{} 里没有 {needle}", p.display()))
        + 1
}

#[test]
fn old_and_new_python_ledgers_match_the_frozen_contract() {
    let d_eval = line_of("d_param.py", "eval(cmd)");
    let literal_eval = line_of("e_literal.py", "eval(\"1 + 1\")");
    // 调用点在语料里必须存在——删了它，下面"新侧对该调用点一无所知"就退化成空断言。
    let call_site = line_of("g_cross_use.py", "sink_it(cmd)");
    assert!(call_site > 0, "语料形状变了：跨文件语的调用点不见了");
    let sink_def = line_of("f_cross_lib.py", "eval(x)");

    // ── 新侧：现跑，逐行比冻结金样 ──────────────────────────────────────────────
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .current_dir(repo_root())
        .arg("scan")
        .arg(CORPUS_REL)
        .arg("--rules")
        .arg("rules")
        .arg("--engine")
        .arg("ast")
        .output()
        .expect("启动 adv 失败");
    assert!(
        out.status.success(),
        "新快轨扫描非零退出：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut got: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('{'))
        .map(str::to_string)
        .collect();
    got.sort();
    let golden_path = manifest_dir()
        .join("tests")
        .join("golden")
        .join("py-old-new.fast.jsonl");
    let golden_raw = std::fs::read_to_string(&golden_path).expect("新侧金样应存在");
    let want: Vec<String> = golden_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    assert_eq!(
        got, want,
        "新快轨在这批语料上的账漂了——改动必须是有意识重录（重录命令见本文件头）"
    );

    let has = |file: &str, line: usize| {
        let f = format!("\"file\":\"{CORPUS_REL}/{file}\"");
        let l = format!("\"start_line\":{line}");
        got.iter().any(|x| x.contains(&f) && x.contains(&l))
    };
    assert!(
        has("d_param.py", d_eval),
        "两边共命中那条：新侧应有 d_param 的 eval"
    );
    assert!(
        has("f_cross_lib.py", sink_def),
        "同键不同义：新侧在 `f_cross_lib.py` 报的是**模式命中**（`eval(x)` 字面就在那儿）"
    );
    assert!(
        has("e_literal.py", literal_eval),
        "「新独有」那条：字面量喂 eval，新侧照样报（无源点概念）"
    );
    assert!(
        !stdout.contains("g_cross_use.py"),
        "新侧对**调用点**一无所知（`sink_it` 不在它的汇点名单里）：{stdout}"
    );
}

/// 旧侧：冻金样 vs 语料结构——同键两处、新独有一处、跨文件 origin 链、两边都不在调用点报。
#[test]
fn old_side_golden_keeps_the_cross_file_chain() {
    let d_eval = line_of("d_param.py", "eval(cmd)");
    let literal_eval = line_of("e_literal.py", "eval(\"1 + 1\")");
    let sink_def = line_of("f_cross_lib.py", "eval(x)");
    // ── 旧侧：冻结金样 + 与语料结构对账 ────────────────────────────────────────
    let old_path = manifest_dir()
        .join("tests")
        .join("golden")
        .join("py-old-new.old.json");
    let old_raw = std::fs::read_to_string(&old_path).expect("旧侧金样应存在");
    let old: serde_json::Value =
        serde_json::from_str(old_raw.trim()).expect("旧侧金样应为合法 JSON");
    let findings = old["findings"].as_array().expect("findings 应为数组");
    let old_at = |file: &str, line: usize| {
        findings
            .iter()
            .any(|f| f["file"] == file && f["line"].as_u64() == Some(line as u64))
    };

    assert_eq!(
        old["files_scanned"].as_u64(),
        Some(4),
        "旧侧扫到的文件数漂了"
    );
    assert_eq!(
        old["cross_file_findings"].as_u64(),
        Some(1),
        "旧引擎的跨文件链是这批语料里最不可替代的一条"
    );
    assert!(old_at("d_param.py", d_eval), "两边共命中那条：旧侧也应有");
    assert!(
        !old_at("e_literal.py", literal_eval),
        "「新独有」的另一半：旧引擎没有源点就不报"
    );
    assert!(
        old_at("f_cross_lib.py", sink_def),
        "旧侧那条也落在 `f_cross_lib.py` 的 sink 上——键同、语义不同"
    );
    assert!(
        !findings.iter().any(|f| f["file"] == "g_cross_use.py"),
        "旧侧不在调用点报（它报的是**被追到的定义处**）"
    );

    // ── 映射表的三行各有实测条目撑着（不是把话说漂亮） ─────────────────────────
    // ① 同键不同义：同一个键 `f_cross_lib.py:3`，旧侧记的是**跨文件流**并带 origin 链。
    let cross = findings
        .iter()
        .find(|f| f["file"] == "f_cross_lib.py" && f["line"].as_u64() == Some(sink_def as u64))
        .expect("上面刚断言过存在");
    assert_eq!(cross["flow"], "cross", "旧侧这条应是跨文件流：{cross}");
    let origin = cross["origin"].as_str().unwrap_or_default();
    assert!(
        origin.contains("g_cross_use.py"),
        "origin 链要点名调用者文件（新账里没有任何对应物）：{cross}"
    );
    // ② 新独有：字面量 eval —— 已在上面两个方向各断言一次。
    // ③ 旧独有语义：`cross_file_findings` 这个计数新账里没有概念，改动它必须是有意识的；
    //    "同键"的另一半（新侧也在这一行报）由上面那条新侧测试断言。
    assert!(old_at("f_cross_lib.py", sink_def));
}
