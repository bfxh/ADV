//! 摘要行与快轨的观察面（片D 收尾补）：`tally.files`、逐规则计数、快轨真出发现。
//!
//! 由来是变异门实测：`crates/adv-cli/src/main.rs` 有 3 条存活变异
//! （`tally.files += 1` → `*= 1`、`*n += 1` → `*= 1`、`scan_fast` 整体掏空），
//! 手工打变异跑 `cargo test -p adv-cli` 全绿 ⇒ 判定是**断言弱**而不是归属错：
//! 摘要数字只走 stderr，而当时的测试只读 stdout 与三方账那行。
//! 口径（片A3/A5 已定）：把判据搬回代码所在包补断言，不靠把键塞进基线缩小度量面。

use std::path::PathBuf;
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus() -> PathBuf {
    manifest_dir().join("tests").join("data").join("py-corpus")
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("adv-cli 在 crates/ 下")
        .parent()
        .expect("crates 的父目录是仓根")
        .to_path_buf()
}

/// 走真路径：起 `adv.exe` 扫语料目录，stdout 收发现、stderr 收摘要。
fn scan_ast() -> (String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_adv"))
        .arg("scan")
        .arg(corpus())
        .arg("--rules")
        .arg(repo_root().join("rules"))
        .arg("--engine")
        .arg("ast")
        .output()
        .expect("启动 adv 失败");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn summary_names_file_count_and_per_rule_counts() {
    let (stdout, stderr) = scan_ast();
    // 一行摘要同时钉住三个数：文件计数（`+= 1`→`*= 1` 会停在 0）、总条数、快轨条数
    // （scan_fast 整体被掏空必须在这里红）。ast 档不跑深轨，故深轨恒 0。
    assert!(
        stderr.contains("adv scan：2 个文件，3 条发现（快轨 3 / 深轨 0"),
        "摘要里的三个数没被钉住：{stderr}"
    );
    // 逐规则计数（`*n *= 1` 会把每条都写成 1）
    assert!(
        stderr.contains("PY-EVAL-USE: 2"),
        "同规则多条命中要计到 2，摘要没体现就说明计数不可观察：{stderr}"
    );
    assert!(stderr.contains("PY-EXEC-USE: 1"), "{stderr}");
    assert_eq!(
        stdout.lines().filter(|l| l.contains("\"rule\":")).count(),
        3,
        "stdout 应出 3 条发现（2 eval + 1 exec）：{stdout}"
    );
}
