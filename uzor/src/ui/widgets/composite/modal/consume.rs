//! Modal event consumption.
//!
//! Level-1 hit registration stays in [`super::input`]. This module owns
//! [`consume_event`] and [`drag_outcome_modal`]: they take modal state and a
//! [`DispatchEvent`](crate::layout::DispatchEvent), not a layout owner.
//!
//! ```
//! use uzor::layout::DispatchEvent;
//! use uzor::ui::widgets::composite::modal::consume::{
//!     consume_event, drag_outcome_modal, ConsumeEventCtx,
//! };
//! use uzor::ui::widgets::composite::modal::ModalState;
//! use uzor::{Rect, WidgetId};
//!
//! let mut state = ModalState::default();
//! let host = WidgetId::from("modal-widget");
//! let ctx = ConsumeEventCtx {
//!     cursor: (1.0, 2.0),
//!     frame_rect: Rect::new(0.0, 0.0, 200.0, 120.0),
//!     viewport: (800.0, 600.0),
//! };
//! let event = DispatchEvent::Unhandled(WidgetId::from("other"));
//! assert!(consume_event(event, &mut state, &host, ctx).is_some());
//! assert!(drag_outcome_modal(&state).is_none());
//! ```

use super::state::ModalState;
use crate::layout::DispatchEvent;
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
        DispatchEvent::ChevronStepRequested {
            ref chevron_id,
            direction,
        } => {
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
        DispatchEvent::ResizeHandleDragStarted {
            host_id: ref hid,
            edge,
        } => {
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
