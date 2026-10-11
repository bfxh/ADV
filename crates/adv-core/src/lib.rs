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
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    today_from(std::env::var("ADV_TODAY").ok(), secs)
}

/// [`today`] 的可测形态：env 覆盖优先，空串算未设（落系统日期），否则按 UTC 天算。
/// 拆分理由同 `xtask::mutants::pinned_tmp`——env 读取留在调用点，判据本身喂参数就能钉死，
/// 不靠改进程环境（本工作区 `unsafe_code = "deny"`，`set_var` 用不了）。
/// 可见性停在 `pub(crate)`：全仓唯一的使用者是本文件的 `today` 与其测试，测试缝不该升格成公共契约。
pub(crate) fn today_from(raw: Option<String>, secs: i64) -> String {
    if let Some(d) = raw.filter(|s| !s.is_empty()) {
        return d;
    }
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

    /// 期望值不手打：全部取自 CPython `datetime.date(1970,1,1) + timedelta(days=n)`
    /// 的反算结果（独立实现，不是本算法自己的输出）。DD-0018 分诊实测：原来只锚
    /// day 0 与 day 20730 两点，闰年/世纪/前 1970 三条分支没有观察面，
    /// `civil` 的除法与判位变异全部存活。
    #[test]
    fn civil_pins_the_leap_and_epoch_boundaries() {
        // 纪元前（负天数）：`div_euclid`/`rem_euclid` 的欧氏取余只有在 z<0 时区别于 `/` 与 `%`
        assert_eq!(super::civil(-1), "1969-12-31");
        assert_eq!(super::civil(-25_509), "1900-02-28");
        assert_eq!(super::civil(-25_508), "1900-03-01"); // 1900 不是闰年 ⇒ 没有 02-29
        // 400 年闰（2000 有 02-29）与世纪不闰（2100 没有）——这两对钉住 `doe/1460`、
        // `doe/36_524`、`doe/146_096` 三个修正项，少任一项就在这两天错位
        assert_eq!(super::civil(11_015), "2000-02-28");
        assert_eq!(super::civil(11_016), "2000-02-29");
        assert_eq!(super::civil(11_017), "2000-03-01");
        assert_eq!(super::civil(47_540), "2100-02-28");
        assert_eq!(super::civil(47_541), "2100-03-01"); // 2100 无 02-29 ⇒ 相邻两天
        // 跨年边界
        assert_eq!(super::civil(20_818), "2026-12-31");
        assert_eq!(super::civil(20_819), "2027-01-01");
    }

    /// `today` 的两条分支 + 秒→天的除数。env 覆盖走参数（不改进程环境，见 `today_from` 注）。
    #[test]
    fn today_prefers_override_and_falls_back_to_utc_day() {
        // ① 非空覆盖 ⇒ 原样返回（"整体换成常量"的变异在这里露）
        assert_eq!(
            super::today_from(Some("2026-01-01".to_string()), 0),
            "2026-01-01"
        );
        // ② 空串 = 未设 ⇒ 落系统日期算法（`delete !` 会把空串当有效覆盖返回 ""）
        assert_eq!(
            super::today_from(Some(String::new()), 20_730 * 86_400),
            "2026-10-04"
        );
        // ③ 一天内的任意秒数都给同一天，跨到下一秒才换天（钉 `86_400` 这个除数与单位）
        assert_eq!(super::today_from(None, 20_730 * 86_400), "2026-10-04");
        assert_eq!(
            super::today_from(None, 20_730 * 86_400 + 86_399),
            "2026-10-04"
        );
        assert_eq!(super::today_from(None, 20_731 * 86_400), "2026-10-05");
        // ④ 纪元前同样成立（负秒 ⇒ 负天，欧氏除法）
        assert_eq!(super::today_from(None, -86_400), "1969-12-31");
    }

    /// `today()` 真取系统时钟那条：形状必须是 YYYY-MM-DD（"换成空串/常量"的变异在这里断）。
    #[test]
    fn today_falls_back_to_a_valid_iso_date() {
        let t = super::today();
        assert_eq!(t.len(), 10, "系统日期必须是 YYYY-MM-DD：{t}");
        let b = t.as_bytes();
        assert!(
            b[4] == b'-'
                && b[7] == b'-'
                && b[..4].iter().all(u8::is_ascii_digit)
                && b[5..7].iter().all(u8::is_ascii_digit)
                && b[8..].iter().all(u8::is_ascii_digit),
            "不是 ISO 日期形状：{t}"
        );
    }
}
