//! 非 Windows 平台：同签名的失败桩。
//!
//! **不假装成功**：CI 在 ubuntu 上跑时，调用这个面必须得到明确的"做不到"，
//! 而不是一个空的 `Harvest`（空结果会被下游当成"这个窗口没有文案"——那就是假绿）。

use uiscan_core::Harvest;

use crate::WindowInfo;

const WHY: &str = "UIA 只在 Windows 上有（本 crate 在非 Windows 下是失败桩，不是空结果）";

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    Err(WHY.to_string())
}

pub fn harvest_window(_title: &str, _depth: usize, _nodes: usize) -> Result<Harvest, String> {
    Err(WHY.to_string())
}

pub fn harvest_pid(_pid: u32, _depth: usize, _nodes: usize) -> Result<Harvest, String> {
    Err(WHY.to_string())
}

pub fn harvest_focused(_depth: usize, _nodes: usize) -> Result<Harvest, String> {
    Err(WHY.to_string())
}
