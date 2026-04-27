//! Scroll chevron geometry trait and presets.
//!
//! Ported from `button/style.rs` section 42.

/// Geometry parameters for `draw_scroll_chevron`.
///
/// Ported from `mlc/toolbar_core.rs` `draw_toolbar_with_icons`:
/// `chevron_size = 16.0`.
pub trait ScrollChevronStyle {
    /// Side length of the square hit target. Default: 16.0.
    fn size(&self) -> f64;

    /// Stroke width of the chevron arms. Default: 1.5.
    fn chevron_thickness(&self) -> f64 {
        1.5
    }

    /// Corner radius of the hover-bg fill rect. Default: 4.0.
    fn hover_bg_radius(&self) -> f64 {
        4.0
    }

    /// Inset from the button edge used to size the chevron triangle. Default: 4.0.
    fn chevron_inset(&self) -> f64 {
        4.0
    }
}

/// Default scroll chevron style — matches `mlc/toolbar_core.rs` (16 px).
pub struct DefaultScrollChevronStyle;

impl ScrollChevronStyle for DefaultScrollChevronStyle {
    fn size(&self) -> f64 { 16.0 }
}
