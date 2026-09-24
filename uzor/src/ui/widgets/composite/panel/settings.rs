//! Panel settings bundle — `PanelTheme` + `PanelStyle` in one box.

use super::style::{DefaultPanelStyle, PanelStyle};
use super::theme::PanelTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Combined visual configuration for the panel composite.
pub struct PanelSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn PanelTheme>,
    /// Geometry parameters (varies with panel kind).
    pub style: Box<dyn PanelStyle>,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultPanelStyle>::default(),
        }
    }
}
