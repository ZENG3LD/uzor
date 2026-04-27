//! Bundled per-instance settings for the scroll chevron widget.

use super::style::{DefaultScrollChevronStyle, ScrollChevronStyle};
use super::theme::{DefaultScrollChevronTheme, ScrollChevronTheme};

/// Aggregates visual configuration for a scroll chevron instance.
pub struct ScrollChevronSettings {
    /// Color slots.
    pub theme: Box<dyn ScrollChevronTheme>,
    /// Geometry (size, stroke width, inset).
    pub style: Box<dyn ScrollChevronStyle>,
}

impl Default for ScrollChevronSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultScrollChevronTheme),
            style: Box::new(DefaultScrollChevronStyle),
        }
    }
}

impl ScrollChevronSettings {
    pub fn with_theme(mut self, theme: Box<dyn ScrollChevronTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn ScrollChevronStyle>) -> Self {
        self.style = style;
        self
    }
}
