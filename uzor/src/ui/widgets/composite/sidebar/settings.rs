//! Sidebar settings bundle.

use super::theme::{DefaultSidebarTheme, SidebarTheme};
use super::style::{DefaultSidebarStyle, SidebarStyle};

pub struct SidebarSettings {
    pub theme: Box<dyn SidebarTheme>,
    pub style: Box<dyn SidebarStyle>,
}

impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultSidebarTheme>::default(),
            style: Box::new(DefaultSidebarStyle),
        }
    }
}
