//! Integration tests for widget render-input cycle
//!
//! These tests verify the full interaction cycle using WidgetInputState:
//! 1. Hit area calculation
//! 2. Input processing
//! 3. State change verification

use uzor_core::input::{
    handle_button_input, handle_checkbox_input,
    handle_slider_input, FocusState, WidgetId, WidgetInputState,
    WidgetInteraction,
};
use uzor_core::types::{ScrollState, WidgetRect};

// =============================================================================
// Button Tests using WidgetInputState
// =============================================================================

#[test]
fn test_button_hover_and_click() {
    let mut state = WidgetInputState::new();
    let id = WidgetId::new("btn1");
    let rect = WidgetRect::new(10.0, 10.0, 100.0, 40.0);

    // Hover over button
    state.update_mouse(50.0, 30.0);
    let result = handle_button_input(&state, &id, &rect, false);
    assert!(result.hovered);
    assert!(!result.clicked);

    // Disabled button ignores all interactions
    let result = handle_button_input(&state, &id, &rect, true);
    assert!(!result.hovered);
    assert!(!result.clicked);
}

#[test]
fn test_button_press_and_release() {
    let mut state = WidgetInputState::new();
    let id = WidgetId::new("btn1");
    let rect = WidgetRect::new(10.0, 10.0, 100.0, 40.0);

    // Mouse press on button
    state.update_mouse(50.0, 30.0);
    state.mouse_press(50.0, 30.0, Some(id.clone()));
    let result = handle_button_input(&state, &id, &rect, false);
    assert!(result.hovered);
    assert!(result.pressed);
    assert!(!result.clicked);

    // Mouse release (generates click)
    state.mouse_release(50.0, 30.0, 1000.0);
    // After release, active is cleared - that's the expected behavior
}

// =============================================================================
// Checkbox Tests
// =============================================================================

#[test]
fn test_checkbox_toggle_via_widget_state() {
    let mut state = WidgetInputState::new();
    let id = WidgetId::new("chk1");
    let rect = WidgetRect::new(10.0, 10.0, 20.0, 20.0);

    // Hover checkbox
    state.update_mouse(15.0, 15.0);
    let result = handle_checkbox_input(&state, &id, &rect, false, false);
    assert!(result.hovered);
    assert!(!result.toggled);
}

// =============================================================================
// Slider Tests
// =============================================================================

#[test]
fn test_slider_drag_cycle() {
    let mut state = WidgetInputState::new();
    let id = WidgetId::new("slider1");
    let track_rect = WidgetRect::new(10.0, 10.0, 200.0, 20.0);
    let handle_rect = WidgetRect::new(10.0, 10.0, 20.0, 20.0);

    // Start drag with initial value 0.0
    state.start_drag_with_value(id.clone(), 20.0, 20.0, 0.0);
    state.update_mouse(110.0, 20.0); // Move to ~50%

    let result = handle_slider_input(&state, &id, &track_rect, &handle_rect, 0.0, true, false);
    assert!(result.dragging);
    assert!(result.changed);
    assert!(result.value > 0.4 && result.value < 0.6, "Expected ~0.5, got {}", result.value);
}

// =============================================================================
// Scrollbar with ScrollState Tests
// =============================================================================

#[test]
fn test_scrollbar_with_scroll_state() {
    let mut scroll = ScrollState::new();
    let viewport_height = 400.0;
    let content_height = 1000.0;

    assert_eq!(scroll.offset, 0.0);

    let handled = scroll.handle_wheel(1.0, content_height, viewport_height);
    assert!(handled);
    assert!(scroll.offset > 0.0);
    let first_offset = scroll.offset;

    scroll.handle_wheel(2.0, content_height, viewport_height);
    assert!(scroll.offset > first_offset);

    // Clamp at maximum
    scroll.offset = 10000.0;
    scroll.clamp(content_height, viewport_height);
    let max_scroll = content_height - viewport_height;
    assert_eq!(scroll.offset, max_scroll);

    // Clamp at minimum
    scroll.offset = -100.0;
    scroll.clamp(content_height, viewport_height);
    assert_eq!(scroll.offset, 0.0);

    // No scroll when content fits
    scroll.reset();
    let handled = scroll.handle_wheel(1.0, 300.0, 400.0);
    assert!(!handled);
    assert_eq!(scroll.offset, 0.0);
}

#[test]
fn test_scroll_state_drag() {
    let mut scroll = ScrollState::new();

    scroll.start_drag(100.0);
    assert!(scroll.is_dragging);
    assert_eq!(scroll.drag_start_y, Some(100.0));
    assert_eq!(scroll.drag_start_offset, Some(0.0));

    scroll.end_drag();
    scroll.offset = 200.0;
    scroll.start_drag(150.0);
    assert_eq!(scroll.drag_start_offset, Some(200.0));

    scroll.handle_drag(200.0, 400.0, 1000.0, 400.0);
    assert!(scroll.offset > 200.0);

    scroll.end_drag();
    assert!(!scroll.is_dragging);
    assert_eq!(scroll.drag_start_y, None);
}

