//! `locale` 子命令：把采集到的可见文案与语言包对账，列出找不到的（疑似硬编码/未翻译）。
//!
//! 退出码：0=全部命中；1=有找不到的；2=用法/环境错。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::exit;

use serde_json::Value;

use crate::bail;

pub fn run(mut args: impl Iterator<Item = String>) {
    let mut dump: Option<PathBuf> = None;
    let mut locales: Vec<PathBuf> = Vec::new();
    while let Some(a) = args.next() {
        let mut val = |n: &str| {
            args.next()
                .unwrap_or_else(|| bail(&format!("{n} 后面缺值")))
        };
        match a.as_str() {
            "--json" => dump = Some(PathBuf::from(val("--json"))),
            "--locale" => locales.push(PathBuf::from(val("--locale"))),
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    let dump = dump.unwrap_or_else(|| bail("locale 需要 --json <采集 dump>"));
    if locales.is_empty() {
        bail("locale 需要至少一个 --locale <语言包 json>");
    }

    let raw = std::fs::read_to_string(&dump)
        .unwrap_or_else(|e| bail(&format!("读不了 {}：{e}", dump.display())));
    let v: Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| bail(&format!("dump 不是 JSON：{e}")));
    let items = v["items"]
        .as_array()
        .unwrap_or_else(|| bail("dump 里没有 items 数组"));

    // 只对"可见 + 像文案"的条目做对账（与 tofu/clip 同一口径）。
    let mut values = BTreeSet::new();
    for p in &locales {
        let text = std::fs::read_to_string(p)
            .unwrap_or_else(|e| bail(&format!("读不了 {}：{e}", p.display())));
        let json: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| bail(&format!("{} 不是 JSON：{e}", p.display())));
        crate::locale::locale_values(&json, &mut values);
    }

    let candidates: Vec<String> = items
        .iter()
        .filter(|i| i["visible"].as_bool().unwrap_or(true))
        .filter_map(|i| i["txt"].as_str())
        .filter(|t| uiscan_core::looks_translatable(t))
        .map(str::to_string)
        .collect();
    let missing = crate::locale::unlocalized(candidates.iter().map(String::as_str), &values);

    println!(
        "语言包值 {} 条；候选文案 {} 条；找不到 {} 条",
        values.len(),
        candidates.len(),
        missing.len()
    );
    for m in &missing {
        println!("  {m:?}");
    }
    if !missing.is_empty() {
        exit(1);
    }
}
