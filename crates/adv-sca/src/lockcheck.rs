//! 判定段（Lockcheck）：锁定完整性的静态判据（RESEARCH D5-025 的 1/2/6 里能纯本地判的那几条）。
//!
//! 三条判据都带**先量**的账（2026-10-08，本仓三份真锁：根 298 包 / `rust/` 1 包 /
//! `third_party/noseyparker/` 552 包，21 个成员）：
//!
//! - **LC-1 缺 checksum**：带 source 却没 checksum 的包 —— 真锁里 0 例（279/279 齐），
//!   所以这条在干净仓上恒绿，可当硬判据。
//! - **LC-2 锁↔清单漂移**：**只在成员上比**。这条的范围是量出来的：不加限制时本仓报 5 处
//!   "缺"（`noseyparker` 的 reqwest/tokio/chrono、`bstring-serde` 的 proptest 等），
//!   逐条看全是**非成员的 path 依赖的 dev-dependencies**——cargo 本来就不把它们解析进锁，
//!   属合法形态。限制到成员后 21 个成员 0 误报。判据写成"任何本地包都要齐"就是一台假红机。
//! - **LC-3 双轨/多版本**：同名既有本地记录又有 registry 记录 ⇒ 判红（dependency confusion
//!   的形状，真锁 0 例）；同名多版本 ⇒ **只出信号不判红**（本仓 8 例、noseyparker 锁 30 例，
//!   判红就是噪声；噪声会让整条门失去牙齿，与 god 尺跳过试验面是同一条纪律）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::inventory::{Inventory, Workspace, load_lock, load_workspace};

/// LC-1：带 `source` 却缺 `checksum` 的包（真锁实测 0 例 ⇒ 可当硬判据）。
pub const LC1_CHECKSUM: &str = "SCA-LC-CHECKSUM-MISSING";
/// LC-2：工作区成员的声明账与锁账不一致（含"成员根本不在锁里"）。
pub const LC2_DRIFT: &str = "SCA-LC-MEMBER-DRIFT";
/// LC-3a：同名同时存在本地包与 registry 包。
pub const LC3_DUAL: &str = "SCA-LC-DUAL-TRACK";
/// LC-3b：同名多版本——**只出信号**，不参与退出码。
pub const LC3_MULTI: &str = "SCA-LC-MULTI-VERSION";

/// 一条判定结果。`red=false` 是**只出信号**（多版本这类在干净仓上恒有的形状），
/// 它进报告但不参与退出码——参与的那部分才叫判据。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockIssue {
    /// 判据码（`SCA-LC-*`，与规则码同一命名空间纪律）。
    pub code: &'static str,
    /// 出问题的锁（`/` 分隔，便于进账本与金样比对）。
    pub lock: String,
    /// 涉及的包名；LC-2 的成员级发现用成员名。
    pub package: String,
    /// 人读的一句账（缺了什么、多了什么、几例）。
    pub detail: String,
    /// 是否参与退出码。
    pub red: bool,
}

impl LockIssue {
    fn new(code: &'static str, lock: &str, package: String, detail: String, red: bool) -> Self {
        Self {
            code,
            lock: lock.to_string(),
            package,
            detail,
            red,
        }
    }
}

/// 一次扫描的完整读数：判据结果之外还带"扫过哪几把锁、跳过了谁"。
/// `checked_locks` 为空 ⇒ 调用方不能把它读成"干净"。
#[derive(Debug, Default)]
pub struct Report {
    /// 全部判定结果（含只出信号的那些），已按 (code, lock, package) 排序。
    pub issues: Vec<LockIssue>,
    /// 真被判过锁的路径——空 ⇒ 这次读数不能算"干净"。
    pub checked_locks: Vec<String>,
    /// 读到但没判成的锁与原因（不许静默）。
    pub skipped: Vec<String>,
}

impl Report {
    /// 参与退出码的那部分发现。
    pub fn red(&self) -> impl Iterator<Item = &LockIssue> {
        self.issues.iter().filter(|i| i.red)
    }
}

