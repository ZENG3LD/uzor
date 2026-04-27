//! Container input registration.
//!
//! **mlc finding:** plain containers have no input handling. The only mlc
//! container with any interaction surface is `TradingContainer`, whose
//! `handle_click` is a stub returning `false`. Popup / Modal containers
//! register hit-test helpers as stand-alone functions, not inside the
//! container primitive.
//!
//! `register` defaults to `Sense::NONE`. A `Sense::CLICK` overload is provided
//! for the rare case where a container acts as a clickable backdrop (e.g. a
//! dismissal surface behind a popup).

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

/// Register a non-interactive container in the input coordinator.
///
/// Use this for all plain/bordered/section/panel containers that do not handle
/// clicks. The coordinator still tracks them for z-ordering purposes.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::NONE, layer);
}

/// Register a clickable container in the input coordinator.
///
/// Use when the container surface itself should absorb pointer events
/// (e.g. a backdrop that dismisses a floating panel on click).
/// In mlc this pattern appears via `InputCoordinator::register_on_layer`
/// called in modal code, not inside container primitives themselves.
pub fn register_clickable(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::CLICK, layer);
}
