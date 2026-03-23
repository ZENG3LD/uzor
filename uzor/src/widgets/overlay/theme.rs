//! Overlay theme trait — visual styling contract for non-tooltip overlays
//!
//! Tooltip-specific theming (including `hover_delay_ms` and `fade_in_duration_ms`)
//! has moved to `input::tooltip::{TooltipTheme, TooltipConfig}`.

/// Theme trait for overlay visual styling.
///
/// Covers info overlays and other non-tooltip overlay variants.
/// For tooltip theming use `TooltipTheme` from the `input::tooltip` module.
pub trait OverlayTheme {
    fn background_color(&self) -> &str;
    fn text_color(&self) -> &str;
    fn border_color(&self) -> &str;
    fn border_width(&self) -> f64;
    fn shadow_color(&self) -> &str;
    fn shadow_blur(&self) -> f64;
    fn shadow_offset(&self) -> (f64, f64);
    fn padding(&self) -> f64;
}

/// Default overlay theme using design specification values
pub struct DefaultOverlayTheme;

impl DefaultOverlayTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultOverlayTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayTheme for DefaultOverlayTheme {
    fn background_color(&self) -> &str { "#323232" }
    fn text_color(&self) -> &str { "#ffffff" }
    fn border_color(&self) -> &str { "#505050" }
    fn border_width(&self) -> f64 { 1.0 }
    fn shadow_color(&self) -> &str { "#00000080" }
    fn shadow_blur(&self) -> f64 { 4.0 }
    fn shadow_offset(&self) -> (f64, f64) { (0.0, 2.0) }
    fn padding(&self) -> f64 { 8.0 }
}
