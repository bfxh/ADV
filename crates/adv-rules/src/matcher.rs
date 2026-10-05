//! 匹配器：规则 × 归一 AST → 发现清单（call 结构匹配 + taint 分发；抑制 + 导入感知）。
//!
//! 性能注记：朴素全遍历；aho-corasick 字面量预过滤按 RESEARCH X4 待基线后加
//!（先量后改，扫视层计账不预优化）。

use crate::rule::{Rule, Severity, rule_language};
use crate::suppression;
use crate::taint;
use adv_parse::{AstKind, GenericAst, Language};
use serde::Serialize;
use std::collections::HashMap;
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
    /// 产出该发现的引擎：快轨为 `tree-sitter`，深轨为 `mir`（对拍分账口径，PLAN-deep-track §3）。
    pub engine: String,
}

impl Finding {
    /// JSONL 报告行（adv-cli stdout 的行契约；金标准门压这里）。
    pub fn to_jsonl(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// 对单文件 AST 跑全部适用规则；`today` 为 ISO 日期（抑制到期判定，注入以便测试）。
pub fn run_matchers(
    ast: &GenericAst,
    file: &Path,
    lang: Language,
    rules: &[Rule],
    today: &str,
) -> Vec<Finding> {
    let (suppressions, _) = suppression::collect(ast);
    let imports = import_map(ast);
    let sep = match lang {
        Language::Python => ".",
        Language::Rust => "::",
    };
    let mut findings = Vec::new();
    for rule in rules {
        if rule_language(rule) != Some(lang) {
            continue;
        }
        let bucket: Vec<Finding> = match (&rule.r#match.call, &rule.r#match.taint) {
            (Some(m), None) => call_findings(ast, file, rule, m, &imports, sep),
            (None, Some(m)) => taint::taint_findings(ast, file, rule, m, &imports, sep),
            _ => continue, // 加载期已校验，此处不可达
        };
        findings.extend(bucket);
    }
    // 抑制过滤（粒度 × 规则 × 未到期）：File 粒度压全文件，Line 粒度压所在行
    findings.retain(|f| {
        !suppressions.iter().any(|s| {
            s.rule == f.rule
                && suppression::active(s, today)
                && match s.scope {
                    suppression::Scope::File => true,
                    suppression::Scope::Line => s.line == f.start_line,
                }
        })
    });
    findings
}

/// 结构匹配：遍历全部 Call 节点（含导入解析后的候选名）。
fn call_findings(
    ast: &GenericAst,
    file: &Path,
    rule: &Rule,
    m: &crate::rule::CallMatch,
    imports: &HashMap<String, String>,
    sep: &str,
) -> Vec<Finding> {
    let wanted = m.callee.to_vec();
    let mut findings = Vec::new();
    for node in &ast.nodes {
        let AstKind::Call { callee, kwargs, .. } = &node.kind else {
            continue;
        };
        let resolved = resolve_callee(callee, imports, sep);
        let hit = callee_matches(callee, &wanted)
            || resolved.is_some_and(|r| callee_matches(r.as_str(), &wanted));
        if !hit || !kwargs_satisfied(ast, kwargs, &m.kwargs) {
            continue;
        }
        findings.push(new_finding(
            rule,
            file,
            node.span.start_line,
            node.span.start_col,
            node.span.end_line,
            node.span.end_col,
        ));
    }
    findings
}

/// 导入感知映射：`from X import a as b` ⇒ b → X.a；rust `use p::n` ⇒ n → p::n；
/// 相对导入与通配跳过；python 纯导入（无别名）不入映射（恒等绑定，防 a→a.b 假账）。
fn import_map(ast: &GenericAst) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for node in &ast.nodes {
        let AstKind::Import { module, names } = &node.kind else {
            continue;
        };
        if module.starts_with('.') {
            continue;
        }
        for (imported, alias) in names {
            if alias != "*" {
                let path = if module.is_empty() {
                    imported.clone()
                } else {
                    format!("{module}.{imported}")
                };
                m.insert(alias.clone(), path);
            }
        }
    }
    m
}

/// 导入解析：整名命中优先；否则首段替换（`env::var` + env→std::env ⇒ std::env::var）。
pub(crate) fn resolve_callee(
    callee: &str,
    imports: &HashMap<String, String>,
    sep: &str,
) -> Option<String> {
    if let Some(mapped) = imports.get(callee) {
        return Some(mapped.clone());
    }
    let (first, rest) = callee.split_once(sep)?;
    imports
        .get(first)
        .map(|mapped| format!("{mapped}{sep}{rest}"))
}

/// 被调名匹配（Semgrep 对齐的三段语义）：
/// - 裸名 `eval`：只命中裸名调用（`obj.eval` 不命中——method 调用是另一模式）；
/// - 前导点 `.unwrap`：方法调用，路径 ≥2 段且末段相等（`x.unwrap` 命中、`unwrap` 不命中）；
/// - 点分路径 `subprocess.run`：整段精确相等。
///
/// 导入感知解析（别名/相对导入）由调用方先重写候选名再进本判定。
pub(crate) fn callee_matches(callee: &str, wanted: &[String]) -> bool {
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

/// 快轨引擎名（行契约字段值；深轨侧用 `mir`）。
pub const ENGINE_FAST: &str = "tree-sitter";

/// 深轨引擎名（行契约字段值）。
pub const ENGINE_MIR: &str = "mir";

/// 严重级的行契约写法（lowercase Debug）——快轨、深轨、adv-cli 共用一处，避免三处各写各的。
pub fn severity_name(severity: Severity) -> String {
    format!("{severity:?}").to_lowercase()
}

fn new_finding(rule: &Rule, file: &Path, sl: usize, sc: usize, el: usize, ec: usize) -> Finding {
    Finding {
        rule: rule.id.clone(),
        severity: severity_name(rule.severity),
        message: rule.message.clone(),
        file: file.to_string_lossy().replace('\\', "/"),
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
        engine: ENGINE_FAST.to_string(),
    }
}
