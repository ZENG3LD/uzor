//! Chevron settings — theme + style bundle.

use super::style::{ChevronStyle, DefaultChevronStyle};
use super::theme::ChevronTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

pub struct ChevronSettings {
    pub theme: Box<dyn ChevronTheme>,
    pub style: Box<dyn ChevronStyle>,
}

impl Default for ChevronSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultChevronStyle>::default(),
        }
    }
}
