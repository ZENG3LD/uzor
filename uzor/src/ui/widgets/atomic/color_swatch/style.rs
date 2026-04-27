//! ColorSwatch geometry presets (sections 27-31).

/// Geometry parameters for color swatch rendering.
///
/// Controls swatch square size, border/outline widths, corner radius,
/// hover-expand amount, and checkerboard tile size (for transparency variant).
pub trait ColorSwatchStyle {
    /// Side length of the swatch square in pixels.
    fn swatch_size(&self) -> f64;
    /// Corner radius. `0.0` = sharp corners, `3.0-4.0` = rounded.
    fn radius(&self) -> f64;
    /// Border stroke width drawn around the idle swatch.
    fn border_width(&self) -> f64;
    /// Extra expand (pixels on each side) for the hover-highlight rect.
    /// `0.0` means no expand. mlc indicator_settings uses `2.0` (+4 total).
    fn hover_expand(&self) -> f64;
    /// Stroke width for the selected/picker-open border.
    /// mlc indicator_settings: `2.0` when picker open, `1.0` otherwise.
    fn selected_border_width(&self) -> f64;
    /// Checkerboard tile side length (relevant only for `WithTransparency` variant).
    /// mlc appearance tab: `swatch_size / 2.0` (9.0 for 18×18 swatch).
    fn checker_tile_size(&self) -> f64 {
        self.swatch_size() / 2.0
    }
}

/// Simple color swatch style — chart_settings instrument tab (section 27).
/// 24×24, radius 4.0, 1 px border, no hover expand.
pub struct SimpleSwatchStyle;

impl ColorSwatchStyle for SimpleSwatchStyle {
    fn swatch_size(&self)           -> f64 { 24.0 }
    fn radius(&self)                -> f64 { 4.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn hover_expand(&self)          -> f64 { 0.0 }
    fn selected_border_width(&self) -> f64 { 1.0 }
}

/// Indicator swatch style — indicator_settings Style tab (section 28).
/// 20×20, radius 0.0 (sharp), 1 px border, 2 px hover expand, 2 px selected border.
pub struct IndicatorSwatchStyle;

impl ColorSwatchStyle for IndicatorSwatchStyle {
    fn swatch_size(&self)           -> f64 { 20.0 }
    fn radius(&self)                -> f64 { 0.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn hover_expand(&self)          -> f64 { 2.0 }
    fn selected_border_width(&self) -> f64 { 2.0 }
}

/// Appearance swatch style — appearance tab with transparency checkerboard (section 29).
/// 18×18, radius 0.0, 1 px border, no hover expand.
/// Checker tile = 9×9 (swatch_size / 2).
pub struct AppearanceSwatchStyle;

impl ColorSwatchStyle for AppearanceSwatchStyle {
    fn swatch_size(&self)           -> f64 { 18.0 }
    fn radius(&self)                -> f64 { 0.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn hover_expand(&self)          -> f64 { 0.0 }
    fn selected_border_width(&self) -> f64 { 1.0 }
}

/// Primitive-level swatch style — primitive_settings level color (section 30).
/// Width = 16, height = variable (caller passes rect). Radius 3.0, 1 px border.
/// `swatch_size` returns the fixed width; height comes from the caller rect.
pub struct PrimitiveSwatchStyle;

impl ColorSwatchStyle for PrimitiveSwatchStyle {
    fn swatch_size(&self)           -> f64 { 16.0 }
    fn radius(&self)                -> f64 { 3.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn hover_expand(&self)          -> f64 { 0.0 }
    fn selected_border_width(&self) -> f64 { 1.0 }
}

/// Geometry parameters for fill-toggle rendering (section 31).
///
/// The fill toggle is a small rounded square that shows the fill color when
/// enabled and a diagonal strikethrough when disabled.
pub trait FillToggleStyle {
    /// Square side length. mlc: 16 × `row_height - 8` (variable height).
    fn size(&self) -> f64;
    /// Corner radius. mlc: 3.0.
    fn radius(&self) -> f64;
    /// Border stroke width. mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default fill-toggle style — primitive_settings level fill (section 31).
/// Width 16, radius 3.0. Height is variable — pass it in the rect.
pub struct PrimitiveFillToggleStyle;

impl FillToggleStyle for PrimitiveFillToggleStyle {
    fn size(&self)         -> f64 { 16.0 }
    fn radius(&self)       -> f64 { 3.0 }
    fn border_width(&self) -> f64 { 1.0 }
}
