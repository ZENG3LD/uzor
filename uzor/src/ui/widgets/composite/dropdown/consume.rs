//! Dropdown event consumption.
//!
//! Level-1 hit registration and keyboard helpers stay in [`super::input`].
//!
//! ```
//! use uzor::layout::DispatchEvent;
//! use uzor::ui::widgets::composite::dropdown::consume::{consume_event, ConsumeEventCtx};
//! use uzor::ui::widgets::composite::dropdown::DropdownState;
//! use uzor::{Rect, WidgetId};
//!
//! let mut state = DropdownState::default();
//! let host = WidgetId::from("dropdown-widget");
//! let ctx = ConsumeEventCtx {
//!     cursor: (0.0, 0.0),
//!     frame_rect: Rect::new(0.0, 0.0, 180.0, 80.0),
//!     viewport: (800.0, 600.0),
//! };
//! let event = DispatchEvent::Unhandled(WidgetId::from("other"));
//! assert!(consume_event(event, &mut state, &host, ctx).is_some());
//! ```

use super::state::DropdownState;
use crate::layout::DispatchEvent;
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context.
///
/// Included for API uniformity with other composites; not used by dropdown
/// event handling today.
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the dropdown this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this dropdown. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this dropdown; pass it through.
///
/// `host_id` is the dropdown composite's WidgetId. Only events whose carried
/// `dropdown_id` equals `host_id` are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut DropdownState,
    host_id: &WidgetId,
    _ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::DropdownSubmenuToggle {
            ref dropdown,
            ref trigger_id,
        } => {
            if dropdown.id == *host_id {
                if state.submenu_open.as_deref() == Some(trigger_id.as_str()) {
                    state.submenu_open = None;
                } else {
                    state.submenu_open = Some(trigger_id.clone());
                }
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}
