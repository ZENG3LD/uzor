use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    // Plain container is non-interactive; coordinator uses Custom kind.
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::NONE, layer);
}
