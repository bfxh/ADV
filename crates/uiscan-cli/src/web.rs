//! `web` 子命令：起 Edge（headless）走 CDP 采集；`--port` 可连已在监听的目标。
use std::path::PathBuf;
use std::time::Duration;

use crate::bail;
use crate::report::report;

pub fn run(mut args: impl Iterator<Item = String>) {
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
