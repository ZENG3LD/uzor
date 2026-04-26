use super::style::{DefaultScrollbarStyle, ScrollbarStyle};
use super::theme::ScrollbarTheme;

pub struct ScrollbarSettings {
    pub theme: Box<dyn ScrollbarTheme>,
    pub style: Box<dyn ScrollbarStyle>,
}

impl Default for ScrollbarSettings {
    fn default() -> Self {
        Self {
            theme: Box::<super::theme::DefaultScrollbarTheme>::default(),
            style: Box::new(DefaultScrollbarStyle),
        }
    }
}
