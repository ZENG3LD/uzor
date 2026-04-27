//! ContextMenu type definitions.

use super::settings::ContextMenuSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - items: &'a [MenuItem]
//   - anchor: (f64, f64)
pub struct ContextMenuView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for ContextMenu.
pub enum ContextMenuRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ContextMenuView<'_>, &ContextMenuSettings)>),
}
