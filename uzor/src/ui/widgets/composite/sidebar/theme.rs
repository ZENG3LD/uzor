//! Sidebar theme trait.

pub trait SidebarTheme {}

#[derive(Default)]
pub struct DefaultSidebarTheme;

impl SidebarTheme for DefaultSidebarTheme {}
