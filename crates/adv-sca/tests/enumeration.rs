//! 枚举面与文本面的金丝雀——三处**手动变异分诊量出来的空洞**，逐条对应：
//!
//! | 变异（门里报 MISSED） | 分诊读数 | 空洞性质 |
//! |---|---|---|
//! | `Report::red -> empty()` | adv-sca 绿 / **adv-cli 红** | 归属错：断言只活在 adv-cli 的退出码测试里，包作用域下不算覆盖 |
//! | `join -> String::new()` / `"xyzzy"` | 两边都绿 | 真·断言弱：`detail` 里"缺了哪些依赖名"没人看 |
//! | `find_locks` 的 `==`「`!=`」、`skip_dir -> true/false`、`exclude` 的 `\|\|`「`&&`」 | adv-sca 绿 | 覆盖空洞：**目录枚举面与 exclude 逻辑零测试** |
//!
//! 补法按 playbook §2.3：判据搬回**代码所在包**（本文件在 adv-sca 内），而不是让门去跑跨包测试。

use std::path::{Path, PathBuf};

use adv_sca::lockcheck::scan_paths;

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

#[test]
fn the_red_filter_and_the_detail_text_are_pinned_in_this_package() {
    let report = scan_paths(&[data("drift")]).expect("漂移语料必须能判定");
    // ① 过滤器本身：红必须真出得来（这条断言就是 adv-cli 里那条退出码测试的**本包形态**）。
    let reds: Vec<_> = report.red().collect();
    assert_eq!(
        reds.len(),
        1,
        "漂移语料该有且只有 1 条判红：{:?}",
        report.issues
    );
    // ② detail 的文本契约：要把缺失的依赖名点出来（人读的账不能退化成空串）。
    let detail = &reds[0].detail;
    assert!(
        detail.contains("清单声明、锁里没有"),
        "详细文案漂了：{detail}"
    );
    assert!(detail.contains("secrets"), "缺失依赖名没点出来：{detail}");
}

#[test]
fn directory_scan_ignores_skipped_subtrees_and_foreign_lock_files() {
    // 语料里摆了三个诱饵：`target/Cargo.lock`、`node_modules/Cargo.lock`、`other.lock`。
    // 只有顶层那把是真的：数出 1 把、0 个跳过、0 条判红。
    let report = scan_paths(&[data("dir_scan")]).expect("目录扫描必须能判定");
    assert_eq!(
        report.checked_locks.len(),
        1,
        "只该判顶层那一把：{:?}",
        report.checked_locks
    );
    assert!(
        report.skipped.is_empty(),
        "诱饵不该被当成待判定的锁（进 skipped 就说明 skip 规则或文件名判据破了）：{:?}",
        report.skipped
    );
    assert_eq!(report.red().count(), 0, "{:?}", report.issues);
}

#[test]
fn excluded_parent_prunes_the_member_under_it() {
    // members 里显式写了 `crates/sub/inner`，而 `exclude = ["crates/sub"]` 把它剪掉——
    // inner 的清单声明了锁里没有的 `ghost`，**不许**判漂移（它是被排除的成员）。
    let inner = data("excluded_parent/crates/sub/inner/Cargo.toml");
    assert!(inner.is_file(), "语料前提没了：{}", inner.display());
    let report = scan_paths(&[data("excluded_parent")]).expect("判定必须能跑");
    assert_eq!(report.checked_locks.len(), 1, "{:?}", report.checked_locks);
    assert!(
        report.skipped.is_empty(),
        "被排除的成员不该让整把锁退成跳过：{:?}",
        report.skipped
    );
    assert_eq!(
        report.red().count(),
        0,
        "被 exclude 的成员上不该有判定：{:?}",
        report.issues
    );
}
