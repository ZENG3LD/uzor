//! Modal settings bundle — `ModalTheme` + `ModalStyle` in one box.

use super::style::{DefaultModalStyle, ModalStyle};
use super::theme::ModalTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Combined visual configuration for the modal composite.
pub struct ModalSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn ModalTheme>,
    /// Geometry parameters (varies with modal kind).
    pub style: Box<dyn ModalStyle>,
}

impl Default for ModalSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultModalStyle>::default(),
        }
    }
}
