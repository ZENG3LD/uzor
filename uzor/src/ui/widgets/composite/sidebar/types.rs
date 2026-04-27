//! Sidebar type definitions.

use super::settings::SidebarSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - position: SidebarPosition (Left/Right/Bottom)
//   - collapsed: bool
//   - width: f64
pub struct SidebarView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for Sidebar.
pub enum SidebarRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &SidebarView<'_>, &SidebarSettings)>),
}
