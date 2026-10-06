// 合成语料（试验面，登记在 spec/maturity.json）：快轨能命中、深轨命不中的 Rust 形状。
// 目的是让三方账的「仅快轨」桶端到端产出非空值——片A6 只能在单测里喂 Finding 列表，
// 端到端两桶恒空就是 DD-0002。期望值锚到这里的文本，不是把跑出来的数字抄一遍。
pub fn pick(v: Option<u32>) -> u32 {
    v.unwrap()
}

pub fn boom(f: bool) {
    if f {
        panic!("boom");
    }
}
