//! 采集结果模型 + "算不算文案"的判据。
//!
//! 判据的期望值一律锚到**真机实测**（Chromium 150 webview、vt100 0.16.2、UIA on windows-gnu），
//! 不是按 CSS 规范或想象写的——`tests/` 里每条断言都能指回一次实测。

use std::collections::BTreeSet;

pub mod checks;

pub use checks::{
    channel_gap, css_content_to_text, looks_translatable, non_ascii, unresolved_generator,
    usable_rect,
};

/// 文案来自哪条通道。区别要紧：`DomText` 谁都扫得到，伪元素/格子只有真渲染面拿得到。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    DomText,
    PseudoBefore,
    PseudoAfter,
    Marker,
    PlaceholderAttr,
    TitleAttr,
    TermCell,
    UiaName,
}

impl Channel {
    /// 生成内容通道（浏览器 computed style 给的，不在 DOM 文本里）。
    pub fn is_generated(self) -> bool {
        matches!(self, Self::PseudoBefore | Self::PseudoAfter | Self::Marker)
    }

    /// 落盘/日志里的稳定名字。**只此一处**：dump 写出与解析各写一份字符串匹配，
    /// 一改就会漂移（这个仓库在别处已经吃过"两份文案"的亏）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomText => "DomText",
            Self::PseudoBefore => "PseudoBefore",
            Self::PseudoAfter => "PseudoAfter",
            Self::Marker => "Marker",
            Self::PlaceholderAttr => "PlaceholderAttr",
            Self::TitleAttr => "TitleAttr",
            Self::TermCell => "TermCell",
            Self::UiaName => "UiaName",
        }
    }

    /// 从落盘名字还原；不认得的返回 `None`（调用方负责跳过，别硬塞进某个通道）。
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "DomText" => Self::DomText,
            "PseudoBefore" => Self::PseudoBefore,
            "PseudoAfter" => Self::PseudoAfter,
            "Marker" => Self::Marker,
            "PlaceholderAttr" => Self::PlaceholderAttr,
            "TitleAttr" => Self::TitleAttr,
            "TermCell" => Self::TermCell,
            "UiaName" => Self::UiaName,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub text: String,
    pub channel: Channel,
    /// 定位串：Web 是选择器、TUI 是 `r{行}c{起始列}`、GUI 是 UIA 路径。
    pub loc: String,
    pub visible: bool,
    pub rect: Option<Rect>,
}

impl Item {
    pub fn new(text: impl Into<String>, channel: Channel, loc: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            channel,
            loc: loc.into(),
            visible: true,
            rect: None,
        }
    }

    pub fn with_rect(mut self, r: Rect) -> Self {
        self.visible = usable_rect(r);
        self.rect = Some(r);
        self
    }
}

/// 一次采集的结果。`surface` 记录这把尺是谁量的（含尺寸口径），跨面比账时不许混。
#[derive(Clone, Debug, Default)]
pub struct Harvest {
    pub surface: String,
    pub items: Vec<Item>,
}

impl Harvest {
    /// 进入**漏翻比对**的条目：可见 + 像文案（有字母、不是未解析的生成器令牌）。
    pub fn candidates(&self) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|it| it.visible && looks_translatable(&it.text))
            .collect()
    }

    /// 进入**字体覆盖检查（判豆腐块）**的码点：所有可见条目都要，含纯符号。
    ///
    /// 与 `candidates` 刻意不同口径：`✓ ✗ →` 不是待翻译文案，但缺字一样画成方框。
    /// 用候选集去查覆盖会漏掉整类缺陷（TUI 面实测到这条）。
    pub fn rendered_codepoints(&self) -> BTreeSet<char> {
        non_ascii(
            self.items
                .iter()
                .filter(|it| it.visible)
                .map(|it| it.text.clone()),
        )
    }
}
