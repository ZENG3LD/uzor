//! InputCoordinator registration helpers for clock widgets.
//!
//! Clock uses HOVER sense — mlc has hover-only behavior (no click action).


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ClockState;

/// Register a clock widget with the coordinator for this frame.
/// Uses `Sense::HOVER` — the clock has hover-only behavior.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Clock, rect, Sense::HOVER, layer);
}

/// Level 1 — register a clock with an explicit `InputCoordinator`.
pub fn register_input_coordinator_clock(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ClockState,
) {
    coord.register_atomic(id, WidgetKind::Clock, rect, Sense::HOVER, layer);
}
