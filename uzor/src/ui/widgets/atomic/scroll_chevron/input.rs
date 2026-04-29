//! InputCoordinator registration helper for scroll chevron widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ScrollChevronState;

/// Register a scroll chevron widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ScrollChevron, rect, Sense::CLICK, layer);
}

/// Level 1 — register a scroll chevron with an explicit `InputCoordinator`.
pub fn register_input_coordinator_scroll_chevron(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ScrollChevronState,
) {
    coord.register_atomic(id, WidgetKind::ScrollChevron, rect, Sense::CLICK, layer);
}

/// Level 2 — register a scroll chevron via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_scroll_chevron(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ScrollChevronState::default);
    register_input_coordinator_scroll_chevron(&mut ctx.input, id, rect, layer, state);
}
