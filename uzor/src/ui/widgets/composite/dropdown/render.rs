//! Dropdown render entry point.

use crate::render::RenderContext;
use crate::types::Rect;
use super::settings::DropdownSettings;
use super::types::{DropdownRenderKind, DropdownView};

pub fn draw_dropdown(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &DropdownView<'_>,
    settings: &DropdownSettings,
    kind: &DropdownRenderKind,
) {
    match kind {
        DropdownRenderKind::Default => {
            // TODO: populate after deep mlc audit
            let _ = (ctx, rect, view, settings);
        }
        DropdownRenderKind::Custom(f) => f(ctx, rect, view, settings),
    }
}
