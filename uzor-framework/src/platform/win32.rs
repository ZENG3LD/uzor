//! Windows-specific platform integration: cursor capture and DWM border styling.

/// Win32 cursor position polling helpers.
///
/// When the user places the first point of a drawing primitive (is_drawing() is true)
/// the mouse is not pressed, so winit stops sending CursorMoved events once the cursor
/// leaves the window boundary and the preview freezes.  Instead of relying on OS capture
/// (which winit interferes with), we poll GetCursorPos on every frame so the preview
/// updates continuously regardless of cursor position.
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
///
/// Sets the thin colored border that Windows 11 draws around undecorated windows.
/// Silently ignored on Windows 10 and older — `DwmSetWindowAttribute` with
/// `DWMWA_BORDER_COLOR` returns an error on those versions which we discard.
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

    /// `DWMWA_BORDER_COLOR` — available since Windows 11 Build 22000.
    const DWMWA_BORDER_COLOR: u32 = 34;

    /// Parse `#RRGGBB` into a Win32 COLORREF (`0x00BBGGRR`).
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
    /// `color` must be a `#RRGGBB` hex string.  Invalid strings or OS versions
    /// that do not support this attribute are silently ignored.
    pub fn set_dwm_border_color(hwnd: isize, color: &str) {
        let Some(colorref) = hex_to_colorref(color) else {
            return;
        };
        // Ignore the return value — non-zero means unsupported (Win10/older), which is fine.
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
