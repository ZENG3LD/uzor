//! Popup settings bundle — `PopupTheme` + `PopupStyle` in one box.

use super::style::{DefaultPopupStyle, PopupStyle};
use super::theme::{DefaultPopupTheme, PopupTheme};

/// Combined visual configuration for the popup composite.
pub struct PopupSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn PopupTheme>,
    /// Geometry parameters (varies with popup kind).
    pub style: Box<dyn PopupStyle>,
}

impl Default for PopupSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultPopupTheme>::default(),
            style: Box::<DefaultPopupStyle>::default(),
        }
    }
}
