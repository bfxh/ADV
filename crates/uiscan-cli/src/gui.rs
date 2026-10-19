//! `gui` 子命令：UIA 枚举/采集；`--list` 只列顶层窗口。
//! 参数解析与采集拆开，是为了让 `scan` 复用同一份采集。

use std::path::PathBuf;

use crate::bail;
use crate::report::finish;

pub enum What {
    List,
    Title(String),
    Pid(u32),
    Focused,
}

pub struct Opts {
    pub what: What,
    pub depth: usize,
    pub nodes: usize,
    pub dump: Option<PathBuf>,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Opts {
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
    Opts {
        what: what.unwrap_or_else(|| bail("gui 需要 --list / --title / --pid / --focused 之一")),
        depth,
        nodes,
        dump,
    }
}

pub fn collect(o: &Opts) -> Result<uiscan_core::Harvest, String> {
    match &o.what {
        What::List => Err("--list 只列窗口、不产出采集；用 --title/--pid/--focused".to_string()),
        What::Title(t) => uiscan_gui::harvest_window(t, o.depth, o.nodes),
        What::Pid(p) => uiscan_gui::harvest_pid(*p, o.depth, o.nodes),
        What::Focused => uiscan_gui::harvest_focused(o.depth, o.nodes),
    }
}

pub fn run(args: impl Iterator<Item = String>) {
    let o = parse(args);
    match &o.what {
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
        _ => {
            let r = collect(&o);
            if let Ok(h) = &r {
                if uiscan_gui::chrome_only(h) {
                    eprintln!("⚠ {}", uiscan_gui::CHROME_ONLY_HINT);
                }
            }
            finish(r, o.dump.clone())
        }
    }
}
