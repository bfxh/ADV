//! UIA（Windows UI Automation）面的映射层：**纯函数**，不碰 COM，任何平台都能测。
//!
//! 为什么要单独一层：UIA 的读数（Name / ControlType / BoundingRectangle / IsOffscreen）
//! 在 CI 上没有桌面可跑；但"读到之后怎么变成 `Item`、怎么定可见性、路径怎么拼"是可判定的，
//! 把这部分抽出来钉住，真跑部分就只剩薄薄一层调用。

use uiscan_core::Channel;
use uiscan_core::{Item, Rect};

/// UIA 的 `UIA_CONTROLTYPE_ID` → 名字。
///
/// 编码来源分两档，别混：
/// - **实测确认**（2026-10-16 本机真跑）：50030=Document（Yan Agent 主窗）、50032=Window、
///   50033=Pane、50037=TitleBar（fmnr 窗口里它是"最大化"按钮的父节点，结构对得上）、
///   50010=MenuBar（同一棵 fmnr 树里出现在 TitleBar 下）。
/// - **按微软 UIA 文档表**（本机没有逐个造窗口去验）：50000 Button、50004 Edit、50008 List、
///   50026 Group、50020 Text、50036 Table……
///
/// 不认得的给 `Unknown(数值)`，**不猜**——我第一版把 50037 猜成 "List?"、50034 猜成 Edit，都是错的。
pub fn control_type_name(id: i32) -> String {
    match id {
        50000 => "Button",
        50001 => "Calendar",
        50002 => "CheckBox",
        50003 => "ComboBox",
        50004 => "Edit",
        50005 => "Hyperlink",
        50006 => "Image",
        50007 => "ListItem",
        50008 => "List",
        50009 => "Menu",
        50010 => "MenuBar",
        50011 => "MenuItem",
        50012 => "ProgressBar",
        50013 => "RadioButton",
        50014 => "ScrollBar",
        50015 => "Slider",
        50016 => "Spinner",
        50017 => "StatusBar",
        50018 => "Tab",
        50019 => "TabItem",
        50020 => "Text",
        50021 => "ToolBar",
        50022 => "ToolTip",
        50023 => "Tree",
        50024 => "TreeItem",
        50025 => "Custom",
        50026 => "Group",
        50027 => "Thumb",
        50028 => "DataGrid",
        50029 => "DataItem",
        50030 => "Document",
        50031 => "SplitButton",
        50032 => "Window",
        50033 => "Pane",
        50034 => "Header",
        50035 => "HeaderItem",
        50036 => "Table",
        50037 => "TitleBar",
        50038 => "Separator",
        other => return format!("Unknown({other})"),
    }
    .to_string()
}

/// 路径段：`ControlType[子序号]#名字`（名字截断，避免一条 loc 撑爆整个报告）。
pub fn path_segment(control_type: i32, index_of_siblings: usize, name: &str) -> String {
    let n: String = name.trim().chars().take(24).collect();
    if n.is_empty() {
        format!("{}[{index_of_siblings}]", control_type_name(control_type))
    } else {
        format!(
            "{}[{index_of_siblings}]#{n}",
            control_type_name(control_type)
        )
    }
}

/// 一条 UIA 读数 → `Item`（或 None：没名字的节点本身不产出文案，但它的孩子照样会被遍历）。
///
/// 可见性按两条实测信号判：
/// - `is_offscreen`：UIA 自己说"不在屏上"（本机实测有 `(-9999,-9999)` 的屏外窗口）；
/// - `usable_rect`：零尺寸/哨兵矩形（实测 `(0,0)-(0,0)`）。
pub fn item_from_uia(name: &str, path: &str, rect: Rect, is_offscreen: bool) -> Option<Item> {
    let text = name.trim();
    if text.is_empty() {
        return None;
    }
    let mut it = Item::new(text, Channel::UiaName, path).with_rect(rect);
    if is_offscreen {
        it.visible = false;
    }
    Some(it)
}

/// UIA 的 RECT（左/上/右/下）→ 我们的 `Rect`（x/y/w/h）。负宽高按 0 处理（别造负尺寸）。
pub fn rect_from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect {
        x: left,
        y: top,
        w: (right - left).max(0),
        h: (bottom - top).max(0),
    }
}
