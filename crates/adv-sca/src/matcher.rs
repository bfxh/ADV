//! 匹配面：锁清单 × OSV 快照 → 发现。
//!
//! 只匹配 `source` 是 crates.io registry 的包：本地 path 包没有"发布版本"可言
//! （本仓 19 个本地包里 6 个是 vendored noseyparker 的 crate，拿它们的 0.0.0 去查库毫无意义）。
//! 版本解析失败、快照目录读不出任何 advisory ⇒ 一律 `Err`（判不了）——**不返回空发现**：
//! "查不到"与"没查成"混在一起，就是漏报的入口。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use semver::Version;

use crate::advisory::{OsvRecord, parse_advisory};
use crate::inventory::{Inventory, load_lock};

/// 命中码（与 `SCA-LC-*` 同一命名空间纪律）。
pub const ADV_HIT: &str = "SCA-ADV-HIT";

/// 快照索引：包名 → 覆盖该包的 advisory 列表。
#[derive(Debug)]
pub struct AdvisoryIndex {
    /// 快照目录（报账时要写清命中来自哪份快照）。
    pub dir: PathBuf,
    /// 读进来的 advisory 份数。
    pub records: usize,
    by_name: BTreeMap<String, Vec<OsvRecord>>,
}

impl AdvisoryIndex {
    /// 覆盖某包名的 advisory（没有就是空表）。
    pub fn advisories_for(&self, name: &str) -> &[OsvRecord] {
        self.by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// 一条匹配结果。`red=true` 参与退出码。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdvFinding {
    /// 命中码。
    pub code: &'static str,
    /// advisory 号。
    pub id: String,
    /// 命中的包名（来自锁）。
    pub package: String,
    /// 命中的版本（来自锁）。
    pub version: String,
    /// 一行严重度（见 `OsvRecord::severity_label`）。
    pub severity: String,
    /// 摘要首行。
    pub detail: String,
    /// RustSec 的 informational 标记（`unsound`/`unmaintained`/`notice`）；有它 ⇒ 不是漏洞通告。
    pub informational: Option<String>,
    /// 是否参与退出码。**informational 类不判红**（unmaintained ≠ 漏洞；一律判红会把 496 条
    /// 公告变成噪声，噪声会让判据失去牙齿——与本仓「多版本只出信号」同一条纪律）。
    pub red: bool,
}

/// 读一个快照目录（`<dir>/crates.io/*.json`）。**一份都读不出 ⇒ Err**；
/// 任何一份解不开 ⇒ Err（快照损坏不许静默缩水覆盖）。
pub fn load_snapshot(dir: &Path) -> Result<AdvisoryIndex> {
    let ecosystem = dir.join("crates.io");
    anyhow::ensure!(
        ecosystem.is_dir(),
        "快照目录里没有 crates.io/ 子目录：{}",
        dir.display()
    );
    let mut by_name: BTreeMap<String, Vec<OsvRecord>> = BTreeMap::new();
    let mut records = 0usize;
    let mut names: Vec<PathBuf> = std::fs::read_dir(&ecosystem)
        .with_context(|| format!("列 {}", ecosystem.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    names.sort();
    for path in names {
        let bytes = std::fs::read(&path).with_context(|| format!("读 {}", path.display()))?;
        let rec = parse_advisory(&bytes).map_err(|e| anyhow!("{}：{e}", path.display()))?;
        records += 1;
        for aff in &rec.affected {
            if aff.package.ecosystem == "crates.io" {
                by_name
                    .entry(aff.package.name.clone())
                    .or_default()
                    .push(rec.clone());
            }
        }
    }
    anyhow::ensure!(
        records > 0,
        "快照里一份 advisory 都没有（{}）——这不等于没有漏洞",
        ecosystem.display()
    );
    Ok(AdvisoryIndex {
        dir: dir.to_path_buf(),
        records,
        by_name,
    })
}

fn registry_package(pkg: &crate::inventory::Package) -> bool {
    pkg.source
        .as_deref()
        .map(|s| s.starts_with("registry+https://github.com/rust-lang/crates.io-index"))
        .unwrap_or(false)
}

/// 一份清单 × 索引 → 发现（按 id、包名、版本排序）。
pub fn match_inventory(inv: &Inventory, idx: &AdvisoryIndex) -> Result<Vec<AdvFinding>> {
    let mut out = Vec::new();
    for pkg in &inv.packages {
        if !registry_package(pkg) {
            continue;
        }
        let version = match Version::parse(&pkg.version) {
            Ok(v) => v,
            Err(_) => {
                return Err(anyhow!(
                    "{} 的 {}={} 不是 semver——判不了（别把解析失败读成不受影响）",
                    inv.lock.display(),
                    pkg.name,
                    pkg.version
                ));
            }
        };
        for adv in idx.advisories_for(&pkg.name) {
            let covered = adv
                .covers("crates.io", &pkg.name, &version)
                .map_err(|e| anyhow!("{}（{}）区间判定失败：{e}", adv.id, pkg.name))?;
            if !covered {
                continue;
            }
            let informational = adv.informational();
            out.push(AdvFinding {
                code: ADV_HIT,
                id: adv.id.clone(),
                package: pkg.name.clone(),
                version: pkg.version.clone(),
                severity: adv.severity_label(),
                detail: adv.summary.lines().next().unwrap_or("").trim().to_string(),
                red: informational.is_none(),
                informational,
            });
        }
    }
    out.sort();
    Ok(out)
}

/// 扫一批路径上的锁（跳过读不出的，留痕），逐把匹配。
/// 返回 (发现, 真判过的锁清单)——第二项用来与 lockcheck 的记账对账。
pub fn scan_with_advisories(
    paths: &[PathBuf],
    idx: &AdvisoryIndex,
) -> Result<(Vec<AdvFinding>, Vec<String>)> {
    let mut findings = Vec::new();
    let mut judged = Vec::new();
    for path in paths {
        for lock in crate::lockcheck::find_locks(path)? {
            let Ok(inv) = load_lock(&lock) else { continue };
            judged.push(lock.to_string_lossy().replace('\\', "/"));
            findings.extend(match_inventory(&inv, idx)?);
        }
    }
    findings.sort();
    ensure_any_lock_judged(&judged)?;
    Ok((findings, judged))
}

fn ensure_any_lock_judged(judged: &[String]) -> Result<()> {
    if judged.is_empty() {
        return Err(anyhow!("一把能匹配的锁都没有——这不等于没有漏洞"));
    }
    Ok(())
}
