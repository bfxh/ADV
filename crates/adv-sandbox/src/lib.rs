//! ADV 沙箱与插件（adv-sandbox）。
//!
//! 选型已定（RESEARCH 08/E7/X14）：插件 ABI = WIT（WASI P2 语义）；wasmtime 主
//! （pooling + **fuel 主计量/epoch 兜底** + host 函数白名单=唯一能力出口）、wasmi 回退；
//! Windows 隔离 = broker/worker 池四件套（full AppContainer、Job Object kill-on-close、
//! 受限 token、alternate desktop）；spawn 出口"默认空环境+白名单"两道闸（X10）；
//! 能力白名单 = {虚拟名→broker 句柄, rights 位集, 派生树}（E7）。M6 落地。

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
