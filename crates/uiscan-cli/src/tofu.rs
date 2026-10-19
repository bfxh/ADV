//! `tofu` 子命令：把采集结果里的可见文本拿去比字体 cmap 覆盖。
//! `scan` 复用这里的 [`check`]（同一条判据，两条入口）。

use std::path::{Path, PathBuf};
use std::process::exit;

use uiscan_core::{Channel, Harvest, Item, Rect};

use crate::bail;

pub struct Opts {
    pub dump: PathBuf,
    pub fonts: Vec<PathBuf>,
    pub system: bool,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Opts {
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
    Opts {
        dump: dump.unwrap_or_else(|| bail("tofu 需要 --json <采集输出文件>")),
        fonts,
        system,
    }
}

/// 读采集落盘（`tui-feed --json` / `gui --json` 的归一化格式，或 `web --json` 的原始载荷）。
/// 两种形状都认：`visible` 缺省为 true、`ch` 不认得就整条丢。
pub fn load_dump(path: &Path) -> Result<Harvest, String> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| format!("读不了 {}：{e}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| format!("不是 JSON：{e}"))?;
    let items = v["items"]
        .as_array()
        .ok_or("JSON 里没有 items 数组（先用 web/gui/tui-feed 的 --json 产出）")?;
    let mut out = Vec::new();
    for it in items {
        let Some(ch) = it["ch"].as_str().and_then(Channel::from_wire) else {
            continue;
        };
        let Some(txt) = it["txt"].as_str() else {
            continue;
        };
        if txt.trim().is_empty() {
            continue;
        }
        let r = &it["rect"];
        let rect = (r.is_object()).then(|| Rect {
            x: r["x"].as_i64().unwrap_or(0) as i32,
            y: r["y"].as_i64().unwrap_or(0) as i32,
            w: r["w"].as_i64().unwrap_or(0) as i32,
            h: r["h"].as_i64().unwrap_or(0) as i32,
        });
        let mut item = Item::new(txt.to_string(), ch, it["loc"].as_str().unwrap_or(""));
        item.visible = it["visible"].as_bool().unwrap_or(true);
        item.rect = rect;
        out.push(item);
    }
    Ok(Harvest {
        surface: v["surface"].as_str().unwrap_or("(dump)").to_string(),
        items: out,
    })
}

/// 判缺字：返回（字体面数，缺字码点）。字体栈由调用方给——工具不替应用猜。
pub fn check(h: &Harvest, fonts: &[PathBuf], system: bool) -> Result<(usize, Vec<char>), String> {
    let mut set = uiscan_font::FontSet::new();
    for f in fonts {
        let n = set.add_file(f)?;
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
        return Err(
            "没有可用字体：给 --font <文件> 或 --system（工具不替你猜应用的字体栈）".to_string(),
        );
    }
    Ok((set.face_count(), set.missing(h.rendered_codepoints())))
}

pub fn report(h: &Harvest, faces: usize, missing: &[char]) {
    println!(
        "surface={} 字体面={} 缺字={}",
        h.surface,
        faces,
        missing.len()
    );
    for c in missing {
        println!("  U+{:04X} {:?}", *c as u32, c);
    }
}

pub fn run(args: impl Iterator<Item = String>) {
    let o = parse(args);
    let h = load_dump(&o.dump).unwrap_or_else(|e| bail(&e));
    let (faces, missing) = check(&h, &o.fonts, o.system).unwrap_or_else(|e| bail(&e));
    report(&h, faces, &missing);
    if !missing.is_empty() {
        exit(1);
    }
}
