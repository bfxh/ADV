//! 夹具①（片A2 判据：直接流）：源点 `std::env::var` 经声明过的 propagator
//! (`unwrap_or_default`) 传到汇点实参 ⇒ 必须报 1 条。

pub fn direct() {
    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    std::process::Command::new(raw);
}
