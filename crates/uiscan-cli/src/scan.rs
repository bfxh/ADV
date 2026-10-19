//! `scan`：一条命令走完"采集 → 判豆腐块"。
//!
//! 它不复制任何采集逻辑：把剩余参数原样交给对应面的 `parse/collect`，
//! 与 `tui-feed` / `web` / `gui` 走**同一份**代码（两条入口，一条判据）。
//! 退出码与 `tofu` 一致：0=全有覆盖、1=发现缺字、2=用法或环境错。

use std::path::PathBuf;
use std::process::exit;

use crate::bail;
use crate::report::{report, write_dump};
use crate::{gui, tofu, tui, web};

pub fn run(mut args: impl Iterator<Item = String>) {
    let mut surface: Option<String> = None;
    let mut fonts: Vec<PathBuf> = Vec::new();
    let mut system = false;
    let mut dump: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        let mut val = |n: &str| {
            args.next()
                .unwrap_or_else(|| bail(&format!("{n} 后面缺值")))
        };
        match a.as_str() {
            "--surface" => surface = Some(val("--surface")),
            "--font" => fonts.push(PathBuf::from(val("--font"))),
            "--system" => system = true,
            "--json" => dump = Some(PathBuf::from(val("--json"))),
            // 其余原样交给面自己的解析器；不认得的它会在那边报错并点名。
            other => rest.push(other.to_string()),
        }
    }
    let surface = surface.unwrap_or_else(|| bail("scan 需要 --surface tui|web|gui"));
    let h = match surface.as_str() {
        "tui" => {
            let o = tui::parse(rest.into_iter());
            tui::collect(&o)
        }
        "web" => {
            let o = web::parse(rest.into_iter());
            web::collect(&o).map(|(h, _payload)| h)
        }
        "gui" => {
            let o = gui::parse(rest.into_iter());
            gui::collect(&o)
        }
        other => bail(&format!("不认得的 surface：{other}（tui|web|gui）")),
    }
    .unwrap_or_else(|e| bail(&e));

    if let Some(path) = &dump {
        write_dump(path, &h);
    }
    report(&h);
    // 空采集 ≠ 干净：页面没渲染/被拦/选错面都会得到 0 条，必须判"判不了"而不是报全绿。
    if h.items.is_empty() {
        bail(&format!(
            "采集到 0 条：{} ⇒ 这不等于'没有缺陷'（页面没渲染、被风控/登录拦下、或选错了面）",
            h.surface
        ));
    }
    let (faces, missing) = tofu::check(&h, &fonts, system).unwrap_or_else(|e| bail(&e));
    tofu::report(&h, faces, &missing);
    if !missing.is_empty() {
        exit(1);
    }
}
