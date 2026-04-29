//! Modal input-coordinator helpers.
//!
//! `register_input_coordinator_modal` is defined in `render.rs` (alongside
//! `register_context_manager_modal`) because both share the layout computation.
//! This module re-exports it and adds the drag helper.

pub use super::render::register_input_coordinator_modal;

use super::render::register_context_manager_modal;

use super::settings::ModalSettings;
use super::state::ModalState;
use super::types::{ModalRenderKind, ModalView};
use crate::docking::panels::DockPanel;
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::WidgetId;

/// Register + draw a modal in one call using a [`LayoutManager`].
///
/// Resolves the rect from the overlay slot identified by `slot_id`, then
/// forwards to [`register_context_manager_modal`].  Returns `None` if the slot
/// is not present in the overlay stack.
pub fn register_layout_manager_modal<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
    layer:    &LayerId,
) -> Option<()> {
    let rect = layout.rect_for_overlay(slot_id)?;
    Some(register_context_manager_modal(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, layer,
    ))
}

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
