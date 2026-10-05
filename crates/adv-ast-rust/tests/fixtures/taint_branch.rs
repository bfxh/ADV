//! 夹具③（片A2 判据：跨基本块 / 合流点）：源点写在 `if` 的 then 块里，汇点在合并块
//! 之后 ⇒ 前向到达必须在合流点把污点并进来才能报。这条抓的是"只看单基本块"的实现。

pub fn branch(flag: bool) {
    let mut cmd = String::from("ls");
    if flag {
        cmd = std::env::var("ADV_INPUT").unwrap_or_default();
    }
    std::process::Command::new(cmd);
}
