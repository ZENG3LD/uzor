//! DWM window-decoration helpers for Windows 11+.
//!
//! Provides:
//! - [`set_dwm_corner_preference`] — maps [`CornerStyle`] to `DWMWA_WINDOW_CORNER_PREFERENCE`
//! - [`set_dwm_border_color`] — sets `DWMWA_BORDER_COLOR` from an ARGB `u32`
//! - [`extract_hwnd`] — extract the Win32 HWND from a winit `Window`
//!
//! All functions are no-ops when called on non-Win32 window handles or when
//! the OS does not support the DWM attribute (pre-Windows 11).

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use uzor_window_hub::lifecycle::CornerStyle;

// ── Win32 bindings ────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        dw_attribute: u32,
        pv_attribute: *const u32,
        cb_attribute: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
#[cfg(target_os = "windows")]
const DWMWA_BORDER_COLOR: u32 = 34;

/// `DWMWA_COLOR_DEFAULT` — sentinel that tells DWM to restore the system
/// default border colour.
#[cfg(target_os = "windows")]
const DWMWA_COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the Win32 HWND from a winit `Window`.
///
/// Returns `None` when the underlying handle is not a Win32 handle (e.g. on
/// macOS / Linux or in headless tests).
pub fn extract_hwnd(window: &Window) -> Option<isize> {
    let handle = window.window_handle().ok()?;
    if let RawWindowHandle::Win32(h) = handle.as_ref() {
        Some(h.hwnd.get())
    } else {
        None
    }
}

/// Apply a DWM corner-rounding preference using a cached HWND.
///
/// On Windows 11+ maps to `DWMWA_WINDOW_CORNER_PREFERENCE`:
/// - `Default`      → 0 (DWMWCP_DEFAULT)
/// - `Sharp`        → 1 (DWMWCP_DONOTROUND)
/// - `Rounded`      → 2 (DWMWCP_ROUND)
/// - `RoundedSmall` → 3 (DWMWCP_ROUNDSMALL)
///
/// Silently no-ops on unsupported OS versions or non-Windows targets.
pub fn set_dwm_corner_preference(hwnd: isize, style: CornerStyle) {
    #[cfg(target_os = "windows")]
    {
        let value: u32 = match style {
            CornerStyle::Default      => 0,
            CornerStyle::Sharp        => 1,
            CornerStyle::Rounded      => 2,
            CornerStyle::RoundedSmall => 3,
        };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &value as *const u32,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (hwnd, style);
    }
}

/// Apply a DWM border colour using a cached HWND.
///
/// `color` is an ARGB `u32` in `0x00RRGGBB` COLORREF form (the alpha byte is
/// ignored by DWM). `None` resets the border to the OS default
/// (`DWMWA_COLOR_DEFAULT`).
///
/// Silently no-ops on unsupported OS versions or non-Windows targets.
pub fn set_dwm_border_color(hwnd: isize, color: Option<u32>) {
    #[cfg(target_os = "windows")]
    {
        // Convert caller's 0xAARRGGBB to Win32 COLORREF (0x00BBGGRR).
        let colorref: u32 = match color {
            None => DWMWA_COLOR_DEFAULT,
            Some(argb) => {
                let r = (argb >> 16) & 0xFF;
                let g = (argb >>  8) & 0xFF;
                let b =  argb        & 0xFF;
                (b << 16) | (g << 8) | r
            }
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
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (hwnd, color);
    }
}
