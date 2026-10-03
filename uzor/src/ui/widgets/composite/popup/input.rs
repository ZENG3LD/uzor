//! Popup input-coordinator helpers.

pub use super::render::register_input_coordinator_popup;

use super::state::PopupState;
use crate::layout::{
    ClickDispatcher, DispatchEvent, EventBuilder, PopupHandle,
};
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context
/// (scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the popup this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this popup. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this popup; pass it through.
///
/// `host_id` is the popup composite's WidgetId (e.g. `"popup-widget"`). Only
/// events whose carried id starts with `{host_id}:` are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut PopupState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0);
            if is_own {
                state.body_chevron_step(direction);
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

/// Inspect popup state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
pub fn drag_outcome_popup(state: &PopupState) -> Option<crate::layout::DragOutcome> {
    if state.scroll.is_dragging {
        return Some(crate::layout::DragOutcome::PopupBodyScroll);
    }
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::PopupResize);
    }
    None
}

/// Register a popup's click patterns (overflow chevrons) into `dispatcher`.
/// Used by [`the old L3 register helper`] and by any engine that owns
/// popup state without a `layout façade`.
pub fn register_popup_dispatch(dispatcher: &mut ClickDispatcher, handle: &PopupHandle) {
    use crate::layout::ChevronStepDirection;
    let id: &WidgetId = &handle.id;
    for (suffix, dir) in [
        ("chevron_up",    ChevronStepDirection::Up),
        ("chevron_down",  ChevronStepDirection::Down),
        ("chevron_left",  ChevronStepDirection::Left),
        ("chevron_right", ChevronStepDirection::Right),
    ] {
        let cid = WidgetId(format!("{}:{}", id.0, suffix));
        dispatcher.on_exact(
            format!("{}:{}", id.0, suffix),
            EventBuilder::ChevronStep { chevron_id: cid, direction: dir },
        );
    }
}

/// Returns `true` if `click_pos` is outside the popup rect and the popup
/// should be dismissed.
///
/// Guards drag gestures: if any drag is in progress the popup stays open even
/// if the pointer leaves its bounds (the user may drag the opacity slider
/// outside the frame).
pub fn handle_popup_dismiss(state: &PopupState, click_pos: (f64, f64), popup_rect: Rect) -> bool {
    if state.is_dragging_any() {
        return false;
    }
    !popup_rect.contains(click_pos.0, click_pos.1)
}

// ---------------------------------------------------------------------------
// Grid cell helper
// ---------------------------------------------------------------------------

/// A single cell in a popup color/icon grid.
pub struct PopupGridCell<'a> {
    /// Stable widget id for this cell (e.g. `"demo-popup-cell-0"`).
    pub id: &'a str,
    /// Fill color string (e.g. `"#ef5350"`).
    pub color: &'a str,
}
