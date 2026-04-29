//! BlackboxPanel settings bundle — theme + style in one box.

use super::style::{BlackboxStyle, DefaultBlackboxStyle};
use super::theme::{BlackboxTheme, DefaultBlackboxTheme};

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
            theme: Box::<DefaultBlackboxTheme>::default(),
            style: Box::<DefaultBlackboxStyle>::default(),
        }
    }
}
