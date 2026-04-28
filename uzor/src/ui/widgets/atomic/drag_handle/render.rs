//! Drag handle rendering.
//!
//! `Invisible` is a no-op (pure hit zone).
//! `GripDots` draws a 2×3 dot grid centered in the rect.
//! `Custom` delegates to the caller's closure.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::DragHandleSettings;
use super::types::{DragHandleRenderKind, DragHandleView};

/// Draw the drag handle, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect.
/// - `view`     — per-frame state.
/// - `settings` — visual configuration (theme + style).
/// - `kind`     — which preset to use or `Custom` escape hatch.
pub fn draw_drag_handle(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &DragHandleView,
    settings: &DragHandleSettings,
    kind:     &DragHandleRenderKind,
) {
    match kind {
        DragHandleRenderKind::Invisible => {
            // No-op — pure hit zone.
        }
        DragHandleRenderKind::GripDots => {
            draw_grip_dots(ctx, rect, settings);
        }
        DragHandleRenderKind::Custom(f) => {
            f(ctx, rect, view, settings);
        }
    }
}

/// Draw a 2×3 grid of dots centered in `rect`.
fn draw_grip_dots(ctx: &mut dyn RenderContext, rect: Rect, settings: &DragHandleSettings) {
    let dot_d  = settings.style.grip_dot_size();
    let gap    = settings.style.grip_spacing();
    let count  = settings.style.grip_count().max(2);
    // Ensure even so we always have complete rows.
    let count  = if count % 2 != 0 { count + 1 } else { count };
    let rows   = count / 2;
    let cols   = 2_usize;

    let grid_w = cols as f64 * dot_d + (cols - 1) as f64 * gap;
    let grid_h = rows as f64 * dot_d + (rows.saturating_sub(1)) as f64 * gap;

    let origin_x = rect.x + (rect.width  - grid_w) / 2.0;
    let origin_y = rect.y + (rect.height - grid_h) / 2.0;

    ctx.set_fill_color(settings.theme.grip_dots_color());

    for row in 0..rows {
        for col in 0..cols {
            let x = origin_x + col as f64 * (dot_d + gap);
            let y = origin_y + row as f64 * (dot_d + gap);
            // Circular dot via filled rounded rect with radius = dot_d / 2.
            ctx.fill_rounded_rect(x, y, dot_d, dot_d, dot_d / 2.0);
        }
    }
}
