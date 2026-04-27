//! ContextMenu settings bundle.

use super::theme::{ContextMenuTheme, DefaultContextMenuTheme};
use super::style::{ContextMenuStyle, DefaultContextMenuStyle};

pub struct ContextMenuSettings {
    pub theme: Box<dyn ContextMenuTheme>,
    pub style: Box<dyn ContextMenuStyle>,
}

impl Default for ContextMenuSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultContextMenuTheme>::default(),
            style: Box::new(DefaultContextMenuStyle),
        }
    }
}
