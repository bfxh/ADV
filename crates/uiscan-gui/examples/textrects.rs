//! 探针（仅 Windows）：把某个窗口里每个元素的 `TextPattern.GetBoundingRectangles`
//! 原样打印出来，用来**测量** UIA 在"文本放得下 / 被裁"两种情况下各给什么矩形。
//!
//! 运行：先 `cargo run -p uiscan-gui --example clip_fixture`（另开终端），再
//! `cargo run -p uiscan-gui --example textrects -- uiscan-clip-fixture`

#[cfg(windows)]
fn main() {
    let needle = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "uiscan-clip-fixture".into());
    unsafe { imp::run(&needle) }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("只在 Windows 上有意义");
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::RECT;
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

    fn num(v: &IUIAutomationElement, key: &str) -> String {
        unsafe {
            let name = v.CurrentName().map(|b| b.to_string()).unwrap_or_default();
            let class = v
                .CurrentClassName()
                .map(|b| b.to_string())
                .unwrap_or_default();
            let ct = v.CurrentControlType().map(|c| c.0).unwrap_or(-1);
            let r: RECT = v.CurrentBoundingRectangle().unwrap_or_default();
            format!(
                "{key} ct={ct} class={class:?} elem=({},{},{},{}) name={name:?}",
                r.left, r.top, r.right, r.bottom
            )
        }
    }

    unsafe fn rects_of(el: &IUIAutomationElement) -> Option<(usize, [f64; 4])> {
        unsafe {
            let pat: IUIAutomationTextPattern = el
                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .ok()?;
            let range = pat.DocumentRange().ok()?;
            let arr = range.GetBoundingRectangles().ok()?;
            let arr = arr.as_ref()?;
            let lo = SafeArrayGetLBound(arr, 1).ok()?;
            let hi = SafeArrayGetUBound(arr, 1).ok()?;
            let n = (hi - lo + 1) as usize;
            let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
            SafeArrayAccessData(arr, &mut data).ok()?;
            let vals = std::slice::from_raw_parts(data as *const f64, n);
            let mut out = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for line in vals.chunks_exact(4) {
                out[0] = out[0].min(line[0]);
                out[1] = out[1].min(line[1]);
                out[2] = out[2].max(line[0] + line[2]);
                out[3] = out[3].max(line[1] + line[3]);
            }
            let _ = SafeArrayUnaccessData(arr);
            Some((n / 4, out))
        }
    }

    unsafe fn walk(auto: &IUIAutomation, el: &IUIAutomationElement, depth: usize) {
        unsafe {
            println!("{}", num(el, &"  ".repeat(depth)));
            if let Some((lines, u)) = rects_of(el) {
                if lines == 0 {
                    println!("{}  text: lines=0（没有可见文字矩形）", "  ".repeat(depth));
                } else {
                    println!(
                        "{}  text: lines={lines} union=({:.0},{:.0},{:.0},{:.0})",
                        "  ".repeat(depth),
                        u[0],
                        u[1],
                        u[2],
                        u[3]
                    );
                }
            }
            if depth >= 14 {
                return;
            }
            let Ok(cond) = auto.CreateTrueCondition() else {
                return;
            };
            let Ok(kids) = el.FindAll(TreeScope_Children, &cond) else {
                return;
            };
            for i in 0..kids.Length().unwrap_or(0) {
                if let Ok(k) = kids.GetElement(i) {
                    walk(auto, &k, depth + 1);
                }
            }
        }
    }

    pub unsafe fn run(needle: &str) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let auto: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).expect("UIA client");
            let root = auto.GetRootElement().expect("root");
            let cond = auto.CreateTrueCondition().expect("cond");
            let tops = root.FindAll(TreeScope_Children, &cond).expect("tops");
            let mut found = false;
            for i in 0..tops.Length().unwrap_or(0) {
                if let Ok(w) = tops.GetElement(i) {
                    let name = w.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                    if name.contains(needle) {
                        found = true;
                        walk(&auto, &w, 0);
                    }
                }
            }
            if !found {
                eprintln!("没找到标题包含 {needle:?} 的窗口");
            }
            CoUninitialize();
        }
    }
}
