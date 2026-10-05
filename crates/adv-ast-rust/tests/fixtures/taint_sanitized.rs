//! 夹具②（片A2 判据：净化工杀灭）：源点之后先过 `len`（规则里的 sanitizer）再
//! `to_string`（propagator，但输入已无污点）⇒ 必须**零发现**。
//! 这条是假阳性尺：只要把 sanitizer 当成传播子，这里就会报。

pub fn sanitized() {
    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    let n = raw.len();
    let s = n.to_string();
    std::process::Command::new(s);
}
