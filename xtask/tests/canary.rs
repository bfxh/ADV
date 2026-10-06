//! 金丝雀（god / lockstep / 抑制 / 试验面）：证明这些门会红——全绿的门不值得信。

use std::collections::BTreeMap;
use xtask::god::{
    MAX_FN_LINES, MEMBER_DIRS, analyze_source, collect_workspace_entries, hard_violations,
    ratchet_violations, rerecord_note,
};
use xtask::lockstep::check_lockstep;
use xtask::maturity::{experimental_patterns, pattern_matches};
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

/// 这条判据决定"哪些文件不进 god 计量"，放宽一分就有承重面被放过。
#[test]
fn canary_experimental_pattern_only_covers_what_it_says() {
    let pat = "crates/*/tests/data/**";
    assert!(pattern_matches(
        pat,
        "crates/adv-cli/tests/data/taint-crate/src/lib.rs"
    ));
    assert!(
        pattern_matches(pat, "crates/adv-cli/tests/data/x.rs"),
        "登记目录下的直属文件也要算试验面"
    );
    assert!(
        !pattern_matches(pat, "crates/adv-cli/tests/unit.rs"),
        "tests/ 下的真测试代码不是试验面"
    );
    assert!(
        !pattern_matches(pat, "crates/adv-cli/src/data/x.rs"),
        "* 不能跨路径段（中间多一段就不该命中）"
    );
    assert!(
        !pattern_matches(pat, "crates/adv-cli/tests/datamore/x.rs"),
        "* 是整段匹配，不是前缀匹配"
    );
    assert!(
        !pattern_matches(pat, "crates/adv-cli/tests/data"),
        "登记的是该目录以下：`**` 至少吃一层，不许比字面更宽"
    );
    // 精确 pattern（无通配）只能一对一
    assert!(pattern_matches("rules/rust/x.yaml", "rules/rust/x.yaml"));
    assert!(!pattern_matches("rules/rust/x.yaml", "rules/rust/y.yaml"));
    // 带星号的中段也要能拆开匹配
    assert!(pattern_matches(
        "crates/*/tests/fixtures/**",
        "crates/a/tests/fixtures/f.rs"
    ));
    assert!(!pattern_matches(
        "crates/*/tests/fixtures/**",
        "crates/a/tests/data/f.rs"
    ));
}

/// 匹配器的星号形态：`*` 只吃段内字符（可出现在段首/段中/段尾），`**` 只认结尾。
/// 登记面能不能被精确圈住，全看这一条——多认一种写法就多一处没人测的分支。
#[test]
fn canary_experimental_matcher_covers_the_star_shapes_we_register() {
    assert!(pattern_matches(
        "crates/*/tests/data/**",
        "crates/a/tests/data/x/y.rs"
    ));
    assert!(pattern_matches("a*b.rs", "axb.rs"));
    assert!(pattern_matches("a*b.rs", "ab.rs"), "`*` 可以吃零个字符");
    assert!(!pattern_matches("a*b.rs", "axc.rs"));
    assert!(pattern_matches(
        "*_gate.py",
        "scripts/debt_gate.py".split('/').next_back().unwrap()
    ));
    assert!(
        !pattern_matches("**/x.rs", "x.rs"),
        "开头的 ** 不被支持（只有结尾 ** 有语义），所以它必须一律不命中"
    );
    // 登记子集之外的写法（`**` 不在结尾）不认——写窄比写宽安全：
    // 误放承重面的代价是门失去分辨力，误紧的代价只是多计量一次。
    assert!(!pattern_matches("**/x.rs", "a/x.rs"));
    assert!(!pattern_matches(
        "crates/*/tests/data/**",
        "crates/a/tests/data"
    ));
    // 一段里多个 `*`：中间那段之后必须还剩字符，否则下一个 `*` 无物可吃。
    // 不测这一形，`seg_eq` 里那条守卫就是无人观察的分支（片A6 同类问题）。
    assert!(
        pattern_matches("a*b*c", "aXXbYYc"),
        "多星形态的常规情况要能匹配"
    );
    assert!(
        pattern_matches("a*b*c", "abc"),
        "glob 口径：`*` 可以吃零个字符"
    );
    // 末段必须贴到段尾（这条是 2026-10-06 补的真实缺陷：旧实现只按顺序找段）
    assert!(pattern_matches("a*b", "aXb"));
    assert!(
        !pattern_matches("a*b", "aXbY"),
        "末段后面还有字符就不该算匹配——旧实现会误配"
    );
    assert!(pattern_matches("*b", "ab"));
    assert!(pattern_matches("*b", "b"), "开头的 * 可以吃零个字符");
    assert!(!pattern_matches("a*", "a/b"), "`*` 不跨路径段");
    assert!(pattern_matches("a*", "a"), "结尾的 * 可以吃零个字符");
}

