//! Slider drag state — tracks the currently-dragging handle.

use crate::types::WidgetId;

use super::types::DualSliderHandle;

/// Active drag state. `None` when no slider is being dragged.
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
    /// Floating value during drag, written each frame.
    pub floating_value: Option<f64>,
}

impl SliderDragState {
    pub fn single(
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
        }
    }

    pub fn dual(
        field_id: impl Into<WidgetId>,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
        handle: DualSliderHandle,
    ) -> Self {
        Self {
            field_id: Some(field_id.into()),
            track_x,
            track_width,
            min,
            max,
            handle: Some(handle),
            floating_value: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.field_id.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
