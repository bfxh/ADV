//! 最小 CDP 通道：开 Edge（headless）→ 等调试端口 → 在页面里跑一段 JS → 拿回值 → 关掉。
//!
//! 为什么不走 WebDriver：本机装了 Edge 155.0.4283.45（`C:\Program Files (x86)\Microsoft\Edge\Application`，
//! 不在 PATH）但**没有 `msedgedriver`**；`--headless=new --remote-debugging-port` 起来后的
//! `/json/version`、`/json/list` 已在本机用 curl 实测可用（返回 `Edg/155.0.4283.45` 与 `webSocketDebuggerUrl`）。
//! 少一个要下载、要版本对齐的外部二进制，就少一类"本机过≠下次还能过"的故障。

use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tungstenite::{Message, WebSocket};

const DEFAULT_EDGE: &str = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";

/// 一次会话。`child` 只有我们自己启动浏览器时才有；attach 到别人起好的端口时**不许**杀它。
pub struct EdgeSession {
    child: Option<Child>,
    ws: WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl Drop for EdgeSession {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// 读 `http://127.0.0.1:<port><path>` 的 JSON。
///
/// 🔴 这里**不能用 `read_to_end`**：DevTools 的 HTTP 端点不会因为我们要求关闭就关连接，
/// 等 EOF 会一路撞到读超时（本机实测：同一端口 curl 秒回，`read_to_end` 报 10060）。
/// 正确做法是读够就走：按 `Content-Length` 补齐，或按 chunked 的 0 长度终止符收。
fn http_json(port: u16, path: &str) -> Result<Value, String> {
    let mut s =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("连不上 {port}：{e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: */*\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    s.flush().map_err(|e| e.to_string())?;

    let mut buf: Vec<u8> = Vec::new();
    let mut piece = [0u8; 4096];
    let split = loop {
        let n = s.read(&mut piece).map_err(|e| format!("读失败：{e}"))?;
        if n == 0 {
            return Err("对端在头部之前就关了".to_string());
        }
        buf.extend_from_slice(&piece[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 64 * 1024 {
            return Err("头部超 64KB，不像 CDP 响应".to_string());
        }
    };
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let mut body = buf[split + 4..].to_vec();

    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        loop {
            if let Some(text) = dechunk(&body) {
                body = text.into_bytes();
                break;
            }
            let n = s
                .read(&mut piece)
                .map_err(|e| format!("读 chunk 失败：{e}"))?;
            if n == 0 {
                return Err("chunked 响应没结束就断了".to_string());
            }
            body.extend_from_slice(&piece[..n]);
        }
    } else {
        let want = head
            .lines()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<usize>().ok())
            })
            .ok_or("响应既不是 chunked 也没有 Content-Length")?;
        while body.len() < want {
            let n = s
                .read(&mut piece)
                .map_err(|e| format!("读 body 失败：{e}"))?;
            if n == 0 {
                return Err(format!("响应断了：只读到 {}/{}", body.len(), want));
            }
            body.extend_from_slice(&piece[..n]);
        }
        body.truncate(want);
    }
    serde_json::from_slice(&body).map_err(|e| format!("JSON 解析失败：{e}"))
}

/// chunked 解码；没收到终止符就返回 `None`，让调用方继续读。
fn dechunk(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body).into_owned();
    let mut out = String::new();
    let mut rest = text.as_str();
    loop {
        let (line, tail) = rest.split_once("\r\n")?;
        let n = usize::from_str_radix(line.trim(), 16).ok()?;
        if n == 0 {
            return Some(out);
        }
        if tail.len() < n + 2 {
            return None;
        }
        out.push_str(&tail[..n]);
        rest = &tail[n + 2..];
    }
}

/// `log` 收 Edge 自己的 stderr —— 起不来/端口没绑上时，唯一有用的就是那几行原文。
pub fn open(
    url: &str,
    profile: &Path,
    window_size: &str,
    wait: Duration,
    log: &Path,
) -> Result<EdgeSession, String> {
    if let Some(parent) = log.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("建不了日志目录 {}：{e}", parent.display()))?;
    }
    let exe = std::env::var("UISCAN_EDGE").unwrap_or_else(|_| DEFAULT_EDGE.to_string());
    if !Path::new(&exe).exists() {
        return Err(format!("Edge 不在 {exe}（设 UISCAN_EDGE 指过去）"));
    }
    let port = 9600 + (std::process::id() % 400) as u16;
    let mut child = Command::new(&exe)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-sync",
        ])
        .arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--window-size={window_size}"))
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            File::create(log).map_err(|e| format!("开不了日志 {}：{e}", log.display()))?,
        ))
        .spawn()
        .map_err(|e| format!("起不了 Edge：{e}"))?;

    match attach(port, url, wait) {
        Ok(mut s) => {
            s.child = Some(child);
            Ok(s)
        }
        Err(e) => {
            // 等不到就收干净，别留一个占着端口、又不回话的僵尸进程
            let _ = child.kill();
            let _ = child.wait();
            Err(format!(
                "{e}；Edge 日志 {}：{}",
                log.display(),
                tail_of(log)
            ))
        }
    }
}

