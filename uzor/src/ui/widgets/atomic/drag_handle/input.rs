//! InputCoordinator registration helper for drag handle widgets.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

/// Register a drag handle widget with the coordinator for this frame.
///
/// Uses `Sense::DRAG` so the coordinator tracks drag gestures but not clicks.
pub fn register_drag_handle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::DragHandle, rect, Sense::DRAG, layer);
}
