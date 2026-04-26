//! Bundled per-instance settings: theme + style.
//!
//! Callers pass a single reference to `render::draw_button` instead of
//! threading two independent params.

use super::style::{ButtonStyle, DefaultButtonStyle};
use super::theme::{ButtonTheme, DefaultButtonTheme};

/// Aggregates the visual configuration for a button instance.
///
/// Behavior (variant data + active/disabled flags) lives in the
/// `ButtonType` value the renderer is drawing; this struct only holds
/// the non-data visual concerns.
pub struct ButtonSettings {
    /// Colours (state-dependent).
    pub theme: Box<dyn ButtonTheme>,
    /// Geometry (radius / padding / icon size / font size / gap).
    pub style: Box<dyn ButtonStyle>,
}

impl Default for ButtonSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultButtonTheme),
            style: Box::new(DefaultButtonStyle),
        }
    }
}

impl ButtonSettings {
    pub fn with_theme(mut self, theme: Box<dyn ButtonTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn ButtonStyle>) -> Self {
        self.style = style;
        self
    }
}
