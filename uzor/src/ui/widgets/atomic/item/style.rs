//! Item widget geometry trait and presets.
//!
//! Ported from `button/style.rs` `ToolbarLabelStyle`.

/// Geometry parameters for `draw_item`.
pub trait ItemStyle {
    /// Font string used to render the text. Default: `"13px sans-serif"`.
    fn font(&self) -> &str {
        "13px sans-serif"
    }

    /// Icon side length in pixels. Default: 16.0.
    fn icon_size(&self) -> f64 {
        16.0
    }

    /// Gap between icon and text when both present. Default: 4.0.
    fn icon_text_gap(&self) -> f64 {
        4.0
    }

    /// Horizontal padding between widget edge and content. Default: 4.0.
    fn padding_x(&self) -> f64 {
        4.0
    }
}

/// Default item style.
pub struct DefaultItemStyle;

impl ItemStyle for DefaultItemStyle {}

/// Toolbar item style — matches `ToolbarLabelStyle` from `button/style.rs`.
/// `font_size=11`, `gap=2`, `padding_x=4`.
pub struct ToolbarItemStyle;

impl ItemStyle for ToolbarItemStyle {
    fn font(&self) -> &str       { "11px sans-serif" }
    fn icon_size(&self) -> f64   { 16.0 }
    fn icon_text_gap(&self) -> f64 { 2.0 }
    fn padding_x(&self) -> f64   { 4.0 }
}
