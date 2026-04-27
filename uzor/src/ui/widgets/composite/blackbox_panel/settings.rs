//! BlackboxPanel settings bundle.

use super::theme::{BlackboxPanelTheme, DefaultBlackboxPanelTheme};
use super::style::{BlackboxPanelStyle, DefaultBlackboxPanelStyle};

pub struct BlackboxPanelSettings {
    pub theme: Box<dyn BlackboxPanelTheme>,
    pub style: Box<dyn BlackboxPanelStyle>,
}

impl Default for BlackboxPanelSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultBlackboxPanelTheme>::default(),
            style: Box::new(DefaultBlackboxPanelStyle),
        }
    }
}
