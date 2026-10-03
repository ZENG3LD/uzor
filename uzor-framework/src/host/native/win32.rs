//! Windows-specific platform helpers + borderless `WM_NCHITTEST` subclass.
//!
//! Combines the former `uzor-desktop` cursor/DWM helpers with tessera's
//! borderless-resize subclass (H3 brief 6 / design §7.3). On non-Windows
//! targets every public entry is a no-op.

use winit::window::Window;

use uzor::CornerStyle;
use uzor_window_desktop::win_dwm;

/// Extract the Win32 HWND from a winit window (`None` on other platforms).
pub fn extract_hwnd(window: &Window) -> Option<isize> {
    win_dwm::extract_hwnd(window)
}

/// Apply a DWM corner preference (Windows 11+); no-op elsewhere.
pub fn set_corner_style(window: &Window, style: CornerStyle) {
    if let Some(hwnd) = extract_hwnd(window) {
        win_dwm::set_dwm_corner_preference(hwnd, style);
    }
}

/// Apply a DWM border colour (`0xAARRGGBB`); `None` restores the OS default.
pub fn set_border_color(window: &Window, color: Option<u32>) {
    if let Some(hwnd) = extract_hwnd(window) {
        win_dwm::set_dwm_border_color(hwnd, color);
    }
}

/// Cursor position in window-local physical pixels (Windows only).
pub fn get_cursor_pos(window: &Window) -> Option<(f64, f64)> {
    #[cfg(target_os = "windows")]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        extern "system" {
            fn GetCursorPos(lp_point: *mut Point) -> i32;
            fn ScreenToClient(h_wnd: isize, lp_point: *mut Point) -> i32;
        }
        #[repr(C)]
        struct Point {
            x: i32,
            y: i32,
        }
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::Win32(h) = handle.as_ref() else {
            return None;
        };
        let mut pt = Point { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut pt) != 0 {
                ScreenToClient(h.hwnd.get(), &mut pt);
                return Some((f64::from(pt.x), f64::from(pt.y)));
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// Install the borderless-resize `WM_NCHITTEST` subclass on `window`.
///
/// Call once after `create_window` when decorations are off. No-op on
/// non-Windows targets and when the HWND cannot be extracted.
pub fn install_borderless_resize(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = extract_hwnd(window) {
            borderless::install(hwnd);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// Windows-only `WM_NCHITTEST` subclass (tessera `borderless_resize.rs`).
#[cfg(target_os = "windows")]
mod borderless {
    type Hwnd = isize;
    type Wparam = usize;
    type Lparam = isize;
    type Lresult = isize;
    type DwordPtr = usize;
    type Uint = u32;

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: Hwnd,
            callback: SubclassProc,
            id: usize,
            ref_data: DwordPtr,
        ) -> i32;
        fn DefSubclassProc(hwnd: Hwnd, msg: Uint, wp: Wparam, lp: Lparam) -> Lresult;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetClientRect(hwnd: Hwnd, lp_rect: *mut Rect) -> i32;
        fn ClientToScreen(hwnd: Hwnd, lp_point: *mut Point) -> i32;
    }

    type SubclassProc = unsafe extern "system" fn(
        hwnd: Hwnd,
        msg: Uint,
        wp: Wparam,
        lp: Lparam,
        id: usize,
        ref_data: DwordPtr,
    ) -> Lresult;

    const WM_NCHITTEST: Uint = 0x0084;
    const HTCLIENT: Lresult = 1;
    const HTLEFT: Lresult = 10;
    const HTRIGHT: Lresult = 11;
    const HTTOP: Lresult = 12;
    const HTTOPLEFT: Lresult = 13;
    const HTTOPRIGHT: Lresult = 14;
    const HTBOTTOM: Lresult = 15;
    const HTBOTTOMLEFT: Lresult = 16;
    const HTBOTTOMRIGHT: Lresult = 17;
    const BORDER_PX: i32 = 6;
    const SUBCLASS_ID: usize = 0x755A_4F52; // "uZOR"

    unsafe extern "system" fn proc(
        hwnd: Hwnd,
        msg: Uint,
        wp: Wparam,
        lp: Lparam,
        _id: usize,
        _ref: DwordPtr,
    ) -> Lresult {
        if msg == WM_NCHITTEST {
            let screen_x = (lp as i32) & 0xFFFF;
            let screen_y = ((lp as i32) >> 16) & 0xFFFF;
            let screen_x = screen_x as i16 as i32;
            let screen_y = screen_y as i16 as i32;

            let mut rect = Rect::default();
            if GetClientRect(hwnd, &mut rect) != 0 {
                let mut origin = Point { x: 0, y: 0 };
                if ClientToScreen(hwnd, &mut origin) != 0 {
                    let x = screen_x - origin.x;
                    let y = screen_y - origin.y;
                    let w = rect.right;
                    let h = rect.bottom;
                    let b = BORDER_PX;
                    let on_left = x >= 0 && x < b;
                    let on_right = x < w && x >= w - b;
                    let on_top = y >= 0 && y < b;
                    let on_bottom = y < h && y >= h - b;
                    let hit = match (on_top, on_bottom, on_left, on_right) {
                        (true, false, true, false) => Some(HTTOPLEFT),
                        (true, false, false, true) => Some(HTTOPRIGHT),
                        (false, true, true, false) => Some(HTBOTTOMLEFT),
                        (false, true, false, true) => Some(HTBOTTOMRIGHT),
                        (true, false, false, false) => Some(HTTOP),
                        (false, true, false, false) => Some(HTBOTTOM),
                        (false, false, true, false) => Some(HTLEFT),
                        (false, false, false, true) => Some(HTRIGHT),
                        _ => None,
                    };
                    if let Some(code) = hit {
                        return code;
                    }
                    return HTCLIENT;
                }
            }
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }

    pub(super) fn install(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            SetWindowSubclass(hwnd, proc, SUBCLASS_ID, 0);
        }
    }
}
