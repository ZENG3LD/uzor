use super::style::{DefaultTooltipStyle, TooltipStyle};
use super::theme::TooltipTheme;
use crate::tokens::{BuiltinSet, TokenTheme, Tokens};

/// Bundles theme + style for a tooltip.
pub struct TooltipSettings {
    pub theme: Box<dyn TooltipTheme>,
    pub style: Box<dyn TooltipStyle>,
}

impl Default for TooltipSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::new(DefaultTooltipStyle),
        }
    }
}
