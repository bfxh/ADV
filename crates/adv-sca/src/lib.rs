//! ADV 供应链扫描（adv-sca）。
//!
//! 选型已定（RESEARCH 02/D5/P8）：Inventory→Matcher→Report 三段（Syft→Grype 范式）；
//! OSV/RustSec 本地快照（ETag 条件 GET + 验签后原子落地，X12）；多语言锁文件读取器
//! （Cargo.lock/UVLock/package-lock/pnpm-lock/pylock）；锁定完整性六判据（D5-25）；
//! XZ 9 条检测规则（5 条纯静态先行）+ Ladisa 攻击树对账表。
//!
//! M3-2 落的是前两段里的静态部分（[`inventory`] 读账、[`lockcheck`] 判定，三条纯本地判据）；
//! matcher 段与 OSV/RustSec 快照留 M3-3，非 Cargo 的锁读取器**等真语料出现**再进产品面
//! （实测本仓只有 3 把 Cargo.lock，npm/uv/pnpm 一把都没有）。

pub mod advisory;
pub mod inventory;
pub mod lockcheck;
pub mod matcher;
pub mod reconcile;
pub mod rustsec;
pub mod snapshot;

pub use lockcheck::{LockIssue, Report, check_lock, find_locks, scan_paths};

/// 域版本（与工作区同源）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_core() {
        assert_eq!(super::version(), adv_core::version());
    }
}
