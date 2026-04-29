//! InputCoordinator registration helper for the checkbox widget.

use crate::app_context::ContextManager;
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

/// Level 2 — register a checkbox via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_checkbox(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), CheckboxState::default);
    register_input_coordinator_checkbox(&mut ctx.input, id, rect, layer, state);
}
