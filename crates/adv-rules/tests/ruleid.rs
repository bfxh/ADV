//! 规则测试门禁（ruleid 注释逐行一致；漏报与误报都算失败）。

use adv_parse::Language;
use std::path::PathBuf;

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rules")
}

fn load() -> Vec<adv_rules::Rule> {
    adv_rules::load_rules(&rules_dir()).unwrap()
}

const PY_EVAL: &str = r#"
import os
value = eval(input())  # ruleid: PY-EVAL-USE
safe = len(input())
evalx(value)
obj.eval(value)
exec("print(1)")  # ruleid: PY-EXEC-USE
"#;

const PY_SUBPROCESS: &str = r#"
import subprocess
subprocess.run("ls", shell=True)  # ruleid: PY-SUBPROCESS-SHELL
subprocess.run(["ls"])
subprocess.run("ls", shell=False)
subprocess.check_output(cmd, shell=True)  # ruleid: PY-SUBPROCESS-SHELL
"#;

const RS_UNWRAP: &str = r#"
fn main() {
    let v = vec![1];
    let a = v.get(0).unwrap(); // ruleid: RS-UNWRAP-USE
    let b = v.get(1);
    let c = std::env::var("X").unwrap(); // ruleid: RS-UNWRAP-USE
    let _ = (a, b, c);
}
"#;

const RS_PANIC: &str = r#"
fn f(x: i32) -> i32 {
    if x < 0 {
        panic!("negative"); // ruleid: RS-PANIC-USE
    }
    x
}
"#;

#[test]
fn ruleid_annotations_match_exactly() {
    let rules = load();
    adv_rules::testing::check_fixture("py_eval", Language::Python, PY_EVAL, &rules).unwrap();
    adv_rules::testing::check_fixture("py_subprocess", Language::Python, PY_SUBPROCESS, &rules)
        .unwrap();
    adv_rules::testing::check_fixture("rs_unwrap", Language::Rust, RS_UNWRAP, &rules).unwrap();
    adv_rules::testing::check_fixture("rs_panic", Language::Rust, RS_PANIC, &rules).unwrap();
}

#[test]
fn negative_case_has_zero_findings() {
    // 反例：无注解的干净代码必须零发现（防"永远红"的假阳规则）
    let rules = load();
    let clean = "x = 1\ny = x + 2\n";
    let ast = adv_parse::parse_source(Language::Python, clean).unwrap();
    let f = adv_rules::matcher::run_matchers(
        &ast,
        std::path::Path::new("clean.py"),
        Language::Python,
        &rules,
    );
    assert!(f.is_empty(), "干净代码不应有发现：{f:?}");
}

#[test]
fn load_rules_is_fail_closed() {
    // 空目录即红（沿用旧仓 arch_gate 空规则即红的教训）
    let tmp = std::env::temp_dir().join("adv-rules-empty-test");
    let _ = std::fs::create_dir_all(&tmp);
    assert!(adv_rules::load_rules(&tmp).is_err(), "空规则目录必须报错");
    // 不支持的语言声明即红
    let bad = tmp.join("bad.yaml");
    std::fs::write(
        &bad,
        "id: X\nlanguages: [cobol]\nseverity: info\nmessage: m\nmatch:\n  call:\n    callee: a\n",
    )
    .unwrap();
    assert!(adv_rules::load_rules(&tmp).is_err(), "不支持语言必须报错");
    let _ = std::fs::remove_file(&bad);
}

#[test]
fn finding_spans_point_at_call() {
    let rules = load();
    let ast = adv_parse::parse_source(Language::Python, PY_EVAL).unwrap();
    let f = adv_rules::matcher::run_matchers(
        &ast,
        std::path::Path::new("t.py"),
        Language::Python,
        &rules,
    );
    let eval = f
        .iter()
        .find(|x| x.rule == "PY-EVAL-USE")
        .expect("eval 发现缺失");
    assert_eq!(eval.start_line, 3, "发现行号应指向违规代码行");
    assert_eq!(eval.file, "t.py");
}
