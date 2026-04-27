//! Modal type definitions.

use super::settings::ModalSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - title: Option<&'a str>
//   - children: ?
//   - close_btn: bool
//   - draggable: bool
pub struct ModalView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for Modal.
pub enum ModalRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ModalView<'_>, &ModalSettings)>),
}
