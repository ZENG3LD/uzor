//! Clock widget rendering — time text with hover background.
//!
//! Ported from `button/render.rs` section 9 (`draw_toolbar_clock`).
//!
//! The clock widget does NOT read system time. The caller supplies a
//! pre-formatted time string via `ClockView::time_text`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::ClockSettings;
use super::types::ClockRenderKind;

/// Per-instance data for `draw_clock`.
pub struct ClockView<'a> {
    /// Pre-formatted time string (e.g. `"14:35:22 UTC"`).
    /// The widget does not read system time — the caller provides this.
    pub time_text: &'a str,
    /// Whether the pointer is over the clock item.
    pub hovered: bool,
}

/// Render a clock widget, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (mlc fixed width 140 px for Toolbar variant).
/// - `state`    — interaction state from the coordinator (hover-only sense).
/// - `view`     — per-frame data (time string, hovered flag).
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
pub fn draw_clock(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &ClockView<'_>,
    settings: &ClockSettings,
    kind:     &ClockRenderKind,
) {
    match kind {
        ClockRenderKind::Toolbar => {
            draw_clock_toolbar(ctx, rect, view, settings);
        }
        ClockRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
        }
    }
}

fn draw_clock_toolbar(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ClockView<'_>,
    settings: &ClockSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    if view.hovered {
        let inset = style.hover_bg_vertical_inset();
        ctx.set_fill_color(theme.clock_bg_hover());
        ctx.fill_rounded_rect(
            rect.x,
            rect.y + inset,
            rect.width,
            rect.height - inset * 2.0,
            style.hover_bg_radius(),
        );
    }

    ctx.set_font(style.font());
    ctx.set_fill_color(theme.clock_text());
    ctx.set_text_align(TextAlign::Right);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        view.time_text,
        rect.right() - style.text_padding_right(),
        rect.center_y(),
    );
}
