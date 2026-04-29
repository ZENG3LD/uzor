//! InputCoordinator registration helper for the toggle widget.

use crate::app_context::ContextManager;
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

/// Level 2 — register a toggle via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_toggle(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ToggleState::default);
    register_input_coordinator_toggle(&mut ctx.input, id, rect, layer, state);
}
