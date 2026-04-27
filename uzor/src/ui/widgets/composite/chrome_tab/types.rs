//! ChromeTab type definitions.

use super::settings::ChromeTabSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - label: &'a str
//   - active: bool
//   - close_btn: bool
//   - icon: Option<IconId>
pub struct ChromeTabView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for ChromeTab.
pub enum ChromeTabRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ChromeTabView<'_>, &ChromeTabSettings)>),
}
