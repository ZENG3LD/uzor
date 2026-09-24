use super::style::{DefaultToastStyle, ToastStyle};
use super::theme::ToastTheme;
use crate::tokens::{BuiltinSet, TokenTheme, Tokens};

pub struct ToastSettings {
    pub theme: Box<dyn ToastTheme>,
    pub style: Box<dyn ToastStyle>,
}

impl Default for ToastSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::new(DefaultToastStyle),
        }
    }
}
