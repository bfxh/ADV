//! 按元素声明的字体量"这段文字需要多宽"（DirectWrite，单位 DIP）。
//!
//! 动机：`TextPattern.GetBoundingRectangles` 给的是**可见片段**（实测：WPF 横向/纵向裁切都跟着缩），
//! 所以"矩形对矩形"判不出裁切。但 TextPattern 同时给了 **字体名与字号**（实测 WPF TextBox：
//! `"Microsoft YaHei UI" / 9.0`），于是可以反过来算：这段全文按该字体排出来要多宽，
//! 再和**可见文本宽度**对比 —— 差一个量级就是"被裁了"。
//!
//! 边界：只在**单行**（可见矩形只有一行）时判，绕开"换行 vs 裁切"的歧义；多行文本跳过。

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWriteCreateFactory, IDWriteFactory,
};
use windows::core::HSTRING;

/// 用 `family` + `size_dip` 排 `text`，返回自然宽度（DIP）。
pub fn measure_width(family: &str, size_dip: f32, text: &str) -> Result<f32, String> {
    if text.is_empty() || size_dip <= 0.0 {
        return Ok(0.0);
    }
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
            .map_err(|e| format!("DWrite 工厂：{e}"))?;
        let format = factory
            .CreateTextFormat(
                &HSTRING::from(family),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size_dip,
                &HSTRING::from("zh-cn"),
            )
            .map_err(|e| format!("CreateTextFormat：{e}"))?;
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let layout = factory
            .CreateTextLayout(&utf16, &format, 100_000.0, 100_000.0)
            .map_err(|e| format!("CreateTextLayout：{e}"))?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        layout
            .GetMetrics(&mut metrics)
            .map_err(|e| format!("GetMetrics：{e}"))?;
        Ok(metrics.width)
    }
}

/// 布局对账判定：单行可见文本宽度 `visible_dip` 明显小于"按字体应有多宽"`expected_dip` ⇒ 被裁。
/// 容差写死 8 DIP：覆盖内边距/次像素差异，又不至于吞掉一段中文（一个字 ≈ 字号宽）。
pub fn layout_clipped(expected_dip: f32, visible_dip: f32) -> bool {
    expected_dip - visible_dip > 8.0
}
