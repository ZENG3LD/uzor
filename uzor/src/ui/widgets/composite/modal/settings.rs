//! Modal settings bundle — `ModalTheme` + `ModalStyle` in one box.

use super::style::{DefaultModalStyle, ModalStyle};
use super::theme::{DefaultModalTheme, ModalTheme};

/// Combined visual configuration for the modal composite.
pub struct ModalSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn ModalTheme>,
    /// Geometry parameters (varies with modal kind).
    pub style: Box<dyn ModalStyle>,
}

impl Default for ModalSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultModalTheme>::default(),
            style: Box::<DefaultModalStyle>::default(),
        }
    }
}
