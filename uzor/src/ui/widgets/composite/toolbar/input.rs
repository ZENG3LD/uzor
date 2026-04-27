//! Toolbar input-coordinator registration.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

pub fn register_toolbar(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) -> WidgetId {
    coord.register_composite(id, WidgetKind::Toolbar, rect, Sense::CLICK, layer)
}
