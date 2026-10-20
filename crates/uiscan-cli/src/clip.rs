//! `clip` 子命令：对 `web --json` 的原始载荷判"文字是否被容器裁掉"。
//!
//! 需要原始载荷（含 scrollWidth/clientWidth/overflow 这些几何量），归一化 dump 里没有。
//! 退出码：0=没发现裁切；1=发现裁切；2=用法/环境错。

use std::path::PathBuf;
use std::process::exit;

use crate::bail;

pub fn run(mut args: impl Iterator<Item = String>) {
    let mut dump: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => {
                dump = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| bail("--json 后面缺值")),
                ))
            }
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    let dump = dump.unwrap_or_else(|| bail("clip 需要 --json <web --json 的原始载荷>"));
    let raw = std::fs::read_to_string(&dump)
        .unwrap_or_else(|e| bail(&format!("读不了 {}：{e}", dump.display())));
    let v: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| bail(&format!("不是 JSON：{e}")));
    if !uiscan_web::clip::has_geometry(&v) {
        bail(
            "载荷里没有几何字段（sw/cw/sh/chh）：用新版 `web --json` 重采；缺字段时 0 条不等于没有裁切",
        );
    }
    let found = uiscan_web::clip::findings(&v);
    println!(
        "page={:?} 截断={}",
        v["title"].as_str().unwrap_or("(无 title)"),
        found.len()
    );
    for f in &found {
        println!(
            "  {:?} {} {:?} sw/cw={}/{} sh/ch={}/{}",
            f.kind, f.loc, f.text, f.sw, f.cw, f.sh, f.chh
        );
    }
    if !found.is_empty() {
        exit(1);
    }
}
