//! ADV 规则引擎（adv-rules）。
//!
//! 选型已定（RESEARCH 01/X4/D1）：YAML taint spec（sources/sanitizers/propagators，
//! Semgrep 兼容形态）编译成 AstKind 分发 matcher；匹配层 regex-automata 选档树 +
//! HIR literal 预抽取 + aho-corasick 三档预算；规则测试门禁（ruleid 注释）；
//! 抑制用 `#[expect]` 语义（粒度/规则码/原因/到期四要素）。M1 落地。

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
