//! 污点跟踪（M2 片2：同文件跨函数传播——函数摘要 + 作用域隔离）。
//!
//! 语义（保守口径，FN 面已记档）：
//! - 进程内单文件；语句序流敏感；**每个函数独立变量状态**（修掉片1 平铺 state 跨函数泄漏）。
//! - 函数摘要三旗标：`returns_source`（体内引用源调用且返回值带污点）、
//!   `returns_param_taint`（参数污点可流到返回值）、`sink_with_param`（参数污点可流进体内汇点）。
//! - 摘要按文件序单遍计算：**嵌套本地函数调用不参与摘要解析**（后来定义的函数对
//!   先前函数不可见）——FN 面记档；递归无不动点。
//! - 摘要内部同名的函数按名字合并（跨类同名方法保守合并）。

use super::matcher::{Finding, resolve_callee};
use crate::rule::Rule;
use crate::rule::TaintMatch;
use adv_parse::{AstId, AstKind, GenericAst};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// 污点判定环境（规则集合 + 导入表 + 语言分隔符 + 本地函数摘要）。
pub(crate) struct TaintEnv<'a> {
    sources: &'a [String],
    propagators: &'a [String],
    sanitizers: &'a [String],
    imports: &'a HashMap<String, String>,
    sep: &'a str,
    returns_source: &'a HashSet<String>,
    returns_param_taint: &'a HashSet<String>,
}

/// 污点发现（行号 = 调用点所在行；跨函数流报在调用点）。
pub fn taint_findings(
    ast: &GenericAst,
    file: &Path,
    rule: &Rule,
    m: &TaintMatch,
    imports: &HashMap<String, String>,
    sep: &str,
) -> Vec<Finding> {
    let sinks = m.sinks.to_vec();
    let sources = m.sources.to_vec();
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
    let (fns, owners) = collect_functions(ast);
    let summaries = summarize(
        &fns,
        &owners,
        ast,
        &sources,
        &propagators,
        &sanitizers,
        imports,
        sep,
        &sinks,
    );
    let env = TaintEnv {
        sources: &sources,
        propagators: &propagators,
        sanitizers: &sanitizers,
        imports,
        sep,
        returns_source: &summaries.returns_source,
        returns_param_taint: &summaries.returns_param_taint,
    };
    let mut state = ScopeState::default();
    let mut findings = Vec::new();
    for (i, node) in ast.nodes.iter().enumerate() {
        match &node.kind {
            AstKind::Assignment { targets, value } => {
                let scope = owners.scope(i);
                let tainted = value
                    .map(|v| expr_taint(ast, v, &env, &mut state, &scope))
                    .unwrap_or(false);
                for t in targets {
                    if let Some(name) = root_var(ast, *t) {
                        state.insert(&scope, name, tainted);
                    }
                }
            }
            AstKind::Call { .. } => {
                let callee = callee_of(ast, AstId(i as u32));
                let resolved = resolve_callee(callee, imports, sep);
                let scope = owners.scope(i);
                let args = args_of(ast, AstId(i as u32));
                let args_tainted = args
                    .iter()
                    .any(|a| expr_taint(ast, *a, &env, &mut state, &scope));
                // 本地函数：参数污点流入其体内汇点 ⇒ 在调用点报
                if args_tainted && summaries.sink_with_param.contains(callee) {
                    findings.push(new_finding(rule, file, node.span));
                    continue;
                }
                // 外部汇点：任一实参带污点
                if name_in(callee, resolved.as_deref(), &sinks) && args_tainted {
                    findings.push(new_finding(rule, file, node.span));
                }
            }
            _ => {}
        }
    }
    findings
}

/// 作用域状态：每个函数独立变量表（模块级用空串键）。
#[derive(Default)]
pub(crate) struct ScopeState {
    scopes: HashMap<String, HashMap<String, bool>>,
}

impl ScopeState {
    fn insert(&mut self, scope: &str, var: String, tainted: bool) {
        self.scopes
            .entry(scope.to_string())
            .or_default()
            .insert(var, tainted);
    }

    fn get(&self, scope: &str, var: &str) -> bool {
        *self
            .scopes
            .get(scope)
            .and_then(|m| m.get(var))
            .unwrap_or(&false)
    }
}

/// 作用域判定：节点 → 所属函数名（"" = 模块级）。
struct Owners {
    map: HashMap<usize, String>,
}

