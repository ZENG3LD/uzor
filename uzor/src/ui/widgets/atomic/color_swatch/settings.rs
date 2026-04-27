//! Bundled per-instance settings for color swatch widgets.

use super::style::{ColorSwatchStyle, FillToggleStyle, PrimitiveFillToggleStyle, SimpleSwatchStyle};
use super::theme::{ColorSwatchTheme, DefaultColorSwatchTheme};

/// Aggregates the visual configuration for a color swatch instance.
pub struct ColorSwatchSettings {
    /// Colors (state-dependent).
    pub theme: Box<dyn ColorSwatchTheme>,
    /// Geometry (size, radius, borders, hover expand).
    pub style: Box<dyn ColorSwatchStyle>,
    /// Geometry for fill-toggle variant.
    pub fill_toggle_style: Box<dyn FillToggleStyle>,
}

impl Default for ColorSwatchSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultColorSwatchTheme),
            style: Box::new(SimpleSwatchStyle),
            fill_toggle_style: Box::new(PrimitiveFillToggleStyle),
        }
    }
}

impl ColorSwatchSettings {
    pub fn with_theme(mut self, theme: Box<dyn ColorSwatchTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn ColorSwatchStyle>) -> Self {
        self.style = style;
        self
    }

    pub fn with_fill_toggle_style(mut self, style: Box<dyn FillToggleStyle>) -> Self {
        self.fill_toggle_style = style;
        self
    }
}
