//! Chrome colour palette trait and default dark-theme implementation.
//!
//! All colour values are CSS hex strings (same convention as `ModalTheme`).
//! The renderer converts them via `RenderContext::set_fill_color`.

// ---------------------------------------------------------------------------
// ChromeTheme trait
// ---------------------------------------------------------------------------

/// Colour tokens for the Chrome composite.
///
/// Implement this on your app theme struct to plug in custom colours.
pub trait ChromeTheme {
    // --- Background ---

    /// Titlebar strip background.  Default: `"#131722"`.
    fn background(&self) -> &str;

    // --- Icons ---

    /// Icon colour in normal (idle) state.  Default: `"#a6adc8"`.
    fn icon_normal(&self) -> &str;

    /// Icon colour on hover / active.  Default: `"#cdd6f4"`.
    fn icon_hover(&self) -> &str;

    // --- Button hover backgrounds ---

    /// Hover background fill for most buttons (min, max, menu, …).
    /// Default: `"#1f2937"`.
    fn button_hover(&self) -> &str;

    /// Hover background fill for the close-app button (red).
    /// Default: `"#e81123"`.
    fn close_hover(&self) -> &str;

    // --- Structural ---

    /// 1 px divider lines between button groups and the bottom edge.
    /// Default: `"#313244"`.
    fn separator(&self) -> &str;

    // --- Tab strip ---

    /// Normal (inactive) tab background.  Default: `"transparent"` (same as bg).
    fn tab_bg_normal(&self) -> &str;

    /// Tab background on hover.  Default: `"#1f2937"`.
    fn tab_bg_hover(&self) -> &str;

    /// Active tab background.  Default: `"#1e222d"` (slightly lighter than bg).
    fn tab_bg_active(&self) -> &str;

    /// Tab label text — inactive / normal.  Default: `"#a6adc8"`.
    fn tab_text_normal(&self) -> &str;

    /// Tab label text — hovered.  Default: `"#cdd6f4"`.
    fn tab_text_hover(&self) -> &str;

    /// Tab label text — active.  Default: `"#ffffff"`.
    fn tab_text_active(&self) -> &str;

    /// Active tab bottom accent line (2 px).  Default: `"#3b82f6"`.
    fn tab_accent(&self) -> &str;

    // --- Drag zone ---

    /// Caption / drag zone background (usually transparent).
    /// Default: `"transparent"`.
    fn drag_zone_bg(&self) -> &str;

    // --- Tooltip ---

    /// Tooltip background.  Default: `"#323232"`.
    fn tooltip_bg(&self) -> &str;

    /// Tooltip text colour.  Default: `"#ffffff"`.
    fn tooltip_text(&self) -> &str;
}

// ---------------------------------------------------------------------------
// DefaultChromeTheme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation — values from `chrome-deep.md` §7.
#[derive(Default)]
pub struct DefaultChromeTheme;

impl ChromeTheme for DefaultChromeTheme {
    // Background
    fn background(&self)    -> &str { "#131722" }

    // Icons
    fn icon_normal(&self)   -> &str { "#a6adc8" }
    fn icon_hover(&self)    -> &str { "#cdd6f4" }

    // Button hover backgrounds
    fn button_hover(&self)  -> &str { "#1f2937" }
    fn close_hover(&self)   -> &str { "#e81123" }

    // Structural
    fn separator(&self)     -> &str { "#313244" }

    // Tab strip
    fn tab_bg_normal(&self) -> &str { "transparent" }
    fn tab_bg_hover(&self)  -> &str { "#1f2937" }
    fn tab_bg_active(&self) -> &str { "#1e222d" }

    fn tab_text_normal(&self) -> &str { "#a6adc8" }
    fn tab_text_hover(&self)  -> &str { "#cdd6f4" }
    fn tab_text_active(&self) -> &str { "#ffffff" }

    fn tab_accent(&self)    -> &str { "#3b82f6" }

    // Drag zone
    fn drag_zone_bg(&self)  -> &str { "transparent" }

    // Tooltip
    fn tooltip_bg(&self)    -> &str { "#323232" }
    fn tooltip_text(&self)  -> &str { "#ffffff" }
}
