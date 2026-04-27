//! Modal render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::ModalSettings;
use super::types::{ModalRenderKind, ModalView};

pub fn draw_modal(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ModalView<'_>,
    settings: &ModalSettings,
    kind: &ModalRenderKind,
) {
    match kind {
        ModalRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        ModalRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
