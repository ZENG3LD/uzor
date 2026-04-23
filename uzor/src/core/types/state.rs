//! Widget state types for interaction tracking

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Widget interaction state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetState {
    /// Normal state, no interaction
    #[default]
    Normal,
    /// Mouse hovering over widget
    Hovered,
    /// Mouse pressed on widget
    Pressed,
    /// Widget is in an active/toggled state
    Active,
    /// Widget is toggled ON (for checkboxes/toggles)
    Toggled,
    /// Widget is disabled
    Disabled,
}

impl WidgetState {
    pub fn is_hovered(&self) -> bool {
        matches!(self, Self::Hovered | Self::Pressed)
    }

    pub fn is_pressed(&self) -> bool {
        matches!(self, Self::Pressed)
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }
}

/// Unique widget identifier
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub String);

impl WidgetId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for WidgetId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for WidgetId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Focus state for widgets
#[derive(Clone, Debug, Default)]
pub struct FocusState {
    /// Currently focused widget ID
    pub focused: Option<WidgetId>,
    /// Widget that will receive focus on next frame
    pub pending_focus: Option<WidgetId>,
}

impl FocusState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set focus to a widget
    pub fn set_focus(&mut self, id: WidgetId) {
        self.focused = Some(id);
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Check if a widget is focused
    pub fn is_focused(&self, id: &WidgetId) -> bool {
        self.focused.as_ref() == Some(id)
    }

    /// Request focus for next frame
    pub fn request_focus(&mut self, id: WidgetId) {
        self.pending_focus = Some(id);
    }

    /// Process pending focus changes
    pub fn process_pending(&mut self) {
        if let Some(id) = self.pending_focus.take() {
            self.focused = Some(id);
        }
    }
}

/// Hover state for widgets
#[derive(Clone, Debug, Default)]
pub struct HoverState {
    /// Currently hovered widget ID
    pub hovered: Option<WidgetId>,
    /// Mouse position
    pub mouse_pos: (f64, f64),
    /// Whether mouse is pressed
    pub mouse_pressed: bool,
}

impl HoverState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update mouse position
    pub fn update_mouse(&mut self, x: f64, y: f64) {
        self.mouse_pos = (x, y);
    }

    /// Set hovered widget
    pub fn set_hovered(&mut self, id: Option<WidgetId>) {
        self.hovered = id;
    }

    /// Check if a widget is hovered
    pub fn is_hovered(&self, id: &WidgetId) -> bool {
        self.hovered.as_ref() == Some(id)
    }

    /// Set mouse pressed state
    pub fn set_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }
}

/// Widget drag state
#[derive(Clone, Debug, Default)]
pub struct DragState {
    /// Widget being dragged
    pub dragging: Option<WidgetId>,
    /// Drag start position
    pub start_pos: (f64, f64),
    /// Current drag position
    pub current_pos: (f64, f64),
    /// Drag offset from widget origin
    pub offset: (f64, f64),
    /// Initial value when drag started (for sliders, scrollbars)
    pub initial_value: f64,
}

impl DragState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start dragging a widget
    pub fn start(&mut self, id: WidgetId, x: f64, y: f64, offset_x: f64, offset_y: f64) {
        self.dragging = Some(id);
        self.start_pos = (x, y);
        self.current_pos = (x, y);
        self.offset = (offset_x, offset_y);
    }

    /// Start dragging with initial value
    pub fn start_with_value(&mut self, id: WidgetId, x: f64, y: f64, value: f64) {
        self.dragging = Some(id);
        self.start_pos = (x, y);
        self.current_pos = (x, y);
        self.offset = (0.0, 0.0);
        self.initial_value = value;
    }

    /// Update drag position
    pub fn update(&mut self, x: f64, y: f64) {
        self.current_pos = (x, y);
    }

    /// End dragging
    pub fn end(&mut self) {
        self.dragging = None;
    }

    /// Check if a widget is being dragged
    pub fn is_dragging(&self, id: &WidgetId) -> bool {
        self.dragging.as_ref() == Some(id)
    }

    /// Get drag delta from start
    pub fn delta(&self) -> (f64, f64) {
        (
            self.current_pos.0 - self.start_pos.0,
            self.current_pos.1 - self.start_pos.1,
        )
    }

    /// Get drag delta from last frame
    pub fn delta_from(&self, last_pos: (f64, f64)) -> (f64, f64) {
        (
            self.current_pos.0 - last_pos.0,
            self.current_pos.1 - last_pos.1,
        )
    }
}

