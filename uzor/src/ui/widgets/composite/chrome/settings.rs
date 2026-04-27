//! Chrome settings bundle.

use super::theme::{ChromeTheme, DefaultChromeTheme};
use super::style::{ChromeStyle, DefaultChromeStyle};

pub struct ChromeSettings {
    pub theme: Box<dyn ChromeTheme>,
    pub style: Box<dyn ChromeStyle>,
}

impl Default for ChromeSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultChromeTheme>::default(),
            style: Box::new(DefaultChromeStyle),
        }
    }
}
