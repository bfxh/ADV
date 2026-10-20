//! 截断夹具（仅 Windows）：开一个小窗口，放两个 EDIT——
//! 一个文本放得下，一个宽度不够、文本被裁。用来验证 UIA TextPattern 的矩形对账。
//!
//! 运行：`cargo run -p uiscan-gui --example clip_fixture`
//! 然后另开终端：`cargo run -p uiscan-cli --bin uiscan -- gui --title uiscan-clip-fixture --depth 8`

#[cfg(windows)]
fn main() {
    unsafe { imp::run() }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("这个夹具只在 Windows 上有意义");
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG,
        RegisterClassW, SW_SHOW, ShowWindow, TranslateMessage, WNDCLASSW, WS_BORDER, WS_CHILD,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    use windows::core::w;

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    pub unsafe fn run() {
        let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None) }
            .map(|m| HINSTANCE(m.0))
            .unwrap_or_default();
        let class = w!("uiscan_clip_fixture");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class,
            ..Default::default()
        };
        let _atom = unsafe { RegisterClassW(&wc) };
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                class,
                w!("uiscan-clip-fixture"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                460,
                240,
                None,
                None,
                Some(hinstance),
                None,
            )
        }
        .expect("建窗口");
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };

        let edit_style = WS_CHILD | WS_VISIBLE | WS_BORDER;
        // 放得下
        let _fits = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!("短文案"),
                edit_style,
                20,
                20,
                300,
                28,
                Some(hwnd),
                None,
                Some(hinstance),
                None,
            )
        }
        .expect("建放得下的 EDIT");
        // 被裁：窗口宽 160，文字远长于它（没有横向滚动条）
        let _clipped = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!("这是一段很长很长很长很长很长很长很长的中文文案会被单行裁掉"),
                edit_style,
                20,
                70,
                160,
                28,
                Some(hwnd),
                None,
                Some(hinstance),
                None,
            )
        }
        .expect("建被裁的 EDIT");

        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            let _ = unsafe { TranslateMessage(&msg) };
            let _ = unsafe { DispatchMessageW(&msg) };
        }
    }
}
