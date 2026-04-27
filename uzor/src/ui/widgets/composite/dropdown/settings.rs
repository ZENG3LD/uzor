//! Dropdown settings bundle.

use super::theme::{DefaultDropdownTheme, DropdownTheme};
use super::style::{DefaultDropdownStyle, DropdownStyle};

pub struct DropdownSettings {
    pub theme: Box<dyn DropdownTheme>,
    pub style: Box<dyn DropdownStyle>,
}

impl Default for DropdownSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultDropdownTheme>::default(),
            style: Box::new(DefaultDropdownStyle),
        }
    }
}
