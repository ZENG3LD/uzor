//! Unified widget response pattern
//!
//! Provides `WidgetResponse` - a unified structure returned by all interactive
//! widgets containing information about user interactions during the current frame.

use super::sense::Sense;
use super::text_field::TextAction;
use super::widget_state::WidgetId;
use crate::input::state::InputState;
use crate::types::WidgetRect;

/// Unified response from widget interaction
///
/// Every widget should return a WidgetResponse containing information
/// about user interactions with that widget during the current frame.
#[derive(Clone, Debug)]
pub struct WidgetResponse {
    /// Widget identifier
    pub id: WidgetId,
    /// Widget's bounding rectangle
    pub rect: WidgetRect,
    /// What interactions this widget senses
    pub sense: Sense,

    // Hover state
    /// Mouse is hovering over widget
    pub hovered: bool,
    /// Mouse entered widget this frame
    pub hover_started: bool,
    /// Mouse left widget this frame
    pub hover_ended: bool,

    // Click state
    /// Left mouse button clicked this frame
    pub clicked: bool,
    /// Left mouse button double-clicked this frame
    pub double_clicked: bool,
    /// Left mouse button triple-clicked this frame
    pub triple_clicked: bool,
    /// Right mouse button clicked this frame
    pub right_clicked: bool,
    /// Middle mouse button clicked this frame
    pub middle_clicked: bool,

    // Drag state
    /// Drag operation started this frame
    pub drag_started: bool,
    /// Widget is currently being dragged
    pub dragged: bool,
    /// Drag operation ended this frame
    pub drag_stopped: bool,
    /// Drag delta since last frame
    pub drag_delta: (f64, f64),
    /// Total drag delta since drag started
    pub drag_total: (f64, f64),

    // Scroll state
    /// Widget received scroll/wheel event this frame
    pub scrolled: bool,
    /// Scroll delta (x, y) — typically (0.0, vertical_delta)
    pub scroll_delta: (f64, f64),

    // Focus state
    /// Widget has keyboard focus
    pub has_focus: bool,
    /// Widget gained focus this frame
    pub gained_focus: bool,
    /// Widget lost focus this frame
    pub lost_focus: bool,

    // Value change (for input widgets)
    /// Underlying value changed
    pub changed: bool,

    // Widget state
    /// Widget is enabled (not disabled/grayed)
    pub enabled: bool,

    // Text input state (only populated for text-sensitive widgets)
    /// Text action produced this frame
    pub text_action: Option<TextAction>,
}

impl Default for WidgetResponse {
    fn default() -> Self {
        Self {
            id: WidgetId::new(""),
            rect: WidgetRect::default(),
            sense: Sense::NONE,
            hovered: false,
            hover_started: false,
            hover_ended: false,
            clicked: false,
            double_clicked: false,
            triple_clicked: false,
            right_clicked: false,
            middle_clicked: false,
            drag_started: false,
            dragged: false,
            drag_stopped: false,
            drag_delta: (0.0, 0.0),
            drag_total: (0.0, 0.0),
            scrolled: false,
            scroll_delta: (0.0, 0.0),
            has_focus: false,
            gained_focus: false,
            lost_focus: false,
            changed: false,
            enabled: true,
            text_action: None,
        }
    }
}

impl WidgetResponse {
    /// Create new response for widget
    pub fn new(id: WidgetId, rect: WidgetRect, sense: Sense) -> Self {
        Self {
            id,
            rect,
            sense,
            ..Default::default()
        }
    }

