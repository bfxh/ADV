//! 试验面登记（`spec/maturity.json`）的读取与匹配 —— docs/DESIGN-DEBT-GATE.md §3 的生效点。
//!
//! 为什么放在 xtask 而不是 Python：放松的是 **god 计量**（Rust 侧），而登记本身由
//! `scripts/debt_gate.py` 判格式（必填、pattern 不许过宽）。两边各管一段，不重复实现判据。
//!
//! 匹配只支持登记实际用到的子集：`*` = 单个路径段内任意字符，结尾 `/**` = 该目录以下任意深度。
//! 故意不做完整 glob 语义——多支持一种写法就多一处没人测的分支（片A6 刚为这类东西补过判据）。

use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Maturity {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    pattern: String,
}

/// 登记里的试验面 pattern。读不到就返回空——方向是**不放松**（god 照旧全量计量），
/// 所以这里不需要 fail-closed，也不会静默把承重面放掉。
pub fn experimental_patterns(root: &Path) -> Vec<String> {
    let path = root.join("spec/maturity.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        eprintln!("提示：读不到 {}，god 按全量计量（不放松）", path.display());
        return Vec::new();
    };
    match serde_json::from_str::<Maturity>(&raw) {
        Ok(m) => m.entries.into_iter().map(|e| e.pattern).collect(),
        Err(e) => {
            eprintln!(
                "提示：{} 解析失败（{e}），god 按全量计量（不放松）",
                path.display()
            );
            Vec::new()
        }
    }
}

/// 该相对路径是否落在登记的试验面里。
pub fn pattern_matches(pattern: &str, rel: &str) -> bool {
    let pats: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let segs: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    // 只支持**结尾**的 `**`。别的段落里出现 `**` 一律不命中——实测过：不挡的话前面的
    // `**` 会被星号循环当成 `*` 用，于是 `**/x.rs` 悄悄能匹配 `a/x.rs`（放松比字面宽）。
    let deep_tail = pats.last().is_some_and(|p| *p == "**");
    if pats
        .iter()
        .enumerate()
        .any(|(i, s)| s.contains("**") && !(deep_tail && i + 1 == pats.len()))
    {
        return false;
    }
    if let Some(pats_rest) = pats.strip_suffix(&["**"][..]) {
        // 目录前缀匹配，且 `**` 至少吃一层：登记写的是"该目录**以下**"，
        // 放松范围不许比登记的字面更宽（少这一条就会把目录同名的文件也算进去）。
        if pats_rest.len() >= segs.len() {
            return false;
        }
        return pats_rest.iter().zip(segs.iter()).all(|(p, s)| seg_eq(p, s));
    }
    pats.len() == segs.len() && pats.iter().zip(segs.iter()).all(|(p, s)| seg_eq(p, s))
}

/// 单段比较：`*` 只吃一个段内的字符（不跨 `/`）。
fn seg_eq(pattern: &str, seg: &str) -> bool {
    match pattern.split('*').collect::<Vec<_>>().as_slice() {
        [only] => *only == seg,
        parts => {
            let mut rest = seg;
            for (i, chunk) in parts.iter().enumerate() {
                if i == 0 {
                    let Some(after) = rest.strip_prefix(chunk) else {
                        return false;
                    };
                    rest = after;
                    continue;
                }
                let Some(pos) = rest.find(chunk) else {
                    return false;
                };
                if i < parts.len() - 1 && pos + chunk.len() >= rest.len() {
                    // 末段之外的 chunk 后面必须还有字符留给下一个 `*`
                    return false;
                }
                rest = &rest[pos + chunk.len()..];
            }
            true
        }
    }
}
