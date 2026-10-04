//! ADV MCP 服务器（adv-server）。
//!
//! 选型已定（RESEARCH 04/E5/X6）：stdio 主路径**自研**（2026-07-28 spec 无状态语义；
//! 同步最小方案 = reader 线程 + crossbeam-channel + 串行 writer，tokio 不进默认档）；
//! 官方 conformance 套件当 CI 验收门；三层渐进披露默认化（L0 轻目录 ≈1K token +
//! discover/load 元工具对 + outputSchema/structuredContent）；SWE-agent ACI 四原则
//! 当工具面验收判据；sampling/roots/logging 已废弃永不采纳。M5 落地。

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
