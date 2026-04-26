use super::style::{DefaultTooltipStyle, TooltipStyle};
use super::theme::{DefaultTooltipTheme, TooltipTheme};

pub struct TooltipSettings {
    pub theme: Box<dyn TooltipTheme>,
    pub style: Box<dyn TooltipStyle>,
}

impl Default for TooltipSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultTooltipTheme>::default(),
            style: Box::new(DefaultTooltipStyle),
        }
    }
}
