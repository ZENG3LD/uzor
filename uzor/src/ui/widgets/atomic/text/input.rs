//! InputCoordinator registration helpers for the Text widget.
//!
//! Text is read-only. It registers with `Sense::HOVER` so callers can detect
//! cursor-over for tooltip or color-change purposes.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};


/// Level 1 — register a Text widget hit zone with an explicit `InputCoordinator`.
pub fn register_input_coordinator_text(
    coord: &mut InputCoordinator,
    id:    impl Into<WidgetId>,
    rect:  Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::HOVER, layer);
}
