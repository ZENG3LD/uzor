//! Panel settings bundle.

use super::theme::{DefaultPanelTheme, PanelTheme};
use super::style::{DefaultPanelStyle, PanelStyle};

pub struct PanelSettings {
    pub theme: Box<dyn PanelTheme>,
    pub style: Box<dyn PanelStyle>,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultPanelTheme>::default(),
            style: Box::new(DefaultPanelStyle),
        }
    }
}
