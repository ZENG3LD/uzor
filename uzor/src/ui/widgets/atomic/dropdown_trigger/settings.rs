//! Bundled per-instance settings for dropdown trigger widgets.

use super::style::{
    DefaultDropdownFieldStyle, DefaultSplitDropdownStyle,
    DropdownFieldStyle, SplitDropdownStyle,
};
use super::theme::{DefaultDropdownTriggerTheme, DropdownTriggerTheme};

/// Aggregates the visual configuration for a dropdown trigger instance.
pub struct DropdownTriggerSettings {
    /// Colors (state-dependent).
    pub theme: Box<dyn DropdownTriggerTheme>,
    /// Geometry for the `Split` variant.
    pub split_style: Box<dyn SplitDropdownStyle>,
    /// Geometry for the `Field` variant.
    pub field_style: Box<dyn DropdownFieldStyle>,
}

impl Default for DropdownTriggerSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultDropdownTriggerTheme),
            split_style: Box::new(DefaultSplitDropdownStyle),
            field_style: Box::new(DefaultDropdownFieldStyle),
        }
    }
}

impl DropdownTriggerSettings {
    pub fn with_theme(mut self, theme: Box<dyn DropdownTriggerTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_split_style(mut self, style: Box<dyn SplitDropdownStyle>) -> Self {
        self.split_style = style;
        self
    }

    pub fn with_field_style(mut self, style: Box<dyn DropdownFieldStyle>) -> Self {
        self.field_style = style;
        self
    }
}
