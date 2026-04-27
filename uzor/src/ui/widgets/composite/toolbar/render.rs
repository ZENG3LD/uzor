//! Toolbar render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::ToolbarSettings;
use super::types::{ToolbarRenderKind, ToolbarView};

pub fn draw_toolbar(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind: &ToolbarRenderKind,
) {
    match kind {
        ToolbarRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        ToolbarRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