/// Scroll state for a scrollable container
///
/// Include this in your modal/widget state to enable scrolling.
#[derive(Clone, Debug, Default)]
pub struct ScrollState {
    /// Current scroll offset (pixels from top)
    pub offset: f64,
    pub velocity: f64,
    pub content_size: f64,
    /// Is scrollbar handle being dragged?
    pub is_dragging: bool,
    /// Y position where drag started
    pub drag_start_y: Option<f64>,
    /// Scroll offset when drag started
    pub drag_start_offset: Option<f64>,
}

impl ScrollState {
    /// Create new scroll state
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset scroll state (e.g., when content changes)
    pub fn reset(&mut self) {
        self.offset = 0.0;
        self.is_dragging = false;
        self.drag_start_y = None;
        self.drag_start_offset = None;
    }

    /// Start scrollbar drag
    pub fn start_drag(&mut self, y: f64) {
        self.is_dragging = true;
        self.drag_start_y = Some(y);
        self.drag_start_offset = Some(self.offset);
    }

    /// End scrollbar drag
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_start_y = None;
        self.drag_start_offset = None;
    }

    /// Handle mouse wheel scroll
    ///
    /// Returns true if scroll was handled (content overflows)
    pub fn handle_wheel(&mut self, delta_y: f64, content_height: f64, viewport_height: f64) -> bool {
        if content_height <= viewport_height {
            return false;
        }
        let max_scroll = (content_height - viewport_height).max(0.0);
        let scroll_step = 30.0; // pixels per scroll tick
        self.offset = (self.offset + delta_y * scroll_step).clamp(0.0, max_scroll);
        true
    }

    /// Handle scrollbar drag motion
    ///
    /// Call this in on_mouse_move when is_dragging is true
    pub fn handle_drag(&mut self, y: f64, track_height: f64, content_height: f64, viewport_height: f64) {
        if !self.is_dragging {
            return;
        }

        let Some(start_y) = self.drag_start_y else { return };
        let Some(start_offset) = self.drag_start_offset else { return };

        let max_scroll = (content_height - viewport_height).max(0.0);
        if max_scroll <= 0.0 {
            return;
        }

        let visible_ratio = viewport_height / content_height;
        let handle_height = (visible_ratio * track_height).max(30.0);
        let available_height = track_height - handle_height;

        if available_height > 0.0 {
            let delta_y = y - start_y;
            let scroll_delta = (delta_y / available_height) * max_scroll;
            self.offset = (start_offset + scroll_delta).clamp(0.0, max_scroll);
        }
    }

    /// Handle click on scrollbar track (jump to position)
    pub fn handle_track_click(&mut self, click_y: f64, track_y: f64, track_height: f64, content_height: f64, viewport_height: f64) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        if max_scroll <= 0.0 {
            return;
        }
        let relative_y = (click_y - track_y) / track_height;
        self.offset = (relative_y * max_scroll).clamp(0.0, max_scroll);
    }

    /// Clamp offset to valid range
    pub fn clamp(&mut self, content_height: f64, viewport_height: f64) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        self.offset = self.offset.clamp(0.0, max_scroll);
    }
}

/// Widget interaction type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum WidgetInteraction {
    /// No interaction
    #[default]
    None,
    /// Mouse hovering over widget
    Hover,
    /// Mouse pressed on widget
    Press,
    /// Widget is being dragged
    Drag,
    /// Widget was clicked (press + release)
    Click,
    /// Widget was double-clicked
    DoubleClick,
    /// Widget has keyboard focus
    Focus,
}


/// Custom per-widget data
#[derive(Clone, Debug, Default)]
pub struct WidgetData {
    /// Slider/scrollbar current value
    pub value: f64,
    /// Input text content
    pub text: String,
    /// Cursor position in text
    pub cursor_pos: usize,
    /// Selection start (for text input)
    pub selection_start: Option<usize>,
    /// Checkbox checked state
    pub checked: bool,
    /// Dropdown open state
    pub open: bool,
    /// Selected index (for dropdown, list)
    pub selected_index: Option<usize>,
    /// Scroll offset
    pub scroll_offset: f64,
}

