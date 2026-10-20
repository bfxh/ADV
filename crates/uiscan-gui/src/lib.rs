//! GUI 面（Windows UI Automation）：把"作者声明的逻辑文案"采出来，连同它的矩形与在屏标志。
//!
//! 定位（说清边界）：
//! - UIA 给的是 **Name 属性**，不是"画出来的像素"；缺字体画的方框、被裁掉的文字在树里**看不出来**。
//!   这一面能查的是：未翻译/硬编码（把 Name 拿去和翻译记忆对）、空名、屏外/零尺寸节点；
//!   要查豆腐块与截断，必须和字体覆盖/像素那两层合起来用。
//! - 节点没有 Name 时本身不产出条目，但**照样往里走**（很多容器没名、孩子才有）。
//! - 深度与节点数都有硬闸（UIA 树能把进程拖死）。

pub mod pure;
#[cfg(windows)]
pub mod textfit;

pub use pure::{
    CHROME_ONLY_HINT, TextFinding, TextKind, chrome_only, control_type_name, item_from_uia,
    judge_text, path_segment, rect_from_ltrb,
};

/// 布局对账发现：按字体应有多宽 vs 实际可见多宽。
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutFinding {
    pub loc: String,
    pub text: String,
    pub font: String,
    pub size: f32,
    pub expected: f32,
    pub visible: f32,
}

/// 窗口采集结果 + TextPattern 对账发现 + 布局宽度对账。
pub struct GuiScan {
    pub harvest: uiscan_core::Harvest,
    pub text: Vec<TextFinding>,
    pub layout: Vec<LayoutFinding>,
}

/// 顶层窗口的一条读数（`list_windows` 用）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub name: String,
    pub pid: u32,
    pub class: String,
    pub rect: [i32; 4],
    pub offscreen: bool,
}

#[cfg(windows)]
mod uia;
#[cfg(windows)]
pub use uia::{harvest_focused, harvest_pid, harvest_window, list_windows};

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::{harvest_focused, harvest_pid, harvest_window, list_windows};
