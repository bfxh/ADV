//! 门必须留痕：任何一条执行路径的终点都要有一行 `<门>: <裁决>`。
//!
//! 由来是 2026-10-06 的实测两次打脸：`mutants` 把 cargo-mutants 的 exit 3（超时）当成失败，
//! 于是整道门只留下 anyhow 的 `Error: cargo mutants 没跑完…`，**没有任何裁决行**；我把
//! "自己在 outcomes.json 上复算的数"当成门的结论记进账本，连着两轮没发现门是哑的。
//! 同一天还兑现过一次：门接进了不守这条分支的 workflow，红与不跑在证据上长得一样。
//!
//! 所以把三态钉成契约：**绿 / 红（N 条）/ 判不了（原因）**，且"判不了"用独立退出码（3），
//! 让任何包装器都能把它与红区分开——不许静默降级成"看起来没坏"。

/// 三态裁决：`(要打印的行, 退出码)`。退出码：0 绿 / 1 红 / 3 判不了。
pub fn verdict(name: &str, outcome: &anyhow::Result<Vec<String>>) -> (String, i32) {
    match outcome {
        Ok(violations) if violations.is_empty() => (format!("{name}: 绿"), 0),
        Ok(violations) => {
            let mut lines = vec![format!("{name}: 红（{} 条）", violations.len())];
            lines.extend(violations.iter().map(|v| format!("  - {v}")));
            (lines.join("\n"), 1)
        }
        // 关键的一条：错误不是"没有裁决"，而是**一种裁决**。文案必须自带"没判"字样与原因，
        // 否则读日志的人会像我在 2026-10-06 那样，把别处的数字当成门说的话。
        Err(e) => {
            let why = format!("{e:#}").replace("\n", " | ");
            (
                format!("{name}: 判不了（本轮没出判定，别把别的数当结论）：{why}"),
                3,
            )
        }
    }
}

/// `gate` 档的聚合点：把各子门的明细合成一档清单（带子门名前缀，便于定位）。
///
/// 为什么要从 `gate_run` 里抽出来（与片A6 抽 `three_way_buckets` 同一手法）：`gate_run`
/// 整体被换成 `Ok(vec![])` 在**全绿的树**上与真值同值，端到端断言区分不了；"怎么合"这件事
/// 只有直接喂数据才可观察。壳的那条残留另立债（DD-0011），不假装已经杀掉。
pub fn merge_gate(parts: &[(&str, Vec<String>)]) -> Vec<String> {
    let mut out = Vec::new();
    for (name, violations) in parts {
        out.extend(violations.iter().map(|v| format!("{name}: {v}")));
    }
    out
}
