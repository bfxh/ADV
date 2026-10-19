//! `uiscan` —— 把"屏幕上真的出现了哪些文案"抓出来。
//!
//! 已接：`tui-feed`（vt100 采集）、`web`（Edge headless + CDP 直连，无 WebDriver）。
//! 未接：`gui`（UIA）—— 可行性在本机证过，适配器还没写，传了就退 2 并点名，不给空结果假装成功。

use std::io::Read;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

const USAGE: &str = "用法：
  uiscan tui-feed [--file <终端字节捕获，缺省读 stdin>] [--rows N=24] [--cols N=80] [--json <输出文件>]
  uiscan web <url|file:///路径> [--window 1280x900] [--wait-secs 25] [--profile <目录>] [--port N] [--json <输出文件>]
  uiscan gui --list
  uiscan gui (--title <子串> | --pid N | --focused) [--depth 12] [--nodes 20000] [--json <输出文件>]
  uiscan tofu --json <采集输出文件> (--font <字体文件>... | --system)";

fn bail(msg: &str) -> ! {
    eprintln!("{msg}\n{USAGE}");
    exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        bail("缺子命令")
    };
    match cmd.as_str() {
        "tui-feed" => tui_feed(args),
        "web" => web(args),
        "gui" => gui(args),
        "tofu" => tofu(args),
        other => bail(&format!("未接的面：{other}（GUI 的 UIA 适配器还没写）")),
    }
}

fn tui_feed(mut args: impl Iterator<Item = String>) {
    let mut file: Option<String> = None;
    let mut rows: u16 = 24;
    let mut cols: u16 = 80;
    let mut dump: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        let mut num = |name: &str| -> u16 {
            args.next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| bail(&format!("{name} 要正整数")))
        };
        match a.as_str() {
            "--file" => file = Some(args.next().unwrap_or_else(|| bail("--file 后面缺值"))),
            "--rows" => rows = num("--rows"),
            "--cols" => cols = num("--cols"),
            "--json" => {
                dump = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| bail("--json 后面缺值")),
                ))
            }
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    let bytes = match file {
        Some(p) => std::fs::read(&p).unwrap_or_else(|e| bail(&format!("读不了 {p}：{e}"))),
        None => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .unwrap_or_else(|e| bail(&format!("读 stdin 失败：{e}")));
            buf
        }
    };
    let h = uiscan_tui::harvest(&bytes, rows, cols);
    if let Some(path) = dump {
        write_dump(&path, &h);
    }
    report(&h);
}

fn web(mut args: impl Iterator<Item = String>) {
    let url = args.next().unwrap_or_else(|| bail("web 需要一个 URL"));
    let mut window = "1280x900".to_string();
    let mut wait_secs = 25u64;
    let mut dump_json: Option<PathBuf> = None;
    // 默认落在系统 temp；`--profile` 是给"我要指到某个盘"留的口（MSYS 的 /e/tmp
    // 这种路径 Windows 程序不认，Edge 会静默起不来）。
    let mut profile = std::env::temp_dir().join(format!("uiscan-edge-{}", std::process::id()));
    // `--port` = 连一个别人已经起好的调试端口（不启动、也不杀它）
    let mut attach_port: Option<u16> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--window" => window = args.next().unwrap_or_else(|| bail("--window 后面缺值")),
            "--profile" => {
                profile = PathBuf::from(args.next().unwrap_or_else(|| bail("--profile 后面缺值")))
            }
            "--json" => {
                dump_json = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| bail("--json 后面缺值")),
                ))
            }
            "--port" => {
                attach_port = Some(
                    args.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| bail("--port 要端口号")),
                )
            }
            "--wait-secs" => {
                wait_secs = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| bail("--wait-secs 要正整数"))
            }
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    let (w, h) = window
        .split_once('x')
        .unwrap_or_else(|| bail("--window 要写成 宽x高，例如 1280x900"));
    let window_size = format!(
        "{},{}",
        w.parse::<u32>()
            .unwrap_or_else(|_| bail("--window 的宽不是整数")),
        h.parse::<u32>()
            .unwrap_or_else(|_| bail("--window 的高不是整数"))
    );

    let mut session = match attach_port {
        Some(port) => uiscan_web::cdp::attach(port, &url, Duration::from_secs(wait_secs)),
        None => {
            let log = profile.join("edge-cdp.log");
            uiscan_web::cdp::open(
                &url,
                &profile,
                &window_size,
                Duration::from_secs(wait_secs),
                &log,
            )
        }
    }
    .unwrap_or_else(|e| bail(&format!("CDP 通道没成：{e}")));
    if let Err(e) = session.wait_document_ready(Duration::from_secs(20)) {
        bail(&format!("页面没就绪：{e}"));
    }
    let payload = match session.evaluate(uiscan_web::HARVEST_JS) {
        Ok(v) => v,
        Err(e) => bail(&format!("页面里采集失败：{e}")),
    };
    if let Some(path) = dump_json {
        if let Err(e) = std::fs::write(
            &path,
            serde_json::to_string_pretty(&payload).unwrap_or_default(),
        ) {
            bail(&format!("写不了 {}：{e}", path.display()));
        }
        println!("payload 已写 {}", path.display());
    }
    let items = uiscan_web::harvest_payload_to_items(&payload);
    report(&uiscan_core::Harvest {
        surface: format!("web:{url} viewport={:?}", payload["viewport"]),
        items,
    });
}

