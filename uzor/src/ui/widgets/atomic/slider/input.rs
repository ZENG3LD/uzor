//! Slider input logic: registration, drag, scroll, click, text, arrow keys.
//!
//! # Architecture
//!
//! All handler functions are **pure** (no hidden state): they take explicit
//! arguments and return an `Option<f64>` (or `bool`) result.  The caller
//! owns the [`SliderDragState`] and decides when to commit.
//!
//! # Floating-value preview pattern
//!
//! ```text
//! mouse_down  → start_slider_drag(…) → may call update_slider_drag_float
//! mouse_move  → update_slider_drag_float(drag, x) → Some(preview_value)
//! mouse_up    → end_slider_drag(drag) → Option<(field_id, value, handle)>
//! ```
//!
//! The renderer should pass
//! `drag.floating_value.unwrap_or(committed_value)` as the `value` param so
//! the handle follows the pointer in real time.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::SliderDragState;
use super::types::{DualSliderHandle, SliderConfig, SliderTrackInfo};

// ─── Registration ─────────────────────────────────────────────────────────────

/// Register a slider widget with the [`InputCoordinator`].
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Slider, rect, Sense::CLICK_AND_DRAG, layer);
}

// ─── Math helpers (pub — usable by callers) ───────────────────────────────────

/// Convert a pixel X coordinate to a slider value.
///
/// - `track_x` — left edge of the track in screen pixels.
/// - `track_width` — total pixel width of the track.
/// - `min` / `max` — value range.
///
/// The returned value is clamped to `[min, max]`.
pub fn pixel_to_value(x: f64, track_x: f64, track_width: f64, min: f64, max: f64) -> f64 {
    if track_width <= 0.0 {
        return min;
    }
    let t = ((x - track_x) / track_width).clamp(0.0, 1.0);
    min + t * (max - min)
}

/// Convert a slider value to a pixel X coordinate on the track.
///
/// The returned position is clamped to `[track_x, track_x + track_width]`.
pub fn value_to_pixel(value: f64, track_x: f64, track_width: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return track_x;
    }
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
    track_x + t * track_width
}

/// Clamp and snap `value` to the nearest step within `[min, max]`.
///
/// When `step == 0.0` the value is only clamped (continuous range).
pub fn clamp_step(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let clamped = value.clamp(min, max);
    if step > 0.0 {
        (clamped / step).round() * step
    } else {
        clamped
    }
}

// ─── Drag start ───────────────────────────────────────────────────────────────

/// Try to start a **single-handle** drag at `(x, y)`.
///
/// Hit-test: the click must fall within the track's hit zone (inflated ±2 px
/// horizontally to match mlc behaviour).
///
/// On hit, writes `drag_state` and optionally sets `floating_value` to the
/// value at the click position (same as mlc calling `update_slider_drag_float`
/// right after `start_slider_drag_from_track`).
///
/// - `field_id` — widget / field identifier for this slider.
/// - `handle` — `Some(DualSliderHandle)` for dual-handle drag; `None` for single.
///
/// Returns `true` when a drag was started.
pub fn start_slider_drag(
    drag_state: &mut SliderDragState,
    field_id: impl Into<WidgetId>,
    x: f64,
    y: f64,
    track: &SliderTrackInfo,
    handle: Option<DualSliderHandle>,
) -> bool {
    let hit = x >= track.track_x - 2.0
        && x <= track.track_x + track.track_width + 2.0
        && y >= track.track_y
        && y <= track.track_y + track.track_height;

    if !hit {
        return false;
    }

    let fid: WidgetId = field_id.into();

    if let Some(h) = handle {
        *drag_state = SliderDragState::start_dual(
            fid,
            track.track_x,
            track.track_width,
            track.min_val,
            track.max_val,
            h,
            x,
        );
    } else {
        *drag_state = SliderDragState::start_single(
            fid,
            track.track_x,
            track.track_width,
            track.min_val,
            track.max_val,
        );
        // Snap floating_value to click position immediately (mlc pattern).
        let initial = pixel_to_value(x, track.track_x, track.track_width, track.min_val, track.max_val);
        drag_state.update_floating(initial);
    }

    true
}

// ─── Drag move ────────────────────────────────────────────────────────────────

/// Update the floating preview value while the pointer moves.
///
/// No step-snapping is applied here — snapping happens at commit time
/// (`end_slider_drag`), matching mlc's deferred-snap design.
///
/// Returns the raw (unsnapped) preview value, or `None` when not dragging.
pub fn update_slider_drag_float(drag_state: &mut SliderDragState, x: f64) -> Option<f64> {
    if !drag_state.is_active() {
        return None;
    }
    let value = pixel_to_value(
        x,
        drag_state.track_x,
        drag_state.track_width,
        drag_state.min,
        drag_state.max,
    );
    drag_state.update_floating(value);
    Some(value)
}

