//! 夹具②（片A2 判据：净化工杀灭 + 反面对照）。
//!
//! 两个函数放同一个文件是**有意的**：
//! - `sanitized`：源点 → `len`（规则里的 sanitizer）→ `to_string` ⇒ 不该报；
//! - `unsanitized`：源点 → `to_string`（不过 sanitizer）⇒ 必须报。
//! 只留第一条的话，`all_sanitizers` 退化成空表、或 `tainted_operand` 恒真，
//! 都还能"零发现"通过——加第二条正反对照，这类变异就会把命中数从 1 顶到 2 而判红。

pub fn sanitized() {
    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    let n = raw.len();
    let s = n.to_string();
    std::process::Command::new(s);
}

pub fn unsanitized() {
    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    let s = raw.to_string();
    std::process::Command::new(s);
}
