//! Sidebar render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::SidebarSettings;
use super::types::{SidebarRenderKind, SidebarView};

pub fn draw_sidebar(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &SidebarView<'_>,
    settings: &SidebarSettings,
    kind: &SidebarRenderKind,
) {
    match kind {
        SidebarRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        SidebarRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
