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
    let paths: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect();
    if paths.len() != 1 {
        eprintln!("用法：adv snapshot <快照目录>（只收一个目录参数）");
        std::process::exit(2);
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
                "red": f.red,
                "detail": f.detail,
            })
        );
    }
    let hits = findings.iter().filter(|f| f.red).count();
    *note = format!("、advisory 命中 {hits} 条（快照 {} 份）", idx.records);
    hits
}
