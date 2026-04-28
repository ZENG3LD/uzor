//! Bundled per-instance settings for the drag handle widget.

use super::style::{DefaultDragHandleStyle, DragHandleStyle};
use super::theme::{DefaultDragHandleTheme, DragHandleTheme};

/// Aggregates visual configuration for a drag handle instance.
pub struct DragHandleSettings {
    /// Colour tokens.
    pub theme: Box<dyn DragHandleTheme>,
    /// Geometry (dot size, spacing, count).
    pub style: Box<dyn DragHandleStyle>,
}

impl Default for DragHandleSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultDragHandleTheme),
            style: Box::new(DefaultDragHandleStyle),
        }
    }
}

impl DragHandleSettings {
    /// Override the theme.
    pub fn with_theme(mut self, theme: Box<dyn DragHandleTheme>) -> Self {
        self.theme = theme;
        self
    }

    /// Override the style.
    pub fn with_style(mut self, style: Box<dyn DragHandleStyle>) -> Self {
        self.style = style;
        self
    }
}
