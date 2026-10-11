//! `adv snapshot` 的实现体：OSV 快照的**同步**（唯一联网入口，显式命令，绝不隐式触发）。
//!
//! 与 `sca` 的分工：`sca` 只做离线判定（锁 + 快照读盘），`snapshot` 才碰网络。
//! 退出码沿用三态：0 同步完成 / 2 用法错 / 3 同步失败或环境缺失（curl 不在 PATH 等）。

use std::collections::BTreeSet;
use std::path::PathBuf;

use adv_sca::matcher::{load_snapshot, scan_with_advisories};
use adv_sca::snapshot::sync;

/// `adv snapshot <目录>`：列目录 + 逐对象条件 GET + ETag/MD5 记账。
pub(crate) fn run(args: &[String]) {
    let (paths, with_rustsec) = match snapshot_args(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if with_rustsec {
        sync_rustsec(&paths[0]);
    }
    match sync(&paths[0]) {
        Ok(r) => {
            for gone in &r.removed_upstream {
                // 上游删掉的只报不删：删不删由人定，但账要留。
                eprintln!("adv snapshot：上游已不再列出 {gone}（本地保留，未删）");
            }
            eprintln!(
                "adv snapshot：列出 {} 个对象，新抓 {}、未变 {}、上游撤下 {}（记账写在 manifest.json）",
                r.listed,
                r.fetched,
                r.unchanged,
                r.removed_upstream.len()
            );
        }
        Err(e) => {
            // 判不了 ≠ 同步成功（与 sca/secrets 同一条纪律）。
            eprintln!("adv snapshot：同步失败（不折算为成功）：{e:#}");
            std::process::exit(3);
        }
    }
}

/// 参数分诊（纯函数）：`Ok((目录参数, 是否带 --with-rustsec))`，用法错出 `Err(要打印的那句话)`。
///
/// 抽出来的理由：原来这六条判据（哪个是旗标、哪个是目录、`--with-rustsec` 不算未知旗标、
/// 只收一个目录）全焊在 `run` 里，而 `run` 往下就走网络同步 ⇒ 离线测试只能碰到"缺目录/多目录/
/// 未知旗标"三条用法错，`!unknown.is_empty()`、`!a.starts_with("--")` 与 `any(== "--with-rustsec")`
/// 这些取反/换向的变异没有任何面能看见（DD-0018 实测：`snapshot.rs::run` 键整片存活）。
pub(crate) fn snapshot_args(args: &[String]) -> Result<(Vec<PathBuf>, bool), String> {
    let with_rustsec = args.iter().any(|a| a == "--with-rustsec");
    let unknown: Vec<&String> = args
        .iter()
        .filter(|a| a.starts_with("--") && a.as_str() != "--with-rustsec")
        .collect();
    if !unknown.is_empty() {
        return Err(format!("snapshot 不认得参数 {}", unknown[0]));
    }
    let paths: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect();
    if paths.len() != 1 {
        return Err(
            "用法：adv snapshot <快照目录> [--with-rustsec]（只收一个目录参数）".to_string(),
        );
    }
    Ok((paths, with_rustsec))
}

/// 匹配面：读快照 → 逐锁匹配 → 打 JSONL。返回它贡献的判红数（判不了直接 exit 3）。
pub(crate) fn advisory_findings(
    paths: &[PathBuf],
    dir: &std::path::Path,
    checked_by_lock: &[String],
    note: &mut String,
) -> usize {
    let idx = match load_snapshot(dir) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("adv sca：快照读不出（不折算为无发现）：{e:#}");
            std::process::exit(3);
        }
    };
    let (findings, judged) = match scan_with_advisories(paths, &idx) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("adv sca：advisory 匹配失败（不折算为无发现）：{e:#}");
            std::process::exit(3);
        }
    };
    // 两面各自遍历一遍，判过的锁集合必须一致；不一致说明两边的跳过规则漂了，
    // 这种时候不许挑一边信（否则"少判了几把锁"会静默成"判过了"）。
    let a: BTreeSet<&str> = checked_by_lock.iter().map(String::as_str).collect();
    let b: BTreeSet<&str> = judged.iter().map(String::as_str).collect();
    if a != b {
        eprintln!("adv sca：两面判过的锁集合不一致（锁面 {a:?} / 匹配面 {b:?}）——判不了");
        std::process::exit(3);
    }
    for f in &findings {
        println!(
            "{}",
            serde_json::json!({
                "engine": "osv",
                "code": f.code,
                "id": f.id,
                "package": f.package,
                "version": f.version,
                "severity": f.severity,
                "informational": f.informational,
                "red": f.red,
                "detail": f.detail,
            })
        );
    }
    let hits = findings.iter().filter(|f| f.red).count();
    *note = format!("、advisory 命中 {hits} 条（快照 {} 份）", idx.records);
    hits
}

