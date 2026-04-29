//! InputCoordinator registration helper for shape selector widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ShapeSelectorState;

/// Register a shape selector widget with the coordinator for this frame.
pub fn register_shape_selector(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ShapeSelector, rect, Sense::CLICK, layer);
}

/// Level 1 — register a shape selector with an explicit `InputCoordinator`.
pub fn register_input_coordinator_shape_selector(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ShapeSelectorState,
) {
    coord.register_atomic(id, WidgetKind::ShapeSelector, rect, Sense::CLICK, layer);
}

/// Level 2 — register a shape selector via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_shape_selector(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ShapeSelectorState::default);
    register_input_coordinator_shape_selector(&mut ctx.input, id, rect, layer, state);
}