/// 读登记表的两条失败路径都必须**不放松**（返回空集 = god 照旧全量计量）。
#[test]
fn canary_maturity_registry_read_failures_do_not_loosen_the_gate() {
    let root = std::env::temp_dir().join(format!("adv-maturity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("spec")).expect("建 spec");
    let missing = experimental_patterns(&root);
    std::fs::write(root.join("spec/maturity.json"), "{ 坏 json").expect("写坏文件");
    let broken = experimental_patterns(&root);
    std::fs::write(
        root.join("spec/maturity.json"),
        r#"{"entries":[{"pattern":"crates/*/tests/data/**","why":"w","guards":"g"}]}"#,
    )
    .expect("写好文件");
    let good = experimental_patterns(&root);
    let _ = std::fs::remove_dir_all(&root);
    assert!(missing.is_empty(), "登记表缺失时不该给出任何放松 pattern");
    assert!(broken.is_empty(), "登记表坏了时不该给出任何放松 pattern");
    assert_eq!(good, vec!["crates/*/tests/data/**".to_string()]);
}

/// 端到端：登记过的试验面真的不进 god 计量，同 crate 的非试验面照进。
#[test]
fn canary_god_really_skips_registered_experimental_faces() {
    let root = std::env::temp_dir().join(format!("adv-god-skip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("spec")).expect("建 spec");
    std::fs::write(
        root.join("spec/maturity.json"),
        r#"{"entries":[{"pattern":"crates/*/tests/data/**","why":"合成材料","guards":"某测试"}]}"#,
    )
    .expect("写登记表");
    for dir in MEMBER_DIRS {
        std::fs::create_dir_all(root.join(dir)).expect("建成员目录");
    }
    let demo = root.join("crates/adv-core");
    std::fs::create_dir_all(demo.join("src")).expect("建 src");
    std::fs::create_dir_all(demo.join("tests/data")).expect("建 tests/data");
    std::fs::write(demo.join("src/lib.rs"), "pub fn kept() {}\n").expect("写承重文件");
    std::fs::write(demo.join("tests/data/blob.rs"), "pub fn skipped() {}\n").expect("写试验文件");

    let got = collect_workspace_entries(&root).expect("计量不该失败");
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        got.contains_key("file:crates/adv-core/src/lib.rs"),
        "承重面被漏掉（试验面放松过头）：{got:?}"
    );
    assert!(
        !got.contains_key("file:crates/adv-core/tests/data/blob.rs"),
        "登记的试验面仍在计量 ⇒ god 的跳过没生效"
    );
}

#[test]
fn canary_rerecord_note_names_every_moved_face() {
    // DD-0006 在 Rust 侧的同一条：整表重录不许只报"写了 N 项"。
    // 2026-10-06 实测兑现：把金丝雀按门拆成三个文件后基线 diff 有 24 条删除，全是改名；
    // 没有这份点名，"没丢面"这件事只能靠逐行读 JSON 来确认。
    let old: BTreeMap<String, u64> = [
        ("file:a.rs".to_string(), 10u64),
        ("file:b.rs".to_string(), 20u64),
        ("fn:a.rs::f".to_string(), 5u64),
    ]
    .into_iter()
    .collect();
    let new: BTreeMap<String, u64> = [
        ("file:a.rs".to_string(), 14u64),
        ("file:c.rs".to_string(), 30u64),
        ("fn:a.rs::f".to_string(), 5u64),
    ]
    .into_iter()
    .collect();
    let note = rerecord_note(&old, &new);
    assert!(
        note.starts_with("重录对账：基线 3 → 3 项（新增 1、移除 1、变大 1）"),
        "{note}"
    );
    assert!(note.contains("新增：file:c.rs"), "{note}");
    assert!(note.contains("移除：file:b.rs"), "{note}");
    assert!(note.contains("变大：file:a.rs: 10 → 14"), "{note}");
    // 一模一样 ⇒ 三个计数都是 0（不许把"无变化"写成含糊的"已写"）。
    let same = rerecord_note(&new, &new);
    assert!(same.contains("新增 0、移除 0、变大 0"), "{same}");
    assert_eq!(same.matches('\n').count(), 0, "{same}");
}