// ─── Drag end ─────────────────────────────────────────────────────────────────

/// Finalise a drag and return the committed value.
///
/// - Returns `Some((field_id, value, handle))` when the pointer moved at least
///   once (i.e. `floating_value` was written).
/// - Returns `None` when the user clicked without moving.  The caller should
///   call [`clear`](SliderDragState::clear) in that case.
///
/// Clears `drag_state` unconditionally.
pub fn end_slider_drag(
    drag_state: &mut SliderDragState,
) -> Option<(WidgetId, f64, Option<DualSliderHandle>)> {
    drag_state.take_value()
}

// ─── Scroll wheel ─────────────────────────────────────────────────────────────

/// Adjust value by one scroll notch.
///
/// - `delta` — raw scroll delta; sign convention: negative = scroll up = increase
///   value (matches mlc and most scroll-wheel events).
/// - `step` — explicit override for the scroll step.  When `0.0`, an auto-step
///   is derived from the range (matches `SliderInputHandler::handle_scroll` in mlc).
///
/// Returns the new clamped+snapped value.
pub fn handle_slider_scroll(
    drag_state: &SliderDragState,
    delta: f64,
    track_info: &SliderTrackInfo,
    current_value: f64,
    step: f64,
) -> Option<f64> {
    // Only adjust when no drag is in progress (or caller passes a dummy drag).
    // The drag check is intentionally skipped here — callers decide.
    let _ = drag_state; // retained param for symmetry with mlc API

    let effective_step = if step > 0.0 {
        step
    } else {
        let range = track_info.max_val - track_info.min_val;
        if range > 100.0 {
            1.0
        } else if range > 10.0 {
            0.1
        } else {
            0.01
        }
    };

    let adjustment = -delta.signum() * effective_step;
    let new_value = current_value + adjustment;
    Some(clamp_step(new_value, track_info.min_val, track_info.max_val, step))
}

// ─── Click-to-jump ────────────────────────────────────────────────────────────

/// Handle a single click on the track (no drag): jump the handle to `x`.
///
/// Returns `None` when `x` is outside the track (±2 px hit inflate).
pub fn handle_slider_click(
    track_info: &SliderTrackInfo,
    x: f64,
    _current_value: f64,
) -> Option<f64> {
    let hit = x >= track_info.track_x - 2.0
        && x <= track_info.track_x + track_info.track_width + 2.0;
    if !hit {
        return None;
    }
    Some(pixel_to_value(
        x,
        track_info.track_x,
        track_info.track_width,
        track_info.min_val,
        track_info.max_val,
    ))
}

// ─── Text input ───────────────────────────────────────────────────────────────

/// Parse a value typed into the slider's inline text box.
///
/// - Step is NOT applied to text-entered values (mlc behaviour: only clamp).
/// - Returns `None` on parse failure — caller should keep editing state open.
pub fn handle_slider_text_input(
    _field_id: &str,
    text: &str,
    config: &SliderConfig,
) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .map(|v| v.clamp(config.min, config.max))
}

// ─── Arrow keys ───────────────────────────────────────────────────────────────

/// Which direction an arrow key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDirection {
    Left,
    Right,
}

/// Step the slider value by one `step` unit in response to an arrow key.
///
/// - Left  → decrease by `step`.
/// - Right → increase by `step`.
///
/// Returns the new clamped+snapped value, or `None` when `step == 0.0`
/// (continuous sliders have no defined keyboard step).
pub fn handle_slider_arrow_key(
    current_value: f64,
    direction: ArrowDirection,
    step: f64,
    min: f64,
    max: f64,
) -> Option<f64> {
    if step <= 0.0 {
        return None;
    }
    let delta = match direction {
        ArrowDirection::Left => -step,
        ArrowDirection::Right => step,
    };
    Some(clamp_step(current_value + delta, min, max, step))
}

// ── Level 1 / Level 2 entry points ───────────────────────────────────────────

/// Level 1 — register a slider with an explicit `InputCoordinator`.
///
/// Drag, scroll, and click events are handled by the separate helper functions
/// (`start_slider_drag`, `update_slider_drag_float`, etc.). This call only
/// registers the widget's hit zone for each frame.
pub fn register_input_coordinator_slider(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    state: &mut SliderDragState,
) {
    let _ = state; // drag state is managed by the drag helper fns
    register(coord, id, rect, layer);
}

/// Level 2 — register a slider via `ContextManager`, pulling `SliderDragState`
/// from the registry.
pub fn register_context_manager_slider(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), SliderDragState::default);
    register_input_coordinator_slider(&mut ctx.input, id, rect, layer, state);
}
