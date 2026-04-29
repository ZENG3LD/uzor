//! InputCoordinator registration helper for drag handle widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::DragHandleState;

/// Register a drag handle widget with the coordinator for this frame.
///
/// Uses `Sense::DRAG` so the coordinator tracks drag gestures but not clicks.
pub fn register_drag_handle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::DragHandle, rect, Sense::DRAG, layer);
}

/// Level 1 — register a drag handle with an explicit `InputCoordinator`.
pub fn register_input_coordinator_drag_handle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut DragHandleState,
) {
    coord.register_atomic(id, WidgetKind::DragHandle, rect, Sense::DRAG, layer);
}

/// Level 2 — register a drag handle via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_drag_handle(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), DragHandleState::default);
    register_input_coordinator_drag_handle(&mut ctx.input, id, rect, layer, state);
}
