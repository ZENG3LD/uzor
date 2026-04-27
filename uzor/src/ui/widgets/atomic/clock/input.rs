//! InputCoordinator registration helper for clock widgets.
//!
//! Clock uses HOVER sense — mlc has hover-only behavior (no click action).

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

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
