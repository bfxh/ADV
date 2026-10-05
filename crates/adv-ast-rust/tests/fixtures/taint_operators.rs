//! 夹具④（片A2 补：传播形态覆盖）——变异门实测发现三条 `Rvalue` 传播 arm
//! 删掉也不影响前三条夹具的结论（= 未被任何断言杀），故补这条走
//! 整型源点 → BinaryOp → Cast → `to_string`（propagator）→ 汇点，再加一条经 `Ref` 的汇点。
//! 任何一条 arm 被删，对应行就不报 ⇒ 断言会红。

fn source_int() -> usize {
    7
}

pub fn operators() {
    let n = source_int() + 1;
    let w = n as u64;
    std::process::Command::new(w.to_string());

    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    std::process::Command::new(&raw);
}
