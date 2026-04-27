//! Close button geometry trait and presets.
//!
//! Ported from `button/style.rs` section 41.

/// Geometry parameters for `draw_close_button`.
///
/// - `chart_settings` / `indicator_settings`: 18 × 18 px
/// - `alert_settings`: 20 × 20 px
/// - `profile_manager`: 28 × 28 px with hover bg fill
pub trait CloseButtonStyle {
    /// Side length of the square hit target. Default: 18.0.
    fn size(&self) -> f64;

    /// Stroke width of each X arm. Default: 1.5.
    fn x_stroke_width(&self) -> f64 {
        1.5
    }

    /// Corner radius of the hover-bg fill rect. Default: 4.0.
    fn hover_bg_radius(&self) -> f64 {
        4.0
    }

    /// Inset from the button edge to the X arm endpoints. Default: 4.0.
    fn x_inset(&self) -> f64 {
        4.0
    }
}

/// Default close button — matches `chart_settings` / `indicator_settings` (18 px).
pub struct DefaultCloseButtonStyle;

impl CloseButtonStyle for DefaultCloseButtonStyle {
    fn size(&self) -> f64 { 18.0 }
}

/// Larger close button — matches `profile_manager` (28 px, with hover bg).
pub struct LargeCloseButtonStyle;

impl CloseButtonStyle for LargeCloseButtonStyle {
    fn size(&self) -> f64 { 28.0 }
}
