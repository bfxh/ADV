//! 匹配器：规则 × 归一 AST → 发现清单（M1 片1：call 后缀匹配 + kwarg 约束）。
//!
//! 性能注记：朴素全遍历；aho-corasick 字面量预过滤按 RESEARCH X4 待基线后加
//!（先量后改，扫视层计账不预优化）。

use crate::rule::{Rule, rule_language};
use adv_parse::{AstKind, GenericAst, Language};
use serde::Serialize;
use std::path::Path;

/// 一条发现（JSONL 报告行 ⇔ 结构一一对应）。
#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    /// 命中的规则 ID。
    pub rule: String,
    /// 严重级（lowercase）。
    pub severity: String,
    /// 规则消息。
    pub message: String,
    /// 文件路径（`/` 分隔，机器可读口径）。
    pub file: String,
    /// 起始行（1-based）。
    pub start_line: usize,
    /// 起始列（0-based）。
    pub start_col: usize,
    /// 结束行（1-based）。
    pub end_line: usize,
    /// 结束列（0-based）。
    pub end_col: usize,
}

/// 对单文件 AST 跑全部适用规则。
pub fn run_matchers(ast: &GenericAst, file: &Path, lang: Language, rules: &[Rule]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for node in &ast.nodes {
        let AstKind::Call { callee, kwargs, .. } = &node.kind else {
            continue;
        };
        for rule in rules {
            if rule_language(rule) != Some(lang) {
                continue;
            }
            let m = &rule.r#match.call;
            if !callee_matches(callee, &m.callee.to_vec()) {
                continue;
            }
            if !kwargs_satisfied(ast, kwargs, &m.kwargs) {
                continue;
            }
            findings.push(Finding {
                rule: rule.id.clone(),
                severity: format!("{:?}", rule.severity).to_lowercase(),
                message: rule.message.clone(),
                file: file.to_string_lossy().replace('\\', "/"),
                start_line: node.span.start_line,
                start_col: node.span.start_col,
                end_line: node.span.end_line,
                end_col: node.span.end_col,
            });
        }
    }
    findings
}

/// 被调名匹配（Semgrep 对齐的三段语义）：
/// - 裸名 `eval`：只命中裸名调用（`obj.eval` 不命中——method 调用是另一模式）；
/// - 前导点 `.unwrap`：方法调用，路径 ≥2 段且末段相等（`x.unwrap` 命中、`unwrap` 不命中）；
/// - 点分路径 `subprocess.run`：整段精确相等。
///
/// 导入感知解析（别名/相对导入）是片2 的活——此限制已记入 FP 会计。
fn callee_matches(callee: &str, wanted: &[String]) -> bool {
    wanted.iter().any(|w| {
        if let Some(last) = w.strip_prefix('.') {
            let segs = split_path(callee);
            segs.len() >= 2 && segs.last().is_some_and(|s| *s == last)
        } else {
            callee == w
        }
    })
}

/// 路径切段（`.` 与 `::` 都算分隔）。
fn split_path(callee: &str) -> Vec<&str> {
    callee
        .split("::")
        .flat_map(|s| s.split('.'))
        .filter(|s| !s.is_empty())
        .collect()
}

/// 全部 kwarg 约束满足才命中；value 有值时要求该参是字面量且原文相等。
fn kwargs_satisfied(
    ast: &GenericAst,
    actual: &[(String, adv_parse::AstId)],
    wanted: &[crate::rule::KwargMatch],
) -> bool {
    if wanted.is_empty() {
        return true;
    }
    wanted.iter().all(|w| {
        actual.iter().any(|(name, vid)| {
            if name != &w.name {
                return false;
            }
            match &w.value {
                None => true,
                Some(v) => match &ast.get(*vid).kind {
                    AstKind::Literal { text, .. } => text == v,
                    _ => false,
                },
            }
        })
    })
}
