//! BlackboxPanel settings bundle — theme + style in one box.

use super::style::{BlackboxStyle, DefaultBlackboxStyle};
use super::theme::BlackboxTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

// ---------------------------------------------------------------------------
// BlackboxPanelSettings
// ---------------------------------------------------------------------------

/// Combined visual configuration for the blackbox panel composite.
pub struct BlackboxPanelSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn BlackboxTheme>,
    /// Geometry parameters.
    pub style: Box<dyn BlackboxStyle>,
}

impl Default for BlackboxPanelSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultBlackboxStyle>::default(),
        }
    }
}
