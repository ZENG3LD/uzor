//! Tooltip input-coordinator registration helper.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::types::{Rect, WidgetId};

use super::state::TooltipState;

/// Register the tooltip overlay rect with the input coordinator.
///
/// Tooltips are atomic and use `Sense::HOVER` so the coordinator tracks
/// "pointer over tooltip" for fade-out logic. The tooltip itself never
/// fires click events.
///
/// Place on `LayerId::tooltip()` so it sits above all other layers.
pub fn register_tooltip(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 1 — register a tooltip with an explicit `InputCoordinator`.
///
/// `state` holds hover-timing and fade-in progress; it is read/written by the
/// caller between frames. Registration only declares the hit zone.
pub fn register_input_coordinator_tooltip(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut TooltipState,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 2 — register a tooltip via `ContextManager`, pulling `TooltipState`
/// from the registry.
pub fn register_context_manager_tooltip(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), TooltipState::default);
    register_input_coordinator_tooltip(&mut ctx.input, id, rect, layer, state);
}
