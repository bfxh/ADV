//! ADV 内核（adv-core）。
//!
//! 职责（蓝图 §4）：错误类型、句柄化存储、路径策略、预算控制。
//! M0 落版本接线与身份标识；M1 片3 增时间工具（抑制到期账共用）。

/// 产品名（CLI/MCP 面共用）。
pub const NAME: &str = "adv";

/// 工作区版本（与根 Cargo.toml `[workspace.package]` 同源；`xtask lockstep` 门负责对账 git tag）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 今日日期（ISO）：`ADV_TODAY` 覆盖优先（判据时序独立性——测试/复算可钉死日期），
/// 否则系统 UTC。CLI 与 xtask 门共用同一实现（判据走真路径）。
pub fn today() -> String {
    if let Ok(d) = std::env::var("ADV_TODAY")
        && !d.is_empty()
    {
        return d;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil(secs / 86_400)
}

/// 天数 → YYYY-MM-DD（Howard Hinnant civil_from_days 算法，零依赖）。
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_workspace_version() {
        assert_eq!(super::version(), "0.1.0");
    }

    #[test]
    fn civil_known_dates() {
        // 1970-01-01 = day 0；2026-10-04 = 20730 天（已知锚）
        assert_eq!(super::civil(0), "1970-01-01");
        assert_eq!(super::civil(20_730), "2026-10-04");
    }
}