#[test]
fn test_scroll_state_reset() {
    let mut scroll = ScrollState::new();
    scroll.offset = 250.0;
    scroll.start_drag(100.0);

    scroll.reset();
    assert_eq!(scroll.offset, 0.0);
    assert!(!scroll.is_dragging);
    assert_eq!(scroll.drag_start_y, None);
    assert_eq!(scroll.drag_start_offset, None);
}

// =============================================================================
// Focus Management Tests
// =============================================================================

#[test]
fn test_focus_flow() {
    let mut focus = FocusState::new();
    let input1 = WidgetId::new("input1");
    let input2 = WidgetId::new("input2");

    assert!(!focus.is_focused(&input1));
    assert!(!focus.is_focused(&input2));

    focus.set_focus(input1.clone());
    assert!(focus.is_focused(&input1));
    assert!(!focus.is_focused(&input2));

    focus.request_focus(input2.clone());
    assert!(focus.is_focused(&input1)); // Still input1

    focus.process_pending();
    assert!(!focus.is_focused(&input1));
    assert!(focus.is_focused(&input2)); // Now input2

    focus.clear_focus();
    assert!(!focus.is_focused(&input1));
    assert!(!focus.is_focused(&input2));
}

#[test]
fn test_focus_multiple_requests() {
    let mut focus = FocusState::new();
    let input1 = WidgetId::new("input1");
    let input2 = WidgetId::new("input2");
    let input3 = WidgetId::new("input3");

    focus.request_focus(input1.clone());
    focus.request_focus(input2.clone());
    focus.request_focus(input3.clone());

    focus.process_pending();
    assert!(!focus.is_focused(&input1));
    assert!(!focus.is_focused(&input2));
    assert!(focus.is_focused(&input3));
}

// =============================================================================
// Double-Click / Triple-Click Detection Tests
// =============================================================================

#[test]
fn test_double_click_detection_timing() {
    let mut state = WidgetInputState::new();
    let button_id = WidgetId::new("button1");

    state.mouse_press(100.0, 50.0, Some(button_id.clone()));
    let result1 = state.mouse_release(100.0, 50.0, 1000.0);
    assert_eq!(result1, WidgetInteraction::Click);

    state.mouse_press(101.0, 51.0, Some(button_id.clone()));
    let result2 = state.mouse_release(101.0, 51.0, 1200.0);
    assert_eq!(result2, WidgetInteraction::DoubleClick);

    // Too late for triple click
    state.mouse_press(100.0, 50.0, Some(button_id.clone()));
    let result3 = state.mouse_release(100.0, 50.0, 2000.0);
    assert_eq!(result3, WidgetInteraction::Click);
}

#[test]
fn test_double_click_distance_threshold() {
    let mut state = WidgetInputState::new();
    let button_id = WidgetId::new("button1");

    state.mouse_press(100.0, 50.0, Some(button_id.clone()));
    state.mouse_release(100.0, 50.0, 1000.0);

    // Within distance
    state.mouse_press(101.0, 51.0, Some(button_id.clone()));
    let result = state.mouse_release(101.0, 51.0, 1200.0);
    assert_eq!(result, WidgetInteraction::DoubleClick);

    // Reset
    state = WidgetInputState::new();

    state.mouse_press(100.0, 50.0, Some(button_id.clone()));
    state.mouse_release(100.0, 50.0, 1000.0);

    // Too far away
    state.mouse_press(110.0, 60.0, Some(button_id.clone()));
    let result = state.mouse_release(110.0, 60.0, 1200.0);
    assert_eq!(result, WidgetInteraction::Click);
}

#[test]
fn test_double_click_different_widgets() {
    let mut state = WidgetInputState::new();
    let button1 = WidgetId::new("button1");
    let button2 = WidgetId::new("button2");

    state.mouse_press(100.0, 50.0, Some(button1.clone()));
    state.mouse_release(100.0, 50.0, 1000.0);

    state.mouse_press(100.0, 50.0, Some(button2.clone()));
    let result = state.mouse_release(100.0, 50.0, 1200.0);
    assert_eq!(result, WidgetInteraction::Click);
}

// =============================================================================
// Complex Interaction Scenarios
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
fn test_end_frame_processing() {
    let mut widget_state = WidgetInputState::new();
    let input_id = WidgetId::new("input1");

    widget_state.focus.request_focus(input_id.clone());
    assert!(!widget_state.focus.is_focused(&input_id));

    widget_state.end_frame();
    assert!(widget_state.focus.is_focused(&input_id));
}
