//! InputCoordinator registration helper for item widgets.
//!
//! Item uses Sense::NONE — it is non-interactive. Registration still occurs
//! so hit-rect queries work for layout/tooltip purposes.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ItemState;

/// Register an item widget with the coordinator for this frame.
/// Uses `Sense::NONE` — items are non-interactive.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Item, rect, Sense::NONE, layer);
}

/// Level 1 — register an item with an explicit `InputCoordinator`.
pub fn register_input_coordinator_item(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ItemState,
) {
    coord.register_atomic(id, WidgetKind::Item, rect, Sense::NONE, layer);
}

/// Level 2 — register an item via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_item(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ItemState::default);
    register_input_coordinator_item(&mut ctx.input, id, rect, layer, state);
}