/// Combined widget input state
#[derive(Clone, Debug, Default)]
pub struct WidgetInputState {
    /// Focus management
    pub focus: FocusState,
    /// Hover tracking
    pub hover: HoverState,
    /// Drag tracking
    pub drag: DragState,
    /// Active widget (pressed but not yet released)
    pub active: Option<WidgetId>,
    /// Last click time for double-click detection
    pub last_click_time: f64,
    /// Last click position
    pub last_click_pos: (f64, f64),
    /// Last clicked widget
    pub last_click_widget: Option<WidgetId>,
    /// Double-click threshold in milliseconds
    pub double_click_threshold_ms: f64,
    /// Double-click distance threshold in pixels
    pub double_click_distance: f64,
    /// Custom widget data storage
    pub widget_data: HashMap<WidgetId, WidgetData>,
}

impl WidgetInputState {
    pub fn new() -> Self {
        Self {
            double_click_threshold_ms: 500.0,
            double_click_distance: 5.0,
            ..Default::default()
        }
    }

    /// Update mouse position
    pub fn update_mouse(&mut self, x: f64, y: f64) {
        self.hover.update_mouse(x, y);
        if self.drag.dragging.is_some() {
            self.drag.update(x, y);
        }
    }

    /// Handle mouse press
    pub fn mouse_press(&mut self, _x: f64, _y: f64, widget_id: Option<WidgetId>) {
        self.hover.set_pressed(true);
        self.active = widget_id;
    }

    /// Handle mouse release
    pub fn mouse_release(&mut self, x: f64, y: f64, now: f64) -> WidgetInteraction {
        self.hover.set_pressed(false);

        let was_dragging = self.drag.dragging.is_some();
        self.drag.end();

        if was_dragging {
            self.active = None;
            return WidgetInteraction::None;
        }

        // Check for click/double-click
        if let Some(ref active_id) = self.active {
            // Check for double-click
            let is_double_click = if let Some(ref last_id) = self.last_click_widget {
                let time_ok = (now - self.last_click_time) < self.double_click_threshold_ms;
                let dist = ((x - self.last_click_pos.0).powi(2) + (y - self.last_click_pos.1).powi(2)).sqrt();
                let dist_ok = dist < self.double_click_distance;
                last_id == active_id && time_ok && dist_ok
            } else {
                false
            };

            // Update last click info
            self.last_click_time = now;
            self.last_click_pos = (x, y);
            self.last_click_widget = Some(active_id.clone());

            self.active = None;

            if is_double_click {
                return WidgetInteraction::DoubleClick;
            } else {
                return WidgetInteraction::Click;
            }
        }

        self.active = None;
        WidgetInteraction::None
    }

    /// Start dragging a widget
    pub fn start_drag(&mut self, id: WidgetId, x: f64, y: f64) {
        self.drag.start(id, x, y, 0.0, 0.0);
    }

    /// Start dragging with value (for sliders)
    pub fn start_drag_with_value(&mut self, id: WidgetId, x: f64, y: f64, value: f64) {
        self.drag.start_with_value(id, x, y, value);
    }

    /// Get widget data
    pub fn get_data(&self, id: &WidgetId) -> Option<&WidgetData> {
        self.widget_data.get(id)
    }

    /// Get mutable widget data
    pub fn get_data_mut(&mut self, id: &WidgetId) -> &mut WidgetData {
        self.widget_data.entry(id.clone()).or_default()
    }

    /// Set widget value (slider, scrollbar)
    pub fn set_value(&mut self, id: &WidgetId, value: f64) {
        self.get_data_mut(id).value = value;
    }

    /// Get widget value
    pub fn get_value(&self, id: &WidgetId) -> f64 {
        self.get_data(id).map(|d| d.value).unwrap_or(0.0)
    }

    /// Set widget checked state
    pub fn set_checked(&mut self, id: &WidgetId, checked: bool) {
        self.get_data_mut(id).checked = checked;
    }

    /// Get widget checked state
    pub fn get_checked(&self, id: &WidgetId) -> bool {
        self.get_data(id).map(|d| d.checked).unwrap_or(false)
    }

