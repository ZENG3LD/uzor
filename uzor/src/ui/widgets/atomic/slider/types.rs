//! Slider variant catalog and supporting types.
//!
//! Layout (rect) is layout-layer concern, not widget data.

use crate::input::Sense;
use crate::types::Rect;
use crate::ui::widgets::WidgetCapabilities;

// ─── Slider variant ──────────────────────────────────────────────────────────

/// Slider variants.
#[derive(Debug, Clone, PartialEq)]
pub enum SliderType {
    /// Single-handle slider (one value).
    Single { value: f64, min: f64, max: f64, step: f64 },
    /// Dual-handle range slider (min..max).
    Dual {
        min_value: f64,
        max_value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
}

impl WidgetCapabilities for SliderType {
    fn sense(&self) -> Sense {
        Sense::CLICK_AND_DRAG
    }
}

impl SliderType {
    pub fn single(value: f64, min: f64, max: f64) -> Self {
        Self::Single { value, min, max, step: 1.0 }
    }

    pub fn dual(min_value: f64, max_value: f64, min: f64, max: f64) -> Self {
        Self::Dual { min_value, max_value, min, max, step: 1.0 }
    }
}

/// Which handle is active during a drag of a `Dual` slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualSliderHandle {
    Min,
    Max,
}

// ─── SliderConfig ────────────────────────────────────────────────────────────

/// Range / step / display config for a slider instance.
///
/// Constructed with `SliderConfig::new(min, max)` then chained builder calls.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderConfig {
    pub min: f64,
    pub max: f64,
    /// `0.0` = continuous; `> 0.0` = discrete steps.
    pub step: f64,
    /// Show the inline value input box. Variant 1.3 sets this to `false`.
    pub show_input: bool,
    /// Width of the inline value input box (mlc default 50.0).
    pub input_width: f64,
    /// Height of the inline value input box (mlc default 22.0).
    pub input_height: f64,
}

impl SliderConfig {
    pub fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            step: 0.0,
            show_input: true,
            input_width: 50.0,
            input_height: 22.0,
        }
    }

    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Variant 1.3 — no value box drawn.
    pub fn without_input(mut self) -> Self {
        self.show_input = false;
        self
    }

    pub fn with_input_width(mut self, w: f64) -> Self {
        self.input_width = w;
        self
    }

    pub fn with_input_height(mut self, h: f64) -> Self {
        self.input_height = h;
        self
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Clamp `value` to `[min, max]`.
    pub fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    /// Snap `value` to the nearest discrete step (no-op when `step == 0`).
    pub fn apply_step(&self, value: f64) -> f64 {
        if self.step > 0.0 {
            (value / self.step).round() * self.step
        } else {
            value
        }
    }

    /// Normalised position `0.0..=1.0`.
    pub fn normalize(&self, value: f64) -> f64 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// Format `value` as display string (matches mlc conventions).
    pub fn format_value(&self, value: f64) -> String {
        if self.step >= 1.0 {
            format!("{:.0}", value)
        } else {
            format!("{:.2}", value)
        }
    }
}

// ─── Editing info ─────────────────────────────────────────────────────────────

/// Render-time editing state for the inline value box (variant 1.2).
///
/// Passed to `draw_single_slider` / `draw_dual_slider` when the user is
/// actively typing in the value box.  All fields are borrowed from the
/// caller's text-editing state.
pub struct SliderEditingInfo<'a> {
    /// Live buffer text (what the user is typing — not the committed value).
    pub text: &'a str,
    /// Cursor position in chars.
    pub cursor: usize,
    /// Selection anchor in chars. `None` means no selection.
    pub selection_start: Option<usize>,
}

// ─── Per-frame view structs ───────────────────────────────────────────────────

/// Per-frame inputs for a single-handle slider row (variants 1.1 / 1.2 / 1.3).
pub struct SingleSliderView<'a> {
    /// Range / step / input-display config.
    pub config: &'a SliderConfig,
    /// Current value (use `floating_value` from drag state while dragging).
    pub value: f64,
    /// Optional label drawn left of the track.
    pub label: Option<&'a str>,
    /// `None` → variant 1.3 (no box) or variant 1.1 (not editing).
    /// `Some` → variant 1.2 (actively typing in the value box).
    pub editing: Option<SliderEditingInfo<'a>>,
    /// Hover / active-drag highlight.
    pub hovered: bool,
    /// Greyed-out non-interactive.
    pub disabled: bool,
}

/// Per-frame inputs for a dual-handle slider row (variant 1.4).
pub struct DualSliderView<'a> {
    pub config: &'a SliderConfig,
    pub min_value: f64,
    pub max_value: f64,
    pub label: Option<&'a str>,
    /// Editing state for the **min** value box.
    pub editing_min: Option<SliderEditingInfo<'a>>,
    /// Editing state for the **max** value box.
    pub editing_max: Option<SliderEditingInfo<'a>>,
    pub hovered: bool,
    pub active_handle: Option<DualSliderHandle>,
    pub disabled: bool,
}

/// Per-frame inputs for the manual line-width slider (variant 1.5).
pub struct LineWidthSliderView {
    /// Current line-width value.
    pub value: f64,
    /// Minimum value (mlc uses 0.5).
    pub min: f64,
    /// Maximum value (mlc uses 8.0).
    pub max: f64,
    pub hovered: bool,
}

// ─── Result types ─────────────────────────────────────────────────────────────

/// Track geometry returned to the caller for drag-start hit-testing.
///
/// Mirrors the *extended* `SliderTrackInfo` from mlc that includes `track_y`
/// and `track_height` so the input layer can do a proper `y` hit-test.
#[derive(Debug, Default, Clone)]
pub struct SliderTrackInfo {
    /// Left edge of the track in screen coords.
    pub track_x: f64,
    pub track_width: f64,
    /// Top of the hit zone (= track_cy − handle_radius).
    pub track_y: f64,
    /// Height of the hit zone (= handle_radius × 2).
    pub track_height: f64,
    pub min_val: f64,
    pub max_val: f64,
}

/// Returned by `draw_single_slider`.
#[derive(Debug, Default, Clone)]
pub struct SingleSliderResult {
    /// Full row rect (label + track + optional input).
    pub full_rect: Rect,
    /// Just the track bar (no handle).
    pub track_rect: Rect,
    /// Handle bounding box (centre ± handle_radius).
    pub handle_rect: Rect,
    /// `None` when `show_input == false`.
    pub input_rect: Option<Rect>,
    /// Always present — used by the input layer to start drags.
    pub track_info: SliderTrackInfo,
}

/// Returned by `draw_dual_slider`.
#[derive(Debug, Default, Clone)]
pub struct DualSliderResult {
    pub full_rect: Rect,
    pub track_rect: Rect,
    pub min_handle_rect: Rect,
    pub max_handle_rect: Rect,
    pub min_input_rect: Option<Rect>,
    pub max_input_rect: Option<Rect>,
    pub track_info: SliderTrackInfo,
}

/// Returned by `draw_line_width_slider`.
#[derive(Debug, Default, Clone)]
pub struct LineWidthSliderResult {
    /// Hit zone rect (inflated by handle_r on each side of the track).
    pub hit_rect: Rect,
    pub track_info: SliderTrackInfo,
}
