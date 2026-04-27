//! ShapeSelector geometry presets (sections 34, 39, 40).

/// Geometry parameters for selector-style toggle buttons (shape, theme preset,
/// UI style).  Each button is a square/rectangle visual preview with a selected
/// outline and an optional label.
pub trait SelectorButtonStyle {
    /// Button width.  For square buttons set equal to `height()`.
    fn width(&self) -> f64;
    /// Button height.
    fn height(&self) -> f64;
    /// Corner radius.  mlc shape selector: 4.0; appearance buttons: 3.0.
    fn radius(&self) -> f64;
    /// Border stroke width for idle and hover outlines.
    fn border_width(&self) -> f64;
    /// Border stroke width for the selected outline (may be thicker).
    fn selected_border_width(&self) -> f64;
    /// Font size for the optional label drawn outside the button rect.
    fn label_font_size(&self) -> f64;
    /// Gap between button bottom edge and the label baseline.
    fn label_gap(&self) -> f64 { 4.0 }
}

/// Shape selector button style — 24×24 square (section 34).
/// mlc indicator_settings signals tab: `shape_btn_size = 24.0`.
pub struct ShapeSelectorStyle;

impl SelectorButtonStyle for ShapeSelectorStyle {
    fn width(&self)                 -> f64 { 24.0 }
    fn height(&self)                -> f64 { 24.0 }
    fn radius(&self)                -> f64 { 4.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn selected_border_width(&self) -> f64 { 1.5 }
    fn label_font_size(&self)       -> f64 { 11.0 }
}

/// Theme preset button style — appearance tab preview button (section 39).
/// mlc chart_settings appearance tab: `button_height = 26.0`, full content width.
/// Width is variable (caller sets the rect width), height 26.
pub struct ThemePresetButtonStyle;

impl SelectorButtonStyle for ThemePresetButtonStyle {
    fn width(&self)                 -> f64 { 120.0 }  // default; caller passes actual rect
    fn height(&self)                -> f64 { 26.0 }
    fn radius(&self)                -> f64 { 3.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn selected_border_width(&self) -> f64 { 1.5 }
    fn label_font_size(&self)       -> f64 { 13.0 }
}

/// UI style button style — appearance tab style selector (section 40).
/// Text-only selector, same height as ThemePreset, radius 3.
pub struct UIStyleSelectorStyle;

impl SelectorButtonStyle for UIStyleSelectorStyle {
    fn width(&self)                 -> f64 { 80.0 }  // default; caller passes actual rect
    fn height(&self)                -> f64 { 26.0 }
    fn radius(&self)                -> f64 { 3.0 }
    fn border_width(&self)          -> f64 { 1.0 }
    fn selected_border_width(&self) -> f64 { 1.5 }
    fn label_font_size(&self)       -> f64 { 13.0 }
}
