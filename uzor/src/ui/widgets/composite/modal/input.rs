//! Modal input-coordinator helpers.
//!
//! `register_input_coordinator_modal` is defined in `render.rs` (alongside
//! `the old L2 register helper`) because both share the layout computation.
//! This module re-exports it and adds the drag helper.

pub use super::render::register_input_coordinator_modal;


use super::state::ModalState;
use crate::layout::{
    ClickDispatcher, DispatchEvent, EventBuilder, ModalHandle,
};
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

/// Register a modal's click patterns (close, footer, tabs, wizard, body
/// scrollbar, chevrons and — when `resizable` — the eight resize handles)
/// into `dispatcher`, so clicks on its parts surface as typed
/// [`DispatchEvent`]s. Used by [`the old L3 register helper`] and by any
/// engine that owns modal state without a `layout façade`.
pub fn register_modal_dispatch(dispatcher: &mut ClickDispatcher, handle: &ModalHandle, resizable: bool) {
    let id: &WidgetId = &handle.id;
    dispatcher.on_exact(
        format!("{}:close", id.0),
        EventBuilder::ModalClose { handle: handle.clone() },
    );
    // Footer buttons close the modal by default — same semantics as the X.
    dispatcher.on_prefix(
        format!("{}:footer:", id.0),
        EventBuilder::ModalClose { handle: handle.clone() },
    );
    dispatcher.on_prefix(
        format!("{}:tab:", id.0),
        EventBuilder::ModalTabFromSuffix { handle: handle.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:next", id.0),
        EventBuilder::ModalWizardNext { handle: handle.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:back", id.0),
        EventBuilder::ModalWizardBack { handle: handle.clone() },
    );

    // Body overflow dispatcher routing — both chevron and scrollbar routes
    // are registered unconditionally; the active guard is chosen per frame
    // inside register_body_overflow based on view.overflow.
    dispatcher.on_exact(
        format!("{}:scrollbar_track", id.0),
        EventBuilder::ScrollbarTrack { track_id: WidgetId::new(format!("{}:scrollbar_track", id.0)) },
    );
    dispatcher.on_exact(
        format!("{}:scrollbar_handle", id.0),
        EventBuilder::ScrollbarThumb { thumb_id: WidgetId::new(format!("{}:scrollbar_handle", id.0)) },
    );
    {
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
    if resizable {
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
}

/// Inspect modal state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
///
/// Call immediately after a successful consume. Returns `None` if no drag was
/// started (e.g. the event was a click, not a drag-start).
pub fn drag_outcome_modal(state: &ModalState) -> Option<crate::layout::DragOutcome> {
    if state.scroll.is_dragging {
        return Some(crate::layout::DragOutcome::ModalBodyScroll);
    }
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::ModalResize);
    }
    None
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

/// Hit-test whether a pointer position is inside the modal header drag zone.
///
/// Returns `true` when `(px, py)` falls within the header rect:
/// - x: `[modal_rect.x, modal_rect.x + modal_rect.width - close_btn_width]`
/// - y: `[modal_rect.y, modal_rect.y + header_height]`
///
/// `header_height` — height of the header strip in pixels (default: `44.0`).
/// `close_btn_width` — width reserved for the close button on the right
///                     (default: `34.0` = 24 px button + 10 px padding).
pub fn modal_header_hit(
    modal_rect:      Rect,
    px:              f64,
    py:              f64,
    header_height:   f64,
    close_btn_width: f64,
) -> bool {
    px >= modal_rect.x
        && px <= modal_rect.x + modal_rect.width - close_btn_width
        && py >= modal_rect.y
        && py <= modal_rect.y + header_height
}
