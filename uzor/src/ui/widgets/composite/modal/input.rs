//! Modal input-coordinator helpers.
//!
//! `register_modal` is defined in `render.rs` (alongside `draw_modal` and the
//! `modal` convenience wrapper) because all three share the layout computation.
//! This module re-exports it and adds the drag helper.

pub use super::render::register_modal;

use super::state::ModalState;

/// Apply a drag delta to modal state.
///
/// Call this in your pointer-move handler when the drag-handle widget reports
/// a drag gesture (`state.dragging` is `true`).
///
/// `cursor_pos`  — current pointer position in screen coordinates.
/// `screen_size` — `(width, height)` used to clamp the modal inside the viewport.
/// `modal_size`  — `(width, height)` of the modal frame.
pub fn handle_modal_drag(
    state:       &mut ModalState,
    cursor_pos:  (f64, f64),
    screen_size: (f64, f64),
    modal_size:  (f64, f64),
) {
    state.update_drag(cursor_pos, screen_size, modal_size);
}
