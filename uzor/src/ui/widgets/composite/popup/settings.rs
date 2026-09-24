//! Popup settings bundle — `PopupTheme` + `PopupStyle` in one box.

use super::style::{DefaultPopupStyle, PopupStyle};
use super::theme::PopupTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Combined visual configuration for the popup composite.
pub struct PopupSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn PopupTheme>,
    /// Geometry parameters (varies with popup kind).
    pub style: Box<dyn PopupStyle>,
}

impl Default for PopupSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultPopupStyle>::default(),
        }
    }
}
