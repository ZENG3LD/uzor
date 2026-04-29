//! InputCoordinator registration helper for button widgets.

use crate::app_context::ContextManager;
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

/// Level 2 — register a button via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_button(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ButtonState::default);
    register_input_coordinator_button(&mut ctx.input, id, rect, layer, state);
}
