//! Panel settings bundle — `PanelTheme` + `PanelStyle` in one box.

use super::style::{DefaultPanelStyle, PanelStyle};
use super::theme::{DefaultPanelTheme, PanelTheme};

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
            theme: Box::<DefaultPanelTheme>::default(),
            style: Box::<DefaultPanelStyle>::default(),
        }
    }
}