/// RustSec 路的同步（`--with-rustsec`）：克隆/更新 + 钉 SHA + 写进同一份记账。
/// 失败 ⇒ 退出码 3（判不了），不把"两路只跑成一路"记成同步成功。
fn sync_rustsec(root: &std::path::Path) {
    let dir = root.join("rustsec");
    match adv_sca::rustsec::sync(&dir) {
        Ok(r) => {
            let meta = adv_sca::snapshot::RustSecMeta {
                sha: r.sha.clone(),
                advisories: r.advisories,
                fresh_clone: r.fresh_clone,
            };
            if let Err(e) = adv_sca::snapshot::record_rustsec(root, meta) {
                eprintln!("adv snapshot：RustSec 记账写不进去（不折算为成功）：{e:#}");
                std::process::exit(3);
            }
            eprintln!(
                "adv snapshot：RustSec {} 条，HEAD {}（{}）",
                r.advisories,
                &r.sha[..12.min(r.sha.len())],
                if r.fresh_clone {
                    "首次克隆"
                } else {
                    "原地更新"
                }
            );
        }
        Err(e) => {
            eprintln!("adv snapshot：RustSec 同步失败（不折算为成功）：{e:#}");
            std::process::exit(3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::snapshot_args;
    use std::path::PathBuf;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_single_directory_is_accepted_without_the_rustsec_flag() {
        let (paths, with_rustsec) = snapshot_args(&args(&["快照"])).expect("一个目录就该收");
        assert_eq!(
            paths,
            vec![PathBuf::from("快照")],
            "目录参数要原样留下：{paths:?}"
        );
        assert!(!with_rustsec, "没给 --with-rustsec 不许当给了");
    }

    #[test]
    fn the_rustsec_flag_is_a_flag_not_a_directory() {
        let (paths, with_rustsec) =
            snapshot_args(&args(&["快照", "--with-rustsec"])).expect("旗标可带可不带");
        assert!(
            with_rustsec,
            "认不出 --with-rustsec 就等于永远走不到 RustSec 路"
        );
        assert_eq!(
            paths,
            vec![PathBuf::from("快照")],
            "旗标不许混进目录参数：{paths:?}"
        );
    }

    #[test]
    fn a_bare_rustsec_flag_is_still_a_missing_directory() {
        let err = snapshot_args(&args(&["--with-rustsec"])).expect_err("只给旗标等于没有目录");
        assert!(err.contains("只收一个目录参数"), "{err}");
    }

    #[test]
    fn unknown_flags_are_refused_by_name_and_take_precedence() {
        let err = snapshot_args(&args(&["--wat", "快照"])).expect_err("未知旗标必须判红");
        assert_eq!(err, "snapshot 不认得参数 --wat");
        // 顺序也要钉住：未知旗标优先于目录数报出（否则"给了 3 个参数"会把拼错的旗标盖掉）。
        let err = snapshot_args(&args(&["--wat", "a", "b"])).expect_err("同上");
        assert!(err.contains("不认得参数 --wat"), "{err}");
    }

    #[test]
    fn zero_or_two_directories_are_usage_errors() {
        for bad in [vec![], vec!["a", "b"], vec!["a", "b", "c"]] {
            let err = snapshot_args(&args(&bad)).expect_err("目录数不是 1 就该用法错");
            assert!(err.contains("用法：adv snapshot"), "{err}");
        }
    }
}
