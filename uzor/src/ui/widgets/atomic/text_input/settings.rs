//! Bundled per-instance settings: theme + style + behavior.
//!
//! Callers pass a single reference to `render::draw_input` instead of
//! threading three independent params.

use super::state::TextFieldConfig;
use super::style::{DefaultTextInputStyle, TextInputStyle};
use super::theme::{DefaultTextInputTheme, TextInputTheme};

/// Aggregates everything the renderer + input layer need to know about
/// a particular text input instance.
pub struct TextInputSettings {
    /// Colours (state-dependent).
    pub theme: Box<dyn TextInputTheme>,
    /// Geometry (radius / padding / font size / cursor blink timing / …).
    pub style: Box<dyn TextInputStyle>,
    /// Validation + filtering + live-update flag.
    pub config: TextFieldConfig,
}

impl TextInputSettings {
    /// Build settings with the supplied behavior config and the default
    /// dark theme + default style.
    pub fn with_config(config: TextFieldConfig) -> Self {
        Self {
            theme:  Box::new(DefaultTextInputTheme),
            style:  Box::new(DefaultTextInputStyle),
            config,
        }
    }

    pub fn with_theme(mut self, theme: Box<dyn TextInputTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_style(mut self, style: Box<dyn TextInputStyle>) -> Self {
        self.style = style;
        self
    }
}
