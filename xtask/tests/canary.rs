//! 金丝雀（god / lockstep / 抑制 / 试验面）：证明这些门会红——全绿的门不值得信。

use std::collections::BTreeMap;
use xtask::god::{
    MAX_FILE_LINES, MAX_FN_LINES, MAX_TYPE_MEMBERS, MEMBER_DIRS, analyze_source,
    collect_workspace_entries, hard_violations, ratchet_violations, rerecord_note, write_baseline,
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

#[test]
fn canary_write_baseline_really_writes_and_rejects_hard_violations() {
    // 门报 `god.rs::write_baseline` 的"整体换成 Ok(())"存活：登记动作本身没有观察面。
    // 这个函数是"把现状转正"的唯一入口，它不写盘/不拦硬阈都必须被抓到。
    let dir = std::env::temp_dir().join(format!("adv-god-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let target = dir.join("tools/baselines/god-baseline.json");

    let ok: BTreeMap<String, u64> = [("file:crates/adv-core/src/lib.rs".to_string(), 42u64)]
        .into_iter()
        .collect();
    write_baseline(&dir, &ok).expect("阈内应当写得进去");
    assert!(
        target.is_file(),
        "write_baseline 说成功了却没落盘：{target:?}"
    );
    let raw = std::fs::read_to_string(&target).expect("读回基线");
    assert!(raw.contains("crates/adv-core/src/lib.rs"), "{raw}");

    // 超硬阈 ⇒ 拒写，而且**不许留下半截基线**（旧仓教训：整表吞掉新超标面）。
    let mut over = ok.clone();
    over.insert("file:crates/adv-core/src/huge.rs".to_string(), 5000);
    std::fs::remove_file(&target).expect("清掉上一轮的基线");
    let err = write_baseline(&dir, &over).expect_err("超硬阈必须拒绝登记");
    assert!(err.to_string().contains("硬阈"), "{err}");
    assert!(!target.is_file(), "拒写却仍然动了基线文件：{target:?}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// DD-0018 分诊：`record_fn` 的 `+ 1` 写成 `* 1` 时每个函数少算一行——棘轮里的函数行数
/// 系统性偏低，而"少算"正是 god 门最不该有的方向（超长函数因此晚一轮才被抓）。
/// 判据不手打行数：从 syn 报出来的 `fn` 区间算，两边必须闭合。
#[test]
fn canary_recorded_fn_span_includes_the_closing_brace() {
    // `fn one() {}\nfn two() {\n  let a = 1;\n  let b = 2;\n}\n` —— 5 行里的两个函数。
    let src = "fn one() {}\nfn two() {\n    let a = 1;\n    let b = 2;\n}\n";
    let entries = analyze_source("crates/fake/src/span.rs", src);
    assert_eq!(
        entries["fn:crates/fake/src/span.rs::one"], 1,
        "单行函数就是 1 行（少算成 0 就是 `+` 写成了 `*`）"
    );
    assert_eq!(
        entries["fn:crates/fake/src/span.rs::two"], 4,
        "从签名行到闭合花括号（含）整段都算（`saturating_sub(start) + 1`）"
    );
}

/// `type_name_of` 的 `syn::Type::Reference` 臂删掉后，`impl Foo for &Bar` 会被记成
/// `type:…::impl(impl)`——所有引用类型的 impl 面混成一个键，棘轮从此分辨不出它们。
#[test]
fn canary_impl_on_a_reference_type_is_named_by_its_inner_type() {
    let src = "struct W;\nimpl std::ops::Deref for &W {\n    type Target = W;\n}\n";
    let entries = analyze_source("crates/fake/src/refimpl.rs", src);
    assert!(
        entries.contains_key("type:crates/fake/src/refimpl.rs::W(impl)"),
        "引用类型上的 impl 要按内层类型名计账：{:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert!(
        !entries.contains_key("type:crates/fake/src/refimpl.rs::impl(impl)"),
        "删掉 Reference 臂后 impl 面会塌成 `impl` 一个键：{:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert_eq!(entries["type:crates/fake/src/refimpl.rs::W(impl)"], 1);
}

/// trait / enum 的成员计数：三个 `visit_*` 的"整体替换成 ()"（整个函数体被抹掉）
/// 在此前没有任何观察面——god 从来没有针对"trait 默认方法 / enum 变体 / trait 项"的夹具。
#[test]
fn canary_trait_enum_and_default_methods_all_get_counted() {
    let src = r#"
enum E { A, B, C }
trait T {
    const N: u32;
    fn provided(&self) -> u32 { 1 }
    fn required(&self);
}
"#;
    let entries = analyze_source("crates/fake/src/traits.rs", src);
    assert_eq!(
        entries["type:crates/fake/src/traits.rs::E(variants)"], 3,
        "enum 变体数必须计数"
    );
    assert_eq!(
        entries["type:crates/fake/src/traits.rs::T(items)"], 3,
        "trait 项数（const + 两个 fn）必须计数"
    );
    assert!(
        entries.contains_key("fn:crates/fake/src/traits.rs::provided"),
        "trait 默认方法有函数体就要计入函数面：{:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert!(
        !entries.contains_key("fn:crates/fake/src/traits.rs::required"),
        "没有默认实现的声明不该计入函数面"
    );
}

/// `collect_workspace_entries` 的两个 `&&` 反过来是**漏扫**：`target` 与点开头的目录
/// 会走进计量，`target/` 里的生成物就会变成承重面（且体积巨大 ⇒ 硬阈误报）。
#[test]
fn canary_workspace_scan_skips_target_and_dot_directories() {
    let root = std::env::temp_dir().join(format!("adv-scan-{}-{}", std::process::id(), file!()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in MEMBER_DIRS {
        std::fs::create_dir_all(root.join(dir)).expect("建成员目录");
    }
    let kept = root.join("crates/adv-core/src");
    std::fs::create_dir_all(&kept).expect("建 src");
    std::fs::write(kept.join("lib.rs"), "pub fn kept() {}\n").expect("写承重文件");
    // 两种该被排除的目录各摆一份超阈内容：一个 170 行的单函数——fn 线当场炸（file 线
    // 不炸），所以漏扫的第一症状是硬阈/未登记面直接点名 generated.rs。
    let big = format!(
        "fn huge_junk() {{\n{}\n}}\n",
        "    let _x = 1;\n".repeat(MAX_FN_LINES as usize + 50)
    );
    for junk in ["crates/adv-core/target", "crates/adv-core/.hidden"] {
        let dir = root.join(junk);
        std::fs::create_dir_all(&dir).expect("建垃圾目录");
        std::fs::write(dir.join("generated.rs"), &big).expect("写垃圾文件");
    }
    // 名字以 `.rs` 结尾的**目录**：受扫判据的 `is_file && ext` 写成 `||` 时它会被当成
    // 源文件去读 ⇒ 读目录失败 ⇒ 整个计量报错（fail-closed 那半也在这里钉住）。
    std::fs::create_dir_all(root.join("crates/adv-core/src/fake.rs")).expect("建 .rs 目录");

    let entries = collect_workspace_entries(&root).expect("计量不该失败");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        entries.contains_key("file:crates/adv-core/src/lib.rs"),
        "承重面漏掉了：{:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert!(
        entries
            .keys()
            .all(|k| !k.contains("target/") && !k.contains(".hidden")),
        "生成物/隐藏目录混进计量面了（`&&` 反过来时的症状）：{:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert!(
        hard_violations(&entries).is_empty(),
        "垃圾目录漏进来就会顶出硬阈违规：{:?}",
        hard_violations(&entries)
    );
}

/// `hard_violations` 的 `>` 写成 `>=`：**正好等于**阈值的面被判成违规，于是
/// `--write` 重录在"没有真违规"时也会拒写（DD-0006 那次登记就会莫名失败）。
#[test]
fn canary_hard_threshold_treats_exactly_at_cap_as_green() {
    let at_cap: BTreeMap<String, u64> = [
        (
            "file:crates/adv-core/src/lib.rs".to_string(),
            MAX_FILE_LINES,
        ),
        ("fn:a.rs::f".to_string(), MAX_FN_LINES),
        ("type:a.rs::T(impl)".to_string(), MAX_TYPE_MEMBERS),
    ]
    .into_iter()
    .collect();
    assert!(
        hard_violations(&at_cap).is_empty(),
        "正好等于上限不算违规（`>` 写成 `>=` 会在这里红）：{:?}",
        hard_violations(&at_cap)
    );
    let mut over = at_cap.clone();
    over.insert("fn:a.rs::f".to_string(), MAX_FN_LINES + 1);
    assert_eq!(
        hard_violations(&over).len(),
        1,
        "超一行就要报红（判据不能被放松成 <）"
    );
}

/// `ratchet_violations` 的 `>` 写成 `>=` 时，**与基线持平**的面全被判成恶化 ⇒
/// 任何一次"没变"的登记都会被报成"变大了"，棘轮失去分辨力。
#[test]
fn canary_ratchet_treats_equal_to_baseline_as_green() {
    let baseline: BTreeMap<String, u64> = [
        ("fn:a.rs::f".to_string(), 10),
        ("fn:a.rs::g".to_string(), 4),
    ]
    .into_iter()
    .collect();
    assert!(
        ratchet_violations(&baseline, &baseline).is_empty(),
        "与基线持平不算恶化（`>` 写成 `>=` 会在这里红）"
    );
    // 只准减的面仍然绿（棘轮的本意）。
    let smaller: BTreeMap<String, u64> =
        [("fn:a.rs::f".to_string(), 8), ("fn:a.rs::g".to_string(), 4)]
            .into_iter()
            .collect();
    assert!(ratchet_violations(&smaller, &baseline).is_empty());
    // 变大 1 就要报。
    let bigger: BTreeMap<String, u64> = [
        ("fn:a.rs::f".to_string(), 11),
        ("fn:a.rs::g".to_string(), 4),
    ]
    .into_iter()
    .collect();
    assert_eq!(ratchet_violations(&bigger, &baseline).len(), 1);
}

/// `init_or_write`（`--write` 的实现）整体换成 `Ok(())` 时，`--write` 会**静默无操作**地
/// 返回成功：人以为登记完了，基线没动，下一轮照旧报红。
#[test]
fn canary_init_or_write_actually_lands_the_baseline() {
    let dir = std::env::temp_dir().join(format!("adv-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for dir_name in MEMBER_DIRS {
        std::fs::create_dir_all(dir.join(dir_name)).expect("建成员目录");
    }
    std::fs::create_dir_all(dir.join("crates/adv-core/src")).expect("建 src");
    std::fs::write(dir.join("crates/adv-core/src/lib.rs"), "pub fn kept() {}\n")
        .expect("写承重文件");
    let target = dir.join("tools/baselines/god-baseline.json");

    xtask::god::init_or_write(&dir).expect("登记不该失败");
    assert!(
        target.is_file(),
        "init_or_write 说成功了却没落盘（整体换成 Ok(()) 的症状）：{target:?}"
    );
    let raw = std::fs::read_to_string(&target).expect("读回基线");
    assert!(raw.contains("crates/adv-core/src/lib.rs"), "{raw}");

    // 超硬阈的面存在时必须拒写（不许留下半截基线）。
    std::fs::remove_file(&target).expect("清掉基线");
    std::fs::write(
        dir.join("crates/adv-core/src/huge.rs"),
        "fn huge() {\n".to_string()
            + &"    let _x = 1;\n".repeat(MAX_FN_LINES as usize + 50)
            + "}\n",
    )
    .expect("写超阈文件");
    let err = xtask::god::init_or_write(&dir).expect_err("超硬阈必须拒绝登记");
    assert!(err.to_string().contains("硬阈"), "{err}");
    assert!(!target.is_file(), "拒写却仍然动了基线文件：{target:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `suppress` 的两个遍历谓词（跳 `target`/点目录、只扫 `.rs` 常规文件）六个突变全部存活：
/// 真跑里没人喂过边界形状。判据是**真值表**，两个方向都写死。
#[test]
fn canary_suppress_walk_and_file_predicates_are_truth_tables() {
    use std::ffi::OsStr;
    use xtask::suppress::{is_rust_source, walk_keep};
    fn n(s: &'static str) -> Option<&'static OsStr> {
        Some(OsStr::new(s))
    }

    // 遍历保留：只有 `target` 与点开头的名字被排除。
    assert!(!walk_keep(OsStr::new("target")));
    assert!(!walk_keep(OsStr::new(".hidden")));
    assert!(!walk_keep(OsStr::new(".git")));
    assert!(walk_keep(OsStr::new("src")), "普通目录要保留");
    assert!(
        walk_keep(OsStr::new("targetx")),
        "`target` 是整名相等，不是前缀（`==` 反过来时这条会红）"
    );
    assert!(
        walk_keep(OsStr::new("target.rs")),
        "带扩展名的文件不是 target 目录"
    );

    // 受扫文件：只扫 `.rs` 常规文件。
    assert!(is_rust_source(true, n("rs")));
    assert!(
        !is_rust_source(false, n("rs")),
        "目录名以 rs 结尾也不是源文件"
    );
    assert!(!is_rust_source(true, n("txt")), "只扫 rs");
    assert!(!is_rust_source(true, None), "没有扩展名不是源文件");
    assert!(!is_rust_source(true, n("RS")), "扩展名大小写敏感");
}

/// `lockstep` 的 tag 列表解析：`delete !` 后空行会被当成一个 tag ⇒ 版本串带着空行去比对，
/// 锁步判定要么恒绿要么恒红，取决于哪边先坏。
#[test]
fn canary_lockstep_tag_parsing_drops_blank_and_trims() {
    use xtask::lockstep::parse_tag_list;
    assert_eq!(
        parse_tag_list("adv-v0.1.0\nadv-v0.2.0\n"),
        vec!["adv-v0.1.0".to_string(), "adv-v0.2.0".to_string()]
    );
    assert_eq!(
        parse_tag_list("adv-v0.1.0\r\n\r\n  adv-v0.2.0  \r\n"),
        vec!["adv-v0.1.0".to_string(), "adv-v0.2.0".to_string()],
        "CRLF 与行首尾空白都要吃掉（`delete !` 时空行会漏进来）"
    );
    assert!(parse_tag_list("").is_empty());
    assert!(parse_tag_list("\n\n").is_empty(), "只有空行 = 没有 tag");
    assert!(
        parse_tag_list("v2.58.0\nadv-v0.1.0\n").contains(&"v2.58.0".to_string()),
        "解析不认前缀：过滤 adv-v* 是 git tag --list 那侧的事"
    );
}

/// `pattern_matches` 的 `&&` 写成 `||` 后：**结尾**的 `**` 也会被判成"非法的中段 `**`"
/// ⇒ 所有登记的目录型试验面一律不命中 ⇒ god 重新全量计量（放松失效，方向是"更严"，
/// 但真实后果是登记面失效后没人知道为什么 god 变慢了）。
#[test]
fn canary_trailing_double_star_still_matches_below_the_registered_dir() {
    // 结尾 `**`：目录以下任意深度都要命中（这是 `&&` 反过来时的漏网面）。
    assert!(
        pattern_matches("crates/*/tests/data/**", "crates/a/tests/data/x.rs"),
        "结尾 ** 至少吃一层"
    );
    assert!(
        pattern_matches("crates/*/tests/data/**", "crates/a/tests/data/x/y/z.rs"),
        "结尾 ** 吃任意深度"
    );
    // 同一段里 `**` 与其它条件同时成立（`contains("**")` 为真）时不能被误判成非法中段。
    assert!(
        pattern_matches("a/**", "a/b/c"),
        "两段 pattern 的结尾 ** 是最深一段（`||` 反过来时这条会红）"
    );
    // 中段的 `**` 一律非法：deep-tail pattern 里出现非末段 `**` 必须整体不命中。
    // 内层守卫的 `&&` 写成 `||` 时这一条会放过（2026-10-11 诊断实测：改错的那一侧
    // 恰好是**内层**——外层 `&&` 的突变早被现有断言杀掉，内层没人照）。
    assert!(
        !pattern_matches("a/**/b/**", "a/x/b/y"),
        "中段 ** 非法：deep-tail pattern 里夹着 ** 必须整体不命中"
    );
    // 反向：开头的 `**` 依旧不支持（登记面比字面宽就是放松过头）。
    assert!(!pattern_matches("**/x.rs", "a/x.rs"));
}

/// `seg_eq` 的回溯语义：回溯时让上一个 `*` 多吃一个字符再试，结尾 `*` 要能吃掉剩余全部。
/// （`pi = sp + 1` 的 `+→*` 突变经差分实证为**等价**——见账本 DD-0018 ②——所以这条
/// 钉的是回溯本身不被改坏，不声称能杀那一条。）
#[test]
fn canary_seg_eq_backtracks_so_trailing_star_eats_the_rest() {
    // 回溯必须发生：'X' 匹配不上 'b'，只能让 `*` 多吃一个字符再试。
    assert!(pattern_matches("a*b", "aXb"));
    assert!(!pattern_matches("a*b", "aXbY"), "末段后面还有字符就不算");
    // 结尾 `*`：回溯后要能把剩下的全吃掉（回溯语义的常规面；`pi = sp + 1` 的 `+→*`
    // 突变经差分实证为等价——见账本 DD-0018 ②——这条钉的是回溯本身不被改坏）。
    assert!(pattern_matches("a*", "aXbYc"));
    assert!(pattern_matches("a*", "a"), "零字符也要算");
    // 多段回溯：第一个 `*` 吃够之后要能落到第二段上。
    assert!(pattern_matches("a*b*c", "aXXbYYc"));
    assert!(!pattern_matches("a*b*c", "aXXbYYd"));
}
