//! InputCoordinator registration helpers for close button widgets.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::CloseButtonState;

/// Register a close button widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::CloseButton, rect, Sense::CLICK, layer);
}

/// Level 1 — register a close button with an explicit `InputCoordinator`.
pub fn register_input_coordinator_close_button(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut CloseButtonState,
) {
    coord.register_atomic(id, WidgetKind::CloseButton, rect, Sense::CLICK, layer);
}
