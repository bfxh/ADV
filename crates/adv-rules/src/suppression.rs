//! 行内抑制（`#[expect]` 四要素语义的注释版，RESEARCH D1）：粒度=行、规则码、原因、到期。
//!
//! 语法（写在违规代码**同一行**的注释里）：
//! - Python：`# adv:allow(RULE-ID, reason=理由, until=YYYY-MM-DD)`
//! - Rust：`// adv:allow(RULE-ID, reason=理由, until=YYYY-MM-DD)`
//!
//! 纪律：reason 与 until **必填**（缺失/畸形 = 畸形抑制，照样报红不静默）；
//! until < 今日 ⇒ 抑制失效，发现照报并由 `expired()` 另出到期账（防永久豁免）。

use adv_parse::GenericAst;

/// 抑制粒度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// 仅注释所在行。
    Line,
    /// 整个文件（声明置于文件头注释）。
    File,
}

/// 一条解析成功的抑制。
#[derive(Clone, Debug)]
pub struct Suppression {
    /// 被抑制的规则 ID。
    pub rule: String,
    /// 抑制原因（必填）。
    pub reason: String,
    /// 到期日（ISO YYYY-MM-DD，必填）。
    pub until: String,
    /// 生效行（1-based；File 粒度 = 声明所在行，仅记账用）。
    pub line: usize,
    /// 粒度。
    pub scope: Scope,
}

/// 一条畸形抑制（缺 reason/until 或格式错）——本身就是发现。
#[derive(Clone, Debug)]
pub struct Malformed {
    /// 所在行（1-based）。
    pub line: usize,
    /// 原文。
    pub text: String,
    /// 畸形原因。
    pub why: &'static str,
}

/// 从 AST 注释里解析全部抑制声明。
pub fn collect(ast: &GenericAst) -> (Vec<Suppression>, Vec<Malformed>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for (line, text) in &ast.comments {
        // 文档注释不是注解通道（本模块自己的语法示例就在文档里——否则自指误报）
        let raw = text.trim_start();
        if raw.starts_with("///")
            || raw.starts_with("//!")
            || raw.starts_with("/**")
            || raw.starts_with("/*!")
        {
            continue;
        }
        // 双注解行（ruleid: X + 抑制声明）也解析：取子串而非行首匹配；
        // 文件级 = allow-file 标记（置于文件头注释）
        let (marker, scope) = if let Some(p) = raw.find("adv:allow-file(") {
            (p, Scope::File)
        } else if let Some(p) = raw.find("adv:allow(") {
            (p, Scope::Line)
        } else {
            continue;
        };
        let inner = &raw[marker + marker_len(scope)..];
        let Some(end) = inner.rfind(')') else {
            bad.push(Malformed {
                line: *line,
                text: text.clone(),
                why: "缺少右括号",
            });
            continue;
        };
        let body = &inner[..end];
        let mut parts = body.split(',').map(str::trim).filter(|s| !s.is_empty());
        let Some(rule) = parts.next() else {
            bad.push(Malformed {
                line: *line,
                text: text.clone(),
                why: "缺少规则 ID",
            });
            continue;
        };
        let mut reason = String::new();
        let mut until = String::new();
        for kv in parts {
            match kv.split_once('=') {
                Some(("reason", v)) if !v.trim().is_empty() => reason = v.trim().to_string(),
                Some(("until", v)) => until = v.trim().to_string(),
                _ => {}
            }
        }
        if reason.is_empty() {
            bad.push(Malformed {
                line: *line,
                text: text.clone(),
                why: "缺 reason",
            });
            continue;
        }
        if !valid_date(&until) {
            bad.push(Malformed {
                line: *line,
                text: text.clone(),
                why: "until 缺失或非 YYYY-MM-DD",
            });
            continue;
        }
        ok.push(Suppression {
            rule: rule.to_string(),
            reason,
            until,
            line: *line,
            scope,
        });
    }
    (ok, bad)
}

/// ISO 日期字符串比较即时间序（YYYY-MM-DD 字典序 = 时间序）。
pub fn active(s: &Suppression, today: &str) -> bool {
    s.until.as_str() >= today
}

/// 到期账：今天已失效的抑制（发现照报，这里另记到期事实）。
pub fn expired(ast: &GenericAst, today: &str) -> Vec<(usize, String)> {
    let (ok, _) = collect(ast);
    ok.iter()
        .filter(|s| !active(s, today))
        .map(|s| (s.line, format!("{}（{}）", s.rule, s.until)))
        .collect()
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
}

/// 标记文本长度（按 scope）。
fn marker_len(scope: Scope) -> usize {
    match scope {
        Scope::File => "adv:allow-file(".len(),
        Scope::Line => "adv:allow(".len(),
    }
}
