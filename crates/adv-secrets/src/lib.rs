//! ADV secrets 检测（adv-secrets）。
//!
//! 决议（2026-10-03 拍板）：Nosey Parker（Apache-2.0）**内化源码**而非外挂依赖，
//! 保留上游 LICENSE/署名；机制 = 内容寻址 datastore + 按 blob 增量 + 单捕获组规则 +
//! 按 secret 内容去重（RESEARCH 02）。报告掩码抄 GitHub 语义（X10：定长 `***`、
//! base64 形态同掩、部分掩码 40 bits 熵保底提案档）。M3 落地。

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
