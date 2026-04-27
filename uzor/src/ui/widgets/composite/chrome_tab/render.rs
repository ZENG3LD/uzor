//! ChromeTab render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::ChromeTabSettings;
use super::types::{ChromeTabRenderKind, ChromeTabView};

pub fn draw_chrome_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ChromeTabView<'_>,
    settings: &ChromeTabSettings,
    kind: &ChromeTabRenderKind,
) {
    match kind {
        ChromeTabRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        ChromeTabRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
