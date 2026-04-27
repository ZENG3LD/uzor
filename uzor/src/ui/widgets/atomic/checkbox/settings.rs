//! Bundled per-instance settings for the checkbox widget.

use super::theme::{CheckboxTheme, DefaultCheckboxTheme};
use super::style::{CheckboxStyle, StandardCheckboxStyle};

/// Aggregates visual configuration for a checkbox instance.
pub struct CheckboxSettings {
    /// Color slots.
    pub theme: Box<dyn CheckboxTheme>,
    /// Geometry (size, radius, stroke widths, label gap).
    pub style: Box<dyn CheckboxStyle>,
}

impl Default for CheckboxSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultCheckboxTheme),
            style: Box::new(StandardCheckboxStyle),
        }
    }
}

impl CheckboxSettings {
    pub fn with_theme(mut self, theme: Box<dyn CheckboxTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn CheckboxStyle>) -> Self {
        self.style = style;
        self
    }
}
