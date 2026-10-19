//! 判据纯函数。每条都注了它锚的那次实测，改行为要先改这里的语料。

use std::collections::BTreeSet;

use crate::Rect;

/// 生成器没被解析时，Chromium 会把**令牌原文**当内容返回（实测：`counter(i) ". "`）。
/// 这类不是文案，进判据之前必须掉掉，否则漏翻比对拿 `counter(...)` 当字符串比。
///
/// 实测边界：`attr(data-tip)` 与 `var(--msg)` 返回的是**已解析值**（`attr-文案`、`变量-文案`），
/// 所以这个函数只对"还带着函数括号"的串成立，不是"看到 attr/var 就当噪音"。
pub fn unresolved_generator(s: &str) -> bool {
    let t = s.trim();
    t.contains("counter(") || t.contains("attr(") || t.contains("var(") || t.contains("calc(")
}

/// computed style 的 `content` → 真正的文本；拿不到文本返回 `None`。
///
/// 实测差异（Chromium 150）：`::before`/`::after` 返回**不带引号**的已解析串，
/// `::marker` 返回**带引号**的 `"标记-"`；`none` 与未设置的 `normal` 都是"没有内容"。
pub fn css_content_to_text(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t == "none" || t == "normal" {
        return None;
    }
    if unresolved_generator(t) {
        return None;
    }
    // 分段形式 `content: "A" " · " "B"`：按引号段拼；没有引号段就用整串。
    let mut segs = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in t.chars() {
        match quote {
            Some(q) if c == q => {
                segs.push(std::mem::take(&mut cur));
                quote = None;
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None => {}
        }
    }
    if segs.is_empty() {
        Some(t.to_string())
    } else {
        Some(segs.concat())
    }
}

/// 矩形能不能当"真的在屏上"用。
///
/// 实测（UIA，windows-gnu，2026-10-16）：顶层窗口里有 `(-9999,-9999)-(-9998,-9998)` 的
/// 屏外哨兵值，也有 `(0,0)-(0,0)` 的零尺寸窗口 —— 两种都不是用户看得见的东西。
pub fn usable_rect(r: Rect) -> bool {
    r.w > 0 && r.h > 0 && r.x > -9000 && r.y > -9000
}

/// 像不像一条待翻译文案：有字母、去完空白非空、且不是生成器令牌。
///
/// 刻意**不**用标点判据（`→ ← ✓ ✗` 这类符号在无字体的终端里同样会成豆腐块，
/// 而码点覆盖检查需要它们进候选集）。
pub fn looks_translatable(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().any(|c| c.is_alphabetic()) && !unresolved_generator(t)
}

/// 非 ASCII 码点集合（豆腐块检查的输入）。
pub fn non_ascii(items: impl IntoIterator<Item = String>) -> BTreeSet<char> {
    let mut out = BTreeSet::new();
    for s in items {
        for c in s.chars() {
            if !c.is_ascii() {
                out.insert(c);
            }
        }
    }
    out
}

/// 两条通道的差集（排序去重）。用途不是"谁多谁少"，而是**通道分歧本身就是缺陷信号**：
/// 只有 AX/伪元素通道有 ⇒ DOM 扫描器天生看不见；只有 DOM 有 ⇒ 该串可能被 CSS 覆盖掉了。
pub fn channel_gap(a: &[String], b: &[String]) -> (Vec<String>, Vec<String>) {
    let sa: BTreeSet<String> = a.iter().cloned().collect();
    let sb: BTreeSet<String> = b.iter().cloned().collect();
    (
        sa.difference(&sb).cloned().collect(),
        sb.difference(&sa).cloned().collect(),
    )
}
