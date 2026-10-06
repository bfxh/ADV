//! 合成语料①：跨模块 + 直接流 + 一条 const 上下文体。
//!
//! const 项（`OFFSET`）是给 `mir_body()` 的 `mir_for_ctfe` 分支用的——片B1 自记过那条臂
//! 没有覆盖（它是编 adv-core 时才炸出来的），这份夹具让它在门里被反复跑到。
mod inner;

/// 每条流的形态都写成 `let raw = 源点 …; 汇点(raw)`，好让期望值能锚回本文件文本。
pub fn direct_from_env() {
    let raw = std::env::var("TAINT_INPUT").unwrap_or_default();
    std::process::Command::new(raw); // adv-expect: hit
}

pub fn via_propagator() {
    let raw = std::env::var("TAINT_INPUT").unwrap_or_default().to_string();
    std::process::Command::new(raw); // adv-expect: hit
}

/// 不该报：`len` 在产品规则里是 sanitizer，之后的值不再带污点。
pub fn sanitized() {
    let n = std::env::var("TAINT_INPUT").unwrap_or_default().len();
    std::process::Command::new(n.to_string()); // adv-expect: miss（len 是 sanitizer）
}

/// const 上下文：`optimized_mir` 对它会 panic（片B1 实测炸在 adv-core），必须走 ctfe 查询。
pub const OFFSET: usize = 40 + 2;
