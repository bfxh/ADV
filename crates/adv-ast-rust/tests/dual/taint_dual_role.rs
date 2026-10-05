//! 夹具⑤（优先级钉桩）：`to_string` 在 `rules-dual/taint-dual.yaml` 里**同时**是
//! propagator 和 sanitizer ⇒ 按"净化工优先"应当零发现。
//! 变异门实测：`all_sanitizers` 退化成空表时这里会翻成报一条。

pub fn dual_role() {
    let raw = std::env::var("ADV_INPUT").unwrap_or_default();
    let s = raw.to_string();
    std::process::Command::new(s);
}