/// 两个面共用同一份输出：候选条目 + 通道分布 + 码点。
fn report(h: &uiscan_core::Harvest) {
    println!("surface={}", h.surface);
    let mut by_channel: std::collections::BTreeMap<&'static str, usize> = Default::default();
    for it in &h.items {
        *by_channel.entry(channel_name(it.channel)).or_default() += 1;
    }
    println!("通道分布：{by_channel:?}");
    for it in h.candidates() {
        println!("{}  {:?}  [{:?}]", it.loc, it.text, it.channel);
    }
    let cps = h.rendered_codepoints();
    println!("非 ASCII 码点 {} 个：{:?}", cps.len(), cps);
}

/// 豆腐块判据：把采集结果里的可见文本，拿去和**调用方给的字体栈**比 cmap 覆盖。
///
/// 退出码：0 = 全有覆盖；1 = 发现缺字（这是判据成立的样子，不是工具坏了）；2 = 用法/环境错。
fn tofu(mut args: impl Iterator<Item = String>) {
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

fn gui(mut args: impl Iterator<Item = String>) {
    enum What {
        List,
        Title(String),
        Pid(u32),
        Focused,
    }
    let mut what: Option<What> = None;
    let mut depth = 12usize;
    let mut nodes = 20_000usize;
    let mut dump: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        let mut val = |name: &str| {
            args.next()
                .unwrap_or_else(|| bail(&format!("{name} 后面缺值")))
        };
        match a.as_str() {
            "--list" => what = Some(What::List),
            "--title" => what = Some(What::Title(val("--title"))),
            "--pid" => {
                what = Some(What::Pid(
                    val("--pid")
                        .parse()
                        .unwrap_or_else(|_| bail("--pid 要数字")),
                ))
            }
            "--focused" => what = Some(What::Focused),
            "--depth" => {
                depth = val("--depth")
                    .parse()
                    .unwrap_or_else(|_| bail("--depth 要正整数"))
            }
            "--nodes" => {
                nodes = val("--nodes")
                    .parse()
                    .unwrap_or_else(|_| bail("--nodes 要正整数"))
            }
            "--json" => dump = Some(PathBuf::from(val("--json"))),
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    match what.unwrap_or_else(|| bail("gui 需要 --list / --title / --pid / --focused 之一")) {
        What::List => {
            let wins =
                uiscan_gui::list_windows().unwrap_or_else(|e| bail(&format!("枚举窗口失败：{e}")));
            for w in wins {
                println!(
                    "pid={:<6} rect={:?} offscreen={:<5} class={:<24} name={:?}",
                    w.pid, w.rect, w.offscreen, w.class, w.name
                );
            }
        }
        What::Title(t) => finish(uiscan_gui::harvest_window(&t, depth, nodes), dump),
        What::Pid(p) => finish(uiscan_gui::harvest_pid(p, depth, nodes), dump),
        What::Focused => finish(uiscan_gui::harvest_focused(depth, nodes), dump),
    }
}

fn write_dump(path: &std::path::Path, h: &uiscan_core::Harvest) {
    let items: Vec<serde_json::Value> = h
        .items
        .iter()
        .map(|i| {
            serde_json::json!({
                "loc": i.loc, "ch": channel_name(i.channel), "txt": i.text,
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

fn finish(h: Result<uiscan_core::Harvest, String>, dump: Option<PathBuf>) {
    let h = h.unwrap_or_else(|e| bail(&format!("采集失败：{e}")));
    if let Some(path) = dump {
        write_dump(&path, &h);
    }
    report(&h);
}

fn channel_name(c: uiscan_core::Channel) -> &'static str {
    match c {
        uiscan_core::Channel::DomText => "DomText",
        uiscan_core::Channel::PseudoBefore => "PseudoBefore",
        uiscan_core::Channel::PseudoAfter => "PseudoAfter",
        uiscan_core::Channel::Marker => "Marker",
        uiscan_core::Channel::PlaceholderAttr => "PlaceholderAttr",
        uiscan_core::Channel::TitleAttr => "TitleAttr",
        uiscan_core::Channel::TermCell => "TermCell",
        uiscan_core::Channel::UiaName => "UiaName",
    }
}
