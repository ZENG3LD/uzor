//! Widget input handlers
//!
//! Platform-agnostic input handling functions for various widgets.

use super::widget_state::{WidgetId, WidgetInputState, WidgetInteraction};
use crate::types::WidgetRect;

/// Widget hit test result
#[derive(Clone, Debug, PartialEq)]
#[derive(Default)]
pub enum WidgetHitResult {
    /// No hit
    #[default]
    None,
    /// Hit a widget body
    Widget { id: WidgetId },
    /// Hit a close button
    CloseButton { parent_id: WidgetId },
    /// Hit a dropdown item
    DropdownItem { dropdown_id: WidgetId, item_index: usize },
    /// Hit a toolbar item
    ToolbarItem { toolbar_id: WidgetId, item_id: String },
    /// Hit a slider track
    SliderTrack { id: WidgetId },
    /// Hit a slider handle
    SliderHandle { id: WidgetId },
    /// Hit a scrollbar track
    ScrollbarTrack { id: WidgetId },
    /// Hit a scrollbar handle
    ScrollbarHandle { id: WidgetId },
    /// Hit a tab
    Tab { parent_id: WidgetId, tab_index: usize },
}


/// Generic widget hit test
pub fn widget_hit_test(rect: &WidgetRect, x: f64, y: f64) -> bool {
    rect.contains(x, y)
}

// =============================================================================
// Button Input
// =============================================================================

/// Button input result
#[derive(Clone, Debug, Default)]
pub struct ButtonInputResult {
    /// Whether button was clicked
    pub clicked: bool,
    /// Whether button is hovered
    pub hovered: bool,
    /// Whether button is pressed (mouse down)
    pub pressed: bool,
    /// Current interaction state
    pub interaction: WidgetInteraction,
}

/// Handle button input
pub fn handle_button_input(
    state: &WidgetInputState,
    id: &WidgetId,
    rect: &WidgetRect,
    disabled: bool,
) -> ButtonInputResult {
    if disabled {
        return ButtonInputResult::default();
    }

    let (mx, my) = state.hover.mouse_pos;
    let hovered = rect.contains(mx, my);
    let pressed = hovered && state.hover.mouse_pressed && state.active.as_ref() == Some(id);
    let clicked = hovered && state.active.as_ref() == Some(id) && !state.hover.mouse_pressed;

    let interaction = if pressed {
        WidgetInteraction::Press
    } else if hovered {
        WidgetInteraction::Hover
    } else {
        WidgetInteraction::None
    };

    ButtonInputResult {
        clicked,
        hovered,
        pressed,
        interaction,
    }
}

// =============================================================================
// Checkbox Input
// =============================================================================

/// Checkbox input result
#[derive(Clone, Debug, Default)]
pub struct CheckboxInputResult {
    /// Whether checkbox was toggled
    pub toggled: bool,
    /// New checked state (if toggled)
    pub new_checked: bool,
    /// Whether checkbox is hovered
    pub hovered: bool,
    /// Current interaction state
    pub interaction: WidgetInteraction,
}

/// Handle checkbox input
pub fn handle_checkbox_input(
    state: &WidgetInputState,
    id: &WidgetId,
    rect: &WidgetRect,
    current_checked: bool,
    disabled: bool,
) -> CheckboxInputResult {
    if disabled {
        return CheckboxInputResult::default();
    }

    let (mx, my) = state.hover.mouse_pos;
    let hovered = rect.contains(mx, my);
    let clicked = hovered && state.active.as_ref() == Some(id) && !state.hover.mouse_pressed;

    let toggled = clicked;
    let new_checked = if toggled { !current_checked } else { current_checked };

    let interaction = if hovered && state.hover.mouse_pressed {
        WidgetInteraction::Press
    } else if hovered {
        WidgetInteraction::Hover
    } else {
        WidgetInteraction::None
    };

    CheckboxInputResult {
        toggled,
        new_checked,
        hovered,
        interaction,
    }
}

