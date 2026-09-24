//! Bundled per-instance settings for the radio widget.

use super::theme::RadioTheme;
use super::style::{DefaultRadioPairStyle, DefaultRadioStyle, RadioPairStyle, RadioStyle};
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Aggregates visual configuration for a radio widget.
pub struct RadioSettings {
    /// Color slots.
    pub theme: Box<dyn RadioTheme>,
    /// Geometry for the `Group` variant.
    pub group_style: Box<dyn RadioStyle>,
    /// Geometry for the `Pair` / `Dot` variants.
    pub pair_style: Box<dyn RadioPairStyle>,
}

impl Default for RadioSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            group_style: Box::new(DefaultRadioStyle),
            pair_style: Box::new(DefaultRadioPairStyle),
        }
    }
}

impl RadioSettings {
    pub fn with_theme(mut self, theme: Box<dyn RadioTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_group_style(mut self, style: Box<dyn RadioStyle>) -> Self {
        self.group_style = style;
        self
    }

    pub fn with_pair_style(mut self, style: Box<dyn RadioPairStyle>) -> Self {
        self.pair_style = style;
        self
    }
}
