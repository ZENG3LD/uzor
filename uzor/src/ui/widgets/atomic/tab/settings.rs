use super::style::{DefaultTabStyle, TabStyle};
use super::theme::{DefaultTabTheme, TabTheme};

pub struct TabSettings {
    pub theme: Box<dyn TabTheme>,
    pub style: Box<dyn TabStyle>,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultTabTheme>::default(),
            style: Box::new(DefaultTabStyle),
        }
    }
}
