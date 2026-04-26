//! Slider drag state — tracks the currently-dragging handle.

use crate::types::WidgetId;

use super::types::DualSliderHandle;

/// Active drag state. `None` when no slider is being dragged.
///
/// Mirrors the authoritative `SliderDragState` from `modal_settings.rs` in mlc,
/// including the `floating_value` / `floating_value2` preview fields.
#[derive(Debug, Default, Clone)]
pub struct SliderDragState {
    /// Field id (matches the slider's WidgetId).
    pub field_id: Option<WidgetId>,
    /// Track left edge in screen pixels.
    pub track_x: f64,
    /// Track total width in pixels.
    pub track_width: f64,
    /// Slider min value (config).
    pub min: f64,
    /// Slider max value (config).
    pub max: f64,
    /// For `Dual` sliders — which handle is being dragged.
    pub handle: Option<DualSliderHandle>,
    /// Floating (preview) value during drag — written each mouse-move frame.
    ///
    /// `None` after `start_single` / `start_dual` until first `update_floating` call.
    /// Callers pass `drag_state.floating_value.unwrap_or(committed_value)` to the
    /// renderer so the handle snaps to the pointer position in real time.
    pub floating_value: Option<f64>,
    /// Reserved for the *opposite* handle on dual sliders.
    ///
    /// Allocated for mlc parity (mlc's `floating_value2` field exists in all
    /// constructors) but never populated during drag — always `None`.
    pub floating_value2: Option<f64>,
}

impl SliderDragState {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Start a single-handle drag.
    ///
    /// `floating_value` is `None` until the first `update_floating` call.
    pub fn start_single(
        field_id: impl Into<WidgetId>,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
    ) -> Self {
        Self {
            field_id: Some(field_id.into()),
            track_x,
            track_width,
            min,
            max,
            handle: None,
            floating_value: None,
            floating_value2: None,
        }
    }

    /// Start a dual-handle drag.
    ///
    /// Sets `floating_value` to the value at `initial_click_x` immediately,
    /// matching mlc's `start_dual_slider_drag_from_track` behaviour.
    pub fn start_dual(
        field_id: impl Into<WidgetId>,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
        handle: DualSliderHandle,
        initial_click_x: f64,
    ) -> Self {
        let t = ((initial_click_x - track_x) / track_width).clamp(0.0, 1.0);
        let initial_value = min + t * (max - min);
        Self {
            field_id: Some(field_id.into()),
            track_x,
            track_width,
            min,
            max,
            handle: Some(handle),
            floating_value: Some(initial_value),
            floating_value2: None,
        }
    }

    // ── Deprecated aliases (kept for call-site compat with group-1 code) ─────

    /// Alias for [`start_single`](Self::start_single).
    pub fn single(
        field_id: impl Into<WidgetId>,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
    ) -> Self {
        Self::start_single(field_id, track_x, track_width, min, max)
    }

    /// Alias for [`start_dual`](Self::start_dual) with `initial_click_x = track_x`
    /// (floating_value starts at `min`).  Prefer `start_dual` for new code.
    pub fn dual(
        field_id: impl Into<WidgetId>,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
        handle: DualSliderHandle,
    ) -> Self {
        Self::start_dual(field_id, track_x, track_width, min, max, handle, track_x)
    }

    // ── Mutation ─────────────────────────────────────────────────────────────

    /// Write a new floating preview value.
    ///
    /// Call on every mouse-move while dragging.
    pub fn update_floating(&mut self, value: f64) {
        self.floating_value = Some(value);
    }

    /// Consume the drag state and return the final value if the pointer moved.
    ///
    /// - Returns `Some((field_id, value, handle))` when `floating_value` is set.
    /// - Returns `None` when the user clicked without moving (no preview was written).
    ///
    /// Clears the state either way.
    pub fn take_value(&mut self) -> Option<(WidgetId, f64, Option<DualSliderHandle>)> {
        if let Some(field_id) = self.field_id.take() {
            let fv = self.floating_value.take();
            let handle = self.handle;
            self.clear();
            fv.map(|v| (field_id, v, handle))
        } else {
            None
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// `true` when a drag is in progress.
    pub fn is_active(&self) -> bool {
        self.field_id.is_some()
    }

    /// `true` when this is a dual-handle drag.
    pub fn is_dual(&self) -> bool {
        self.handle.is_some()
    }

    /// Reset to idle (no drag).
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
