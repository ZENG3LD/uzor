//! Active-frame paint + registration.
//!
//! No state, no theme, no input — just a stroke registered as Sense::NONE
//! so the input coordinator records it (for completeness) but never delivers
//! events to it.

use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::RenderContext;
use crate::types::{CompositeId, WidgetId};

use super::types::{ActiveFrameKind, ActiveFrameView};

/// Draw the active frame using `kind`'s preset. Currently only `Stroke`.
pub fn draw_active_frame(
    ctx:  &mut dyn RenderContext,
    view: &ActiveFrameView<'_>,
    kind: ActiveFrameKind,
) {
    match kind {
        ActiveFrameKind::Stroke => {
            ctx.set_stroke_color(view.color);
            ctx.set_stroke_width(view.width);
            ctx.set_line_dash(&[]);
            ctx.stroke_rect(view.rect.x, view.rect.y, view.rect.width, view.rect.height);
        }
    }
}

/// Register an active-frame as a child of `parent` and paint it.
///
/// Sense is `NONE` — the frame never claims clicks/hover; its only role is
/// to draw the highlight. Composites call this after they have registered
/// the item rect itself, so the frame ends up over the item visually.
pub fn register_child_active_frame(
    coord:  &mut InputCoordinator,
    parent: &CompositeId,
    id:     impl Into<WidgetId>,
    view:   &ActiveFrameView<'_>,
    kind:   ActiveFrameKind,
    ctx:    &mut dyn RenderContext,
) {
    coord.register_child(parent, id.into(), WidgetKind::Custom, view.rect, Sense::NONE);
    draw_active_frame(ctx, view, kind);
}