impl Owners {
    fn scope(&self, idx: usize) -> String {
        self.map.get(&idx).cloned().unwrap_or_default()
    }
}

struct FnEntry {
    name: String,
    node: usize,
}

/// 函数清单 + 节点归属（沿 parent 链找最近 FunctionDef；其自身节点归属自身）。
fn collect_functions(ast: &GenericAst) -> (Vec<FnEntry>, Owners) {
    let mut names: HashMap<usize, String> = HashMap::new();
    let mut fns = Vec::new();
    for (i, n) in ast.nodes.iter().enumerate() {
        if let AstKind::FunctionDef { name } = &n.kind {
            let name = ast.string(*name).to_string();
            names.insert(i, name.clone());
            fns.push(FnEntry { name, node: i });
        }
    }
    let mut owners = Vec::with_capacity(ast.nodes.len());
    for i in 0..ast.nodes.len() {
        let mut cur = i;
        let mut found = String::new();
        loop {
            if let Some(name) = names.get(&cur) {
                found = name.clone();
                break;
            }
            match ast.nodes.get(cur).and_then(|n| n.parent) {
                Some(p) => cur = p.0 as usize,
                None => break,
            }
        }
        owners.push(found);
    }
    (
        fns,
        Owners {
            map: owners.into_iter().enumerate().collect(),
        },
    )
}

struct Summaries {
    returns_source: HashSet<String>,
    returns_param_taint: HashSet<String>,
    sink_with_param: HashSet<String>,
}

#[allow(clippy::too_many_arguments)]
fn summarize(
    fns: &[FnEntry],
    owners: &Owners,
    ast: &GenericAst,
    sources: &[String],
    propagators: &[String],
    sanitizers: &[String],
    imports: &HashMap<String, String>,
    sep: &str,
    sinks: &[String],
) -> Summaries {
    let empty = HashSet::new();
    let mut out = Summaries {
        returns_source: HashSet::new(),
        returns_param_taint: HashSet::new(),
        sink_with_param: HashSet::new(),
    };
    for f in fns {
        let env = TaintEnv {
            sources,
            propagators,
            sanitizers,
            imports,
            sep,
            returns_source: &empty,
            returns_param_taint: &empty,
        };
        let mut state = ScopeState::default();
        for p in fn_param_names(ast, AstId(f.node as u32)) {
            state.insert(&f.name, p, true);
        }
        let mut returns_taint = false;
        for (i, n) in ast.nodes.iter().enumerate() {
            if owners.scope(i) != f.name {
                continue;
            }
            match &n.kind {
                AstKind::Assignment { targets, value } => {
                    let tainted = value
                        .map(|v| expr_taint(ast, v, &env, &mut state, &f.name))
                        .unwrap_or(false);
                    for t in targets {
                        if let Some(name) = root_var(ast, *t) {
                            state.insert(&f.name, name, tainted);
                        }
                    }
                }
                AstKind::Return => {
                    if let Some(v) = n.children.last()
                        && expr_taint(ast, *v, &env, &mut state, &f.name)
                    {
                        returns_taint = true;
                    }
                }
                AstKind::Call { .. } => {
                    let callee = callee_of(ast, AstId(i as u32));
                    let resolved = resolve_callee(callee, imports, sep);
                    let dirty = args_of(ast, AstId(i as u32))
                        .iter()
                        .any(|a| expr_taint(ast, *a, &env, &mut state, &f.name));
                    if dirty && name_in(callee, resolved.as_deref(), sinks) {
                        out.sink_with_param.insert(f.name.clone());
                    }
                }
                _ => {}
            }
        }
        if returns_taint {
            if body_touches_source(ast, sources, AstId(f.node as u32)) {
                out.returns_source.insert(f.name.clone());
            } else {
                out.returns_param_taint.insert(f.name.clone());
            }
        }
    }
    out
}

fn body_touches_source(ast: &GenericAst, sources: &[String], fn_node: AstId) -> bool {
    let mut found = false;
    walk_check(ast, fn_node, sources, &mut found);
    found
}

fn walk_check(ast: &GenericAst, id: AstId, sources: &[String], found: &mut bool) {
    if *found {
        return;
    }
    if let AstKind::Call { callee, .. } = &ast.get(id).kind
        && sources.iter().any(|s| s == callee)
    {
        *found = true;
        return;
    }
    for c in &ast.get(id).children {
        walk_check(ast, *c, sources, found);
    }
}

