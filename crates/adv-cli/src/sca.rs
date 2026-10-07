//! `adv sca` 的实现体：把 [`adv_sca`] 的判定读数打成 JSONL 并翻译成退出码。
//!
//! 为什么住在单独模块而不是 `main.rs`：`main.rs` 已被 god 尺钉在 625 行的基线上，
//! 而每接一个新引擎就往里堆一个处理器，堆到第 N 个的时候没人愿意再拆它——
//! 棘轮的作用就是让"再塞一个函数"这件事必须付一次拆分的代价。仓里已有 `mod mir;` 先例。

use std::path::PathBuf;

/// `adv sca <路径…>`：锁定完整性判定（M3-2：清单段 ↔ 判定段分离，三条纯本地判据）。
///
/// 退出码：0 没有判红 / 1 有判红 / 2 用法错 / 3 判不了（一把能判定的锁都没读到）。
/// "只出信号"的那些（同名多版本）进 JSONL 但不参与退出码——真锁实测 8/30 例，
/// 参与就成噪声，而噪声会让整条判据失去牙齿。
pub(crate) fn run(args: &[String]) {
    let paths: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() {
        eprintln!("sca 需要至少一个路径参数");
        std::process::exit(2);
    }
    match adv_sca::scan_paths(&paths) {
        Ok(report) => {
            for issue in &report.issues {
                // 走 serde_json 而不是 format! 手拼：detail 里带锁路径与包名，含引号/反斜杠时
                // 手拼会输出非法 JSONL 行（secrets 那档的值域是上游规则码，没有这个形状）。
                let line = serde_json::json!({
                    "engine": "sca",
                    "code": issue.code,
                    "lock": issue.lock,
                    "package": issue.package,
                    "red": issue.red,
                    "detail": issue.detail,
                });
                println!("{line}");
            }
            for skip in &report.skipped {
                eprintln!("adv sca：跳过 {skip}");
            }
            let red = report.red().count();
            eprintln!(
                "adv sca：判过 {} 把锁，{} 条判红、{} 条只出信号（engine=sca）",
                report.checked_locks.len(),
                red,
                report.issues.len() - red
            );
            if red > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            // 判不了 ≠ 没有发现（与 secrets / scan 档同一条纪律）。
            eprintln!("adv sca：探测失败（不折算为无发现）：{e:#}");
            std::process::exit(3);
        }
    }
}
