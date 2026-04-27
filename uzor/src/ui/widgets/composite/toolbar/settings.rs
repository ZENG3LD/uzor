//! Toolbar settings bundle.

use super::theme::{DefaultToolbarTheme, ToolbarTheme};
use super::style::{DefaultToolbarStyle, ToolbarStyle};

pub struct ToolbarSettings {
    pub theme: Box<dyn ToolbarTheme>,
    pub style: Box<dyn ToolbarStyle>,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultToolbarTheme>::default(),
            style: Box::new(DefaultToolbarStyle),
        }
    }
}
