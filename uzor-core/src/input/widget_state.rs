//! Widget input state management
//!
//! Centralized state tracking for widget interactions across frames.

pub use crate::types::state::WidgetId;

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

/// Widget interaction type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetInteraction {
    None,
    Hover,
    Press,
    Drag,
    Click,
    DoubleClick,
    TripleClick,
    Focus,
}

impl Default for WidgetInteraction {
    fn default() -> Self {
        Self::None
    }
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
    /// Click count for multi-click detection (1 = single, 2 = double, 3 = triple)
    pub click_count: u8,
    /// Triple-click threshold in milliseconds
    pub triple_click_threshold_ms: f64,
}

impl WidgetInputState {
    pub fn new() -> Self {
        Self {
            double_click_threshold_ms: 500.0,
            double_click_distance: 5.0,
            click_count: 0,
            triple_click_threshold_ms: 300.0,
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

    /// Handle mouse release with click/double-click/triple-click detection
    pub fn mouse_release(&mut self, x: f64, y: f64, now: f64) -> WidgetInteraction {
        self.hover.set_pressed(false);

        let was_dragging = self.drag.dragging.is_some();
        self.drag.end();

        if was_dragging {
            self.active = None;
            return WidgetInteraction::None;
        }

        if let Some(ref active_id) = self.active {
            let is_same_widget = self.last_click_widget.as_ref() == Some(active_id);
            let time_since_last = now - self.last_click_time;
            let dist = ((x - self.last_click_pos.0).powi(2) + (y - self.last_click_pos.1).powi(2)).sqrt();
            let dist_ok = dist < self.double_click_distance;

            let interaction = if is_same_widget && dist_ok {
                if time_since_last < self.triple_click_threshold_ms && self.click_count == 2 {
                    self.click_count = 3;
                    WidgetInteraction::TripleClick
                } else if time_since_last < self.double_click_threshold_ms && self.click_count == 1 {
                    self.click_count = 2;
                    WidgetInteraction::DoubleClick
                } else {
                    self.click_count = 1;
                    WidgetInteraction::Click
                }
            } else {
                self.click_count = 1;
                WidgetInteraction::Click
            };

            self.last_click_time = now;
            self.last_click_pos = (x, y);
            self.last_click_widget = Some(active_id.clone());

            self.active = None;
            return interaction;
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

    /// Process frame end (update pending states)
    pub fn end_frame(&mut self) {
        self.focus.process_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_click_detection() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(100.0, 50.0, 1000.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }

    #[test]
    fn test_double_click_detection() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);

        state.mouse_press(101.0, 51.0, Some(id.clone()));
        let interaction = state.mouse_release(101.0, 51.0, 1200.0);
        assert_eq!(interaction, WidgetInteraction::DoubleClick);
    }

    #[test]
    fn test_triple_click_detection() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction1 = state.mouse_release(100.0, 50.0, 1000.0);
        assert_eq!(interaction1, WidgetInteraction::Click);

        state.mouse_press(101.0, 51.0, Some(id.clone()));
        let interaction2 = state.mouse_release(101.0, 51.0, 1200.0);
        assert_eq!(interaction2, WidgetInteraction::DoubleClick);

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction3 = state.mouse_release(100.0, 50.0, 1400.0);
        assert_eq!(interaction3, WidgetInteraction::TripleClick);
    }

    #[test]
    fn test_triple_click_timeout() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);

        state.mouse_press(101.0, 51.0, Some(id.clone()));
        state.mouse_release(101.0, 51.0, 1200.0);

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(100.0, 50.0, 2000.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }

    #[test]
    fn test_triple_click_different_widget() {
        let mut state = WidgetInputState::new();
        let id1 = WidgetId::new("button1");
        let id2 = WidgetId::new("button2");

        state.mouse_press(100.0, 50.0, Some(id1.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);

        state.mouse_press(101.0, 51.0, Some(id1.clone()));
        state.mouse_release(101.0, 51.0, 1200.0);

        state.mouse_press(200.0, 50.0, Some(id2.clone()));
        let interaction = state.mouse_release(200.0, 50.0, 1400.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }

    #[test]
    fn test_triple_click_too_far() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);

        state.mouse_press(101.0, 51.0, Some(id.clone()));
        state.mouse_release(101.0, 51.0, 1200.0);

        state.mouse_press(200.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(200.0, 50.0, 1400.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }

    #[test]
    fn test_click_count_reset_after_triple() {
        let mut state = WidgetInputState::new();
        let id = WidgetId::new("button1");

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1000.0);
        state.mouse_press(100.0, 50.0, Some(id.clone()));
        state.mouse_release(100.0, 50.0, 1200.0);
        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(100.0, 50.0, 1400.0);
        assert_eq!(interaction, WidgetInteraction::TripleClick);

        state.mouse_press(100.0, 50.0, Some(id.clone()));
        let interaction = state.mouse_release(100.0, 50.0, 1600.0);
        assert_eq!(interaction, WidgetInteraction::Click);
    }
}
