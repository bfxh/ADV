//! 输出层：候选条目、通道分布、码点、dump 写出。四个子命令共用同一种打印与落盘。

use std::path::{Path, PathBuf};

use crate::bail;

/// 采集结果落成 JSON（web 的原始载荷除外——那个由 `web --json` 自己写）。
pub fn write_dump(path: &Path, h: &uiscan_core::Harvest) {
    let items: Vec<serde_json::Value> = h
        .items
        .iter()
        .map(|i| {
            serde_json::json!({
                "loc": i.loc, "ch": i.channel.as_str(), "txt": i.text,
                "visible": i.visible,
                "rect": i.rect.map(|r| serde_json::json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h}))
            })
        })
        .collect();
    let doc = serde_json::json!({"surface": h.surface, "items": items});
    if let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default()) {
        bail(&format!("写不了 {}：{e}", path.display()));
    }
    println!("items 已写 {}", path.display());
}

/// 带 dump 的收尾：先写（可选）再打印。
pub fn finish(h: Result<uiscan_core::Harvest, String>, dump: Option<PathBuf>) {
    let h = h.unwrap_or_else(|e| bail(&format!("采集失败：{e}")));
    if let Some(path) = dump {
        write_dump(&path, &h);
    }
    report(&h);
}

/// 两个面共用同一份输出：候选条目 + 通道分布 + 码点。
pub fn report(h: &uiscan_core::Harvest) {
    println!("surface={}", h.surface);
    let mut by_channel: std::collections::BTreeMap<&'static str, usize> = Default::default();
    for it in &h.items {
        *by_channel.entry(it.channel.as_str()).or_default() += 1;
    }
    println!("通道分布：{by_channel:?}");
    for it in h.candidates() {
        println!("{}  {:?}  [{:?}]", it.loc, it.text, it.channel);
    }
    let cps = h.rendered_codepoints();
    println!("非 ASCII 码点 {} 个：{:?}", cps.len(), cps);
}
