//! Bundled per-instance settings for shape selector widgets.

use super::style::{SelectorButtonStyle, ShapeSelectorStyle};
use super::theme::{DefaultShapeSelectorTheme, ShapeSelectorTheme};

/// Aggregates the visual configuration for a shape selector instance.
pub struct ShapeSelectorSettings {
    /// Colors (state-dependent).
    pub theme: Box<dyn ShapeSelectorTheme>,
    /// Geometry (size, radius, borders, label gap).
    pub style: Box<dyn SelectorButtonStyle>,
}

impl Default for ShapeSelectorSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultShapeSelectorTheme),
            style: Box::new(ShapeSelectorStyle),
        }
    }
}

impl ShapeSelectorSettings {
    pub fn with_theme(mut self, theme: Box<dyn ShapeSelectorTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn SelectorButtonStyle>) -> Self {
        self.style = style;
        self
    }
}
