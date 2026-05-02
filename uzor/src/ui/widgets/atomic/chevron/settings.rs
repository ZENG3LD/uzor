//! Chevron settings — theme + style bundle.

use super::style::{ChevronStyle, DefaultChevronStyle};
use super::theme::{ChevronTheme, DefaultChevronTheme};

pub struct ChevronSettings {
    pub theme: Box<dyn ChevronTheme>,
    pub style: Box<dyn ChevronStyle>,
}

impl Default for ChevronSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultChevronTheme>::default(),
            style: Box::<DefaultChevronStyle>::default(),
        }
    }
}
