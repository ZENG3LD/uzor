//! Chevron input registration helpers — three composition layers, mirroring
//! the rest of the atomic widgets.


use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::types::{ChevronView, HitAreaPolicy};

/// L1 — register the chevron's hit rect with the coordinator only. No drawing.
///
/// Hit area is derived from `view.hit_area`:
/// - `Visual` ⇒ register `rect`.
/// - `Inflated { padding }` ⇒ register the inflated rect.
/// - `None` ⇒ skip registration entirely (the host owns the click).
pub fn register_input_coordinator_chevron(
    coord: &mut InputCoordinator,
    id:    impl Into<WidgetId>,
    rect:  Rect,
    view:  &ChevronView,
    layer: &LayerId,
) {
    let hit = match view.hit_area {
        HitAreaPolicy::None => return,
        HitAreaPolicy::Visual => rect,
        HitAreaPolicy::Inflated { padding } => Rect::new(
            rect.x - padding,
            rect.y - padding,
            rect.width  + padding * 2.0,
            rect.height + padding * 2.0,
        ),
    };
    coord.register_atomic(id, WidgetKind::ScrollChevron, hit, Sense::CLICK | Sense::HOVER, layer);
}
