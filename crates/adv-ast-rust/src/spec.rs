//! 污点规则镜像（不含 rustc 类型，因此可在 lib 目标里被直接单测/集成测）。
//!
//! 单一事实源：四要素一律从 `rules/**.yaml` 经 `adv_rules::rule::load_rules` 读出，
//! 深轨不自带规则表。口径与快轨一致（见 rules/python/taint-eval.yaml 的注释）：
//! **只有声明过的 propagators 传播**，sanitizers 杀灭，未声明的调用不传播（FN 面如实记账）。

use adv_rules::rule::{Rule, load_rules};
use std::path::Path;

/// 一条 rust 污点规则在深轨侧的镜像。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// 规则 ID（进 finding 供对拍分账）。
    pub id: String,
    /// 源点 callee 路径。
    pub sources: Vec<String>,
    /// 汇点 callee 路径。
    pub sinks: Vec<String>,
    /// 传播子 callee 路径。
    pub propagators: Vec<String>,
    /// 净化工 callee 路径。
    pub sanitizers: Vec<String>,
}

/// 从规则目录取 rust 侧的污点规则（非 rust 语言、非 taint 形态的规则跳过）。
pub fn specs_from_rules(dir: &Path) -> Result<Vec<Spec>, String> {
    let rules = load_rules(dir)?;
    Ok(rules.iter().filter_map(spec_of).collect())
}

fn spec_of(rule: &Rule) -> Option<Spec> {
    if !rule.languages.iter().any(|l| l == "rust") {
        return None;
    }
    let taint = rule.r#match.taint.as_ref()?;
    Some(Spec {
        id: rule.id.clone(),
        sources: taint.sources.to_vec(),
        sinks: taint.sinks.to_vec(),
        propagators: taint
            .propagators
            .as_ref()
            .map(|p| p.to_vec())
            .unwrap_or_default(),
        sanitizers: taint
            .sanitizers
            .as_ref()
            .map(|s| s.to_vec())
            .unwrap_or_default(),
    })
}

/// callee 路径匹配：全等，或以 `.pat` / `::pat` 结尾。
///
/// 需要后缀档是因为 MIR 里同一目标的 `def_path_str` 可能带更长的命名空间前缀
/// （如 `<impl>` 段），而规则作者写的是用户可见路径。
pub fn callee_matches(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| path == p || path.ends_with(&format!("::{p}")) || path.ends_with(&format!(".{p}")))
}
