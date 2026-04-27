//! Bundled per-instance settings for the item widget.

use super::style::{DefaultItemStyle, ItemStyle};
use super::theme::{DefaultItemTheme, ItemTheme};

/// Aggregates visual configuration for an item instance.
pub struct ItemSettings {
    /// Color slots.
    pub theme: Box<dyn ItemTheme>,
    /// Geometry (font, icon size, gap, padding).
    pub style: Box<dyn ItemStyle>,
}

impl Default for ItemSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultItemTheme),
            style: Box::new(DefaultItemStyle),
        }
    }
}

impl ItemSettings {
    pub fn with_theme(mut self, theme: Box<dyn ItemTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn ItemStyle>) -> Self {
        self.style = style;
        self
    }
}