/// 参数名：FunctionDef 的孩子里找 parameters 节点，仅在该子树收 Identifier
///（默认值表达式里的标识符也会被收——保守，罕见场景记档）。
fn fn_param_names(ast: &GenericAst, fn_node: AstId) -> Vec<String> {
    let mut names = Vec::new();
    for c in &ast.get(fn_node).children {
        if let AstKind::Other { ts_kind } = &ast.get(*c).kind
            && ts_kind == "parameters"
        {
            collect_identifiers(ast, *c, &mut names);
        }
    }
    names
}

fn collect_identifiers(ast: &GenericAst, id: AstId, names: &mut Vec<String>) {
    match &ast.get(id).kind {
        AstKind::Identifier { name } => names.push(ast.string(*name).to_string()),
        _ => {
            for c in &ast.get(id).children {
                collect_identifiers(ast, *c, names);
            }
        }
    }
}

fn args_of(ast: &GenericAst, id: AstId) -> Vec<AstId> {
    match &ast.get(id).kind {
        AstKind::Call { args, kwargs, .. } => args
            .iter()
            .chain(kwargs.iter().map(|(_, v)| v))
            .copied()
            .collect(),
        _ => Vec::new(),
    }
}

fn callee_of(ast: &GenericAst, id: AstId) -> &str {
    match &ast.get(id).kind {
        AstKind::Call { callee, .. } => callee,
        _ => "",
    }
}

/// 原样名或导入解析名任一命中（沿用三段语义：裸名/前导点/点分）。
pub(crate) fn name_in(callee: &str, resolved: Option<&str>, set: &[String]) -> bool {
    crate::matcher::callee_matches(callee, set)
        || resolved.is_some_and(|r| crate::matcher::callee_matches(r, set))
}

/// 表达式污点（递归；Call 分类顺序：源 → 本地摘要 → 净化 → 传播；其他节点看孩子）。
pub(crate) fn expr_taint(
    ast: &GenericAst,
    id: AstId,
    env: &TaintEnv,
    state: &mut ScopeState,
    scope: &str,
) -> bool {
    let node = ast.get(id);
    match &node.kind {
        AstKind::Literal { .. } => false,
        AstKind::Identifier { name } => state.get(scope, ast.string(*name)),
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
            // 本地函数摘要（同文件已定义者）
            if env.returns_source.contains(callee) {
                return true;
            }
            // 方法调用的接收者也是污点入口（s.strip() 的 s 在函数路径里，不在 args）
            let receiver = node
                .children
                .first()
                .map(|c| expr_taint(ast, *c, env, state, scope))
                .unwrap_or(false);
            let any_arg = args
                .iter()
                .chain(kwargs.iter().map(|(_, v)| v))
                .any(|a| expr_taint(ast, *a, env, state, scope));
            if env.returns_param_taint.contains(callee) {
                return receiver || any_arg;
            }
            if name_in(callee, resolved.as_deref(), env.propagators) {
                return receiver || any_arg;
            }
            false // 未声明传播的调用不吃污点（保守；FN 面记档）
        }
        AstKind::Attribute { .. } => node
            .children
            .first()
            .map(|c| expr_taint(ast, *c, env, state, scope))
            .unwrap_or(false),
        _ => node
            .children
            .iter()
            .any(|c| expr_taint(ast, *c, env, state, scope)),
    }
}

/// 取表达式绑定的根变量名（Identifier 本名 / Attribute / 下标的基名）。
pub(crate) fn root_var(ast: &GenericAst, id: AstId) -> Option<String> {
    match &ast.get(id).kind {
        AstKind::Identifier { name } => Some(ast.string(*name).to_string()),
        AstKind::Attribute { .. } | AstKind::Other { .. } => {
            root_var(ast, *ast.get(id).children.first()?)
        }
        _ => None,
    }
}

fn new_finding(rule: &Rule, file: &Path, span: adv_parse::Span) -> Finding {
    Finding {
        rule: rule.id.clone(),
        severity: format!("{:?}", rule.severity).to_lowercase(),
        message: rule.message.clone(),
        file: file.to_string_lossy().replace('\\', "/"),
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}