    /// Toggle widget open state (dropdown)
    pub fn toggle_open(&mut self, id: &WidgetId) {
        let data = self.get_data_mut(id);
        data.open = !data.open;
    }

    /// Get widget open state
    pub fn is_open(&self, id: &WidgetId) -> bool {
        self.get_data(id).map(|d| d.open).unwrap_or(false)
    }

    /// Close all open dropdowns
    pub fn close_all_dropdowns(&mut self) {
        for data in self.widget_data.values_mut() {
            data.open = false;
        }
    }

    /// Process frame end (update pending states)
    pub fn end_frame(&mut self) {
        self.focus.process_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_state() {
        assert!(WidgetState::Hovered.is_hovered());
        assert!(WidgetState::Pressed.is_hovered());
        assert!(WidgetState::Pressed.is_pressed());
        assert!(!WidgetState::Normal.is_hovered());
    }

    #[test]
    fn test_widget_id() {
        let id1 = WidgetId::new("button1");
        let id2: WidgetId = "button2".into();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_focus_state() {
        let mut focus = FocusState::new();
        let id = WidgetId::new("input1");

        assert!(!focus.is_focused(&id));

        focus.set_focus(id.clone());
        assert!(focus.is_focused(&id));

        focus.clear_focus();
        assert!(!focus.is_focused(&id));
    }

    #[test]
    fn test_hover_state() {
        let mut hover = HoverState::new();
        let id = WidgetId::new("button1");

        hover.update_mouse(100.0, 50.0);
        assert_eq!(hover.mouse_pos, (100.0, 50.0));

        hover.set_hovered(Some(id.clone()));
        assert!(hover.is_hovered(&id));

        hover.set_hovered(None);
        assert!(!hover.is_hovered(&id));
    }

    #[test]
    fn test_drag_state() {
        let mut drag = DragState::new();
        let id = WidgetId::new("slider1");

        drag.start(id.clone(), 100.0, 50.0, 5.0, 0.0);
        assert!(drag.is_dragging(&id));

        drag.update(150.0, 60.0);
        assert_eq!(drag.delta(), (50.0, 10.0));

        drag.end();
        assert!(!drag.is_dragging(&id));
    }

    #[test]
    fn test_scroll_state_wheel() {
        let mut state = ScrollState::new();

        // No scroll when content fits
        assert!(!state.handle_wheel(1.0, 100.0, 200.0));
        assert_eq!(state.offset, 0.0);

        // Scroll when content overflows
        assert!(state.handle_wheel(1.0, 500.0, 200.0));
        assert!(state.offset > 0.0);

        // Clamp to max
        state.offset = 1000.0;
        state.clamp(500.0, 200.0);
        assert_eq!(state.offset, 300.0); // max_scroll = 500 - 200 = 300
    }

    #[test]
    fn test_scroll_state_drag() {
        let mut state = ScrollState::new();

        assert!(!state.is_dragging);
        state.start_drag(100.0);
        assert!(state.is_dragging);
        assert_eq!(state.drag_start_y, Some(100.0));

        state.end_drag();
        assert!(!state.is_dragging);
        assert_eq!(state.drag_start_y, None);
    }

    #[test]
    fn test_scroll_state_reset() {
        let mut state = ScrollState::new();
        state.offset = 50.0;
        state.is_dragging = true;
        state.drag_start_y = Some(100.0);

        state.reset();
        assert_eq!(state.offset, 0.0);
        assert!(!state.is_dragging);
        assert_eq!(state.drag_start_y, None);
    }

    #[test]
    fn test_widget_input_state() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("slider1");

        state.set_value(&id, 0.5);
        assert!((state.get_value(&id) - 0.5).abs() < 0.001);

        state.set_checked(&id, true);
        assert!(state.get_checked(&id));
    }

    #[test]
    fn test_click_detection() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        // Simulate click
        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(100.0, 50.0, 1000.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }

    #[test]
    fn test_double_click_detection() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        // First click
        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);

        // Second click within threshold
        state.mouse_press(101.0, 51.0, Some(id.clone()));
        let interaction = state.mouse_release(101.0, 51.0, 1200.0);
        assert_eq!(interaction, WidgetInteraction::DoubleClick);
    }
}
