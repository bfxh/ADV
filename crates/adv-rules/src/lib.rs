//! ADV 规则引擎（adv-rules）。
//!
//! M1 片1：结构匹配规则（YAML → matcher，AstKind 分发）；污点 spec（sources/
//! sanitizers/propagators，Semgrep 兼容形态）在片2 接入同一 schema。
//! 抑制语义按 `#[expect]` 四要素（粒度/规则码/原因/到期）设计（RESEARCH D1），片2 落地。

pub mod matcher;
pub mod rule;
pub mod testing;

pub use matcher::{Finding, run_matchers};
pub use rule::{Rule, Severity, load_rules};

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
