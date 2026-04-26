use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

/// Register scrollbar — composite of track + thumb. Caller registers
/// each leaf separately for hit-testing.
pub fn register_track(coord: &mut InputCoordinator, id: impl Into<WidgetId>, rect: Rect, layer: &LayerId) {
    coord.register_atomic(id, WidgetKind::ScrollbarTrack, rect, Sense::CLICK, layer);
}

pub fn register_thumb(coord: &mut InputCoordinator, id: impl Into<WidgetId>, rect: Rect, layer: &LayerId) {
    coord.register_atomic(id, WidgetKind::ScrollbarHandle, rect, Sense::DRAG, layer);
}
