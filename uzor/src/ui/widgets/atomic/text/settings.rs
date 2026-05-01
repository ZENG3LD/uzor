//! Text widget settings — bundles theme + style.

use super::style::{DefaultTextStyle, TextStyle};
use super::theme::{DefaultTextTheme, TextTheme};

/// Bundles theme and style for a Text widget instance.
pub struct TextSettings {
    pub theme: Box<dyn TextTheme>,
    pub style: Box<dyn TextStyle>,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultTextTheme),
            style: Box::new(DefaultTextStyle),
        }
    }
}

impl TextSettings {
    /// Override the theme.
    pub fn with_theme(mut self, theme: Box<dyn TextTheme>) -> Self {
        self.theme = theme;
        self
    }

    /// Override the style.
    pub fn with_style(mut self, style: Box<dyn TextStyle>) -> Self {
        self.style = style;
        self
    }
}
