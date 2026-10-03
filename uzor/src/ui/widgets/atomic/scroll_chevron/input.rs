//! InputCoordinator registration helpers for scroll chevron widgets.


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
