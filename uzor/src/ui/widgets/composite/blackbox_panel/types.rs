//! BlackboxPanel type definitions.

use super::settings::BlackboxPanelSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - (internal dispatch managed externally — minimal view surface expected)
pub struct BlackboxPanelView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for BlackboxPanel.
pub enum BlackboxPanelRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &BlackboxPanelView<'_>, &BlackboxPanelSettings)>),
}
