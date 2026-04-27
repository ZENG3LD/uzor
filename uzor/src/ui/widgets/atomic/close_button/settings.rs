//! Bundled per-instance settings for the close button widget.

use super::style::{CloseButtonStyle, DefaultCloseButtonStyle};
use super::theme::{CloseButtonTheme, DefaultCloseButtonTheme};

/// Aggregates visual configuration for a close button instance.
pub struct CloseButtonSettings {
    /// Color slots.
    pub theme: Box<dyn CloseButtonTheme>,
    /// Geometry (size, stroke width, inset).
    pub style: Box<dyn CloseButtonStyle>,
}

impl Default for CloseButtonSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultCloseButtonTheme),
            style: Box::new(DefaultCloseButtonStyle),
        }
    }
}

impl CloseButtonSettings {
    pub fn with_theme(mut self, theme: Box<dyn CloseButtonTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn CloseButtonStyle>) -> Self {
        self.style = style;
        self
    }
}
