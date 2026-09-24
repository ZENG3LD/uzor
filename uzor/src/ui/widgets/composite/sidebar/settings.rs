//! Sidebar settings bundle — `SidebarTheme` + `SidebarStyle` in one box.

use super::style::{DefaultSidebarStyle, SidebarStyle};
use super::theme::SidebarTheme;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Combined visual configuration for the sidebar composite.
pub struct SidebarSettings {
    /// Colour tokens (varies with app theme).
    pub theme: Box<dyn SidebarTheme>,
    /// Geometry parameters (varies with sidebar kind).
    pub style: Box<dyn SidebarStyle>,
}

impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::<DefaultSidebarStyle>::default(),
        }
    }
}
