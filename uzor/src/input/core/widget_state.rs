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
    /// Widget id that was hovered at the moment `mouse_pressed` most
    /// recently transitioned from `false` to `true` — `None` once the
    /// button is released. This is the "press origin": the widget the
    /// press actually started on. `InputCoordinator::widget_state` reads
    /// it so dragging a held button in from elsewhere never shows
    /// `Pressed` on a widget merely hovered underneath the pointer — only
    /// the widget the press began on can ever report `Pressed`.
    pub press_origin: Option<WidgetId>,
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

    /// Set the press-origin widget (see [`Self::press_origin`]).
    pub fn set_press_origin(&mut self, id: Option<WidgetId>) {
        self.press_origin = id;
    }

    /// Check whether `id` is the widget the current press originated on.
    pub fn is_press_origin(&self, id: &WidgetId) -> bool {
        self.press_origin.as_ref() == Some(id)
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
#[derive(Default)]
pub enum WidgetInteraction {
    #[default]
    None,
    Hover,
    Press,
    Drag,
    Click,
    DoubleClick,
    TripleClick,
    Focus,
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
}

impl WidgetInputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update mouse position
    pub fn update_mouse(&mut self, x: f64, y: f64) {
        self.hover.update_mouse(x, y);
        if self.drag.dragging.is_some() {
            self.drag.update(x, y);
        }
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
}
