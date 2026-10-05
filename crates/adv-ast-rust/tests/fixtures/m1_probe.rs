//! 片A1 冒烟夹具：四条可出 MIR 的函数体，其中 `flow` 是一条直接数据流（源→净化→汇），
//! 供驱动输出的函数名清单对账。片A2 的 GenKill 断言将复用本文件。

pub fn source() -> String {
    std::env::var("ADV_DEMO_INPUT").unwrap_or_default()
}

pub fn flow(raw: String) -> usize {
    let cleaned = raw.trim().to_string();
    let len = cleaned.len();
    len
}

pub fn helper(x: i32) -> i32 {
    x + 1
}

pub fn unused_unit() {}
