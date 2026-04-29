//! Toolbar settings bundle.

use super::style::{DefaultToolbarStyle, ToolbarStyle};
use super::theme::{DefaultToolbarTheme, ToolbarTheme};

/// Bundles theme + style for a toolbar instance.
pub struct ToolbarSettings {
    /// Colour palette.
    pub theme: Box<dyn ToolbarTheme>,
    /// Geometry parameters.
    pub style: Box<dyn ToolbarStyle>,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultToolbarTheme>::default(),
            style: Box::<DefaultToolbarStyle>::default(),
        }
    }
}

impl ToolbarSettings {
    /// Construct with explicit theme and style implementations.
    pub fn new(theme: Box<dyn ToolbarTheme>, style: Box<dyn ToolbarStyle>) -> Self {
        Self { theme, style }
    }
}
