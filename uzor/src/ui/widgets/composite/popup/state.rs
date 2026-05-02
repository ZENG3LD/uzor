//! Popup persistent state.
//!
//! `PopupState` is a flat struct — fields irrelevant to the active
//! `PopupRenderKind` are simply never touched.

use crate::types::{Rect, ScrollState};
use super::super::resize_drag::ResizeDrag;
use super::types::{ColorPickerLevel, HsvColor};

/// All per-popup frame state.
#[derive(Debug, Clone)]
pub struct PopupState {
    // --- Lifecycle ---

    /// Whether the popup is currently open.
    pub open: bool,

    // --- Position ---

    /// Top-left corner of the popup in screen coordinates.
    pub position: (f64, f64),

    /// Anchor trigger-rect origin for smart re-positioning on resize.
    pub anchor: Option<(f64, f64)>,

    // --- Close button hover (Plain / any kind with an X button) ---

    /// Whether the pointer is over the close button this frame.
    pub hovered_close: bool,

    // --- ColorPickerGrid / ColorPickerHsv ---

    /// Current level for color picker state machine.
    pub level: ColorPickerLevel,

    /// Index of the hovered palette swatch.
    pub hovered_swatch: Option<usize>,

    /// Currently selected color (hex string).
    pub current_color: String,

    /// User-added custom colors (hex strings), max 10.
    pub custom_colors: Vec<String>,

    /// Opacity value for color picker, 0.0–1.0.
    pub opacity: f64,

    /// Opacity stored before toggling off (restored on toggle-on).
    pub previous_opacity: Option<f64>,

    /// Whether the opacity slider is being dragged.
    pub dragging_opacity: bool,

    // --- ColorPickerHsv (L2) ---

    /// Current HSV color.
    pub hsv: HsvColor,

    /// Hex input string (may differ from HSV during editing).
    pub hex_input: String,

    /// Whether the hex input field is focused.
    pub hex_editing: bool,

    /// Cursor char index within hex input.
    pub hex_cursor: usize,

    /// Whether the SV square is being dragged.
    pub dragging_sv: bool,

    /// Whether the hue bar is being dragged.
    pub dragging_hue: bool,

    // --- SwatchGrid ---

    /// Index of the hovered swatch in the SwatchGrid.
    pub hovered_swatch_index: Option<usize>,

    /// Whether the Remove row is hovered.
    pub hovered_remove: bool,

    /// Whether the "+" add-custom button is hovered.
    pub hovered_add: bool,

    // --- ItemList ---

    /// Id of the currently hovered item in an ItemList popup.
    pub hovered_item_id: Option<String>,

    // --- IndicatorStrip ---

    /// Id of the hovered indicator row.
    pub hovered_indicator_id: Option<u64>,

    /// `(indicator_id, action_name)` of the hovered action button.
    pub hovered_action: Option<(u64, &'static str)>,

    // --- Resize ---

    /// Resize drag in progress.
    pub resize_drag: Option<ResizeDrag>,

    /// User-resized override for the popup frame. `None` = use the
    /// caller-supplied measured rect.
    pub resized_rect: Option<Rect>,

    // --- Body scroll (Scrollbar / Chevrons overflow modes) ---

    pub scroll: ScrollState,
    pub body_content_h: f64,
    pub body_scroll_track: Option<Rect>,
    pub body_viewport_h: f64,
}

impl Default for PopupState {
    fn default() -> Self {
        Self {
            open: false,
            position: (0.0, 0.0),
            anchor: None,
            hovered_close: false,
            level: ColorPickerLevel::Closed,
            hovered_swatch: None,
            current_color: String::from("#2962ff"),
            custom_colors: Vec::new(),
            opacity: 1.0,
            previous_opacity: None,
            dragging_opacity: false,
            hsv: HsvColor::default(),
            hex_input: String::from("2962ff"),
            hex_editing: false,
            hex_cursor: 0,
            dragging_sv: false,
            dragging_hue: false,
            hovered_swatch_index: None,
            hovered_remove: false,
            hovered_add: false,
            hovered_item_id: None,
            hovered_indicator_id: None,
            hovered_action: None,
            resize_drag: None,
            resized_rect: None,
            scroll: ScrollState::default(),
            body_content_h: 0.0,
            body_scroll_track: None,
            body_viewport_h: 0.0,
        }
    }
}

impl PopupState {
    /// Open the color picker at L1.
    pub fn open_color_picker(&mut self, origin: (f64, f64)) {
        self.open = true;
        self.position = origin;
        self.level = ColorPickerLevel::L1;
    }

    /// Transition from L1 to L2 (HSV editor).
    pub fn open_l2(&mut self) {
        self.level = ColorPickerLevel::L2;
        self.hex_editing = false;
        self.hex_cursor = 0;
    }

    /// Transition from L2 back to L1.
    pub fn back_to_l1(&mut self) {
        self.level = ColorPickerLevel::L1;
        self.dragging_sv = false;
        self.dragging_hue = false;
        self.dragging_opacity = false;
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.open = false;
        self.level = ColorPickerLevel::Closed;
        self.dragging_sv = false;
        self.dragging_hue = false;
        self.dragging_opacity = false;
        self.hex_editing = false;
    }

    /// Toggle opacity between 0 and the previous non-zero value.
    pub fn toggle_opacity(&mut self) {
        if self.opacity > 0.0 {
            self.previous_opacity = Some(self.opacity);
            self.opacity = 0.0;
        } else {
            self.opacity = self.previous_opacity.unwrap_or(1.0);
        }
    }

    /// Returns `true` if any drag gesture is in progress (guards click-outside dismiss).
    pub fn is_dragging_any(&self) -> bool {
        self.dragging_sv || self.dragging_hue || self.dragging_opacity || self.resize_drag.is_some()
    }

    /// Begin a resize drag from a `ResizeHandleDragStarted` event.
    pub fn start_resize(
        &mut self,
        edge:       crate::layout::ResizeEdge,
        start_rect: Rect,
        cursor:     (f64, f64),
        min:        (f64, f64),
        cap:        (f64, f64),
    ) {
        self.resize_drag = Some(ResizeDrag::begin(edge, start_rect, cursor, min, cap));
    }

    /// Update the resized rect from a fresh cursor position.
    pub fn update_resize(&mut self, cursor: (f64, f64)) {
        if let Some(drag) = self.resize_drag {
            let rect = drag.resolve(cursor);
            self.resized_rect = Some(rect);
            self.position = (rect.x, rect.y);
        }
    }

    /// End any active resize drag.
    pub fn end_resize(&mut self) {
        self.resize_drag = None;
    }

    // --- Body scroll helpers ---

    pub fn start_body_scroll_drag(&mut self, cursor_y: f64) {
        self.scroll.start_drag(cursor_y);
    }
    pub fn update_body_scroll_drag(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_drag(cursor_y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }
    pub fn body_scroll_track_click(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_track_click(cursor_y, track.y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }
    pub fn body_chevron_step(&mut self, direction: crate::layout::ChevronStepDirection) {
        let max = (self.body_content_h - self.body_viewport_h).max(0.0);
        let step = self.body_viewport_h.max(40.0);
        let signed = match direction {
            crate::layout::ChevronStepDirection::Up
            | crate::layout::ChevronStepDirection::Left => -step,
            _ => step,
        };
        self.scroll.offset = (self.scroll.offset + signed).clamp(0.0, max);
    }
    pub fn end_body_scroll_drag(&mut self) {
        self.scroll.end_drag();
    }
}
