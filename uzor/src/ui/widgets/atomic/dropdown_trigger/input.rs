//! InputCoordinator registration helper for dropdown trigger widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::DropdownTriggerState;

/// Register a dropdown trigger widget with the coordinator for this frame.
pub fn register_dropdown_trigger(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::DropdownTrigger, rect, Sense::CLICK, layer);
}

/// Level 1 — register a dropdown trigger with an explicit `InputCoordinator`.
pub fn register_input_coordinator_dropdown_trigger(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut DropdownTriggerState,
) {
    coord.register_atomic(id, WidgetKind::DropdownTrigger, rect, Sense::CLICK, layer);
}

/// Level 2 — register a dropdown trigger via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_dropdown_trigger(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), DropdownTriggerState::default);
    register_input_coordinator_dropdown_trigger(&mut ctx.input, id, rect, layer, state);
}
