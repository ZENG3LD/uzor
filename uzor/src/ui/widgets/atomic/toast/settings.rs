use super::style::{DefaultToastStyle, ToastStyle};
use super::theme::{DefaultToastTheme, ToastTheme};

pub struct ToastSettings {
    pub theme: Box<dyn ToastTheme>,
    pub style: Box<dyn ToastStyle>,
}

impl Default for ToastSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultToastTheme>::default(),
            style: Box::new(DefaultToastStyle),
        }
    }
}
