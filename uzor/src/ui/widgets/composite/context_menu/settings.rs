//! ContextMenu settings bundle — `ContextMenuTheme` + `ContextMenuStyle` in one box.

use super::style::{ContextMenuStyle, DefaultContextMenuStyle};
use super::theme::{ContextMenuTheme, DefaultContextMenuTheme};

/// Combined visual configuration for the ContextMenu composite.
pub struct ContextMenuSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn ContextMenuTheme>,
    /// Geometry parameters (varies with render kind / preset).
    pub style: Box<dyn ContextMenuStyle>,
}

impl Default for ContextMenuSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultContextMenuTheme>::default(),
            style: Box::<DefaultContextMenuStyle>::default(),
        }
    }
}
