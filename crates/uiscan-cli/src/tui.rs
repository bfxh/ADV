//! `tui-feed` 子命令：读一段终端字节流，用固定尺采集屏幕态。
use std::io::Read;
use std::path::PathBuf;

use crate::bail;
use crate::report::{report, write_dump};

pub fn run(mut args: impl Iterator<Item = String>) {
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
