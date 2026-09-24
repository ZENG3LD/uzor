//! Dropdown settings bundle — `DropdownTheme` + `DropdownStyle` in one box.

use super::style::{DefaultDropdownStyle, DropdownStyle};
use super::theme::DropdownTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

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
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultDropdownStyle>::default(),
        }
    }
}
