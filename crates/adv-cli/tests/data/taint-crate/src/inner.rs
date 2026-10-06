//! 合成语料②：另一模块里的流——证明深轨在 crate 形状下能扫到非根文件（单文件档做不到
//! 跨模块，因为它根本没有依赖/模块解析）。
pub fn inner_flow() {
    let raw = std::env::var("TAINT_INNER").unwrap_or_default();
    std::process::Command::new(raw); // adv-expect: hit
}
