//! InputCoordinator registration helper for dropdown trigger widgets.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

/// Register a dropdown trigger widget with the coordinator for this frame.
pub fn register_dropdown_trigger(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::DropdownTrigger, rect, Sense::CLICK, layer);
}