// =============================================================================
// Slider Input
// =============================================================================

/// Slider input result
#[derive(Clone, Debug, Default)]
pub struct SliderInputResult {
    /// Whether value changed
    pub changed: bool,
    /// New value (normalized 0.0 - 1.0)
    pub value: f64,
    /// Whether slider is hovered
    pub hovered: bool,
    /// Whether slider handle is being dragged
    pub dragging: bool,
    /// Current interaction state
    pub interaction: WidgetInteraction,
}

/// Handle slider input
pub fn handle_slider_input(
    state: &WidgetInputState,
    id: &WidgetId,
    track_rect: &WidgetRect,
    handle_rect: &WidgetRect,
    current_value: f64,
    horizontal: bool,
    disabled: bool,
) -> SliderInputResult {
    if disabled {
        return SliderInputResult {
            value: current_value,
            ..Default::default()
        };
    }

    let (mx, my) = state.hover.mouse_pos;
    let hovered = track_rect.contains(mx, my);
    let handle_hovered = handle_rect.contains(mx, my);
    let dragging = state.drag.is_dragging(id);

    let mut value = current_value;
    let mut changed = false;

    if dragging {
        let (dx, dy) = state.drag.delta();
        let range = if horizontal {
            track_rect.width - handle_rect.width
        } else {
            track_rect.height - handle_rect.height
        };

        if range > 0.0 {
            let delta = if horizontal { dx } else { dy };
            let delta_normalized = delta / range;
            value = (state.drag.initial_value + delta_normalized).clamp(0.0, 1.0);
            changed = (value - current_value).abs() > 0.0001;
        }
    }

    if hovered && !handle_hovered && state.hover.mouse_pressed && state.active.as_ref() == Some(id) && !dragging {
        let range = if horizontal {
            track_rect.width - handle_rect.width
        } else {
            track_rect.height - handle_rect.height
        };

        if range > 0.0 {
            let pos = if horizontal {
                mx - track_rect.x - handle_rect.width / 2.0
            } else {
                my - track_rect.y - handle_rect.height / 2.0
            };
            value = (pos / range).clamp(0.0, 1.0);
            changed = true;
        }
    }

    let interaction = if dragging {
        WidgetInteraction::Drag
    } else if handle_hovered && state.hover.mouse_pressed {
        WidgetInteraction::Press
    } else if hovered || handle_hovered {
        WidgetInteraction::Hover
    } else {
        WidgetInteraction::None
    };

    SliderInputResult {
        changed,
        value,
        hovered: hovered || handle_hovered,
        dragging,
        interaction,
    }
}

// =============================================================================
// Scrollbar Input
// =============================================================================

/// Scrollbar input result
#[derive(Clone, Debug, Default)]
pub struct ScrollbarInputResult {
    /// Whether scroll position changed
    pub changed: bool,
    /// New scroll position (normalized 0.0 - 1.0)
    pub position: f64,
    /// Whether scrollbar is hovered
    pub hovered: bool,
    /// Whether handle is being dragged
    pub dragging: bool,
    /// Track click (page up/down)
    pub page_direction: Option<i32>,
    /// Current interaction state
    pub interaction: WidgetInteraction,
}

