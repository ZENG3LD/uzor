//! BlackboxPanel render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::BlackboxPanelSettings;
use super::types::{BlackboxPanelRenderKind, BlackboxPanelView};

pub fn draw_blackbox_panel(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &BlackboxPanelView<'_>,
    settings: &BlackboxPanelSettings,
    kind: &BlackboxPanelRenderKind,
) {
    match kind {
        BlackboxPanelRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        BlackboxPanelRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
