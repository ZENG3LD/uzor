//! ContextMenu render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::ContextMenuSettings;
use super::types::{ContextMenuRenderKind, ContextMenuView};

pub fn draw_context_menu(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind: &ContextMenuRenderKind,
) {
    match kind {
        ContextMenuRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        ContextMenuRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
