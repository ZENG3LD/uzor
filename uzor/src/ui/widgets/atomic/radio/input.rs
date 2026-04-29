//! InputCoordinator registration helper for the radio widget.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::RadioState;

/// Register a radio widget with the coordinator for this frame.
pub fn register_radio(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Radio, rect, Sense::CLICK, layer);
}

/// Level 1 — register a radio widget with an explicit `InputCoordinator`.
pub fn register_input_coordinator_radio(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut RadioState,
) {
    coord.register_atomic(id, WidgetKind::Radio, rect, Sense::CLICK, layer);
}

/// Level 2 — register a radio widget via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_radio(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), RadioState::default);
    register_input_coordinator_radio(&mut ctx.input, id, rect, layer, state);
}
