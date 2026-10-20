//! UIA 的真调用层（只在 `cfg(windows)` 下编译）。
//!
//! 实测环境：Windows 11 + **x86_64-pc-windows-gnu（无 MSVC）**，`windows 0.62.2` 靠 raw-dylib
//! 链接，已真跑出焦点元素与 28 个顶层窗口（name/ControlType/BoundingRectangle）。
//! 两个用法坑：`CUIAutomation` 是 **CLSID 常量**不是类型（要 `CoCreateInstance`）；
//! `UIA_CONTROLTYPE_ID` 不实现 Display（取 `.0`）。

use std::ffi::c_void;

use uiscan_core::Harvest;

use crate::GuiScan;
use crate::LayoutFinding;

use crate::WindowInfo;
use crate::pure;
use crate::pure::TextFinding;

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    TreeScope_Children, UIA_TextPatternId,
};

/// COM 的 init/uninit 配对。`CoInitializeEx` 返回 Ok（含 S_FALSE=已初始化）才配 `CoUninitialize`；
/// 返回 `RPC_E_CHANGED_MODE` 说明本线程已是别的套间，**不能**去反初始化别人的。
struct Com;

impl Com {
    fn enter() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        Self
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn automation() -> Result<IUIAutomation, String> {
    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| format!("建不了 UIA 客户端：{e}"))
}

struct Reading {
    name: String,
    control_type: i32,
    rect: uiscan_core::Rect,
    offscreen: bool,
    pid: u32,
    class: String,
}

fn read(el: &IUIAutomationElement) -> Reading {
    unsafe {
        let name = el.CurrentName().map(|b| b.to_string()).unwrap_or_default();
        let control_type = el.CurrentControlType().map(|c| c.0).unwrap_or(-1);
        let r = el.CurrentBoundingRectangle().unwrap_or_default();
        let offscreen = el.CurrentIsOffscreen().map(|b| b.as_bool()).unwrap_or(true);
        let pid = el.CurrentProcessId().unwrap_or(0);
        let class = el
            .CurrentClassName()
            .map(|b| b.to_string())
            .unwrap_or_default();
        Reading {
            name,
            control_type,
            rect: pure::rect_from_ltrb(r.left, r.top, r.right, r.bottom),
            offscreen,
            // windows 0.62 里 CurrentProcessId 是 i32；负值不该出现，按 0 处理
            pid: pid.max(0) as u32,
            class,
        }
    }
}

/// 取 `TextPattern.GetBoundingRectangles`：返回（可见行数，行矩形并集）。没有该图案返回 None。
///
/// 实测：经典 Win32 `EDIT` **没有** TextPattern；Chromium/Electron 的 Text/Edit 有，
/// 且给的是**可见行**的矩形（普通文本 union ⊆ 元素矩形）。
struct TextFacts {
    lines: usize,
    union: Option<[f64; 4]>,
    font: Option<String>,
    size: Option<f32>,
    text: String,
}

fn text_rects(el: &IUIAutomationElement) -> Option<TextFacts> {
    unsafe {
        let pat: IUIAutomationTextPattern = el
            .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .ok()?;
        let range = pat.DocumentRange().ok()?;
        let arr = range.GetBoundingRectangles().ok()?;
        let arr = arr.as_ref()?;
        let lo = SafeArrayGetLBound(arr, 1).ok()?;
        let hi = SafeArrayGetUBound(arr, 1).ok()?;
        let n = (hi - lo + 1).max(0) as usize;
        let mut data: *mut c_void = std::ptr::null_mut();
        SafeArrayAccessData(arr, &mut data).ok()?;
        let vals = std::slice::from_raw_parts(data as *const f64, n);
        let mut u = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut lines = 0usize;
        for line in vals.chunks_exact(4) {
            lines += 1;
            u[0] = u[0].min(line[0]);
            u[1] = u[1].min(line[1]);
            u[2] = u[2].max(line[0] + line[2]);
            u[3] = u[3].max(line[1] + line[3]);
        }
        let _ = SafeArrayUnaccessData(arr);
        let font = range
            .GetAttributeValue(windows::Win32::UI::Accessibility::UIA_FontNameAttributeId)
            .ok()
            .and_then(|v| var_bstr(&v));
        let size = range
            .GetAttributeValue(windows::Win32::UI::Accessibility::UIA_FontSizeAttributeId)
            .ok()
            .and_then(|v| var_f64(&v))
            .map(|f| f as f32);
        let text = range
            .GetText(-1)
            .ok()
            .map(|b| b.to_string())
            .unwrap_or_default();
        Some(TextFacts {
            lines,
            union: (lines > 0).then_some(u),
            font,
            size,
            text,
        })
    }
}

