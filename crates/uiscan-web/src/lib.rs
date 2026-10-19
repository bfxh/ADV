//! Web 面：在真 Chromium 里跑采集脚本，把返回值解析成 `Item`。
//!
//! **状态如实写**：本格只落地"采集脚本 + 解析层"。解析层的语料是 2026-10-16 在
//! Chromium 150（Qoder 内置 webview）真跑 `getComputedStyle` 拿到的原样 JSON
//! （`tests/fixtures/pseudo-chromium.json`）；**CDP 实跑通道还没接**——Edge 155 的
//! `--headless=new --remote-debugging-port` 已验过可用（`/json/version` 返回
//! `Edg/155.0.4283.45`），且本机没有 `msedgedriver`，所以走 CDP 直连而不是 WebDriver。

use serde::Deserialize;
use uiscan_core::{Channel, Item, Rect, css_content_to_text};

pub mod cdp;

/// 页面内采集脚本。它用到的每个 API（`getComputedStyle(el,'::before'|'::after'|'::marker')
/// .content`、`getBoundingClientRect`、遍历 `childNodes` 取直属文本节点、`getAttribute`）
/// 都单独实测过返回形状；**整段脚本还没在浏览器里跑过** —— 接上 CDP 后要做的第一件事
/// 就是拿它跑真页面，并和 `pseudo-chromium.json` 这份语料逐条对账。
pub const HARVEST_JS: &str = r#"() => {
  const out = [];
  const box = el => { const r = el.getBoundingClientRect();
    return {x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height)}; };
  const loc = el => el.tagName.toLowerCase()
    + (el.id ? '#' + el.id : '')
    + (typeof el.className === 'string' && el.className.trim()
       ? '.' + el.className.trim().split(/\s+/).join('.') : '');
  const push = (l, ch, txt, rect) => { if (txt) out.push({loc: l, ch, txt, rect}); };
  for (const el of document.querySelectorAll('*')) {
    const l = loc(el), rect = box(el);
    push(l, 'PseudoBefore', getComputedStyle(el, '::before').content, rect);
    push(l, 'PseudoAfter', getComputedStyle(el, '::after').content, rect);
    push(l, 'Marker', getComputedStyle(el, '::marker').content, rect);
    let own = '';
    for (const n of el.childNodes) if (n.nodeType === 3) own += n.nodeValue;
    push(l, 'DomText', own.trim(), rect);
    push(l, 'PlaceholderAttr', el.getAttribute && el.getAttribute('placeholder') || '', rect);
    push(l, 'TitleAttr', el.getAttribute('title') || '', rect);
  }
  return {
    viewport: [innerWidth, innerHeight, document.documentElement.clientWidth],
    title: document.title,
    href: location.href,
    body_len: (document.body && document.body.innerText ? document.body.innerText.length : 0),
    elements: document.querySelectorAll('*').length,
    items: out
  };
}"#;

#[derive(Deserialize)]
struct ProbeRow {
    cls: String,
    #[serde(default)]
    before: String,
    #[serde(default)]
    after: String,
    #[serde(default)]
    marker: String,
    /// 实测值恒为 `normal`：CSS `::placeholder{content:…}` 被浏览器忽略，
    /// 占位文案的真通道是 `placeholder` **属性**（HARVEST_JS 里已换成属性通道）。
    /// 这个字段刻意不进 `Item`。
    #[serde(default)]
    #[allow(dead_code)]
    placeholder: String,
    #[serde(default, rename = "domText")]
    dom_text: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    w: i32,
    #[serde(default)]
    h: i32,
}

#[derive(Deserialize)]
struct Probe {
    rows: Vec<ProbeRow>,
}

fn emit(items: &mut Vec<Item>, loc: &str, rect: Rect, channel: Channel, text: Option<String>) {
    let Some(text) = text else { return };
    items.push(Item::new(text, channel, loc).with_rect(rect));
}

/// 语料 JSON → 条目。通道归属在这里，能不能算文案的判据全在 `uiscan-core`。
pub fn probe_rows_to_items(json: &str) -> Result<Vec<Item>, serde_json::Error> {
    let probe: Probe = serde_json::from_str(json)?;
    let mut items = Vec::new();
    for r in probe.rows {
        let rect = Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        };
        emit(
            &mut items,
            &r.cls,
            rect,
            Channel::PseudoBefore,
            css_content_to_text(&r.before),
        );
        emit(
            &mut items,
            &r.cls,
            rect,
            Channel::PseudoAfter,
            css_content_to_text(&r.after),
        );
        emit(
            &mut items,
            &r.cls,
            rect,
            Channel::Marker,
            css_content_to_text(&r.marker),
        );
        let dom = (!r.dom_text.trim().is_empty()).then(|| r.dom_text.clone());
        emit(&mut items, &r.cls, rect, Channel::DomText, dom);
        let title = (!r.title.trim().is_empty()).then(|| r.title.clone());
        emit(&mut items, &r.cls, rect, Channel::TitleAttr, title);
    }
    Ok(items)
}

/// `HARVEST_JS` 跑回来的载荷 → 条目。与 `probe_rows_to_items` 的区别是形状：
/// 这个是按通道逐条 (`{loc,ch,txt,rect}`) 的现跑格式，前者是 10-16 那次定点探针的 `rows[]`。
/// 两条都留着，是为了让"现跑结果"能和"已核对的语料"并列比，而不是拿新结果覆盖旧结果。
pub fn harvest_payload_to_items(payload: &serde_json::Value) -> Vec<Item> {
    let mut items = Vec::new();
    let rows = match payload["items"].as_array() {
        Some(a) => a,
        None => return items,
    };
    for row in rows {
        // 通道名只认 uiscan-core 里的那张表；不认得的整条丢，不静默归类。
        let Some(ch) = row["ch"].as_str().and_then(Channel::from_wire) else {
            continue;
        };
        let raw = row["txt"].as_str().unwrap_or("");
        let text = if ch.is_generated() {
            match css_content_to_text(raw) {
                Some(t) => t,
                None => continue,
            }
        } else {
            raw.to_string()
        };
        if text.trim().is_empty() {
            continue;
        }
        let r = &row["rect"];
        let rect = Rect {
            x: r["x"].as_i64().unwrap_or(0) as i32,
            y: r["y"].as_i64().unwrap_or(0) as i32,
            w: r["w"].as_i64().unwrap_or(0) as i32,
            h: r["h"].as_i64().unwrap_or(0) as i32,
        };
        items.push(Item::new(text, ch, row["loc"].as_str().unwrap_or("")).with_rect(rect));
    }
    items
}