/// 纯函数判定（金丝雀与变异门都打这一层：输入合成、观察面可判）。
pub fn check_lock(inv: &Inventory, ws: &Workspace) -> Vec<LockIssue> {
    let lock = inv.lock.to_string_lossy().replace('\\', "/");
    let mut out = Vec::new();

    for pkg in &inv.packages {
        if pkg.source.is_some() && pkg.checksum.is_none() {
            out.push(LockIssue::new(
                LC1_CHECKSUM,
                &lock,
                pkg.name.clone(),
                format!(
                    "带 source（{}）却没有 checksum",
                    pkg.source.as_deref().unwrap_or("")
                ),
                true,
            ));
        }
    }

    for member in &ws.members {
        match inv.find_local(&member.name) {
            None => out.push(LockIssue::new(
                LC2_DRIFT,
                &lock,
                member.name.clone(),
                format!("成员 {} 不在锁的本地包记录里", member.manifest.display()),
                true,
            )),
            Some(pkg) => {
                let inlock: std::collections::BTreeSet<String> =
                    pkg.dependencies.iter().cloned().collect();
                let missing: Vec<&String> = member
                    .declared
                    .iter()
                    .filter(|d| !inlock.contains(*d))
                    .collect();
                let extra: Vec<&String> = inlock
                    .iter()
                    .filter(|d| !member.declared.contains(*d))
                    .collect();
                if !missing.is_empty() {
                    out.push(LockIssue::new(
                        LC2_DRIFT,
                        &lock,
                        member.name.clone(),
                        format!("清单声明、锁里没有：{}", join(&missing)),
                        true,
                    ));
                }
                if !extra.is_empty() {
                    out.push(LockIssue::new(
                        LC2_DRIFT,
                        &lock,
                        member.name.clone(),
                        format!("锁里有、清单没声明：{}", join(&extra)),
                        true,
                    ));
                }
            }
        }
    }

    let mut seen: BTreeMap<String, (usize, bool, bool)> = BTreeMap::new();
    for pkg in &inv.packages {
        let e = seen.entry(pkg.name.clone()).or_insert((0, false, false));
        e.0 += 1;
        if pkg.is_local() {
            e.2 = true;
        } else {
            e.1 = true;
        }
    }
    for (name, (count, registry, local)) in seen {
        if registry && local {
            out.push(LockIssue::new(
                LC3_DUAL,
                &lock,
                name.clone(),
                "同名同时有本地包与 registry 包（dependency confusion 的形状）".to_string(),
                true,
            ));
        }
        if count > 1 {
            out.push(LockIssue::new(
                LC3_MULTI,
                &lock,
                name,
                format!("{count} 个版本并存（只出信号：干净仓上本仓 8 例、noseyparker 锁 30 例）"),
                false,
            ));
        }
    }
    out
}

fn join(items: &[&String]) -> String {
    let v: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
    v.join(", ")
}

/// 枚举一个路径下的 `Cargo.lock`（目录输入跳过 `.git`/`target`/`node_modules`/`__pycache__`）。
pub fn find_locks(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    anyhow::ensure!(path.is_dir(), "路径不存在：{}", path.display());
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && skip_dir(e.path())))
    {
        let entry = entry.with_context(|| format!("遍历 {}", path.display()))?;
        if entry.file_type().is_file() && entry.file_name() == "Cargo.lock" {
            out.push(entry.path().to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

fn skip_dir(dir: &Path) -> bool {
    matches!(
        dir.file_name().and_then(|s| s.to_str()),
        Some(".git" | "target" | "node_modules" | "__pycache__")
    )
}

/// 扫一批路径。锁旁边读不出工作区 ⇒ **跳过并留痕**，不折算成"这把锁没问题"。
pub fn scan_paths(paths: &[PathBuf]) -> Result<Report> {
    let mut report = Report::default();
    for path in paths {
        for lock in find_locks(path)? {
            let dir = lock
                .parent()
                .with_context(|| format!("{} 没有父目录", lock.display()))?
                .to_path_buf();
            let inv = match load_lock(&lock) {
                Ok(inv) => inv,
                Err(e) => {
                    report.skipped.push(format!("{}（{}）", lock.display(), e));
                    continue;
                }
            };
            let ws = match load_workspace(&dir) {
                Ok(ws) => ws,
                Err(e) => {
                    report
                        .skipped
                        .push(format!("{}（读不出工作区：{e}）", lock.display()));
                    continue;
                }
            };
            report
                .checked_locks
                .push(lock.to_string_lossy().replace('\\', "/"));
            report.issues.extend(check_lock(&inv, &ws));
        }
    }
    if report.checked_locks.is_empty() {
        return Err(anyhow!(
            "一把能判定的锁都没读到（跳过 {} 个）——这不等于干净",
            report.skipped.len()
        ));
    }
    report.issues.sort();
    Ok(report)
}
