//! Popup input-coordinator helpers (level 1).
//!
//! Re-exports `register_input_coordinator_popup`. Event consumption lives in
//! [`super::consume`] and is re-exported here.

pub use super::consume::{consume_event, drag_outcome_popup, ConsumeEventCtx};
pub use super::render::register_input_coordinator_popup;

use super::state::PopupState;
use crate::layout::{ClickDispatcher, EventBuilder, PopupHandle};
use crate::types::{Rect, WidgetId};

/// Register a popup's click patterns (overflow chevrons) into `dispatcher`.
pub fn register_popup_dispatch(dispatcher: &mut ClickDispatcher, handle: &PopupHandle) {
    use crate::layout::ChevronStepDirection;
    let id: &WidgetId = &handle.id;
    for (suffix, dir) in [
        ("chevron_up", ChevronStepDirection::Up),
        ("chevron_down", ChevronStepDirection::Down),
        ("chevron_left", ChevronStepDirection::Left),
        ("chevron_right", ChevronStepDirection::Right),
    ] {
        let cid = WidgetId(format!("{}:{}", id.0, suffix));
        dispatcher.on_exact(
            format!("{}:{}", id.0, suffix),
            EventBuilder::ChevronStep {
                chevron_id: cid,
                direction: dir,
            },
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
