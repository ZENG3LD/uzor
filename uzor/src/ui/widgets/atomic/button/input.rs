//! InputCoordinator registration helpers for button widgets.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ButtonState;

/// Register a button widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Button, rect, Sense::CLICK, layer);
}

/// Level 1 — register a button with an explicit `InputCoordinator`.
pub fn register_input_coordinator_button(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ButtonState,
) {
    coord.register_atomic(id, WidgetKind::Button, rect, Sense::CLICK, layer);
}
