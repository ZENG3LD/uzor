//! Bundled per-instance settings.

use super::style::{DefaultSliderStyle, SliderStyle};
use super::theme::SliderTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

pub struct SliderSettings {
    pub theme: Box<dyn SliderTheme>,
    pub style: Box<dyn SliderStyle>,
}

impl Default for SliderSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::new(DefaultSliderStyle),
        }
    }
}
