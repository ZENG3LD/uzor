//! InputCoordinator registration helper for color swatch widgets.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

/// Register a color swatch widget with the coordinator for this frame.
pub fn register_color_swatch(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ColorSwatch, rect, Sense::CLICK, layer);
}
