use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::types::SeparatorType;

pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: &SeparatorType,
    layer: &LayerId,
) {
    let sense = if kind.is_interactive() { Sense::DRAG } else { Sense::NONE };
    coord.register_atomic(id, WidgetKind::Separator, rect, sense, layer);
}
