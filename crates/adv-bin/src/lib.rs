//! ADV 二进制面（adv-bin，v0.2 新增，里程碑 M3b）。
//!
//! 分档（RESEARCH R1–R6/R4）：L0 格式解析（object+pelite+gimli，`ImageFacts` 一次解析
//! lazy 深挖）→ L1 缓解检查（winchecksec/BinSkim 字段集）+ 模式扫描（YARA-X 内化评估）+
//! cargo-auditable 读取 → L2 反汇编（iced-x86/yaxpeax）+ 函数识别（capa 范式）→
//! L3 Ghidra P-code/SLEIGH 深轨。相似性 = BSim 思想最小骨架。M3b 起步。

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
