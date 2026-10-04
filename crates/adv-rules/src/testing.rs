//! 规则测试门禁（ruleid 注释，Semgrep 惯例）：断言发现清单与注释注解**逐行一致**。
//!
//! 漏报（该红没红）与误报（不该红却红）都算测试失败——规则必须自带反例。
//! 注解格式：`# ruleid: <RULE_ID>`（Python）/ `// ruleid: <RULE_ID>`（Rust），
//! 写在违规代码**同一行**。

use crate::matcher::{Finding, run_matchers};
use crate::rule::Rule;
use adv_parse::{Language, parse_source};
use std::path::Path;

/// 跑一条夹具并对照注解；不一致返回人类可读差异（测试里 unwrap 后 assert 即红）。
pub fn check_fixture(name: &str, lang: Language, src: &str, rules: &[Rule]) -> Result<(), String> {
    let ast = parse_source(lang, src).map_err(|e| format!("{name}: 解析失败 {e}"))?;
    let findings = run_matchers(&ast, Path::new(name), lang, rules);
    let mut expected: Vec<(usize, String)> = Vec::new();
    for (line, text) in &ast.comments {
        if let Some(id) = extract_ruleid(text) {
            expected.push((*line, id));
        }
    }
    expected.sort();
    let mut actual: Vec<(usize, String)> = findings
        .iter()
        .map(|f: &Finding| (f.start_line, f.rule.clone()))
        .collect();
    actual.sort();
    if expected == actual {
        return Ok(());
    }
    let miss: Vec<_> = expected.iter().filter(|e| !actual.contains(e)).collect();
    let extra: Vec<_> = actual.iter().filter(|a| !expected.contains(a)).collect();
    Err(format!(
        "{name}: 规则注解不一致\n  漏报（注了没报）: {miss:?}\n  误报（没注却报）: {extra:?}\n  实际: {actual:?}"
    ))
}

/// 从注释文本提取 `ruleid: <ID>`；非注解注释返回 None。
fn extract_ruleid(comment: &str) -> Option<String> {
    let t = comment.trim_start_matches(['#', '/', ' ']);
    let rest = t.strip_prefix("ruleid:")?;
    let id = rest.trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// 注解行集合（调试用）。
pub fn annotated_lines(src: &str, lang: Language) -> Vec<usize> {
    let ast = match parse_source(lang, src) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    ast.comments
        .iter()
        .filter(|(_, t)| extract_ruleid(t).is_some())
        .map(|(l, _)| *l)
        .collect()
}
