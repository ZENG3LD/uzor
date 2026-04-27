//! ChromeTab settings bundle.

use super::theme::{ChromeTabTheme, DefaultChromeTabTheme};
use super::style::{ChromeTabStyle, DefaultChromeTabStyle};

pub struct ChromeTabSettings {
    pub theme: Box<dyn ChromeTabTheme>,
    pub style: Box<dyn ChromeTabStyle>,
}

impl Default for ChromeTabSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultChromeTabTheme>::default(),
            style: Box::new(DefaultChromeTabStyle),
        }
    }
}
