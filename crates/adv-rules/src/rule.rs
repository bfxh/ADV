//! 规则 schema 与加载（YAML；M1 片1 结构匹配子集）。
//!
//! 形态对齐 Semgrep 兼容约定（RESEARCH 01）：一条规则一个文件，`id/languages/
//! severity/message/match`；license 上规则即数据，YAML 解析失败即红（fail-closed，
//! 沿用旧仓 arch_gate 空规则即红的教训）。

use adv_parse::Language;
use serde::Deserialize;
use std::path::Path;

/// 规则严重级（与报告层契约；xX-#### 错误码在片2 接错误码表）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// 错误（阻断级）。
    Error,
    /// 警告（默认级）。
    Warning,
    /// 提示。
    Info,
}

/// 规则匹配子句（`call`/`taint` 二选一；两者都缺或都在 = 加载即红，fail-closed）。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Match {
    /// 结构匹配（调用点）。
    pub call: Option<CallMatch>,
    /// 污点流（sources → sinks，进程内单文件）。
    pub taint: Option<TaintMatch>,
}

impl Match {
    /// 加载期校验：恰好一种形态。
    pub fn validate(&self) -> Result<(), String> {
        match (&self.call, &self.taint) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(_), Some(_)) => Err("match 里 call 与 taint 同时出现（二选一）".into()),
            (None, None) => Err("match 里 call 与 taint 都缺失".into()),
        }
    }
}

/// 污点规则（RESEARCH 01/02：Semgrep 兼容 sources/sanitizers/propagators 形态）。
/// 语义 = 进程内单文件、语句序流敏感的保守跟踪：只有声明过的 propagators 传播，
/// 未经声明路径的调用视为净化点（FN 面已记 FP 会计）。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintMatch {
    /// 污点源（调用名）。
    pub sources: OneOrMany,
    /// 汇点（调用名，任一实参带污点即命中）。
    pub sinks: OneOrMany,
    /// 传播子（任一实参带污点则返回值带污点）。
    #[serde(default)]
    pub propagators: Option<OneOrMany>,
    /// 净化子（返回值视为干净）。
    #[serde(default)]
    pub sanitizers: Option<OneOrMany>,
}

/// 调用匹配：点分名后缀按段匹配（`eval` 匹配 `eval` 与 `x.eval`，不匹配 `x.evalx`）。
#[derive(Clone, Debug, Deserialize)]
pub struct CallMatch {
    /// 被调名（单值或列表）：裸名=只命中裸名；前导点=方法调用末段；点分=整段精确。
    pub callee: OneOrMany,
    /// 要求存在的关键字参数（name 必有；value 填了则要求字面量原文相等）。
    #[serde(default)]
    pub kwargs: Vec<KwargMatch>,
}

/// 关键字参数约束。
#[derive(Clone, Debug, Deserialize)]
pub struct KwargMatch {
    /// 关键字参数名。
    pub name: String,
    /// 字面量原文（如 "True"）；不填则只要求该参存在。
    pub value: Option<String>,
}

/// YAML 单值或列表的兼容读取。
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
/// YAML 单值或列表的兼容读取。
pub enum OneOrMany {
    /// 单值。
    One(String),
    /// 列表。
    Many(Vec<String>),
}

impl OneOrMany {
    /// 展平成列表。
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }
}

/// 规则（YAML 文件 ⇔ 结构一一对应；未知字段拒绝，防静默错配）。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// 规则 ID（`<LANG>-<NAME>`；错误码表在片2 接管 `ADV-E####` 对账）。
    pub id: String,
    /// 适用语言（python/rust；其他语言该规则自动跳过）。
    pub languages: Vec<String>,
    /// 严重级。
    pub severity: Severity,
    /// 报告消息（面向开发者的修复指引）。
    pub message: String,
    /// 匹配子句。
    pub r#match: Match,
}

/// 规则加载（目录递归 *.yaml/*.yml；解析失败即错——空规则即红，fail-closed）。
pub fn load_rules(dir: &Path) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries =
            std::fs::read_dir(&d).map_err(|e| format!("读目录 {} 失败：{e}", d.display()))?;
        for e in entries {
            let p = e.map_err(|err| err.to_string())?.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if ext != "yaml" && ext != "yml" {
                continue;
            }
            let text = std::fs::read_to_string(&p)
                .map_err(|err| format!("读 {} 失败：{err}", p.display()))?;
            let rule: Rule = serde_yaml_ng::from_str(&text)
                .map_err(|err| format!("解析 {} 失败：{err}", p.display()))?;
            rule.r#match
                .validate()
                .map_err(|err| format!("规则 {}：{err}", rule.id))?;
            rules.push(rule);
        }
    }
    if rules.is_empty() {
        return Err(format!("规则目录 {} 为空（空规则即红）", dir.display()));
    }
    for r in &rules {
        for lang in &r.languages {
            if lang != "python" && lang != "rust" {
                return Err(format!("规则 {} 声明了不支持的语言 {lang}", r.id));
            }
        }
    }
    Ok(rules)
}

/// 规则声明的语言映射（供 matcher 过滤）。
pub fn rule_language(r: &Rule) -> Option<Language> {
    match r.languages.first().map(String::as_str) {
        Some("python") => Some(Language::Python),
        Some("rust") => Some(Language::Rust),
        _ => None,
    }
}
