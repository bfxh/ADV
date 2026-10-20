//! `uiscan` —— 把"屏幕上真的出现了哪些文案"抓出来。
//!
//! 已接：`tui-feed`（vt100 采集）、`web`（Edge headless + CDP 直连，无 WebDriver）。
//! 未接：`gui`（UIA）—— 可行性在本机证过，适配器还没写，传了就退 2 并点名，不给空结果假装成功。

use std::process::exit;

mod clip;
mod gui;
mod locale;
mod locale_cmd;
mod report;
mod scan;
mod tofu;
mod tui;
mod web;

const USAGE: &str = "用法：
  uiscan tui-feed [--file <终端字节捕获，缺省读 stdin>] [--rows N=24] [--cols N=80] [--json <输出文件>]
  uiscan web <url|file:///路径> [--window 1280x900] [--wait-secs 25] [--profile <目录>] [--port N] [--json <输出文件>]
  uiscan gui --list
  uiscan gui (--title <子串> | --pid N | --focused) [--depth 12] [--nodes 20000] [--json <输出文件>]
  uiscan tofu --json <采集输出文件> (--font <字体文件>... | --system)
  uiscan scan --surface tui|web|gui <该面参数...> (--font <字体文件>... | --system) [--json <输出文件>]
  uiscan clip --json <web --json 的原始载荷>
  uiscan locale --json <采集 dump> --locale <语言包 json> [--locale ...]";

pub(crate) fn bail(msg: &str) -> ! {
    eprintln!("{msg}\n{USAGE}");
    exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        bail("缺子命令")
    };
    match cmd.as_str() {
        "tui-feed" => tui::run(args),
        "web" => web::run(args),
        "gui" => gui::run(args),
        "tofu" => tofu::run(args),
        "scan" => scan::run(args),
        "clip" => clip::run(args),
        "locale" => locale_cmd::run(args),
        other => bail(&format!("未接的面：{other}（GUI 的 UIA 适配器还没写）")),
    }
}