fn var_f64(v: &windows::Win32::System::Variant::VARIANT) -> Option<f64> {
    let a = unsafe { &v.Anonymous.Anonymous };
    if a.vt == windows::Win32::System::Variant::VT_R8 {
        Some(unsafe { a.Anonymous.dblVal })
    } else {
        None
    }
}

fn var_bstr(v: &windows::Win32::System::Variant::VARIANT) -> Option<String> {
    let a = unsafe { &v.Anonymous.Anonymous };
    if a.vt == windows::Win32::System::Variant::VT_BSTR {
        Some(unsafe { a.Anonymous.bstrVal.to_string() })
    } else {
        None
    }
}

fn children(auto: &IUIAutomation, el: &IUIAutomationElement) -> Vec<IUIAutomationElement> {
    unsafe {
        let Ok(cond) = auto.CreateTrueCondition() else {
            return Vec::new();
        };
        let Ok(arr) = el.FindAll(TreeScope_Children, &cond) else {
            return Vec::new();
        };
        let n = arr.Length().unwrap_or(0);
        (0..n).filter_map(|i| arr.GetElement(i).ok()).collect()
    }
}

fn top_windows(auto: &IUIAutomation) -> Vec<IUIAutomationElement> {
    match unsafe { auto.GetRootElement() } {
        Ok(root) => children(auto, &root),
        Err(_) => Vec::new(),
    }
}

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    let _com = Com::enter();
    let auto = automation()?;
    Ok(top_windows(&auto)
        .iter()
        .map(|el| {
            let r = read(el);
            WindowInfo {
                name: r.name,
                pid: r.pid,
                class: r.class,
                rect: [r.rect.x, r.rect.y, r.rect.w, r.rect.h],
                offscreen: r.offscreen,
            }
        })
        .collect())
}

