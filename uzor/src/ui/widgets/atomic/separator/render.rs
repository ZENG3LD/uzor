//! Separator rendering — single line.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::SeparatorSettings;
use super::types::{SeparatorOrientation, SeparatorType};

pub struct SeparatorView {
    pub kind: SeparatorType,
    /// Hovered (resize-handle highlighting).
    pub hovered: bool,
    pub dragging: bool,
}

pub fn draw_separator(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &SeparatorView,
    settings: &SeparatorSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let color = match (&view.kind, view.dragging, view.hovered) {
        (SeparatorType::ResizeHandle { .. }, true, _)  => theme.handle_active(),
        (SeparatorType::ResizeHandle { .. }, false, true) => theme.handle_hover(),
        _ => theme.line(),
    };

    let t = style.thickness();
    let m = style.margin();

    let line_rect = match view.kind.orientation() {
        SeparatorOrientation::Horizontal => {
            // Centred horizontally; horizontal line spans the rect width minus margins.
            Rect::new(rect.x + m, rect.y + (rect.height - t) / 2.0, rect.width - m * 2.0, t)
        }
        SeparatorOrientation::Vertical => {
            Rect::new(rect.x + (rect.width - t) / 2.0, rect.y + m, t, rect.height - m * 2.0)
        }
    };

    ctx.set_fill_color(color);
    ctx.fill_rect(line_rect.x, line_rect.y, line_rect.width, line_rect.height);
}
