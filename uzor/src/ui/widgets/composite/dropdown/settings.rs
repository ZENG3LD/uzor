//! Dropdown settings bundle — `DropdownTheme` + `DropdownStyle` in one box.

use super::style::{DefaultDropdownStyle, DropdownStyle};
use super::theme::{DefaultDropdownTheme, DropdownTheme};

/// Combined visual configuration for the Dropdown composite.
pub struct DropdownSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn DropdownTheme>,
    /// Geometry parameters (varies with dropdown kind).
    pub style: Box<dyn DropdownStyle>,
}

impl Default for DropdownSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultDropdownTheme>::default(),
            style: Box::<DefaultDropdownStyle>::default(),
        }
    }
}
