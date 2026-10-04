//! 金丝雀：证明门会红（旧仓纪律「金丝雀先记基线再验红」——全绿的门不值得信）。

use std::collections::BTreeMap;
use xtask::god::{MAX_FN_LINES, analyze_source, hard_violations, ratchet_violations};
use xtask::lockstep::check_lockstep;
use xtask::mutants::new_missed;
use xtask::suppress::file_violations;

fn long_fn_source(lines: usize) -> String {
    let mut s = String::from("fn big() {\n");
    for i in 0..lines {
        s.push_str(&format!("    let _x{i} = {i}; // filler\n"));
    }
    s.push_str("}\n");
    s
}

#[test]
fn canary_long_fn_is_caught() {
    let over = MAX_FN_LINES as usize + 10;
    let src = format!("mod m {{\n{}\n}}\n", long_fn_source(over));
    let entries = analyze_source("crates/fake/src/canary.rs", &src);
    let caught = entries
        .iter()
        .filter(|(k, _)| k.starts_with("fn:"))
        .any(|(_, v)| *v > MAX_FN_LINES);
    assert!(
        caught,
        "金丝雀失败：{} 行的函数没被抓到（阈 {MAX_FN_LINES}）",
        over
    );
    assert!(
        !hard_violations(&entries).is_empty(),
        "金丝雀失败：硬阈检查没有报红"
    );
}

#[test]
fn canary_impl_members_are_counted() {
    let src = r#"
struct S { a: u32, b: u32, c: u32 }
impl S {
    fn one(&self) -> u32 { self.a }
    fn two(&self) -> u32 { self.b }
    fn three(&self) -> u32 { self.c }
}
"#;
    let entries = analyze_source("crates/fake/src/impl_canary.rs", src);
    assert_eq!(entries["type:crates/fake/src/impl_canary.rs::S(impl)"], 3);
    assert_eq!(entries["type:crates/fake/src/impl_canary.rs::S(fields)"], 3);
    assert!(entries.contains_key("fn:crates/fake/src/impl_canary.rs::S::two"));
}

#[test]
fn canary_ratchet_fails_on_growth_and_unregistered() {
    let mut baseline = BTreeMap::new();
    baseline.insert("fn:a.rs::f".to_string(), 10);
    let mut current = BTreeMap::new();
    current.insert("fn:a.rs::f".to_string(), 11); // 恶化
    current.insert("fn:a.rs::g".to_string(), 3); // 新面未登记
    let v = ratchet_violations(&current, &baseline);
    assert_eq!(v.len(), 2, "棘轮必须同时抓恶化与未登记：{v:?}");

    // 只准减：基线大于当前 = 绿
    let mut smaller = BTreeMap::new();
    smaller.insert("fn:a.rs::f".to_string(), 9);
    assert!(ratchet_violations(&smaller, &baseline).is_empty());
}

#[test]
fn canary_lockstep_rejects_drift() {
    // 引导期：无 adv-v tag 只许 0.1.0
    assert!(check_lockstep("0.1.0", &[]).is_ok());
    assert!(check_lockstep("0.2.0", &[]).is_err());
    // 旧仓 v* tag 不参与判定（adv-v 前缀隔离，2026-10-04 首跑裁定）
    assert!(check_lockstep("0.1.0", &["v2.58.0".to_string()]).is_ok());
    // 有 adv-v tag：必须精确对齐最新 tag（旧仓 36 minor 断档教训的负样本）
    let tags = vec!["adv-v0.1.0".to_string(), "adv-v0.2.0".to_string()];
    assert!(check_lockstep("0.2.0", &tags).is_ok());
    assert!(check_lockstep("0.3.0", &tags).is_err());
    // 非法版本
    assert!(check_lockstep("abc", &[]).is_err());
}

#[test]
fn canary_expired_suppression_is_red() {
    let src = "fn f() { v.unwrap() } // adv:allow(RS-UNWRAP-USE, reason=旧账, until=2026-01-01)
";
    let v = file_violations("t.rs", src, "2026-10-04");
    assert!(
        v.iter().any(|x| x.contains("抑制到期")),
        "金丝雀失败：过期抑制没被抓到：{v:?}"
    );
}

#[test]
fn canary_valid_suppression_and_malformed() {
    // 有效抑制 = 绿
    let ok = "fn f() { v.unwrap() } // adv:allow(RS-UNWRAP-USE, reason=测试, until=2999-01-01)
";
    assert!(file_violations("t.rs", ok, "2026-10-04").is_empty());
    // 畸形（缺 until）= 红
    let bad = "// adv:allow(RS-UNWRAP-USE, reason=忘了日期)
fn f() {}
";
    let v = file_violations("t.rs", bad, "2026-10-04");
    assert!(
        v.iter().any(|x| x.contains("畸形")),
        "金丝雀失败：畸形抑制没被抓到：{v:?}"
    );
}

#[test]
fn canary_new_missed_mutants_are_red() {
    let baseline = vec!["a.rs::old_fn".to_string()];
    let current = vec![
        "a.rs::old_fn".to_string(),     // 存量债 = 绿
        "taint.rs::new_fn".to_string(), // 新债 = 红
    ];
    let v = new_missed(&current, &baseline);
    assert_eq!(
        v,
        vec!["taint.rs::new_fn".to_string()],
        "新 missed 必须红，存量 missed 绿"
    );
    // 全捕获 = 绿
    assert!(new_missed(&[], &baseline).is_empty());
}
