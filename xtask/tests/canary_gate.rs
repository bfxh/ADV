//! 金丝雀（`gate` 聚合壳 `xtask::gate`）：红的树上门不许说绿。
//!
//! 这一档原本是 bin 里的 `gate_run`，集成测试 import 不到它 ⇒ 变异门报
//! `replace gate_run -> Ok(vec![])` 存活（在全绿的树上它与真值同值），只能记成债
//! （DD-0011）。搬进 lib、root 做成参数后，这里可以直接喂进一根「子门必然红」的根。

use xtask::gate;
use xtask::god::MEMBER_DIRS;

fn fixture(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("adv-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// 造一根「god 棘轮恶化」的根：源码 3 行、基线登记 1 行 ⇒ 真值是红，不是绿。
/// 锁步在这里必须也能出判定（版本 0.1.0 且临时目录不在任何 git 仓内 ⇒ 无 `adv-v*` tag ⇒ 引导期放行）。
fn red_root() -> std::path::PathBuf {
    let root = fixture("gate-red");
    for dir in MEMBER_DIRS {
        std::fs::create_dir_all(root.join(dir)).expect("建成员目录");
    }
    let src = root.join("crates/adv-core/src");
    std::fs::create_dir_all(&src).expect("建 src");
    std::fs::write(
        src.join("lib.rs"),
        "pub fn a() {}\npub fn b() {}\npub fn c() {}\n",
    )
    .expect("写源码");
    std::fs::create_dir_all(root.join("tools/baselines")).expect("建基线目录");
    std::fs::write(
        root.join("tools/baselines/god-baseline.json"),
        r#"{"tool":"fixture","entries":{"file:crates/adv-core/src/lib.rs":1}}"#,
    )
    .expect("写基线");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace.package]\nversion = \"0.1.0\"\n",
    )
    .expect("写版本");
    root
}

#[test]
fn canary_gate_run_does_not_swallow_a_red_subgate() {
    let root = red_root();
    let violations = gate::run(&root).expect("三个子门在这根上都能出判定（判不了就别当绿）");
    std::fs::remove_dir_all(&root).expect("清理夹具");

    assert!(
        !violations.is_empty(),
        "棘轮恶化的根被聚合壳判成绿 ⇒ 整档换成空清单的变异在这里必须判红"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.starts_with("god: ") && v.contains("棘轮")),
        "god 的明细没带子门名进清单：{violations:?}"
    );
    // 前缀不许漂：定位一条违规靠的就是这个名字。
    assert!(
        violations
            .iter()
            .all(|v| v.starts_with("god: ") || v.starts_with("suppress: ")),
        "清单里出现无归属的明细：{violations:?}"
    );
}

#[test]
fn canary_gate_run_reports_a_broken_subgate_as_unjudgeable() {
    // 子门抛错（成员目录缺失）⇒ 整档是 Err，由 `verdict` 落成「判不了」+ 退出码 3。
    // 换成 `Ok(vec![])` 的变异在这条路径上同样会被抓到：Err 不是空清单。
    let root = fixture("gate-err");
    std::fs::create_dir_all(&root).expect("建空根");
    let err = gate::run(&root).expect_err("成员目录缺失时不许给空清单（那会被读成绿）");
    std::fs::remove_dir_all(&root).expect("清理夹具");
    assert!(
        format!("{err:#}").contains("成员目录缺失"),
        "报错原因漂了：{err:#}"
    );
}
