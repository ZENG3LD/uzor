//! Bundled per-instance settings for the clock widget.

use super::style::{ClockStyle, DefaultClockStyle};
use super::theme::{ClockTheme, DefaultClockTheme};

/// Aggregates visual configuration for a clock instance.
pub struct ClockSettings {
    /// Color slots.
    pub theme: Box<dyn ClockTheme>,
    /// Geometry (font, hover bg radius, vertical inset, padding).
    pub style: Box<dyn ClockStyle>,
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultClockTheme),
            style: Box::new(DefaultClockStyle),
        }
    }
}

impl ClockSettings {
    pub fn with_theme(mut self, theme: Box<dyn ClockTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn ClockStyle>) -> Self {
        self.style = style;
        self
    }
}
