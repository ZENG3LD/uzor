//! Chrome settings bundle — `ChromeTheme` + `ChromeStyle` in one box.

use super::style::{ChromeStyle, DefaultChromeStyle};
use super::theme::ChromeTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Combined visual configuration for the Chrome composite.
pub struct ChromeSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn ChromeTheme>,
    /// Geometry parameters (varies with chrome kind).
    pub style: Box<dyn ChromeStyle>,
}

impl Default for ChromeSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultChromeStyle>::default(),
        }
    }
}
