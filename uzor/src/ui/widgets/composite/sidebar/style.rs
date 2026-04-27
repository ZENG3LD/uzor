//! Sidebar style trait.

pub trait SidebarStyle {}

#[derive(Default)]
pub struct DefaultSidebarStyle;

impl SidebarStyle for DefaultSidebarStyle {}