/// 折叠 `/./` 与 `/../`（只用于比较 URL 路径，不碰查询串）。
fn collapse_dot_segments(s: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "." => {}
            ".." => {
                out.pop();
            }
            x => out.push(x),
        }
    }
    out.join("/")
}

fn tail_of(log: &Path) -> String {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 连一个**已经在监听**的端口（`--port`）：既能 attach 别人起好的浏览器，
/// 也能把"是我的客户端发错了"和"是浏览器不理我"分开——同一份代码，两条来路。
pub fn attach(port: u16, url: &str, wait: Duration) -> Result<EdgeSession, String> {
    let deadline = Instant::now() + wait;
    let mut last_err = String::from("还没试过");
    loop {
        let polled = http_json(port, "/json/list");
        if let Ok(v) = &polled {
            // 按 URL 精确挑目标：列表里混着一堆扩展的 background_page，
            // "抓到第一个 page"在装了扩展的机器上是运气，不是判据。
            let target = v.as_array().and_then(|arr| {
                arr.iter()
                    .find(|t| t["type"] == "page" && t["url"].as_str() == Some(url))
                    .or_else(|| {
                        arr.iter()
                            .find(|t| t["type"] == "page" && t["url"].as_str().is_some())
                    })
            });
            if let Some(target) = target {
                let ws_url = target["webSocketDebuggerUrl"]
                    .as_str()
                    .ok_or("目标没有 webSocketDebuggerUrl")?
                    .to_string();
                let (ws, _resp) =
                    tungstenite::connect(&ws_url).map_err(|e| format!("连 ws 失败：{e}"))?;
                return Ok(EdgeSession {
                    child: None,
                    ws,
                    next_id: 0,
                });
            }
            last_err = format!(
                "列表里没有 page 目标（{} 项）",
                v.as_array().map_or(0, |a| a.len())
            );
        } else if let Err(e) = &polled {
            last_err = e.clone();
        }
        if Instant::now() > deadline {
            return Err(format!(
                "{port} 上等不到 page 目标（>{wait:?}）；最后一次轮询：{last_err}"
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

impl EdgeSession {
    /// 等**目标文档**就绪再采。两条实测教训叠加：
    /// ① page 目标早于 DOM 出现，立刻采会得到"viewport 正常、items 空"的假成功；
    /// ② Edge 起窗后先有一个 `about:blank` 的 page 目标，`readyState` 在它上面就已经是 `complete`
    ///    ——2026-10-16 扫 qingjian.app 时就是这样采到空页（href=about:blank、elements=3）。
    /// ③ 真实站点常常**永远到不了 `complete`**（挂起的统计/长连接）——2026-10-16 扫 qingjian.app
    ///    就是 `interactive` 卡满 30s。所以判据是：`location.href` 已是目标 **且** `readyState`
    ///    不是 `loading`（http(s) 允许多次跳转，只要求同源；file:// 要求完全相等），再给半秒沉降。
    pub fn wait_document_ready(&mut self, url: &str, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let st = self.evaluate("() => document.readyState")?;
            let href = self
                .evaluate("() => location.href")?
                .as_str()
                .unwrap_or_default()
                .to_string();
            let same_doc = if url.starts_with("file:") {
                // 夹具路径常带 `..`，浏览器会把 href 归一化 ⇒ 两边都折叠点段后再比。
                collapse_dot_segments(&href) == collapse_dot_segments(url)
            } else {
                let origin = url.split('/').take(3).collect::<Vec<_>>().join("/");
                href.starts_with(&origin)
            };
            let settled = matches!(st.as_str(), Some("interactive") | Some("complete"));
            if settled && href != "about:blank" && same_doc {
                // 不要求 complete：真实站点的挂起请求会让它永不 complete。半秒沉降给 SPA 首屏渲染。
                std::thread::sleep(Duration::from_millis(500));
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "页面 {timeout:?} 内没就绪（readyState={st:?} href={href:?}，目标 {url:?}）"
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// 在页面里**调用**一段函数源码并拿回值（`returnByValue`）。
    ///
    /// ⚠ 必须包成 `(fn)()`：直接求值一个箭头函数只会得到函数对象本身，
    /// `returnByValue` 把它序列化成 `{}` —— 我第一版就是这么栽的（viewport=Null、0 条）。
    pub fn evaluate(&mut self, function_src: &str) -> Result<Value, String> {
        let expression = format!("({function_src})()");
        self.next_id += 1;
        let id = self.next_id;
        let req = serde_json::json!({
            "id": id,
            "method": "Runtime.evaluate",
            "params": { "expression": expression, "returnByValue": true, "awaitPromise": false }
        });
        self.ws
            .send(Message::Text(req.to_string().into()))
            .map_err(|e| e.to_string())?;
        loop {
            let msg = self.ws.read().map_err(|e| e.to_string())?;
            let text = match msg {
                Message::Text(t) => t.to_string(),
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(err) = v.get("error") {
                return Err(format!("CDP 报错：{err}"));
            }
            let res = &v["result"]["result"];
            if res["exceptionDetails"].is_object() {
                return Err(format!("页面里抛异常：{}", res["exceptionDetails"]));
            }
            return Ok(res["value"].clone());
        }
    }
}
