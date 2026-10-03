//! InputCoordinator registration helpers for toast widgets.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ToastStackState;

/// Register a toast widget with the coordinator for this frame.
/// Toast is hover-only (so user can pause auto-dismiss by hovering).
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 1 — register a toast widget with an explicit `InputCoordinator`.
///
/// Each visible toast entry should be registered with its own rect and id.
/// `state` holds the entire stack; individual toast display is managed by the
/// render helpers (`draw_toast_stack`, `draw_toast_at`).
pub fn register_input_coordinator_toast(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ToastStackState,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}
