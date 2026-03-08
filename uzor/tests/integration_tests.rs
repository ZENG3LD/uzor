//! Integration tests for uzor - widget + input interaction
//!
//! These tests verify that widgets work correctly with the input system,
//! covering InputState query methods, scroll state, keyboard modifiers,
//! and boundary conditions.

use uzor::input::{InputState, MouseButton, ModifierKeys, PointerState};
use uzor::input::state::DragState as InputDragState;
use uzor::types::{
    ScrollState, WidgetId, WidgetInputState, WidgetInteraction, WidgetRect, WidgetState,
};

// =============================================================================
// Helper Functions
// =============================================================================

/// Create InputState with pointer at position
fn input_at(x: f64, y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((x, y)),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Create InputState with pointer at position and button clicked
fn input_clicked_at(x: f64, y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((x, y)),
            clicked: Some(MouseButton::Left),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Create InputState with pointer at position and button pressed
fn input_pressed_at(x: f64, y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((x, y)),
            button_down: Some(MouseButton::Left),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Create InputState with pointer at position and right button clicked
fn input_right_clicked_at(x: f64, y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((x, y)),
            clicked: Some(MouseButton::Right),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Create InputState with scroll wheel delta
fn input_scroll_at(x: f64, y: f64, delta_y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((x, y)),
            ..Default::default()
        },
        scroll_delta: (0.0, delta_y),
        ..Default::default()
    }
}

/// Create InputState with drag
fn input_dragging_at(start_x: f64, start_y: f64, current_x: f64, current_y: f64) -> InputState {
    InputState {
        pointer: PointerState {
            pos: Some((current_x, current_y)),
            button_down: Some(MouseButton::Left),
            prev_pos: Some((start_x, start_y)),
            ..Default::default()
        },
        drag: Some(InputDragState::new(
            (start_x, start_y),
            (current_x, current_y),
            MouseButton::Left,
        )),
        ..Default::default()
    }
}

/// Determine WidgetState from InputState and rect
fn widget_state_from_input(input: &InputState, rect: &WidgetRect, disabled: bool) -> WidgetState {
    if disabled {
        return WidgetState::Disabled;
    }

    let hovered = input.is_hovered(rect);

    if hovered && input.is_mouse_down() {
        WidgetState::Pressed
    } else if hovered {
        WidgetState::Hovered
    } else {
        WidgetState::Normal
    }
}

// =============================================================================
// 1. Button Interaction Flow
// =============================================================================

#[test]
fn test_button_full_interaction_flow() {
    let rect = WidgetRect::new(100.0, 100.0, 100.0, 40.0);

    // 1. Pointer outside - Normal state
    let input = input_at(50.0, 50.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Normal);

    // 2. Move pointer over button - Hovered state
    let input = input_at(150.0, 120.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Hovered);

    // 3. Press mouse button - Pressed state
    let input = input_pressed_at(150.0, 120.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Pressed);

    // 4. Release (click)
    let input = input_clicked_at(150.0, 120.0);
    assert!(input.is_clicked());
    assert!(input.is_hovered(&rect));

    // 5. Disabled button
    let input = input_clicked_at(150.0, 120.0);
    let state = widget_state_from_input(&input, &rect, true);
    assert_eq!(state, WidgetState::Disabled);
}

#[test]
fn test_button_click_outside_not_registered() {
    let rect = WidgetRect::new(100.0, 100.0, 100.0, 40.0);

    let input_press = input_pressed_at(150.0, 120.0);
    assert!(input_press.is_hovered(&rect));

    let input_release = input_at(50.0, 50.0);
    assert!(!input_release.is_hovered(&rect));
}

// =============================================================================
// 2. Checkbox Toggle Flow
// =============================================================================

#[test]
fn test_checkbox_hover_states() {
    let rect = WidgetRect::new(100.0, 100.0, 150.0, 24.0);

    // Outside - Normal
    let input = input_at(50.0, 50.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Normal);

    // Inside - Hovered
    let input = input_at(150.0, 112.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Hovered);

    // Inside + Pressed
    let input = input_pressed_at(150.0, 112.0);
    let state = widget_state_from_input(&input, &rect, false);
    assert_eq!(state, WidgetState::Pressed);
}

// =============================================================================
// 3. Slider Drag Flow
// =============================================================================

#[test]
fn test_slider_drag_flow() {
    let rect = WidgetRect::new(100.0, 100.0, 200.0, 30.0);

    // Click on track at 50% position
    let track_center_x = rect.x + rect.width / 2.0;
    let input = input_clicked_at(track_center_x, rect.center_y());
    assert!(input.is_hovered(&rect));
    assert!(input.is_clicked());

    // Press on handle
    let input = input_pressed_at(track_center_x, rect.center_y());
    assert!(input.is_mouse_down());
    assert!(input.is_hovered(&rect));

    // Drag to 75% position
    let drag_x = rect.x + rect.width * 0.75;
    let input = input_dragging_at(track_center_x, rect.center_y(), drag_x, rect.center_y());
    assert!(input.is_dragging());

    // Release
    let input = input_at(drag_x, rect.center_y());
    assert!(!input.is_dragging());
}

#[test]
fn test_slider_disabled_state() {
    let rect = WidgetRect::new(100.0, 100.0, 200.0, 30.0);

    let input = input_dragging_at(150.0, 115.0, 200.0, 115.0);
    let state = widget_state_from_input(&input, &rect, true);
    assert_eq!(state, WidgetState::Disabled);
}

// =============================================================================
// 4. Scrollbar Interaction
// =============================================================================

#[test]
fn test_scrollbar_wheel_and_drag() {
    let mut scroll_state = ScrollState::new();
    let content_height = 500.0;
    let viewport_height = 200.0;
    let track_height = 200.0;

    let handled = scroll_state.handle_wheel(1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll_state.offset > 0.0);
    let offset_after_one_scroll = scroll_state.offset;

    let handled = scroll_state.handle_wheel(-1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll_state.offset < offset_after_one_scroll);

    // Drag handle
    scroll_state.offset = 0.0;
    scroll_state.start_drag(100.0);
    assert!(scroll_state.is_dragging);

    scroll_state.handle_drag(150.0, track_height, content_height, viewport_height);
    assert!(scroll_state.offset > 0.0);

    scroll_state.end_drag();
    assert!(!scroll_state.is_dragging);
}

#[test]
fn test_scrollbar_clamping() {
    let mut scroll_state = ScrollState::new();
    let content_height = 500.0;
    let viewport_height = 200.0;
    let max_scroll = content_height - viewport_height;

    scroll_state.offset = 1000.0;
    scroll_state.clamp(content_height, viewport_height);
    assert_eq!(scroll_state.offset, max_scroll);

    scroll_state.offset = -100.0;
    scroll_state.clamp(content_height, viewport_height);
    assert_eq!(scroll_state.offset, 0.0);
}

#[test]
fn test_scrollbar_no_scroll_when_content_fits() {
    let mut scroll_state = ScrollState::new();
    let content_height = 100.0;
    let viewport_height = 200.0;

    let handled = scroll_state.handle_wheel(1.0, content_height, viewport_height);
    assert!(!handled);
    assert_eq!(scroll_state.offset, 0.0);
}

// =============================================================================
// 5. WidgetInputState Flow
// =============================================================================

#[test]
fn test_widget_input_state_management() {
    let mut state = WidgetInputState::new();
    let widget1 = WidgetId::new("widget1");
    let widget2 = WidgetId::new("widget2");

    // Focus changes
    state.focus.set_focus(widget1.clone());
    assert!(state.focus.is_focused(&widget1));
    assert!(!state.focus.is_focused(&widget2));

    state.focus.set_focus(widget2.clone());
    assert!(!state.focus.is_focused(&widget1));
    assert!(state.focus.is_focused(&widget2));

    // Hover tracking
    state.hover.set_hovered(Some(widget1.clone()));
    assert!(state.hover.is_hovered(&widget1));
    assert!(!state.hover.is_hovered(&widget2));

    // Drag start/update/end
    state.start_drag(widget1.clone(), 100.0, 50.0);
    assert!(state.drag.is_dragging(&widget1));
    assert_eq!(state.drag.start_pos, (100.0, 50.0));

    state.drag.update(150.0, 60.0);
    assert_eq!(state.drag.current_pos, (150.0, 60.0));
    assert_eq!(state.drag.delta(), (50.0, 10.0));

    state.drag.end();
    assert!(!state.drag.is_dragging(&widget1));

    // Double-click detection
    let widget_id = WidgetId::new("button1");

    state.mouse_press(100.0, 50.0, Some(widget_id.clone()));
    let interaction = state.mouse_release(100.0, 50.0, 1000.0);
    assert_eq!(interaction, WidgetInteraction::Click);

    state.mouse_press(101.0, 51.0, Some(widget_id.clone()));
    let interaction = state.mouse_release(101.0, 51.0, 1200.0);
    assert_eq!(interaction, WidgetInteraction::DoubleClick);
}

#[test]
fn test_widget_input_state_pending_focus() {
    let mut state = WidgetInputState::new();
    let widget_id = WidgetId::new("input1");

    state.focus.request_focus(widget_id.clone());
    assert!(!state.focus.is_focused(&widget_id));

    state.end_frame();
    assert!(state.focus.is_focused(&widget_id));
}

// =============================================================================
// 6. Keyboard and Modifier Keys
// =============================================================================

#[test]
fn test_keyboard_modifiers() {
    let mut input = input_clicked_at(150.0, 120.0);
    input.modifiers.ctrl = true;
    assert!(input.ctrl());
    assert!(input.is_clicked());

    let mut input = input_clicked_at(150.0, 120.0);
    input.modifiers.shift = true;
    assert!(input.shift());
    assert!(input.is_clicked());

    let mut input = input_clicked_at(150.0, 120.0);
    input.modifiers.ctrl = true;
    input.modifiers.shift = true;
    assert!(input.ctrl());
    assert!(input.shift());
    assert!(input.modifiers.ctrl_shift());
    assert!(!input.modifiers.ctrl_alt());

    let modifiers = ModifierKeys {
        ctrl: true,
        alt: true,
        ..Default::default()
    };
    assert!(modifiers.ctrl_alt());
    assert!(!modifiers.ctrl_shift());
}

#[test]
fn test_modifier_any() {
    let none = ModifierKeys::none();
    assert!(!none.any());

    let shift = ModifierKeys::shift();
    assert!(shift.any());
    assert!(shift.shift);

    let ctrl = ModifierKeys::ctrl();
    assert!(ctrl.any());
    assert!(ctrl.ctrl);
}

// =============================================================================
// 7. Right-click / Context Menu
// =============================================================================

#[test]
fn test_right_click_context_menu() {
    let rect = WidgetRect::new(100.0, 100.0, 200.0, 150.0);

    let input = input_right_clicked_at(150.0, 120.0);
    assert!(input.is_hovered(&rect));
    assert!(input.is_right_clicked());

    let input = input_right_clicked_at(150.0, 120.0);
    assert!(input.is_right_clicked());
}

#[test]
fn test_middle_click() {
    let rect = WidgetRect::new(100.0, 100.0, 200.0, 150.0);

    let input = InputState {
        pointer: PointerState {
            pos: Some((150.0, 120.0)),
            clicked: Some(MouseButton::Middle),
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(input.is_hovered(&rect));
    assert!(input.is_middle_clicked());
}

// =============================================================================
// 8. Scroll Events
// =============================================================================

#[test]
fn test_scroll_in_scrollable_area() {
    let mut scroll_state = ScrollState::new();
    let content_height = 1000.0;
    let viewport_height = 300.0;
    let max_scroll = content_height - viewport_height;

    let handled = scroll_state.handle_wheel(1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll_state.offset > 0.0);
    let first_offset = scroll_state.offset;

    let handled = scroll_state.handle_wheel(1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll_state.offset > first_offset);

    scroll_state.offset = 10000.0;
    scroll_state.clamp(content_height, viewport_height);
    assert_eq!(scroll_state.offset, max_scroll);

    scroll_state.offset = -500.0;
    scroll_state.clamp(content_height, viewport_height);
    assert_eq!(scroll_state.offset, 0.0);
}

#[test]
fn test_scroll_track_click() {
    let mut scroll_state = ScrollState::new();
    let content_height = 1000.0;
    let viewport_height = 300.0;
    let track_y = 100.0;
    let track_height = 300.0;

    let click_y = track_y + track_height * 0.5;
    scroll_state.handle_track_click(
        click_y,
        track_y,
        track_height,
        content_height,
        viewport_height,
    );

    let max_scroll = content_height - viewport_height;
    let expected_offset = 0.5 * max_scroll;
    assert!((scroll_state.offset - expected_offset).abs() < 1.0);
}

// =============================================================================
// 9. Boundary Testing
// =============================================================================

#[test]
fn test_boundary_clicks() {
    let rect = WidgetRect::new(100.0, 100.0, 100.0, 50.0);

    let input = input_clicked_at(100.0, 125.0);
    assert!(input.is_hovered(&rect));

    let input = input_clicked_at(200.0, 125.0);
    assert!(input.is_hovered(&rect));

    let input = input_clicked_at(150.0, 100.0);
    assert!(input.is_hovered(&rect));

    let input = input_clicked_at(150.0, 150.0);
    assert!(input.is_hovered(&rect));

    let input = input_clicked_at(99.9, 125.0);
    assert!(!input.is_hovered(&rect));

    let input = input_clicked_at(200.1, 125.0);
    assert!(!input.is_hovered(&rect));
}

#[test]
fn test_zero_size_rect() {
    let rect = WidgetRect::new(100.0, 100.0, 0.0, 0.0);

    let input = input_clicked_at(100.0, 100.0);
    assert!(input.is_hovered(&rect));

    let input = input_clicked_at(100.1, 100.0);
    assert!(!input.is_hovered(&rect));
}

#[test]
fn test_consume_events() {
    let mut input = input_clicked_at(150.0, 120.0);

    assert!(input.is_clicked());
    assert!(input.consume_click());
    assert!(!input.is_clicked());

    let mut input = input_scroll_at(150.0, 120.0, 1.0);
    assert_eq!(input.scroll_delta, (0.0, 1.0));
    let delta = input.consume_scroll();
    assert_eq!(delta, (0.0, 1.0));
    assert_eq!(input.scroll_delta, (0.0, 0.0));

    let input = InputState {
        pointer: PointerState {
            pos: Some((150.0, 120.0)),
            double_clicked: Some(MouseButton::Left),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(input.is_double_clicked());
}

#[test]
fn test_drag_delta_calculation() {
    let input = input_dragging_at(100.0, 100.0, 150.0, 130.0);

    assert!(input.is_dragging());
    let drag = input.drag.as_ref().unwrap();
    assert_eq!(drag.start, (100.0, 100.0));
    assert_eq!(drag.current, (150.0, 130.0));
    assert_eq!(drag.total_delta, (50.0, 30.0));
}

// =============================================================================
// 10. Complex Interaction Sequences
// =============================================================================

#[test]
fn test_complex_drag_sequence() {
    let mut widget_state = WidgetInputState::new();
    let slider_id = WidgetId::new("slider1");

    widget_state.start_drag_with_value(slider_id.clone(), 100.0, 50.0, 0.0);
    assert!(widget_state.drag.is_dragging(&slider_id));
    assert_eq!(widget_state.drag.initial_value, 0.0);

    widget_state.update_mouse(120.0, 50.0);
    assert_eq!(widget_state.drag.current_pos, (120.0, 50.0));
    assert_eq!(widget_state.drag.delta(), (20.0, 0.0));

    widget_state.update_mouse(150.0, 50.0);
    assert_eq!(widget_state.drag.current_pos, (150.0, 50.0));
    assert_eq!(widget_state.drag.delta(), (50.0, 0.0));

    widget_state.drag.end();
    assert!(!widget_state.drag.is_dragging(&slider_id));
}

#[test]
fn test_modal_with_scrollable_content() {
    let mut scroll_state = ScrollState::new();
    let modal_rect = WidgetRect::new(200.0, 100.0, 600.0, 500.0);
    let content_rect = modal_rect.inset(16.0);

    let content_height = 1000.0;
    let viewport_height = content_rect.height;

    let input = input_scroll_at(400.0, 300.0, 1.0);
    assert!(input.is_hovered(&modal_rect));

    let handled = scroll_state.handle_wheel(1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll_state.offset > 0.0);
}

#[test]
fn test_slider_with_modifiers() {
    let _rect = WidgetRect::new(100.0, 100.0, 200.0, 30.0);

    let input = input_dragging_at(150.0, 115.0, 180.0, 115.0);
    assert!(input.is_dragging());
    assert!(!input.ctrl());

    let mut input = input_dragging_at(150.0, 115.0, 180.0, 115.0);
    input.modifiers.ctrl = true;
    assert!(input.is_dragging());
    assert!(input.ctrl());

    let mut input = input_dragging_at(150.0, 115.0, 180.0, 115.0);
    input.modifiers.shift = true;
    assert!(input.is_dragging());
    assert!(input.shift());
}
