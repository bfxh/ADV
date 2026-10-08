//! M3-3b 金丝雀：RustSec 读取面 + 对账器（三种差异各一条）。
//!
//! 语料两份：`rustsec_db/` 是三份**真** `RUSTSEC-*.md` 的逐字切片（出处见 `osv/README.md` 同源一节），
//! `reconcile/` 是合成树——它一份语料同时钉住三条码：A 两边都有但字段不一致、
//! B 仅 OSV 有（信号）、C 仅 RustSec 有（**红**，漏的形状）。

use std::path::{Path, PathBuf};

use adv_sca::reconcile::{FIELD_MISMATCH, OSV_ONLY, RS_ONLY, diff, osv_views, reconcile};
use adv_sca::rustsec::load_db;

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

#[test]
fn real_slices_load_with_their_marks() {
    let db = load_db(&data("rustsec_db")).unwrap();
    assert_eq!(db.advisories.len(), 3);
    let abom = &db.advisories["RUSTSEC-2021-0120"];
    assert_eq!(abom.package, "abomonation");
    assert_eq!(abom.informational.as_deref(), Some("unsound"));
    let time = &db.advisories["RUSTSEC-2020-0071"];
    assert!(
        !time.patched.is_empty(),
        "time 那条有 patched 表：{:?}",
        time.patched
    );
    let serde = &db.advisories["RUSTSEC-2022-0004"];
    assert!(
        serde.aliases.iter().any(|a| a == "GHSA-2226-4v3c-cff8"),
        "别名要把 GHSA 互认带出来：{:?}",
        serde.aliases
    );
}

#[test]
fn the_synthetic_tree_fires_exactly_the_three_codes() {
    let report = reconcile(&data("reconcile")).unwrap();
    let mut codes: Vec<&str> = report.divergences.iter().map(|d| d.code).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(
        codes,
        vec![FIELD_MISMATCH, OSV_ONLY, RS_ONLY],
        "{:?}",
        report.divergences
    );
    assert_eq!(report.osv_total, 2);
    assert_eq!(report.rustsec_total, 2);
    assert_eq!(report.both, 1);
}

#[test]
fn only_rustsec_side_is_the_red_direction() {
    let report = reconcile(&data("reconcile")).unwrap();
    let reds: Vec<&str> = report.red().map(|d| d.code).collect();
    assert_eq!(
        reds,
        vec![RS_ONLY],
        "只有「仅 RustSec 有」判红：{:?}",
        report.divergences
    );
    for d in &report.divergences {
        if d.code == RS_ONLY {
            assert!(d.id == "RUSTSEC-2099-0003", "{}", d.detail);
        } else {
            assert!(!d.red, "{} 不该判红", d.code);
        }
    }
}

#[test]
fn the_real_pair_agrees_on_the_three_shared_ids() {
    // 三份真切片里有两份能在两路同时出现（abomonation / time）；对账要求的字段应逐字一致。
    let report = reconcile(&data("reconcile_real")).unwrap();
    assert_eq!(report.osv_total, 2);
    assert_eq!(report.rustsec_total, 2);
    assert_eq!(report.both, 2);
    assert!(
        report.divergences.is_empty(),
        "真配对不该有差异（有差异说明两库口径真的分了叉，要记下来）：{:?}",
        report.divergences
    );
}

#[test]
fn a_tree_without_the_rustsec_side_is_refused() {
    let err = reconcile(&data("osv")).unwrap_err();
    assert!(err.to_string().contains("没有 crates/ 子目录"), "{err}");
}

#[test]
fn diff_is_a_pure_function_over_two_views() {
    // 判据本体可脱离 IO 单独喂（变异门与将来的别的数据源都打这一层）。
    let db = load_db(&data("rustsec_db")).unwrap();
    let osv = osv_views(&data("reconcile_real/crates.io")).unwrap();
    let ds = diff(&db, &osv);
    assert_eq!(ds.len(), 1, "{ds:?}");
    assert_eq!(ds[0].code, RS_ONLY);
    assert_eq!(ds[0].id, "RUSTSEC-2022-0004", "DB 里多出来的那份要判红");
    assert!(ds[0].red);
}
