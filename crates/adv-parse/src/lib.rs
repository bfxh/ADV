//! ADV 解析层（adv-parse）。
//!
//! 选型已定（RESEARCH 01/09、X1、D10）：tree-sitter 0.25 多语言主解析；
//! 自建最小归一 AST（Semgrep「CST→generic AST」形状 + oxc arena/AstKind 底座）；
//! 句柄化存储 id-arena/slotmap + lasso 标识符驻留；rowan 零失败解析参照。
//! M1 落地；M0 仅版本接线。

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