    /// Set hover state
    pub fn with_hover(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Set clicked state
    pub fn with_click(mut self) -> Self {
        self.clicked = true;
        self
    }

    /// Set scroll delta
    pub fn with_scroll(mut self, delta: (f64, f64)) -> Self {
        self.scrolled = true;
        self.scroll_delta = delta;
        self
    }

    /// Set focus state
    pub fn with_focus(mut self, has_focus: bool) -> Self {
        self.has_focus = has_focus;
        self
    }

    /// Mark as changed
    pub fn with_changed(mut self) -> Self {
        self.changed = true;
        self
    }

    /// Mark as disabled
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// Query helper methods
impl WidgetResponse {
    /// Check if any click occurred (left, right, or middle)
    pub fn any_click(&self) -> bool {
        self.clicked || self.right_clicked || self.middle_clicked
    }

    /// Check if widget was interacted with this frame (click, drag start, or gained focus)
    pub fn interacted(&self) -> bool {
        self.any_click() || self.drag_started || self.gained_focus || self.scrolled
    }

    /// Check if widget is active (being dragged or has focus)
    pub fn is_active(&self) -> bool {
        self.dragged || self.has_focus
    }

    /// Check if pointer is over widget or dragging it
    pub fn is_pointer_over(&self) -> bool {
        self.hovered || self.dragged
    }
}

// Response combination
impl WidgetResponse {
    /// Combine two responses (logical OR of interaction states)
    ///
    /// Uses the rect and id of `self`.
    pub fn union(self, other: WidgetResponse) -> WidgetResponse {
        WidgetResponse {
            id: self.id,
            rect: self.rect,
            sense: self.sense.union(other.sense),
            hovered: self.hovered || other.hovered,
            hover_started: self.hover_started || other.hover_started,
            hover_ended: self.hover_ended || other.hover_ended,
            clicked: self.clicked || other.clicked,
            double_clicked: self.double_clicked || other.double_clicked,
            triple_clicked: self.triple_clicked || other.triple_clicked,
            right_clicked: self.right_clicked || other.right_clicked,
            middle_clicked: self.middle_clicked || other.middle_clicked,
            drag_started: self.drag_started || other.drag_started,
            dragged: self.dragged || other.dragged,
            drag_stopped: self.drag_stopped || other.drag_stopped,
            drag_delta: if self.dragged {
                self.drag_delta
            } else {
                other.drag_delta
            },
            drag_total: if self.dragged {
                self.drag_total
            } else {
                other.drag_total
            },
            scrolled: self.scrolled || other.scrolled,
            scroll_delta: if self.scrolled { self.scroll_delta } else { other.scroll_delta },
            has_focus: self.has_focus || other.has_focus,
            gained_focus: self.gained_focus || other.gained_focus,
            lost_focus: self.lost_focus || other.lost_focus,
            changed: self.changed || other.changed,
            enabled: self.enabled && other.enabled,
            text_action: self.text_action.or(other.text_action),
        }
    }
}

impl std::ops::BitOr for WidgetResponse {
    type Output = WidgetResponse;

    /// Combine two responses using the `|` operator (equivalent to union)
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Create a WidgetResponse by testing InputState against a widget rect
pub fn create_response(
    id: WidgetId,
    rect: WidgetRect,
    sense: Sense,
    input: &InputState,
    prev_hovered: bool,
    prev_focused: bool,
) -> WidgetResponse {
    let hovered = if sense.hover {
        input.is_hovered(&rect)
    } else {
        false
    };

    let hover_started = hovered && !prev_hovered;
    let hover_ended = !hovered && prev_hovered;

    let clicked = sense.click && hovered && input.is_clicked();
    let double_clicked = sense.click && hovered && input.is_double_clicked();
    let right_clicked = sense.click && hovered && input.is_right_clicked();
    let middle_clicked = sense.click && hovered && input.is_middle_clicked();

    let (dragged, drag_delta, drag_started, drag_stopped) = if sense.drag {
        let is_dragging = input.is_dragging();
        let delta = input.drag_delta().unwrap_or((0.0, 0.0));
        (is_dragging, delta, false, false)
    } else {
        (false, (0.0, 0.0), false, false)
    };

    WidgetResponse {
        id,
        rect,
        sense,
        hovered,
        hover_started,
        hover_ended,
        clicked,
        double_clicked,
        triple_clicked: false,
        right_clicked,
        middle_clicked,
        drag_started,
        dragged,
        drag_stopped,
        drag_delta,
        drag_total: (0.0, 0.0),
        scrolled: false,
        scroll_delta: (0.0, 0.0),
        has_focus: prev_focused,
        gained_focus: false,
        lost_focus: false,
        changed: false,
        enabled: true,
        text_action: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_response() {
        let response = WidgetResponse::default();
        assert!(!response.clicked);
        assert!(!response.hovered);
        assert!(response.enabled);
        assert_eq!(response.sense, Sense::NONE);
        assert!(response.text_action.is_none());
    }

    #[test]
    fn test_response_new() {
        let id = WidgetId::new("test_button");
        let rect = WidgetRect::new(10.0, 10.0, 100.0, 40.0);
        let response = WidgetResponse::new(id.clone(), rect, Sense::CLICK);

        assert_eq!(response.id, id);
        assert_eq!(response.rect.x, 10.0);
        assert_eq!(response.sense, Sense::CLICK);
    }

    #[test]
    fn test_builder_methods() {
        let response = WidgetResponse::new(
            WidgetId::new("test"),
            WidgetRect::default(),
            Sense::CLICK,
        )
        .with_hover(true)
        .with_click()
        .with_focus(true)
        .with_changed();

        assert!(response.hovered);
        assert!(response.clicked);
        assert!(response.has_focus);
        assert!(response.changed);
    }

    #[test]
    fn test_disabled_builder() {
        let response = WidgetResponse::new(
            WidgetId::new("test"),
            WidgetRect::default(),
            Sense::CLICK,
        )
        .disabled();

        assert!(!response.enabled);
    }

    #[test]
    fn test_any_click() {
        let mut response = WidgetResponse::default();
        assert!(!response.any_click());

        response.clicked = true;
        assert!(response.any_click());

        response.clicked = false;
        response.right_clicked = true;
        assert!(response.any_click());

        response.right_clicked = false;
        response.middle_clicked = true;
        assert!(response.any_click());
    }

    #[test]
    fn test_interacted() {
        let mut response = WidgetResponse::default();
        assert!(!response.interacted());

        response.clicked = true;
        assert!(response.interacted());

        response = WidgetResponse::default();
        response.drag_started = true;
        assert!(response.interacted());

        response = WidgetResponse::default();
        response.gained_focus = true;
        assert!(response.interacted());
    }

    #[test]
    fn test_is_active() {
        let mut response = WidgetResponse::default();
        assert!(!response.is_active());

        response.dragged = true;
        assert!(response.is_active());

        response.dragged = false;
        response.has_focus = true;
        assert!(response.is_active());
    }

    #[test]
    fn test_is_pointer_over() {
        let mut response = WidgetResponse::default();
        assert!(!response.is_pointer_over());

        response.hovered = true;
        assert!(response.is_pointer_over());

        response.hovered = false;
        response.dragged = true;
        assert!(response.is_pointer_over());
    }

    #[test]
    fn test_response_union() {
        let response1 = WidgetResponse::new(
            WidgetId::new("widget1"),
            WidgetRect::new(0.0, 0.0, 100.0, 100.0),
            Sense::CLICK,
        )
        .with_hover(true);

        let response2 = WidgetResponse::new(
            WidgetId::new("widget2"),
            WidgetRect::new(50.0, 50.0, 100.0, 100.0),
            Sense::DRAG,
        )
        .with_click();

        let combined = response1.clone().union(response2);

        assert_eq!(combined.id, response1.id);
        assert_eq!(combined.rect.x, response1.rect.x);
        assert!(combined.hovered);
        assert!(combined.clicked);
        assert!(combined.sense.hover);
        assert!(combined.sense.click);
        assert!(combined.sense.drag);
    }

    #[test]
    fn test_response_bitor_operator() {
        let response1 = WidgetResponse::default().with_hover(true);
        let response2 = WidgetResponse::default().with_click();

        let combined = response1 | response2;

        assert!(combined.hovered);
        assert!(combined.clicked);
    }

    #[test]
    fn test_union_preserves_drag_data() {
        let mut response1 = WidgetResponse::default();
        response1.dragged = true;
        response1.drag_delta = (10.0, 20.0);
        response1.drag_total = (30.0, 40.0);

        let mut response2 = WidgetResponse::default();
        response2.drag_delta = (5.0, 5.0);
        response2.drag_total = (15.0, 15.0);

        let combined = response1.clone().union(response2);

        assert!(combined.dragged);
        assert_eq!(combined.drag_delta, (10.0, 20.0));
        assert_eq!(combined.drag_total, (30.0, 40.0));
    }

    #[test]
    fn test_union_enabled_state() {
        let response1 = WidgetResponse::default();
        let response2 = WidgetResponse::default().disabled();

        let combined = response1.union(response2);
        assert!(!combined.enabled);
    }

    #[test]
    fn test_union_text_action() {
        let mut response1 = WidgetResponse::default();
        response1.text_action = Some(TextAction::TextChanged("hello".into()));

        let response2 = WidgetResponse::default();

        let combined = response1.union(response2);
        assert_eq!(combined.text_action, Some(TextAction::TextChanged("hello".into())));
    }

    #[test]
    fn test_create_response_hover() {
        let mut input = InputState::new();
        input.pointer.pos = Some((50.0, 50.0));

        let rect = WidgetRect::new(10.0, 10.0, 100.0, 100.0);
        let response = create_response(
            WidgetId::new("test"),
            rect,
            Sense::HOVER,
            &input,
            false,
            false,
        );

        assert!(response.hovered);
        assert!(response.hover_started);
    }

    #[test]
    fn test_create_response_hover_ended() {
        let mut input = InputState::new();
        input.pointer.pos = Some((5.0, 5.0));

        let rect = WidgetRect::new(10.0, 10.0, 100.0, 100.0);
        let response = create_response(
            WidgetId::new("test"),
            rect,
            Sense::HOVER,
            &input,
            true,
            false,
        );

        assert!(!response.hovered);
        assert!(response.hover_ended);
    }

    #[test]
    fn test_create_response_click() {
        let mut input = InputState::new();
        input.pointer.pos = Some((50.0, 50.0));
        input.pointer.clicked = Some(crate::input::state::MouseButton::Left);

        let rect = WidgetRect::new(10.0, 10.0, 100.0, 100.0);
        let response = create_response(
            WidgetId::new("test"),
            rect,
            Sense::CLICK,
            &input,
            false,
            false,
        );

        assert!(response.hovered);
        assert!(response.clicked);
    }

    #[test]
    fn test_create_response_no_sense() {
        let mut input = InputState::new();
        input.pointer.pos = Some((50.0, 50.0));
        input.pointer.clicked = Some(crate::input::state::MouseButton::Left);

        let rect = WidgetRect::new(10.0, 10.0, 100.0, 100.0);
        let response = create_response(
            WidgetId::new("test"),
            rect,
            Sense::NONE,
            &input,
            false,
            false,
        );

        assert!(!response.hovered);
        assert!(!response.clicked);
    }

    #[test]
    fn test_create_response_right_click() {
        let mut input = InputState::new();
        input.pointer.pos = Some((50.0, 50.0));
        input.pointer.clicked = Some(crate::input::state::MouseButton::Right);

        let rect = WidgetRect::new(10.0, 10.0, 100.0, 100.0);
        let response = create_response(
            WidgetId::new("test"),
            rect,
            Sense::CLICK,
            &input,
            false,
            false,
        );

        assert!(response.right_clicked);
        assert!(!response.clicked);
    }
}
