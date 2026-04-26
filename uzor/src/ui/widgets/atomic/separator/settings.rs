use super::style::{DefaultSeparatorStyle, SeparatorStyle};
use super::theme::{DefaultSeparatorTheme, SeparatorTheme};

pub struct SeparatorSettings {
    pub theme: Box<dyn SeparatorTheme>,
    pub style: Box<dyn SeparatorStyle>,
}

impl Default for SeparatorSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(DefaultSeparatorStyle),
        }
    }
}