/// Handle scrollbar input
#[allow(clippy::too_many_arguments)]
pub fn handle_scrollbar_input(
    state: &WidgetInputState,
    id: &WidgetId,
    track_rect: &WidgetRect,
    handle_rect: &WidgetRect,
    current_position: f64,
    handle_size_ratio: f64,
    horizontal: bool,
    disabled: bool,
) -> ScrollbarInputResult {
    if disabled {
        return ScrollbarInputResult {
            position: current_position,
            ..Default::default()
        };
    }

    let (mx, my) = state.hover.mouse_pos;
    let hovered = track_rect.contains(mx, my);
    let handle_hovered = handle_rect.contains(mx, my);
    let dragging = state.drag.is_dragging(id);

    let mut position = current_position;
    let mut changed = false;
    let mut page_direction = None;

    if dragging {
        let (dx, dy) = state.drag.delta();
        let track_size = if horizontal { track_rect.width } else { track_rect.height };
        let usable_range = track_size * (1.0 - handle_size_ratio);

        if usable_range > 0.0 {
            let delta = if horizontal { dx } else { dy };
            let delta_normalized = delta / usable_range;
            position = (state.drag.initial_value + delta_normalized).clamp(0.0, 1.0);
            changed = (position - current_position).abs() > 0.0001;
        }
    }

    if hovered && !handle_hovered && state.hover.mouse_pressed && state.active.as_ref() == Some(id) && !dragging {
        let handle_pos = if horizontal { handle_rect.x } else { handle_rect.y };
        let mouse_pos = if horizontal { mx } else { my };

        if mouse_pos < handle_pos {
            page_direction = Some(-1);
        } else {
            page_direction = Some(1);
        }
    }

    let interaction = if dragging {
        WidgetInteraction::Drag
    } else if handle_hovered && state.hover.mouse_pressed {
        WidgetInteraction::Press
    } else if hovered || handle_hovered {
        WidgetInteraction::Hover
    } else {
        WidgetInteraction::None
    };

    ScrollbarInputResult {
        changed,
        position,
        hovered: hovered || handle_hovered,
        dragging,
        page_direction,
        interaction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> WidgetInputState {
        WidgetInputState::new()
    }

    #[test]
    fn test_button_input_hover() {
        let mut state = make_state();
        let id = WidgetId::new("btn1");
        let rect = WidgetRect::new(10.0, 10.0, 100.0, 40.0);

        state.update_mouse(50.0, 30.0);
        let result = handle_button_input(&state, &id, &rect, false);
        assert!(result.hovered);
        assert!(!result.clicked);
    }

    #[test]
    fn test_button_input_disabled() {
        let mut state = make_state();
        let id = WidgetId::new("btn1");
        let rect = WidgetRect::new(10.0, 10.0, 100.0, 40.0);

        state.update_mouse(50.0, 30.0);
        let result = handle_button_input(&state, &id, &rect, true);
        assert!(!result.hovered);
        assert!(!result.clicked);
    }

    #[test]
    fn test_checkbox_toggle() {
        let mut state = make_state();
        let id = WidgetId::new("chk1");
        let rect = WidgetRect::new(10.0, 10.0, 20.0, 20.0);

        state.update_mouse(15.0, 15.0);
        state.mouse_press(15.0, 15.0, Some(id.clone()));
        state.mouse_release(15.0, 15.0, 1000.0);

        let result = handle_checkbox_input(&state, &id, &rect, false, false);
        assert!(result.hovered);
    }

    #[test]
    fn test_slider_drag() {
        let mut state = make_state();
        let id = WidgetId::new("slider1");
        let track_rect = WidgetRect::new(10.0, 10.0, 200.0, 20.0);
        let handle_rect = WidgetRect::new(10.0, 10.0, 20.0, 20.0);

        state.start_drag_with_value(id.clone(), 20.0, 20.0, 0.0);
        state.update_mouse(110.0, 20.0);

        let result = handle_slider_input(&state, &id, &track_rect, &handle_rect, 0.0, true, false);
        assert!(result.dragging);
        assert!(result.changed);
        assert!(result.value > 0.4 && result.value < 0.6);
    }

    #[test]
    fn test_widget_hit_test() {
        let rect = WidgetRect::new(10.0, 10.0, 100.0, 50.0);
        assert!(widget_hit_test(&rect, 50.0, 30.0));
        assert!(!widget_hit_test(&rect, 5.0, 30.0));
        assert!(!widget_hit_test(&rect, 50.0, 65.0));
    }
}
