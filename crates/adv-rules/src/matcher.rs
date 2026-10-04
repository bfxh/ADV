//! 匹配器：规则 × 归一 AST → 发现清单（M1 片2：call/taint 双形态 + 抑制 + 导入感知）。
//!
//! 性能注记：朴素全遍历；aho-corasick 字面量预过滤按 RESEARCH X4 待基线后加
//!（先量后改，扫视层计账不预优化）。

use crate::rule::{Rule, rule_language};
use crate::suppression;
use adv_parse::{AstId, AstKind, GenericAst, Language};
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
    let mut findings = Vec::new();
    for rule in rules {
        if rule_language(rule) != Some(lang) {
            continue;
        }
        let sep = match lang {
            Language::Python => ".",
            Language::Rust => "::",
        };
        let bucket: Vec<Finding> = match (&rule.r#match.call, &rule.r#match.taint) {
            (Some(m), None) => call_findings(ast, file, rule, m, &imports, sep),
            (None, Some(m)) => taint_findings(ast, file, rule, m, &imports, sep),
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

/// 导入感知映射：`from X import a as b` ⇒ b → X.a（import 语句绑定；相对导入跳过）。
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

/// 污点跟踪（进程内单文件，语句序；只有声明过的 propagators 传播——保守口径，FN 面已记档）。
fn taint_findings(
    ast: &GenericAst,
    file: &Path,
    rule: &Rule,
    m: &crate::rule::TaintMatch,
    imports: &HashMap<String, String>,
    sep: &str,
) -> Vec<Finding> {
    let sources = m.sources.to_vec();
    let sinks = m.sinks.to_vec();
    let propagators = m
        .propagators
        .as_ref()
        .map(|p| p.to_vec())
        .unwrap_or_default();
    let sanitizers = m
        .sanitizers
        .as_ref()
        .map(|s| s.to_vec())
        .unwrap_or_default();
    let env = TaintEnv {
        sources: &sources,
        propagators: &propagators,
        sanitizers: &sanitizers,
        imports,
        sep,
    };
    let mut state: HashMap<String, bool> = HashMap::new();
    let mut findings = Vec::new();
    for (i, node) in ast.nodes.iter().enumerate() {
        match &node.kind {
            AstKind::Assignment { targets, value } => {
                let tainted = value
                    .map(|v| expr_taint(ast, v, &env, &mut state))
                    .unwrap_or(false);
                for t in targets {
                    if let Some(name) = root_var(ast, *t) {
                        state.insert(name, tainted);
                    }
                }
            }
            AstKind::Call { .. } => {
                // 汇点判定：任一实参带污点（源调用/污染变量/传播子均按表达式污点判定）
                let callee = callee_of(ast, AstId(i as u32));
                let resolved = resolve_callee(callee, env.imports, env.sep);
                if !name_in(callee, resolved.as_deref(), &sinks) {
                    continue;
                }
                let n = ast.get(AstId(i as u32));
                let AstKind::Call { args, kwargs, .. } = &n.kind else {
                    continue;
                };
                let dirty = args
                    .iter()
                    .chain(kwargs.iter().map(|(_, v)| v))
                    .any(|a| expr_taint(ast, *a, &env, &mut state));
                if dirty {
                    findings.push(new_finding(
                        rule,
                        file,
                        node.span.start_line,
                        node.span.start_col,
                        node.span.end_line,
                        node.span.end_col,
                    ));
                }
            }
            _ => {}
        }
    }
    findings
}

fn callee_of(ast: &GenericAst, id: AstId) -> &str {
    match &ast.get(id).kind {
        AstKind::Call { callee, .. } => callee,
        _ => "",
    }
}

/// 原样名或导入解析名任一命中（沿用三段语义：裸名/前导点/点分）。
fn name_in(callee: &str, resolved: Option<&str>, set: &[String]) -> bool {
    callee_matches(callee, set) || resolved.is_some_and(|r| callee_matches(r, set))
}

/// 导入解析：整名命中优先；否则首段替换（`env::var` + env→std::env ⇒ std::env::var）。
fn resolve_callee(callee: &str, imports: &HashMap<String, String>, sep: &str) -> Option<String> {
    if let Some(mapped) = imports.get(callee) {
        return Some(mapped.clone());
    }
    let (first, rest) = callee.split_once(sep)?;
    imports
        .get(first)
        .map(|mapped| format!("{mapped}{sep}{rest}"))
}

/// 污点判定环境（规则集合 + 导入表 + 语言分隔符）。
struct TaintEnv<'a> {
    sources: &'a [String],
    propagators: &'a [String],
    sanitizers: &'a [String],
    imports: &'a HashMap<String, String>,
    sep: &'a str,
}

/// 表达式污点（递归；Call 按源/净化/传播三分类，其他节点看孩子变量与子表达式）。
fn expr_taint(
    ast: &GenericAst,
    id: AstId,
    env: &TaintEnv,
    state: &mut HashMap<String, bool>,
) -> bool {
    let node = ast.get(id);
    match &node.kind {
        AstKind::Literal { .. } => false,
        AstKind::Identifier { name } => *state.get(ast.string(*name)).unwrap_or(&false),
        AstKind::Call {
            callee,
            args,
            kwargs,
        } => {
            let resolved = resolve_callee(callee, env.imports, env.sep);
            if name_in(callee, resolved.as_deref(), env.sources) {
                return true;
            }
            if name_in(callee, resolved.as_deref(), env.sanitizers) {
                return false;
            }
            // 方法调用的接收者也是污点入口（s.strip() 的 s 在函数路径里，不在 args）
            let receiver = node
                .children
                .first()
                .map(|c| expr_taint(ast, *c, env, state))
                .unwrap_or(false);
            let any_arg = args
                .iter()
                .chain(kwargs.iter().map(|(_, v)| v))
                .any(|a| expr_taint(ast, *a, env, state));
            if name_in(callee, resolved.as_deref(), env.propagators) {
                return receiver || any_arg;
            }
            false // 未声明传播的调用不吃污点（保守；FN 面记档）
        }
        AstKind::Attribute { .. } => node
            .children
            .first()
            .map(|c| expr_taint(ast, *c, env, state))
            .unwrap_or(false),
        _ => node
            .children
            .iter()
            .any(|c| expr_taint(ast, *c, env, state)),
    }
}

/// 取表达式绑定的根变量名（Identifier 本名 / Attribute / 下标的基名）。
fn root_var(ast: &GenericAst, id: AstId) -> Option<String> {
    match &ast.get(id).kind {
        AstKind::Identifier { name } => Some(ast.string(*name).to_string()),
        AstKind::Attribute { .. } | AstKind::Other { .. } => {
            root_var(ast, *ast.get(id).children.first()?)
        }
        _ => None,
    }
}

fn new_finding(rule: &Rule, file: &Path, sl: usize, sc: usize, el: usize, ec: usize) -> Finding {
    Finding {
        rule: rule.id.clone(),
        severity: format!("{:?}", rule.severity).to_lowercase(),
        message: rule.message.clone(),
        file: file.to_string_lossy().replace('\\', "/"),
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
    }
}

/// 被调名匹配（Semgrep 对齐的三段语义）：
/// - 裸名 `eval`：只命中裸名调用（`obj.eval` 不命中——method 调用是另一模式）；
/// - 前导点 `.unwrap`：方法调用，路径 ≥2 段且末段相等（`x.unwrap` 命中、`unwrap` 不命中）；
/// - 点分路径 `subprocess.run`：整段精确相等。
///
/// 导入感知解析（别名/相对导入）由调用方先重写候选名再进本判定。
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
