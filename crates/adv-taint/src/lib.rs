//! ADV 污点分析（adv-taint）。
//!
//! 双轨（RESEARCH 01/P3/P5）：全语言快轨 = YAML spec→AstKind matcher；
//! Rust 深轨 = 边车 crate（adv-ast-rust，M2）MIR + IFDS 需求驱动 +
//! Ascent 不动点；告警必须接自动验证出口（USENIX Sec'26 Bond）。M2 落地。

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
