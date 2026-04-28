//! Modal input-coordinator registration helpers.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ModalState;

/// Register the modal composite widget with the coordinator.
///
/// Call this **before** `draw_modal` so the composite id is present when child
/// widgets (`drag zone`, `close button`, tab buttons, footer buttons) are
/// registered as children inside `draw_modal`.
///
/// Returns the `WidgetId` assigned to the modal — pass it to `register_child`
/// calls if you need to add extra children outside of `draw_modal`.
pub fn register_modal(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) -> WidgetId {
    // `Sense::CLICK` on the composite catches clicks that land inside the modal
    // frame but don't hit any child widget, preventing click-through to the
    // canvas behind the modal.
    coord.register_composite(id, WidgetKind::Modal, rect, Sense::CLICK, layer)
}

/// Helper — apply a drag delta to modal state.
///
/// Call this in your pointer-move handler when `state.dragging` is `true`.
///
/// `cursor_pos`  — current pointer position in screen coordinates.
/// `screen_size` — `(width, height)` used to clamp the modal inside the viewport.
/// `modal_size`  — `(width, height)` of the modal frame.
pub fn handle_modal_drag(
    state: &mut ModalState,
    cursor_pos: (f64, f64),
    screen_size: (f64, f64),
    modal_size: (f64, f64),
) {
    state.update_drag(cursor_pos, screen_size, modal_size);
}
