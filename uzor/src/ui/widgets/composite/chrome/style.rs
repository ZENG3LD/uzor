//! Chrome geometry parameters trait and preset implementations.
//!
//! Style holds layout numbers only — colours live in `ChromeTheme`.
//! All defaults are sourced from `chrome-deep.md` §7.

// ---------------------------------------------------------------------------
// BackgroundFill  (re-used from modal style pattern)
// ---------------------------------------------------------------------------

/// Selects how the Chrome titlebar strip background is filled.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Solid colour from `theme.background()`.
    Solid,
    /// GPU blur + semi-transparent overlay (glass effect).
    Glass {
        /// Blur kernel radius in pixels.
        blur_radius: f64,
    },
    /// Tiled texture looked up via the asset system.
    Texture {
        /// Asset identifier.
        asset_id: &'static str,
    },
}

// ---------------------------------------------------------------------------
// ChromeStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the Chrome composite.
pub trait ChromeStyle {
    /// Height of the titlebar strip in pixels.  Default: `32.0`.
    fn chrome_height(&self) -> f64 { 32.0 }

    /// Horizontal padding per side of a tab (left + right label gutter).
    /// Default: `12.0`.
    fn tab_padding_h(&self) -> f64 { 12.0 }

    /// Width of the close-X zone inside each tab.  Default: `16.0`.
    fn tab_close_size(&self) -> f64 { 16.0 }

    /// Gap between adjacent tabs in pixels.  Default: `1.0`.
    fn tab_gap(&self) -> f64 { 1.0 }

    /// Left margin before the first tab.  Default: `4.0`.
    fn tab_left_margin(&self) -> f64 { 4.0 }

    /// Height of the active / hover bottom accent line on a tab.  Default: `2.0`.
    fn tab_accent_height(&self) -> f64 { 2.0 }

    /// Width of the "+" new-tab button.  Default: `28.0`.
    fn new_tab_btn_width(&self) -> f64 { 28.0 }

    /// Width of the minimize / maximize / close-app buttons.  Default: `46.0`.
    fn button_size_min(&self) -> f64 { 46.0 }

    /// Width of the close-window / menu / new-window buttons.  Default: `36.0`.
    fn button_size_max(&self) -> f64 { 36.0 }

    /// Width of the close-window button specifically.  Default: `36.0`.
    fn button_size_close(&self) -> f64 { 36.0 }

    /// Resize-zone thickness at each edge of the window.  Default: `4.0`.
    fn border_zone(&self) -> f64 { 4.0 }

    /// Minimum width of the caption drag zone.  Default: `0.0` (can shrink to zero).
    fn drag_zone_min_width(&self) -> f64 { 0.0 }

    /// SVG action icon target size.  Default: `18.0`.
    fn action_icon_size(&self) -> f64 { 18.0 }

    /// Close-× icon size inside a tab.  Default: `14.0`.
    fn tab_close_icon_size(&self) -> f64 { 14.0 }

    /// Context-menu width.  Default: `160.0`.
    fn context_menu_width(&self) -> f64 { 160.0 }

    /// Context-menu item height.  Default: `28.0`.
    fn context_menu_item_height(&self) -> f64 { 28.0 }

    /// Context-menu top/bottom padding.  Default: `4.0`.
    fn context_menu_padding(&self) -> f64 { 4.0 }

    /// Tooltip internal padding.  Default: `6.0`.
    fn tooltip_padding(&self) -> f64 { 6.0 }

    /// Distance below cursor to draw the tooltip.  Default: `20.0`.
    fn tooltip_below_cursor(&self) -> f64 { 20.0 }

    /// Background fill strategy for the chrome strip.
    fn background_fill(&self) -> BackgroundFill { BackgroundFill::Solid }
}

// ---------------------------------------------------------------------------
// Preset: DefaultChromeStyle  (full chrome)
// ---------------------------------------------------------------------------

/// Default style — full chrome with tabs, drag zone, and window controls.
#[derive(Default)]
pub struct DefaultChromeStyle;

impl ChromeStyle for DefaultChromeStyle {}

// ---------------------------------------------------------------------------
// Preset: MinimalChromeStyle  (tabs only)
// ---------------------------------------------------------------------------

/// Minimal style — tab strip only, no window controls.
///
/// Suitable for embedded / child windows.
#[derive(Default)]
pub struct MinimalChromeStyle;

impl ChromeStyle for MinimalChromeStyle {
    // Same geometry; the `Minimal` render kind handles hiding controls.
}

// ---------------------------------------------------------------------------
// Preset: WindowControlsOnlyChromeStyle
// ---------------------------------------------------------------------------

/// Style preset for the `WindowControlsOnly` render kind.
///
/// No tabs rendered; only the min/max/close button column is shown.
#[derive(Default)]
pub struct WindowControlsOnlyChromeStyle;

impl ChromeStyle for WindowControlsOnlyChromeStyle {
    // Same metrics; the render kind suppresses everything except buttons.
}
