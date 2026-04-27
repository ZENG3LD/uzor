//! Toolbar type definitions.

use super::settings::ToolbarSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - position: ToolbarPosition (Top/Bottom/Left/Right)
//   - items: &'a [ToolbarItem]
pub struct ToolbarView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for Toolbar.
pub enum ToolbarRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ToolbarView<'_>, &ToolbarSettings)>),
}
