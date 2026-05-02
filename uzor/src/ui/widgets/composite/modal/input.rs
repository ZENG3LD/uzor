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
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{DispatchEvent, EventBuilder, LayoutManager, LayoutNodeId, ModalNode, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context
/// (resize start, scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the modal this frame (post-drag, post-resize).
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this modal. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this modal; pass it through.
///
/// `host_id` is the modal composite's WidgetId (e.g. `"modal-widget"`). Only
/// events whose carried id starts with `{host_id}:` (or equals `host_id` for
/// resize) are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut ModalState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0)
                || chevron_id.0 == format!("{}:chevron_left", host_id.0)
                || chevron_id.0 == format!("{}:chevron_right", host_id.0);
            if is_own {
                state.body_chevron_step(direction);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, edge } => {
            if hid == host_id {
                let min = (200.0_f64, 120.0_f64);
                let cap = (f64::INFINITY, f64::INFINITY);
                state.start_resize(edge, ctx.frame_rect, ctx.cursor, min, cap);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarTrackClicked { ref track_id } => {
            if track_id.0 == format!("{}:scrollbar_track", host_id.0) {
                state.body_scroll_track_click(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarThumbDragStarted { ref thumb_id } => {
            if thumb_id.0 == format!("{}:scrollbar_handle", host_id.0) {
                state.start_body_scroll_drag(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Register + draw a modal in one call using a [`LayoutManager`].
///
/// Resolves the rect from the overlay slot identified by `slot_id`, then
/// pushes the modal layer onto the coordinator (so it blocks lower layers)
/// and forwards to [`register_context_manager_modal`].
/// Returns `None` if the slot is not present in the overlay stack.
pub fn register_layout_manager_modal<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) -> Option<ModalNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for_overlay(slot_id)?;
    let layer = LayerId::modal();
    let z_order = layout.z_layers().modal as u32;
    // Push the modal layer so that the coordinator's hit-test blocks lower layers.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Modal, rect, sense: Sense::CLICK });

    // Register dispatcher patterns so the app gets semantic events instead of
    // raw "modal-widget:close" string matching.
    let dispatcher = layout.dispatcher_mut();
    dispatcher.on_exact(
        format!("{}:close", id.0),
        EventBuilder::ModalClose { modal_id: id.clone() },
    );
    // Footer buttons close the modal by default — same semantics as the X.
    dispatcher.on_prefix(
        format!("{}:footer:", id.0),
        EventBuilder::ModalClose { modal_id: id.clone() },
    );
    dispatcher.on_prefix(
        format!("{}:tab:", id.0),
        EventBuilder::ModalTabFromSuffix { modal_id: id.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:next", id.0),
        EventBuilder::ModalWizardNext { modal_id: id.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:back", id.0),
        EventBuilder::ModalWizardBack { modal_id: id.clone() },
    );

    // Body overflow patterns (Scrollbar / Chevrons) and resize handles.
    if matches!(view.overflow, crate::types::OverflowMode::Scrollbar) {
        dispatcher.on_exact(
            format!("{}:scrollbar_track", id.0),
            EventBuilder::ScrollbarTrack { track_id: WidgetId::new(format!("{}:scrollbar_track", id.0)) },
        );
        dispatcher.on_exact(
            format!("{}:scrollbar_handle", id.0),
            EventBuilder::ScrollbarThumb { thumb_id: WidgetId::new(format!("{}:scrollbar_handle", id.0)) },
        );
    }
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        use crate::layout::ChevronStepDirection;
        for (suffix, dir) in [
            ("chevron_up",    ChevronStepDirection::Up),
            ("chevron_down",  ChevronStepDirection::Down),
            ("chevron_left",  ChevronStepDirection::Left),
            ("chevron_right", ChevronStepDirection::Right),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep {
                    chevron_id: WidgetId::new(format!("{}:{}", id.0, suffix)),
                    direction:  dir,
                },
            );
        }
    }
    // Resize handles (8): N S W E + NW NE SW SE.
    if view.resizable {
        use crate::layout::ResizeEdge;
        for (suffix, edge) in &[
            ("resize_n",  ResizeEdge::N),
            ("resize_s",  ResizeEdge::S),
            ("resize_w",  ResizeEdge::W),
            ("resize_e",  ResizeEdge::E),
            ("resize_nw", ResizeEdge::NW),
            ("resize_ne", ResizeEdge::NE),
            ("resize_sw", ResizeEdge::SW),
            ("resize_se", ResizeEdge::SE),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ResizeHandle { host_id: id.clone(), edge: *edge },
            );
        }
    }

    register_context_manager_modal(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
    Some(ModalNode(node_id))
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
