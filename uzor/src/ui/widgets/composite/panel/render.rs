//! Panel render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::PanelSettings;
use super::types::{PanelRenderKind, PanelView};

pub fn draw_panel(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &PanelView<'_>,
    settings: &PanelSettings,
    kind: &PanelRenderKind,
) {
    match kind {
        PanelRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        PanelRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
