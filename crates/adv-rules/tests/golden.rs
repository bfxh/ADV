//! 报告层金标准门（旧仓 cli-golden 思想：JSONL 行契约冻结，改动必须是有意识的）。
//!
//! 夹具 = 源码字符串 + 标签路径（零进程、零临时文件）；行序不是契约（规则加载序
//! 随目录枚举波动）⇒ 比对前排序。更新基线：`ADV_UPDATE_GOLDEN=1 cargo test -p adv-rules`。

use adv_parse::Language;
use std::path::{Path, PathBuf};

const PY_FIXTURE: &str = "import subprocess\n\ndef handler(cmd):\n    s = input()\n    eval(s)\n    subprocess.run(cmd, shell=True)\n    exec(\"print(1)\")\n";
const RS_FIXTURE: &str =
    "pub fn load() -> Option<u32> {\n    let v = vec![1u32];\n    v.first().copied().unwrap()\n}\n";

fn golden_lines() -> Vec<String> {
    let rules = adv_rules::load_rules(Path::new("../../rules")).unwrap();
    let mut out = Vec::new();
    for (file, lang, src) in [
        ("FIXTURE/py/eval.py", Language::Python, PY_FIXTURE),
        ("FIXTURE/rs/unw.rs", Language::Rust, RS_FIXTURE),
    ] {
        let ast = adv_parse::parse_source(lang, src).unwrap();
        for f in adv_rules::run_matchers(&ast, Path::new(file), lang, &rules, "2026-10-04") {
            out.push(f.to_jsonl());
        }
    }
    out.sort();
    out
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/scan.jsonl")
}

#[test]
fn report_jsonl_matches_golden() {
    let actual = golden_lines();
    let golden_file = golden_path();
    if std::env::var("ADV_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_file.parent().expect("parent")).expect("建 golden 目录");
        std::fs::write(&golden_file, actual.join("\n") + "\n").expect("写 golden");
        eprintln!("golden 已更新（{} 行）", actual.len());
        return;
    }
    let expected_raw = std::fs::read_to_string(&golden_file).expect("golden 文件应存在");
    let mut expected: Vec<String> = expected_raw.lines().map(str::to_string).collect();
    expected.sort();
    assert_eq!(
        actual, expected,
        "报告层与金标准不一致——若是有意变更，用 ADV_UPDATE_GOLDEN=1 重录并在提交里披露"
    );
}
