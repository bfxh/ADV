//! M3-2 判定段金丝雀：三条判据各自的"必红"与"必不红"都钉住。
//!
//! 语料命名禁区：`tests/data/**` 这些树**不在 workspace members 里、从不被构建**，
//! 只当数据读——所以它们内部的依赖名可以是假名。反过来，任何"扫整个仓"的用法
//! 都会把这些故意做坏的锁读进来（本文件因此逐把显式给路径，不做递归扫根目录）。

use std::path::{Path, PathBuf};

use adv_sca::inventory::{load_lock, load_workspace, parse_lock};
use adv_sca::lockcheck::{LC1_CHECKSUM, LC2_DRIFT, LC3_DUAL, LC3_MULTI, check_lock, scan_paths};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// 一把语料锁的读数，按"参与退出码 / 只出信号"分开 returned。
fn read(dir: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    let root = data(dir);
    let inv = load_lock(&root.join("Cargo.lock")).unwrap_or_else(|e| panic!("{dir} 锁读不出：{e}"));
    let ws = load_workspace(&root).unwrap_or_else(|e| panic!("{dir} 工作区读不出：{e}"));
    let mut red = Vec::new();
    let mut signal = Vec::new();
    for issue in check_lock(&inv, &ws) {
        if issue.red {
            red.push(issue.code);
        } else {
            signal.push(issue.code);
        }
    }
    red.sort_unstable();
    signal.sort_unstable();
    (red, signal)
}

#[test]
fn clean_corpus_has_nothing_to_report() {
    let (red, signal) = read("clean");
    assert!(red.is_empty(), "干净语料不该判红：{red:?}");
    assert!(signal.is_empty(), "干净语料不该出信号：{signal:?}");
}

#[test]
fn drift_corpus_is_the_5d0b459_shape() {
    let (red, _) = read("drift");
    assert_eq!(red, vec![LC2_DRIFT], "成员加了依赖、锁没跟上 ⇒ 必须判漂移");
}

#[test]
fn member_absent_from_lock_is_drift() {
    let (red, _) = read("member_absent");
    assert_eq!(red, vec![LC2_DRIFT]);
}

#[test]
fn registry_package_without_checksum_is_red() {
    let (red, _) = read("no_checksum");
    assert_eq!(red, vec![LC1_CHECKSUM]);
}

#[test]
fn dual_track_is_red_while_multi_version_is_only_a_signal() {
    // 同一份语料把两半都钉住：helper 既双轨（0.1.0 本地 + 0.2.0 registry）又多版本。
    let (red, signal) = read("dual_track");
    assert_eq!(
        red,
        vec![LC3_DUAL],
        "同名本地+registry 是 dependency confusion 的形状，必须判红"
    );
    assert_eq!(signal, vec![LC3_MULTI], "多版本只许出信号");
}

#[test]
fn nonmember_path_dev_deps_are_not_drift() {
    // 探针实测：本仓 5 处"缺"全是非成员 path 依赖的 dev-dependencies，cargo 本就不解析进锁。
    // 这条语料就是把那个观测钉成期望——判据若把它读成漂移就是一台假红机。
    let (red, signal) = read("nonmember_devdeps");
    assert!(red.is_empty(), "非成员的 dev 依赖不该算漂移：{red:?}");
    assert!(signal.is_empty(), "{signal:?}");
}

#[test]
fn renamed_dependency_matches_by_its_real_name() {
    let (red, signal) = read("rename");
    assert!(red.is_empty(), "重命名按真名对账，别名不该算缺失：{red:?}");
    assert!(signal.is_empty(), "{signal:?}");
}

#[test]
fn the_three_real_locks_in_this_repo_produce_no_red() {
    // 承重面金样：本仓三份真锁（根 298 包 / rust/ 1 包 / noseyparker 552 包）。
    // 逐把显式给路径——不递归扫根目录，否则会读到 tests/data 里故意做坏的语料。
    // CARGO_MANIFEST_DIR 是 crates/adv-sca ⇒ 仓根在它上面**两级**。
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let locks: Vec<PathBuf> = [
        "Cargo.lock",
        "rust/Cargo.lock",
        "third_party/noseyparker/Cargo.lock",
    ]
    .iter()
    .map(|l| repo.join(l))
    .collect();
    for lock in &locks {
        assert!(lock.is_file(), "语料前提没了：{}", lock.display());
    }
    let report = scan_paths(&locks).expect("真锁必须能判定");
    assert_eq!(report.checked_locks.len(), 3, "{:?}", report.checked_locks);
    assert!(
        report.skipped.is_empty(),
        "真锁一把都不许被跳过：{:?}",
        report.skipped
    );
    let red: Vec<&str> = report.red().map(|i| i.code).collect();
    assert!(red.is_empty(), "本仓锁不许判红：{red:?}");
    let multi = report.issues.iter().filter(|i| i.code == LC3_MULTI).count();
    assert!(
        multi > 0,
        "根锁实测有 8 组多版本；读数里一个信号都没有说明判据没跑起来"
    );
}

#[test]
fn a_lock_without_packages_is_refused_not_empty() {
    let err = parse_lock("version = 4\n", Path::new("Cargo.lock")).unwrap_err();
    assert!(err.to_string().contains("[[package]]"), "{}", err);
}

#[test]
fn nothing_judged_is_an_error_not_a_clean_run() {
    // 本 crate 的 src/ 里没有锁 ⇒ 读数不能算"干净"。（不建临时目录，C 盘紧。）
    let no_locks = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let err = scan_paths(&[no_locks]).unwrap_err();
    assert!(err.to_string().contains("不等于干净"), "{}", err);
}
