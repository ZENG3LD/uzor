//! Chrome render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::ChromeSettings;
use super::types::{ChromeRenderKind, ChromeView};

pub fn draw_chrome(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ChromeView<'_>,
    settings: &ChromeSettings,
    kind: &ChromeRenderKind,
) {
    match kind {
        ChromeRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        ChromeRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
