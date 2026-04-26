use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    // Toast is hover-only (so user can pause auto-dismiss by hovering).
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}
