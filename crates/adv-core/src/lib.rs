//! ADV 内核（adv-core）。
//!
//! 职责（蓝图 §4）：错误类型、句柄化存储、路径策略、预算控制。
//! M0 仅落版本接线与身份标识；域逻辑按 M1+ 里程碑填充。

/// 产品名（CLI/MCP 面共用）。
pub const NAME: &str = "adv";

/// 工作区版本（与根 Cargo.toml `[workspace.package]` 同源；`xtask lockstep` 门负责对账 git tag）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_workspace_version() {
        assert_eq!(super::version(), "0.1.0");
    }
}
