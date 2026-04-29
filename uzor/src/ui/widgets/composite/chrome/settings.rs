//! Chrome settings bundle — `ChromeTheme` + `ChromeStyle` in one box.

use super::style::{ChromeStyle, DefaultChromeStyle};
use super::theme::{ChromeTheme, DefaultChromeTheme};

/// Combined visual configuration for the Chrome composite.
pub struct ChromeSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn ChromeTheme>,
    /// Geometry parameters (varies with chrome kind).
    pub style: Box<dyn ChromeStyle>,
}

impl Default for ChromeSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultChromeTheme>::default(),
            style: Box::<DefaultChromeStyle>::default(),
        }
    }
}
