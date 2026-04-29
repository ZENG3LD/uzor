//! InputCoordinator registration helper for color swatch widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ColorSwatchState;

/// Register a color swatch widget with the coordinator for this frame.
pub fn register_color_swatch(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ColorSwatch, rect, Sense::CLICK, layer);
}

/// Level 1 — register a color swatch with an explicit `InputCoordinator`.
pub fn register_input_coordinator_color_swatch(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ColorSwatchState,
) {
    coord.register_atomic(id, WidgetKind::ColorSwatch, rect, Sense::CLICK, layer);
}

/// Level 2 — register a color swatch via `ContextManager`, pulling state from the registry.
pub fn register_context_manager_color_swatch(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ColorSwatchState::default);
    register_input_coordinator_color_swatch(&mut ctx.input, id, rect, layer, state);
}
