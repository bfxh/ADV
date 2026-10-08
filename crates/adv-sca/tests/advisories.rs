//! M3-3a 金丝雀：快照读取、区间判定边界、以及"查不到≠没查成"。
//!
//! 语料是真数据（出处与 sha256 见同目录 `osv/README.md`）；两条 advisory 各自钉一个边界：
//! `time` 的多段区间（8 段、16 事件）与 `rustc-serialize` 的 `last_affected` 闭上界。

use std::path::{Path, PathBuf};

use adv_sca::inventory::{Inventory, parse_lock};
use adv_sca::lockcheck::scan_paths;
use adv_sca::matcher::{load_snapshot, match_inventory, scan_with_advisories};
use adv_sca::snapshot::{load_manifest, md5_hex, parse_listing_page};

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

/// 一段最小锁文本 → Inventory（只有 registry 包才参与匹配，source 串必须与判定面一致）。
fn lock_with(entries: &[(&str, &str)]) -> Inventory {
    let mut text = String::from(
        "# synthetic\nversion = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n",
    );
    for (name, version) in entries {
        text.push_str(&format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\
             source = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"deadbeef\"\n"
        ));
    }
    parse_lock(&text, Path::new("Cargo.lock")).unwrap()
}

#[test]
fn the_fixture_snapshot_loads_all_three_records() {
    let idx = load_snapshot(&data("osv")).unwrap();
    assert_eq!(idx.records, 3);
    assert_eq!(idx.advisories_for("time").len(), 1);
    assert_eq!(idx.advisories_for("rustc-serialize").len(), 1);
    assert!(idx.advisories_for("definitely-not-a-crate").is_empty());
}

#[test]
fn time_boundaries_follow_the_real_advisory() {
    // 真语料的 8 段区间（16 事件）逐段是 `[x-0, x)`，最后一段是 **[0.2.7-0, 0.2.23)**
    // ——写测试时我按直觉以为"0.2.7 已修"，真数据当场纠正：0.2.7 落在最后一段里，是受影响的。
    // 这条测试就按真边界钉死四个端点。
    let idx = load_snapshot(&data("osv")).unwrap();
    let hit = match_inventory(&lock_with(&[("time", "0.1.44")]), &idx).unwrap();
    assert_eq!(hit.len(), 1, "{hit:?}");
    assert_eq!(hit[0].id, "RUSTSEC-2020-0071");
    assert!(
        hit[0].severity.starts_with("CVSS_V3"),
        "严重度要从 severity[].score 带出：{}",
        hit[0].severity
    );
    assert!(hit[0].red);

    let cases = [
        ("0.2.0", false),
        ("0.2.7", true),
        ("0.2.22", true),
        ("0.2.23", false),
    ];
    for (version, expect) in cases {
        let got = !match_inventory(&lock_with(&[("time", version)]), &idx)
            .unwrap()
            .is_empty();
        assert_eq!(
            got, expect,
            "time {version} 的命中期望错了（真区间：[0.2.7-0,0.2.23) 是最后一段）"
        );
    }
}

#[test]
fn last_affected_is_inclusive_on_a_real_advisory() {
    let idx = load_snapshot(&data("osv")).unwrap();
    let inside = match_inventory(&lock_with(&[("rustc-serialize", "0.3.24")]), &idx).unwrap();
    assert_eq!(
        inside.len(),
        1,
        "0.3.24 就是 last_affected，必须命中：{inside:?}"
    );
    let outside = match_inventory(&lock_with(&[("rustc-serialize", "0.3.25")]), &idx).unwrap();
    assert!(
        outside.is_empty(),
        "0.3.25 在闭区间外，不许命中：{outside:?}"
    );
}

#[test]
fn local_path_packages_are_never_matched() {
    // 本地 path 包没有"发布版本"；同名同版本也不该去查库（本仓有 6 个 vendored crate 是这种）。
    let text = "# synthetic\nversion = 4\n\n[[package]]\nname = \"time\"\nversion = \"0.1.44\"\n";
    let inv = parse_lock(text, Path::new("Cargo.lock")).unwrap();
    let idx = load_snapshot(&data("osv")).unwrap();
    assert!(match_inventory(&inv, &idx).unwrap().is_empty());
}

