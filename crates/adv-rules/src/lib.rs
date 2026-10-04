//! ADV 规则引擎（adv-rules）。
//!
//! M1 片2：结构匹配（call）+ 污点流（taint：sources/sanitizers/propagators，
//! Semgrep 兼容形态）+ 行内抑制四要素（粒度=行/规则码/reason/until）+ 导入感知解析。

pub mod matcher;
pub mod rule;
pub mod suppression;
pub mod testing;

pub use matcher::{Finding, run_matchers};
pub use rule::{Rule, Severity, load_rules};
pub use suppression::{Malformed, Suppression};

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
