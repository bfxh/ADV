//! `tui-feed` 子命令：读一段终端字节流，用固定尺采集屏幕态。
//! 参数解析与采集拆开，是为了让 `scan` 复用同一份采集（一条命令出结论）。

use std::io::Read;
use std::path::PathBuf;

use crate::bail;
use crate::report::{report, write_dump};

pub struct Opts {
    pub file: Option<String>,
    pub rows: u16,
    pub cols: u16,
    pub dump: Option<PathBuf>,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Opts {
    let mut o = Opts {
        file: None,
        rows: 24,
        cols: 80,
        dump: None,
    };
    while let Some(a) = args.next() {
        let mut num = |name: &str| -> u16 {
            args.next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| bail(&format!("{name} 要正整数")))
        };
        match a.as_str() {
            "--file" => o.file = Some(args.next().unwrap_or_else(|| bail("--file 后面缺值"))),
            "--rows" => o.rows = num("--rows"),
            "--cols" => o.cols = num("--cols"),
            "--json" => {
                o.dump = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| bail("--json 后面缺值")),
                ))
            }
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    o
}

pub fn collect(o: &Opts) -> Result<uiscan_core::Harvest, String> {
    let bytes = match &o.file {
        Some(p) => std::fs::read(p).map_err(|e| format!("读不了 {p}：{e}"))?,
        None => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("读 stdin 失败：{e}"))?;
            buf
        }
    };
    Ok(uiscan_tui::harvest(&bytes, o.rows, o.cols))
}

pub fn run(args: impl Iterator<Item = String>) {
    let o = parse(args);
    let h = collect(&o).unwrap_or_else(|e| bail(&e));
    if let Some(path) = &o.dump {
        write_dump(path, &h);
    }
    report(&h);
}
