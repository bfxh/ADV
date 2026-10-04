//! ADV 检索索引（adv-index）。
//!
//! 选型已定（RESEARCH 09/E4/E5/X13；2026-10-03 拍板：**不搞嵌入/向量层**）：
//! `ignore` 遍历 + Tantivy BM25（identifier/content 双字段）+ SCIP 符号 + RRF +
//! cAST 式 tree-sitter 分块；aider repo-map 移植（tree-sitter 标签→PageRank→预算内文件集）；
//! 存储 index+report 同库 / cache 独立文件；增量失效 = SAC early cutoff + 内容 hash 重验；
//! difftastic 语法 diff 收窄变更集（先量收窄比）。M4 落地。

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
