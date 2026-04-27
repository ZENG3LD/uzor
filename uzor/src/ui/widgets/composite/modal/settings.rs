//! Modal settings bundle.

use super::theme::{DefaultModalTheme, ModalTheme};
use super::style::{DefaultModalStyle, ModalStyle};

pub struct ModalSettings {
    pub theme: Box<dyn ModalTheme>,
    pub style: Box<dyn ModalStyle>,
}

impl Default for ModalSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultModalTheme>::default(),
            style: Box::new(DefaultModalStyle),
        }
    }
}
