//! 豆腐块（missing glyph）判据：**码点 × 字体 cmap 覆盖**。
//!
//! 为什么不是 OCR/截图：缺字在字体层就已经发生了——`cmap` 里查不到字形，渲染器只能画 `.notdef`
//! （第一号字形，通常是个方框）。`ttf-parser` 的 `glyph_index()` 正好能回答"这个码点有没有字形"，
//! 纯 Rust、无需 C 依赖、也不需要把界面跑起来。
//!
//! 边界（必须说清，否则会被当成"像素级验证"）：
//! - 这是**字体能力**检查，不是"最终画面对不对"：应用可能只内嵌了某个字体、根本不做系统回退
//!   （egui 的默认字体就是这样，fmnr 的中文豆腐块就是这么来的）。所以字体栈要**由调用方给**，
//!   工具不替它猜。
//! - 覆盖 ≠ 一定渲染成功：复合字形、连字、语言特性可能另有问题；这里只回答"有没有字形"。
//! - 反过来也成立：系统里有字体覆盖、但应用没用它 ⇒ 这里会判"没问题"，而屏幕上是方框。
//!   这正是要把应用**实际字体栈**传进来的原因。

use std::path::{Path, PathBuf};

/// 一组字体文件（可含 `.ttc` 集合）。一个码点只要在**任意一个面**里有字形就算覆盖。
#[derive(Default)]
pub struct FontSet {
    blobs: Vec<Vec<u8>>,
}

impl FontSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// 载入一个字体文件（`.ttf/.otf/.ttc`），返回它包含的面数。
    pub fn add_file(&mut self, path: &Path) -> Result<usize, String> {
        let data = std::fs::read(path).map_err(|e| format!("读不了 {}：{e}", path.display()))?;
        let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1) as usize;
        if faces == 0 {
            return Err(format!("{} 里没有字体面", path.display()));
        }
        // 先验证至少第一个面能解析，坏文件当场报而不是让后面每个码点都静默 miss
        ttf_parser::Face::parse(&data, 0)
            .map_err(|e| format!("{} 不是可解析的字体：{e}", path.display()))?;
        self.blobs.push(data);
        Ok(faces)
    }

    pub fn face_count(&self) -> usize {
        self.blobs
            .iter()
            .map(|d| ttf_parser::fonts_in_collection(d).unwrap_or(1) as usize)
            .sum()
    }

    /// 这个码点在字体栈的**任意一个面**里有字形吗。
    pub fn covers(&self, ch: char) -> bool {
        self.blobs.iter().any(|data| {
            let faces = ttf_parser::fonts_in_collection(data).unwrap_or(1);
            (0..faces).any(|i| {
                ttf_parser::Face::parse(data, i)
                    .map(|f| f.glyph_index(ch).is_some())
                    .unwrap_or(false)
            })
        })
    }

    /// 缺字码点（升序去重）：只返回"没有任何面覆盖"的那些。
    pub fn missing(&self, chars: impl IntoIterator<Item = char>) -> Vec<char> {
        missing_where(chars, |c| self.covers(c))
    }
}

/// 与 [`FontSet`] 解耦的纯聚合：给一个 `covers` 判据，返回缺字集合。
/// 单独抽出来是为了能在没有字体文件的机器上测（CI）。
pub fn missing_where(
    chars: impl IntoIterator<Item = char>,
    covers: impl Fn(char) -> bool,
) -> Vec<char> {
    let mut out: Vec<char> = chars
        .into_iter()
        .filter(|c| !c.is_ascii() && !c.is_control())
        .filter(|c| !covers(*c))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Windows 系统字体目录里的常见字体文件（`.ttf/.ttc/.otf`）。
/// 只作为"想试系统字体栈"的便捷入口，不代表应用真的用了它们。
pub fn system_font_files() -> Vec<PathBuf> {
    let dir = std::env::var("WINDIR")
        .map(|w| PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|_| PathBuf::from(r"C:\Windows\Fonts"));
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        if matches!(ext.as_deref(), Some("ttf") | Some("ttc") | Some("otf")) {
            out.push(p);
        }
    }
    out.sort();
    out
}
