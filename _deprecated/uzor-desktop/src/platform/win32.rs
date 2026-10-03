//! Windows-specific platform integration: cursor capture and DWM border styling.

/// Win32 cursor position polling helpers.
pub mod win32_capture {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    extern "system" {
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        fn ScreenToClient(hWnd: isize, lpPoint: *mut POINT) -> i32;
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    /// Get cursor position in window-local coordinates.
    pub fn get_cursor_pos(window: &Window) -> Option<(f64, f64)> {
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(h) = handle.as_ref() {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    if GetCursorPos(&mut pt) != 0 {
                        ScreenToClient(h.hwnd.get(), &mut pt);
                        return Some((pt.x as f64, pt.y as f64));
                    }
                }
            }
        }
        None
    }
}

/// DWM window border color control (Windows 11+).
pub mod win32_border {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            dw_attribute: u32,
            pv_attribute: *const u32,
            cb_attribute: u32,
        ) -> i32;
    }

    const DWMWA_BORDER_COLOR: u32 = 34;

    fn hex_to_colorref(hex: &str) -> Option<u32> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((b as u32) << 16 | (g as u32) << 8 | r as u32)
    }

    /// Extract HWND from a winit `Window` (must be called on the main thread).
    pub fn extract_hwnd(window: &Window) -> Option<isize> {
        let handle = window.window_handle().ok()?;
        if let RawWindowHandle::Win32(h) = handle.as_ref() {
            Some(h.hwnd.get())
        } else {
            None
        }
    }

    /// Apply the DWM border color using a cached HWND.
    ///
    /// `color` must be a `#RRGGBB` hex string. Invalid strings or OS versions
    /// that do not support this attribute are silently ignored.
    pub fn set_dwm_border_color(hwnd: isize, color: &str) {
        let Some(colorref) = hex_to_colorref(color) else {
            return;
        };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &colorref as *const u32,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}
