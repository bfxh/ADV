//! 文案 × 语言包对账的**纯判据**：采集到的可见文案里，哪些在语言包中找不到。
//!
//! 精确匹配语言包的**值**（trim 后）；键名不算（键名出现在界面上同样是硬编码）。
//! 已知误报面：带插值/运行时拼接的动态文案必然匹配不上，会被报成"缺失"——按"疑似"看。

use std::collections::BTreeSet;

use serde_json::Value;

/// 递归收集 JSON 里所有字符串值（数组/对象都下钻）。
pub fn locale_values(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() {
                out.insert(t.to_string());
            }
        }
        Value::Array(a) => a.iter().for_each(|x| locale_values(x, out)),
        Value::Object(m) => m.values().for_each(|x| locale_values(x, out)),
        _ => {}
    }
}

/// 候选文案里不在语言包值集合中的（排序去重，保留原文）。
pub fn unlocalized<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    values: &BTreeSet<String>,
) -> Vec<String> {
    let mut out: Vec<String> = candidates
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| !values.contains(*s))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}
