//! `adv sca` 的实现体：**离线判定**（锁完整性 + 可选的 OSV 快照匹配）→ JSONL + 三态退出码。
//!
//! 为什么住在单独模块而不是 `main.rs`：`main.rs` 被 god 尺钉着，每接一个新引擎就往里堆一个
//! 处理器，堆到第 N 个时没人愿意再拆——棘轮的作用就是让"再塞一个函数"必须付一次拆分的代价。
//! 网络面不在这里：`adv snapshot` 才是唯一的联网入口（显式命令），本模块只读盘。

use std::path::PathBuf;

use adv_sca::lockcheck::scan_paths;

struct Args {
    paths: Vec<PathBuf>,
    snapshot: Option<PathBuf>,
    reconcile: bool,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        reconcile: false,
        paths: Vec::new(),
        snapshot: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--snapshot" => {
                let v = it.next().ok_or("--snapshot 后面要给一个快照目录")?;
                out.snapshot = Some(PathBuf::from(v));
            }
            "--reconcile" => out.reconcile = true,
            other if other.starts_with("--") => return Err(format!("sca 不认得参数 {other}")),
            _ => out.paths.push(PathBuf::from(a)),
        }
    }
    Ok(out)
}

/// `adv sca <路径…> [--snapshot <快照目录>]`：锁定完整性判定；给了 `--snapshot` 再叠一层
/// "锁清单 × OSV advisory"的匹配。
///
/// 退出码：0 没有判红 / 1 有判红 / 2 用法错 / 3 判不了（没有可判定的锁、快照读不出、两面记账不一致）。
/// "只出信号"的那些（同名多版本）进 JSONL 但不参与退出码。
pub(crate) fn run(args: &[String]) {
    let parsed = match parse(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if parsed.paths.is_empty() {
        eprintln!("sca 需要至少一个路径参数");
        std::process::exit(2);
    }
    let report = match scan_paths(&parsed.paths) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("adv sca：探测失败（不折算为无发现）：{e:#}");
            std::process::exit(3);
        }
    };
    for issue in &report.issues {
        println!(
            "{}",
            serde_json::json!({
                "engine": "sca",
                "code": issue.code,
                "lock": issue.lock,
                "package": issue.package,
                "red": issue.red,
                "detail": issue.detail,
            })
        );
    }
    let mut red = report.red().count();
    let mut advisories_note = String::new();
    if let Some(dir) = &parsed.snapshot {
        red += crate::snapshot::advisory_findings(
            &parsed.paths,
            dir,
            &report.checked_locks,
            &mut advisories_note,
        );
        if parsed.reconcile {
            red += reconcile_report(dir);
        }
    } else if parsed.reconcile {
        eprintln!("--reconcile 要配 --snapshot <目录> 一起用");
        std::process::exit(2);
    }
    for skip in &report.skipped {
        eprintln!("adv sca：跳过 {skip}");
    }
    eprintln!(
        "adv sca：判过 {} 把锁，{} 条判红、{} 条只出信号{advisories_note}（engine=sca）",
        report.checked_locks.len(),
        red,
        report.issues.len() - report.red().count()
    );
    if red > 0 {
        std::process::exit(1);
    }
}

/// 对账面（`--reconcile`）：OSV 快照 ↔ RustSec DB 的 ID 级覆盖与字段一致性。
/// 返回它贡献的判红数（仅 RustSec 有 ⇒ 红；判不了直接 exit 3）。
pub(crate) fn reconcile_report(snapshot_dir: &std::path::Path) -> usize {
    match adv_sca::reconcile::reconcile(snapshot_dir) {
        Ok(rep) => {
            for d in &rep.divergences {
                println!(
                    "{}",
                    serde_json::json!({
                        "engine": "reconcile",
                        "code": d.code,
                        "id": d.id,
                        "red": d.red,
                        "detail": d.detail,
                    })
                );
            }
            let red = rep.red().count();
            eprintln!(
                "adv sca：对账 OSV {} / RustSec {} / 两边都有 {}，差异 {} 条（判红 {}）",
                rep.osv_total,
                rep.rustsec_total,
                rep.both,
                rep.divergences.len(),
                red
            );
            red
        }
        Err(e) => {
            eprintln!("adv sca：对账失败（不折算为无差异）：{e:#}");
            std::process::exit(3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PathBuf, parse};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn positionals_are_paths_in_order_and_defaults_are_off() {
        let p = parse(&args(&["Cargo.lock", "../其他/Cargo.toml"])).expect("路径该收");
        assert_eq!(
            p.paths,
            vec![
                PathBuf::from("Cargo.lock"),
                PathBuf::from("../其他/Cargo.toml")
            ],
            "位置参数按顺序进 paths：{:?}",
            p.paths
        );
        assert!(p.snapshot.is_none(), "没给 --snapshot 不该有快照目录");
        assert!(!p.reconcile, "没给 --reconcile 不该开对账");
    }

    #[test]
    fn snapshot_takes_the_next_argument_as_its_value() {
        let p = parse(&args(&["锁", "--snapshot", "快照目录"])).expect("--snapshot 是认得的旗标");
        assert_eq!(
            p.snapshot,
            Some(PathBuf::from("快照目录")),
            "{:?}",
            p.snapshot
        );
        assert_eq!(
            p.paths,
            vec![PathBuf::from("锁")],
            "快照的值不许混进路径：{:?}",
            p.paths
        );
    }

    #[test]
    fn reconcile_is_a_flag_and_can_follow_the_snapshot_value() {
        let p = parse(&args(&["--snapshot", "s", "--reconcile", "锁"])).expect("两个旗标都认得");
        assert!(
            p.reconcile,
            "--reconcile 没被认出来（它会掉进未知旗标那条 Err）"
        );
        assert_eq!(p.snapshot, Some(PathBuf::from("s")));
        assert_eq!(p.paths, vec![PathBuf::from("锁")]);
    }

    #[test]
    fn a_snapshot_flag_without_a_value_is_refused_by_name() {
        let err = parse(&args(&["锁", "--snapshot"]))
            .err()
            .unwrap_or_else(|| panic!("后面没值必须判红"));
        assert!(
            err.contains("--snapshot") && err.contains("快照目录"),
            "{err}"
        );
    }

    #[test]
    fn unknown_long_flags_are_refused_instead_of_becoming_paths() {
        // 这条是「守卫 `other.starts_with("--")` 恒 false」的判据：守卫一倒，--wat 就被当路径收下。
        let err = parse(&args(&["--wat"]))
            .err()
            .unwrap_or_else(|| panic!("未知旗标不许被当成锁文件"));
        assert_eq!(err, "sca 不认得参数 --wat");
    }

    #[test]
    fn a_dash_flag_is_a_path_but_a_double_dash_is_a_flag() {
        // 判据锚在"只认 `--` 前缀"这条已定口径上：`-j` 不是 sca 的旗标，它是路径。
        let p = parse(&args(&["-j"])).expect("-j 该被当路径");
        assert_eq!(p.paths, vec![PathBuf::from("-j")], "{:?}", p.paths);
    }

    #[test]
    fn no_arguments_parses_fine_and_leaves_the_path_list_empty() {
        // 空路径的拒绝在 run（退出码 2），不在 parse——这里钉住分工，别让 parse 提前判红。
        let p = parse(&args(&[])).expect("空参数不是用法错");
        assert!(p.paths.is_empty() && p.snapshot.is_none() && !p.reconcile);
    }
}
