//! `tofu` 子命令：把采集结果里的可见文本拿去比字体 cmap 覆盖。
use std::path::PathBuf;
use std::process::exit;

use crate::bail;

pub fn run(mut args: impl Iterator<Item = String>) {
    let mut dump: Option<PathBuf> = None;
    let mut fonts: Vec<PathBuf> = Vec::new();
    let mut system = false;
    while let Some(a) = args.next() {
        let mut val = |n: &str| {
            args.next()
                .unwrap_or_else(|| bail(&format!("{n} 后面缺值")))
        };
        match a.as_str() {
            "--json" => dump = Some(PathBuf::from(val("--json"))),
            "--font" => fonts.push(PathBuf::from(val("--font"))),
            "--system" => system = true,
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    let dump = dump.unwrap_or_else(|| bail("tofu 需要 --json <采集输出文件>"));
    let raw = std::fs::read_to_string(&dump)
        .unwrap_or_else(|e| bail(&format!("读不了 {}：{e}", dump.display())));
    let v: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| bail(&format!("不是 JSON：{e}")));
    let items = v["items"]
        .as_array()
        .unwrap_or_else(|| bail("JSON 里没有 items 数组（先用 web/gui 的 --json 产出）"));
    // 只看可见条目：不可见的文案不会有豆腐块，报它只会制造噪音。
    let chars: Vec<char> = items
        .iter()
        .filter(|i| i["visible"].as_bool().unwrap_or(true))
        .filter_map(|i| i["txt"].as_str())
        .flat_map(|s| s.chars())
        .collect();

    let mut set = uiscan_font::FontSet::new();
    for f in &fonts {
        let n = set.add_file(f).unwrap_or_else(|e| bail(&e));
        println!("字体 {}：{n} 个面", f.display());
    }
    if system {
        let files = uiscan_font::system_font_files();
        let mut ok = 0;
        for f in &files {
            if set.add_file(f).is_ok() {
                ok += 1;
            }
        }
        println!("系统字体：载入 {ok}/{} 个文件", files.len());
    }
    if set.face_count() == 0 {
        bail("没有可用字体：给 --font <文件> 或 --system（工具不替你猜应用的字体栈）");
    }

    let missing = set.missing(chars);
    println!(
        "surface={} 字体面={} 缺字={}",
        v["surface"].as_str().unwrap_or("(dump)"),
        set.face_count(),
        missing.len()
    );
    for c in &missing {
        println!("  U+{:04X} {:?}", *c as u32, c);
    }
    if !missing.is_empty() {
        exit(1);
    }
}
