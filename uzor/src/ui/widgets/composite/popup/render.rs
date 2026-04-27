//! Popup render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::PopupSettings;
use super::types::{PopupRenderKind, PopupView};

pub fn draw_popup(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &PopupView<'_>,
    settings: &PopupSettings,
    kind: &PopupRenderKind,
) {
    match kind {
        PopupRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        PopupRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
