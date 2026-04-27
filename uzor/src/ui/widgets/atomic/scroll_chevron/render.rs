//! Scroll chevron rendering — directional chevron for toolbar overflow navigation.
//!
//! Ported from `button/render.rs` section 42.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::ScrollChevronSettings;
use super::types::ScrollChevronRenderKind;

// Re-export ChevronDirection from button/types.rs via button module.
// The canonical definition stays in button::types; scroll_chevron re-exports it.
pub use crate::ui::widgets::atomic::button::ChevronDirection;

/// Per-instance data for `draw_scroll_chevron`.
pub struct ScrollChevronView {
    /// Which way the chevron points. Direction is per-instance, not a render kind.
    pub direction: ChevronDirection,
    /// Whether the pointer is over the button.
    pub hovered: bool,
    /// Whether there are no more items to scroll to in this direction.
    pub disabled: bool,
}

/// Interaction result returned by `draw_scroll_chevron`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScrollChevronResult {
    pub clicked: bool,
    pub hovered: bool,
}

/// Render a scroll chevron, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (caller sizes to `style.size() × style.size()`).
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame state (direction, hovered, disabled).
/// - `settings` — visual configuration (theme + style override).
/// - `kind`     — which preset to use or `Custom` escape hatch.
pub fn draw_scroll_chevron(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &ScrollChevronView,
    settings: &ScrollChevronSettings,
    kind:     &ScrollChevronRenderKind,
) -> ScrollChevronResult {
    match kind {
        ScrollChevronRenderKind::Default => {
            draw_scroll_chevron_inner(ctx, rect, view, settings)
        }
        ScrollChevronRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
            ScrollChevronResult {
                clicked: false,
                hovered: view.hovered && !view.disabled,
            }
        }
    }
}

fn draw_scroll_chevron_inner(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ScrollChevronView,
    settings: &ScrollChevronSettings,
) -> ScrollChevronResult {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let color = if view.disabled {
        theme.scroll_chevron_color_disabled()
    } else if view.hovered {
        theme.scroll_chevron_color_hover()
    } else {
        theme.scroll_chevron_color()
    };

    if view.hovered && !view.disabled {
        ctx.set_fill_color(theme.scroll_chevron_bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.hover_bg_radius());
    }

    let inset = style.chevron_inset();
    let cx    = rect.center_x();
    let cy    = rect.center_y();
    let half  = (rect.width.min(rect.height) / 2.0 - inset).max(2.0);

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(style.chevron_thickness());
    ctx.set_line_dash(&[]);
    ctx.begin_path();

    match view.direction {
        ChevronDirection::Left => {
            ctx.move_to(cx + half * 0.5, cy - half);
            ctx.line_to(cx - half * 0.5, cy);
            ctx.line_to(cx + half * 0.5, cy + half);
        }
        ChevronDirection::Right => {
            ctx.move_to(cx - half * 0.5, cy - half);
            ctx.line_to(cx + half * 0.5, cy);
            ctx.line_to(cx - half * 0.5, cy + half);
        }
        ChevronDirection::Up => {
            ctx.move_to(cx - half, cy + half * 0.5);
            ctx.line_to(cx,        cy - half * 0.5);
            ctx.line_to(cx + half, cy + half * 0.5);
        }
        ChevronDirection::Down => {
            ctx.move_to(cx - half, cy - half * 0.5);
            ctx.line_to(cx,        cy + half * 0.5);
            ctx.line_to(cx + half, cy - half * 0.5);
        }
    }

    ctx.stroke();

    ScrollChevronResult {
        clicked: false,
        hovered: view.hovered && !view.disabled,
    }
}
