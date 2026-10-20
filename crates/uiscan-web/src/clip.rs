//! 截断判据（Web 面）：用浏览器自己给的几何量判断"文字是不是被容器裁掉了"。
//!
//! 判据来自 2026-10-20 的实测（`tests/fixtures/truncation.html`，Edge 155 headless）：
//!
//! | 元素 | scrollWidth/clientWidth | scrollHeight/clientHeight | overflow | text-overflow |
//! |---|---|---|---|---|
//! | 放得下 | 160/160 | 24/24 | visible | clip |
//! | 横向硬裁 | **336**/160 | 24/24 | hidden | clip |
//! | 横向省略号 | **336**/160 | 24/24 | hidden | **ellipsis** |
//! | 纵向裁切 | 160/160 | **72**/24 | hidden | clip |
//! | line-clamp | 160/160 | **72**/24 | hidden | clip |
//!
//! 所以规则是纯几何 + 两个 computed 值，不猜 CSS 语义：
//! - 进判据的只有**可见且非空**的 DomText（`title`/`style` 这类 h=0 的天然排除）；
//! - 横向：`scrollWidth > clientWidth + 1` 且 `overflow-x` ∈ {hidden, clip}
//!   （纯 `overflow: visible` 的溢出会画到外面，不是"裁掉"）；
//! - 纵向同理看 `scrollHeight/clientHeight` 与 `overflow-y`；
//! - `text-overflow: ellipsis` 单独记一种：它仍是被裁，只是设计上留了省略号。
//!
//! 边界：这是**布局层**判据。Web 字体缺失导致的方框不在这里（走 `tofu`）；
//! 绝对定位/transform 出来的裁切（overflow visible 但被祖先裁）这里看不到。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipKind {
    /// 横向硬裁（看不到省略号）
    Horizontal,
    /// 横向裁切但带省略号（设计如此，仍是"文案放不下"的信号）
    Ellipsis,
    /// 纵向裁切（多行 / line-clamp）
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub loc: String,
    pub text: String,
    pub kind: ClipKind,
    pub sw: i64,
    pub cw: i64,
    pub sh: i64,
    pub chh: i64,
}

fn num(v: &serde_json::Value, key: &str) -> i64 {
    v[key].as_i64().unwrap_or(0)
}

fn hidden(v: &serde_json::Value, key: &str) -> bool {
    matches!(v[key].as_str(), Some("hidden") | Some("clip"))
}

/// 从 `HARVEST_JS` 的原始载荷里找裁切。只对带几何字段的 DomText 条目生效。
pub fn findings(payload: &serde_json::Value) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(items) = payload["items"].as_array() else {
        return out;
    };
    for it in items {
        if it["ch"].as_str() != Some("DomText") {
            continue;
        }
        let text = it["txt"].as_str().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        // 可见性：rect 高/宽为 0 的（title/style 等）不参与；几何为零的也跳过。
        let rect_h = it["rect"]["h"].as_i64().unwrap_or(0);
        let rect_w = it["rect"]["w"].as_i64().unwrap_or(0);
        let (sw, cw, sh, chh) = (num(it, "sw"), num(it, "cw"), num(it, "sh"), num(it, "chh"));
        if rect_h <= 0 || rect_w <= 0 || (cw <= 0 && chh <= 0) {
            continue;
        }
        let kind = if hidden(it, "ovx") && sw > cw + 1 {
            if it["ell"].as_str() == Some("ellipsis") {
                ClipKind::Ellipsis
            } else {
                ClipKind::Horizontal
            }
        } else if hidden(it, "ovy") && sh > chh + 1 {
            ClipKind::Vertical
        } else {
            continue;
        };
        out.push(Finding {
            loc: it["loc"].as_str().unwrap_or("").to_string(),
            text: text.to_string(),
            kind,
            sw,
            cw,
            sh,
            chh,
        });
    }
    out
}

/// 载荷里有没有可用于判定的几何字段。
///
/// **fail-closed 依据**：旧版 `web --json` 的载荷没有 `sw/cw/sh/chh`，`findings` 会安静地返回 0 条
/// ——那就是"没在看"被当成"干净"。CLI 必须先问这一句，缺字段就退 2 让人重采。
pub fn has_geometry(payload: &serde_json::Value) -> bool {
    payload["items"].as_array().is_some_and(|items| {
        items.iter().any(|it| {
            it["ch"].as_str() == Some("DomText") && it.get("sw").is_some() && it.get("cw").is_some()
        })
    })
}