#[test]
fn an_empty_snapshot_directory_is_refused_not_read_as_clean() {
    // 语料是仓里**已提交**的空 crates.io/（带 .gitkeep）——测试不往工作树里写东西。
    let err = load_snapshot(&data("osv_empty")).unwrap_err();
    assert!(err.to_string().contains("这不等于没有漏洞"), "{err}");
}

#[test]
fn manifest_etags_equal_the_md5_of_the_files_they_describe() {
    // GCS 对普通对象的 ETag 就是内容 MD5（2026-10-08 对两份真对象实测）。
    // 这条测试把那个事实钉在语料上：manifest 里 etag == md5 == 文件的 md5。
    let m = load_manifest(&data("osv")).unwrap();
    assert_eq!(m.objects.len(), 3);
    for (key, meta) in &m.objects {
        let bytes = std::fs::read(data("osv").join(key)).unwrap();
        assert_eq!(md5_hex(&bytes), meta.md5, "{key} 的 md5 与记账不符");
        assert_eq!(
            meta.etag, meta.md5,
            "{key} 的 etag 与 md5 不符（GCS 语义变了？）"
        );
        assert_eq!(bytes.len() as u64, meta.size, "{key} 的字节数与记账不符");
    }
}

#[test]
fn the_real_listing_slice_parses_with_its_token() {
    let xml = std::fs::read_to_string(data("osv/listing-page.txt")).unwrap();
    let (objs, token) = parse_listing_page(&xml).unwrap();
    assert_eq!(objs.len(), 2);
    assert_eq!(objs[0].key, "crates.io/GHSA-2226-4v3c-cff8.json");
    assert_eq!(objs[1].etag, "d5b43c07c6a025c2be387b782f3f23e0");
    assert!(token.is_some(), "真响应带分页 token，形状要认出来");
}

#[test]
fn scan_with_advisories_judges_exactly_the_locks_lockcheck_judged() {
    // 两面各自走一遍遍历；集合不一致就该判不了（这里钉住它们一致）。
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let locks: Vec<PathBuf> = [
        "Cargo.lock",
        "rust/Cargo.lock",
        "third_party/noseyparker/Cargo.lock",
    ]
    .iter()
    .map(|l| repo.join(l))
    .collect();
    let report = scan_paths(&locks).unwrap();
    let idx = load_snapshot(&data("osv")).unwrap();
    let (_, judged) = scan_with_advisories(&locks, &idx).unwrap();
    assert_eq!(judged, report.checked_locks, "两面判过的锁必须逐字一致");
}

#[test]
fn informational_advisories_are_signals_not_red() {
    // 真语料 RUSTSEC-2021-0120 是 `informational = "unsound"`（RustSec 的「不是漏洞通告」一类）。
    // 本层口径：带 informational 的命中**不参与退出码**，但要把标记与严重度如实带出来。
    let idx = load_snapshot(&data("osv")).unwrap();
    let hit = match_inventory(&lock_with(&[("abomonation", "0.1.7")]), &idx).unwrap();
    assert_eq!(hit.len(), 1, "{hit:?}");
    assert_eq!(hit[0].id, "RUSTSEC-2021-0120");
    assert_eq!(hit[0].informational.as_deref(), Some("unsound"));
    assert!(!hit[0].red, "informational 类不许参与退出码");
    assert_eq!(hit[0].severity, "informational(unsound)");
}
#[test]
fn scan_with_advisories_refuses_when_no_lock_was_judged() {
    // 匹配面自己的"零锁"出口（变异门抓到过 `ensure_any_lock_judged -> Ok(())`：那样"一把锁都没判"
    // 会被读成"没有命中"，正是漏报的形状）。
    let empty = data("osv_empty"); // 有 crates.io/ 但没有 Cargo.lock
    let idx = load_snapshot(&data("osv")).unwrap();
    let err = scan_with_advisories(&[empty], &idx).unwrap_err();
    assert!(err.to_string().contains("不等于没有漏洞"), "{err}");
}
