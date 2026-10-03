//! InputCoordinator registration helpers for the checkbox widget.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::CheckboxState;

/// Register a checkbox widget with the coordinator for this frame.
pub fn register_checkbox(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Checkbox, rect, Sense::CLICK, layer);
}

/// Level 1 — register a checkbox with an explicit `InputCoordinator`.
pub fn register_input_coordinator_checkbox(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut CheckboxState,
) {
    coord.register_atomic(id, WidgetKind::Checkbox, rect, Sense::CLICK, layer);
}
