//! 对账器（M3-3b）：OSV 快照 ↔ RustSec advisory-db 的 **ID 级覆盖**与字段一致性。
//!
//! 差异方向的红/信号口径（用户 2026-10-08 拍板）：
//!
//! | 形状 | 档位 | 为什么 |
//! |---|---|---|
//! | 仅 RustSec 有 | **判红** | 快照滞后/丢条 —— 这是"漏"的形状，漏比误报贵 |
//! | 仅 OSV 有 | 只出信号 | 快照超前（上游合并延迟）是常态，判红会变噪声 |
//! | 同 id 字段不一致 | 只出信号 + 逐字段 | 两库口径本来就未必逐字一致，先看趋势 |
//!
//! 判据本体是纯函数 [`diff`]（金丝雀与变异门都打这一层）；IO 壳只负责把两路读成可比的视图。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::advisory::parse_advisory;
use crate::rustsec::{RustSecDb, load_db};

/// 仅 RustSec 有（红）。
pub const RS_ONLY: &str = "SCA-RS-ONLY-RUSTSEC";
/// 仅 OSV 有（信号）。
pub const OSV_ONLY: &str = "SCA-RS-ONLY-OSV";
/// 同 id 字段不一致（信号）。
pub const FIELD_MISMATCH: &str = "SCA-RS-FIELD-MISMATCH";

/// 一条对账差异。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Divergence {
    /// 差异码。
    pub code: &'static str,
    /// 涉及的 RUSTSEC id。
    pub id: String,
    /// 人读的一句账。
    pub detail: String,
    /// 是否参与退出码。
    pub red: bool,
}

/// 一次对账的完整读数（计数一并返回，免得调用方自己去数）。
#[derive(Debug, Default)]
pub struct ReconcileReport {
    /// 差异清单。
    pub divergences: Vec<Divergence>,
    /// OSV 侧 RUSTSEC 条目数。
    pub osv_total: usize,
    /// RustSec 侧条数。
    pub rustsec_total: usize,
    /// 两边都有的条数。
    pub both: usize,
}

impl ReconcileReport {
    /// 参与退出码的那部分差异。
    pub fn red(&self) -> impl Iterator<Item = &Divergence> {
        self.divergences.iter().filter(|d| d.red)
    }
}

/// OSV 侧的可比视图（只取三个对账字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsvView {
    /// advisory 号。
    pub id: String,
    /// 包名（取首个 affected 的 package.name）。
    pub package: String,
    /// `affected[].database_specific.informational`。
    pub informational: Option<String>,
}

/// 读快照里的 RUSTSEC 条目（GHSA 等不进对账——RustSec 路结构上没有它们）。
pub fn osv_views(ecosystem_dir: &Path) -> Result<Vec<OsvView>> {
    let mut out = Vec::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(ecosystem_dir)
        .with_context(|| format!("列 {}", ecosystem_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    names.sort();
    for path in names {
        let bytes = std::fs::read(&path).with_context(|| format!("读 {}", path.display()))?;
        let rec = parse_advisory(&bytes).map_err(|e| anyhow::anyhow!("{}：{e}", path.display()))?;
        if !rec.id.starts_with("RUSTSEC-") {
            continue;
        }
        let first = rec.affected.first();
        let informational = rec.informational();
        out.push(OsvView {
            id: rec.id,
            package: first.map(|a| a.package.name.clone()).unwrap_or_default(),
            informational,
        });
    }
    Ok(out)
}

/// 纯函数对账：两路视图 → 差异表（按码、id 排序）。
pub fn diff(rs: &RustSecDb, osv: &[OsvView]) -> Vec<Divergence> {
    let by_id: BTreeMap<&str, &OsvView> = osv.iter().map(|o| (o.id.as_str(), o)).collect();
    let mut out = Vec::new();
    for (id, adv) in &rs.advisories {
        match by_id.get(id.as_str()) {
            None => out.push(Divergence {
                code: RS_ONLY,
                id: id.clone(),
                detail: format!(
                    "RustSec 有、OSV 快照没有（包 {}，{}）：快照滞后或丢条",
                    adv.package, adv.date
                ),
                red: true,
            }),
            Some(view) => {
                if view.package != adv.package {
                    out.push(Divergence {
                        code: FIELD_MISMATCH,
                        id: id.clone(),
                        detail: format!(
                            "包名不一致：RustSec {} / OSV {}",
                            adv.package, view.package
                        ),
                        red: false,
                    });
                }
                if view.informational != adv.informational {
                    out.push(Divergence {
                        code: FIELD_MISMATCH,
                        id: id.clone(),
                        detail: format!(
                            "informational 不一致：RustSec {:?} / OSV {:?}",
                            adv.informational, view.informational
                        ),
                        red: false,
                    });
                }
            }
        }
    }
    for view in osv {
        if !rs.advisories.contains_key(&view.id) {
            out.push(Divergence {
                code: OSV_ONLY,
                id: view.id.clone(),
                detail: format!(
                    "OSV 有、RustSec DB 没有（包 {}）：快照超前或上游未合并",
                    view.package
                ),
                red: false,
            });
        }
    }
    out.sort();
    out
}

/// IO 壳：从快照目录读两路（`<dir>/crates.io` 与 `<dir>/rustsec`）并对账。
pub fn reconcile(snapshot_dir: &Path) -> Result<ReconcileReport> {
    let osv = osv_views(&snapshot_dir.join("crates.io"))?;
    let rs = load_db(&snapshot_dir.join("rustsec"))?;
    let both = osv
        .iter()
        .filter(|o| rs.advisories.contains_key(&o.id))
        .count();
    let divergences = diff(&rs, &osv);
    Ok(ReconcileReport {
        divergences,
        osv_total: osv.len(),
        rustsec_total: rs.advisories.len(),
        both,
    })
}
