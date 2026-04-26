//! Tooltip input-coordinator registration helper.

use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::types::{Rect, WidgetId};

/// Register the tooltip overlay rect with the input coordinator.
///
/// Tooltips are atomic and use `Sense::HOVER` so the coordinator tracks
/// "pointer over tooltip" for fade-out logic. The tooltip itself never
/// fires click events.
///
/// Place on `LayerId::tooltip()` so it sits above all other layers.
pub fn register_tooltip(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}
