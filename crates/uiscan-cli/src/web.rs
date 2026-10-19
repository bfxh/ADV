//! `web` 子命令：起 Edge（headless）走 CDP 采集；`--port` 可连已在监听的目标。
//! 参数解析与采集拆开，是为了让 `scan` 复用同一份采集。

use std::path::PathBuf;
use std::time::Duration;

use crate::bail;
use crate::report::report;

pub struct Opts {
    pub url: String,
    pub window: String,
    pub wait_secs: u64,
    pub profile: PathBuf,
    pub attach_port: Option<u16>,
    /// 原始载荷落盘（`web --json` 专用：scan 走归一化 dump，不用它）。
    pub raw_dump: Option<PathBuf>,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Opts {
    let url = args.next().unwrap_or_else(|| bail("web 需要一个 URL"));
    let mut o = Opts {
        url,
        window: "1280x900".to_string(),
        wait_secs: 25,
        // 默认落在系统 temp；`--profile` 是给"我要指到某个盘"留的口（MSYS 的 /e/tmp
        // 这种路径 Windows 程序不认，Edge 会静默起不来）。
        profile: std::env::temp_dir().join(format!("uiscan-edge-{}", std::process::id())),
        attach_port: None,
        raw_dump: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--window" => o.window = args.next().unwrap_or_else(|| bail("--window 后面缺值")),
            "--profile" => {
                o.profile = PathBuf::from(args.next().unwrap_or_else(|| bail("--profile 后面缺值")))
            }
            "--json" => {
                o.raw_dump = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| bail("--json 后面缺值")),
                ))
            }
            "--port" => {
                o.attach_port = Some(
                    args.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| bail("--port 要端口号")),
                )
            }
            "--wait-secs" => {
                o.wait_secs = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| bail("--wait-secs 要正整数"))
            }
            other => bail(&format!("不认得的参数：{other}")),
        }
    }
    o
}

fn window_size(window: &str) -> String {
    let (w, h) = window
        .split_once('x')
        .unwrap_or_else(|| bail("--window 要写成 宽x高，例如 1280x900"));
    format!(
        "{},{}",
        w.parse::<u32>()
            .unwrap_or_else(|_| bail("--window 的宽不是整数")),
        h.parse::<u32>()
            .unwrap_or_else(|_| bail("--window 的高不是整数"))
    )
}

/// 返回（归一条目，原始载荷）。原始载荷给 `web --json` 用；scan 只用条目。
pub fn collect(o: &Opts) -> Result<(uiscan_core::Harvest, serde_json::Value), String> {
    let size = window_size(&o.window);
    let mut session = match o.attach_port {
        Some(port) => uiscan_web::cdp::attach(port, &o.url, Duration::from_secs(o.wait_secs)),
        None => {
            let log = o.profile.join("edge-cdp.log");
            uiscan_web::cdp::open(
                &o.url,
                &o.profile,
                &size,
                Duration::from_secs(o.wait_secs),
                &log,
            )
        }
    }
    .map_err(|e| format!("CDP 通道没成：{e}"))?;
    session
        .wait_document_ready(Duration::from_secs(20))
        .map_err(|e| format!("页面没就绪：{e}"))?;
    let payload = session
        .evaluate(uiscan_web::HARVEST_JS)
        .map_err(|e| format!("页面里采集失败：{e}"))?;
    let items = uiscan_web::harvest_payload_to_items(&payload);
    let h = uiscan_core::Harvest {
        surface: format!("web:{} viewport={:?}", o.url, payload["viewport"]),
        items,
    };
    Ok((h, payload))
}

pub fn run(args: impl Iterator<Item = String>) {
    let o = parse(args);
    let (h, payload) = collect(&o).unwrap_or_else(|e| bail(&e));
    if let Some(path) = &o.raw_dump {
        if let Err(e) = std::fs::write(
            path,
            serde_json::to_string_pretty(&payload).unwrap_or_default(),
        ) {
            bail(&format!("写不了 {}：{e}", path.display()));
        }
        println!("payload 已写 {}", path.display());
    }
    report(&h);
}