/// 从某个元素起递归采集。`max_depth`/`max_nodes` 是硬闸：UIA 树可能深到把进程拖死。
fn walk(
    auto: &IUIAutomation,
    el: &IUIAutomationElement,
    path: &str,
    depth: usize,
    max_depth: usize,
    max_nodes: usize,
    out: &mut Harvest,
    findings: &mut Vec<TextFinding>,
    layout: &mut Vec<LayoutFinding>,
) -> bool {
    if out.items.len() >= max_nodes {
        return false;
    }
    let me = read(el);
    if let Some(it) = pure::item_from_uia(&me.name, path, me.rect, me.offscreen) {
        out.items.push(it);
    }
    // TextPattern 对账：**在屏**就做（不要求 Name——实测 WPF TextBox 的 Name 为空，
    // 但它有 TextPattern 与字体属性；被 Name 门槛挡住会把整类控件漏掉）。没图案的自然跳过。
    if !me.offscreen {
        if let Some(facts) = text_rects(el) {
            let label = if me.name.trim().is_empty() {
                facts.text.clone()
            } else {
                me.name.clone()
            };
            if !label.trim().is_empty() {
                let elem = [
                    me.rect.x as f64,
                    me.rect.y as f64,
                    (me.rect.x + me.rect.w) as f64,
                    (me.rect.y + me.rect.h) as f64,
                ];
                if let Some(kind) = pure::judge_text(elem, facts.union, 2.0) {
                    findings.push(TextFinding {
                        loc: path.to_string(),
                        text: label.clone(),
                        kind,
                        elem,
                        union: facts.union,
                    });
                }
                // 布局宽度对账：只在**单行**、且提供方给了字体/字号时做（多行有换行歧义）。
                if facts.lines == 1 {
                    if let (Some(union), Some(font), Some(size)) =
                        (facts.union, facts.font.clone(), facts.size)
                    {
                        let visible = (union[2] - union[0]) as f32;
                        // **单位口径不统一**（实测）：WPF 的 FontSize 是磅（FontSize=12 DIP 报 9.0），
                        // 而任务栏那个提供方报 14.0，只有按 DIP 量才与可见宽度吻合（按磅会误报 37 vs 28）。
                        // 所以两种口径都量，**两者都判超**才记——宁可少报，不制造假红。
                        let as_pt =
                            crate::textfit::measure_width(&font, size * 4.0 / 3.0, &facts.text);
                        let as_dip = crate::textfit::measure_width(&font, size, &facts.text);
                        if let (Ok(pt), Ok(dip)) = (as_pt, as_dip) {
                            if crate::textfit::layout_clipped(pt, visible)
                                && crate::textfit::layout_clipped(dip, visible)
                            {
                                layout.push(LayoutFinding {
                                    loc: path.to_string(),
                                    text: facts.text.clone(),
                                    font,
                                    size,
                                    expected: pt.max(dip),
                                    visible,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    if depth >= max_depth {
        return true;
    }
    for (i, kid) in children(auto, el).iter().enumerate() {
        let kr = read(kid);
        let seg = pure::path_segment(kr.control_type, i, &kr.name);
        let child_path = format!("{path}/{seg}");
        if !walk(
            auto,
            kid,
            &child_path,
            depth + 1,
            max_depth,
            max_nodes,
            out,
            findings,
            layout,
        ) {
            return false;
        }
    }
    true
}

fn harvest_from(
    auto: &IUIAutomation,
    el: &IUIAutomationElement,
    surface: String,
    max_depth: usize,
    max_nodes: usize,
) -> (Harvest, Vec<TextFinding>, Vec<LayoutFinding>) {
    let mut out = Harvest {
        surface,
        items: Vec::new(),
    };
    let mut findings = Vec::new();
    let mut layout = Vec::new();
    let r = read(el);
    let root_path = pure::path_segment(r.control_type, 0, &r.name);
    walk(
        auto,
        el,
        &root_path,
        0,
        max_depth,
        max_nodes,
        &mut out,
        &mut findings,
        &mut layout,
    );
    (out, findings, layout)
}

fn open_harvest(
    surface: String,
    max_depth: usize,
    max_nodes: usize,
    pick: impl Fn(&IUIAutomation) -> Option<(IUIAutomationElement, String)>,
) -> Result<GuiScan, String> {
    let _com = Com::enter();
    let auto = automation()?;
    let (el, name) = pick(&auto).ok_or_else(|| {
        let tops: Vec<String> = top_windows(&auto)
            .iter()
            .map(|e| read(e).name)
            .filter(|n| !n.is_empty())
            .take(12)
            .collect();
        format!("没找到目标窗口；当前顶层窗口（前 12 个）：{tops:?}")
    })?;
    let (harvest, text, layout) = harvest_from(
        &auto,
        &el,
        format!("{surface} target={name:?}"),
        max_depth,
        max_nodes,
    );
    Ok(GuiScan {
        harvest,
        text,
        layout,
    })
}

pub fn harvest_window(
    title_substr: &str,
    max_depth: usize,
    max_nodes: usize,
) -> Result<GuiScan, String> {
    let needle = title_substr.to_lowercase();
    open_harvest(
        format!("gui:uia title~{title_substr:?}"),
        max_depth,
        max_nodes,
        |auto| {
            top_windows(auto).into_iter().find_map(|el| {
                let r = read(&el);
                r.name
                    .to_lowercase()
                    .contains(&needle)
                    .then_some((el, r.name))
            })
        },
    )
}

pub fn harvest_pid(pid: u32, max_depth: usize, max_nodes: usize) -> Result<GuiScan, String> {
    open_harvest(format!("gui:uia pid={pid}"), max_depth, max_nodes, |auto| {
        top_windows(auto).into_iter().find_map(|el| {
            let r = read(&el);
            (r.pid == pid).then_some((el, format!("pid{pid}:{}", r.name)))
        })
    })
}

pub fn harvest_focused(max_depth: usize, max_nodes: usize) -> Result<GuiScan, String> {
    open_harvest(
        "gui:uia focused".to_string(),
        max_depth,
        max_nodes,
        |auto| {
            unsafe { auto.GetFocusedElement() }.ok().map(|el| {
                let r = read(&el);
                (el, r.name)
            })
        },
    )
}
