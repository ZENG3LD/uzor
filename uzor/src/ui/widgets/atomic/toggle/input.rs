//! InputCoordinator registration helpers for the toggle widget.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ToggleState;

/// Register a toggle widget with the coordinator for this frame.
pub fn register_toggle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Toggle, rect, Sense::CLICK, layer);
}

/// Level 1 — register a toggle with an explicit `InputCoordinator`.
pub fn register_input_coordinator_toggle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ToggleState,
) {
    coord.register_atomic(id, WidgetKind::Toggle, rect, Sense::CLICK, layer);
}
