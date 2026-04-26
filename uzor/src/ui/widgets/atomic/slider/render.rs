//! Slider rendering — math from `mlc/chart/src/ui/widgets/slider.rs`.
//!
//! Geometry: track fills the rect width minus space reserved for label
//! (left) and value input (right). Handle is a filled circle with border;
//! filled track portion stretches from left edge to handle centre.
//!
//! This file renders ONLY the slider visuals — the inline value
//! input/label rendering is the caller's job because it composes with
//! the `text_input` widget.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::SliderSettings;
use super::types::{DualSliderHandle, SliderType};

/// Per-frame inputs.
pub struct SliderView {
    pub kind: SliderType,
    /// Hovered state (from coordinator).
    pub hovered: bool,
    /// Disabled.
    pub disabled: bool,
    /// Optional active dragging handle (for `Dual`); `None` for `Single`
    /// or when not dragging.
    pub dragging_handle: Option<DualSliderHandle>,
}

/// Rects + positions returned to the caller for hit testing and
/// follow-up input handling.
#[derive(Debug, Default, Clone)]
pub struct SliderResult {
    /// Rect of the track (for click-to-jump and drag start hit-testing).
    pub track_rect: Rect,
    /// X of the (single) handle centre, or the min handle for `Dual`.
    pub handle_x: f64,
    /// X of the max handle for `Dual`. `0.0` for `Single`.
    pub handle_max_x: f64,
}

fn value_to_x(value: f64, min: f64, max: f64, track_x: f64, track_width: f64) -> f64 {
    if max <= min { return track_x; }
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
    track_x + t * track_width
}

/// Render either a single or a dual slider into `track_rect`.
pub fn draw_slider(
    ctx: &mut dyn RenderContext,
    track_rect: Rect,
    state: WidgetState,
    view: &SliderView,
    settings: &SliderSettings,
) -> SliderResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let effective = if view.disabled { WidgetState::Disabled } else { state };
    let is_hovered = view.hovered || matches!(effective, WidgetState::Hovered | WidgetState::Pressed);

    let track_y = track_rect.y + track_rect.height / 2.0 - style.track_height() / 2.0;
    let track_x = track_rect.x;
    let track_w = track_rect.width;
    let track_h = style.track_height();
    let radius  = style.track_radius();

    // Empty track
    ctx.set_fill_color(theme.track_empty());
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, radius);

    let (handle_x, handle_max_x) = match view.kind {
        SliderType::Single { value, min, max, .. } => {
            let x = value_to_x(value, min, max, track_x, track_w);
            // Filled portion left of handle
            ctx.set_fill_color(theme.accent());
            ctx.fill_rounded_rect(track_x, track_y, x - track_x, track_h, radius);
            (x, 0.0)
        }
        SliderType::Dual { min_value, max_value, min, max, .. } => {
            let x_min = value_to_x(min_value, min, max, track_x, track_w);
            let x_max = value_to_x(max_value, min, max, track_x, track_w);
            // Filled portion between handles
            ctx.set_fill_color(theme.accent());
            ctx.fill_rounded_rect(x_min, track_y, (x_max - x_min).max(0.0), track_h, radius);
            (x_min, x_max)
        }
    };

    // Handle helper closure
    let mut draw_handle = |cx: f64, hovered_handle: bool| {
        let r = style.handle_radius();
        let cy = track_y + track_h / 2.0;
        if hovered_handle {
            // Hover ring (translucent accent halo)
            ctx.set_fill_color(theme.accent());
            ctx.fill_rounded_rect(
                cx - r - style.handle_hover_ring(),
                cy - r - style.handle_hover_ring(),
                (r + style.handle_hover_ring()) * 2.0,
                (r + style.handle_hover_ring()) * 2.0,
                r + style.handle_hover_ring(),
            );
        }
        // Handle body
        ctx.set_fill_color(if view.disabled { theme.text_disabled() } else { theme.text_normal() });
        ctx.fill_rounded_rect(cx - r, cy - r, r * 2.0, r * 2.0, r);
        // Border
        ctx.set_stroke_color(theme.accent());
        ctx.set_stroke_width(style.handle_border_width());
        ctx.stroke_rounded_rect(cx - r, cy - r, r * 2.0, r * 2.0, r);
    };

    match view.kind {
        SliderType::Single { .. } => {
            draw_handle(handle_x, is_hovered);
        }
        SliderType::Dual { .. } => {
            let min_hot = is_hovered && view.dragging_handle == Some(DualSliderHandle::Min);
            let max_hot = is_hovered && view.dragging_handle == Some(DualSliderHandle::Max);
            // If hovered without explicit drag-handle pick, light both.
            let min_hl = min_hot || (is_hovered && view.dragging_handle.is_none());
            let max_hl = max_hot || (is_hovered && view.dragging_handle.is_none());
            draw_handle(handle_x, min_hl);
            draw_handle(handle_max_x, max_hl);
        }
    }

    SliderResult { track_rect, handle_x, handle_max_x }
}
