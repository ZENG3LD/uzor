//! Popup settings bundle.

use super::theme::{DefaultPopupTheme, PopupTheme};
use super::style::{DefaultPopupStyle, PopupStyle};

pub struct PopupSettings {
    pub theme: Box<dyn PopupTheme>,
    pub style: Box<dyn PopupStyle>,
}

impl Default for PopupSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultPopupTheme>::default(),
            style: Box::new(DefaultPopupStyle),
        }
    }
}
