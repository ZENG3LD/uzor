//! Toolbar event consumption.
//!
//! Level-1 hit registration stays in [`super::input`].
//!
//! ```
//! use uzor::layout::DispatchEvent;
//! use uzor::ui::widgets::composite::toolbar::consume::{
//!     consume_event, drag_outcome_toolbar, ConsumeEventCtx,
//! };
//! use uzor::ui::widgets::composite::toolbar::ToolbarState;
//! use uzor::{Rect, WidgetId};
//!
//! let mut state = ToolbarState::default();
//! let host = WidgetId::from("toolbar-widget");
//! let ctx = ConsumeEventCtx {
//!     cursor: (0.0, 0.0),
//!     frame_rect: Rect::new(0.0, 0.0, 400.0, 28.0),
//!     viewport: (800.0, 600.0),
//! };
//! let event = DispatchEvent::Unhandled(WidgetId::from("other"));
//! assert!(consume_event(event, &mut state, &host, ctx).is_some());
//! assert!(drag_outcome_toolbar(&state, "top").is_none());
//! ```

use super::state::ToolbarState;
use crate::layout::{ChevronStepDirection, DispatchEvent};
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context
/// (resize start).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the toolbar this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this toolbar. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this toolbar; pass it through.
///
/// `host_id` is the toolbar composite's WidgetId (e.g. `"toolbar-widget"`).
/// Only events whose carried id starts with `{host_id}:` (or equals `host_id`
/// for resize) are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut ToolbarState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested {
            ref chevron_id,
            direction,
        } => {
            let is_own = chevron_id.0 == format!("{}:chevron_back", host_id.0)
                || chevron_id.0 == format!("{}:chevron_fwd", host_id.0);
            if is_own {
                let step = 80.0_f64;
                let signed = match direction {
                    ChevronStepDirection::Up | ChevronStepDirection::Left => -step,
                    _ => step,
                };
                state.scroll_offset = (state.scroll_offset + signed).max(0.0);
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
                let min_size = 24.0_f64;
                let cap_size = (ctx.viewport.0.max(ctx.viewport.1) * 0.20).max(60.0);
                state.start_resize(edge, ctx.frame_rect, ctx.cursor, min_size, cap_size);
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Inspect toolbar state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
///
/// `which` — app-supplied tag for the toolbar (e.g. `"top"`, `"demo-left2"`).
pub fn drag_outcome_toolbar(
    state: &ToolbarState,
    which: &'static str,
) -> Option<crate::layout::DragOutcome> {
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::ToolbarResize { which });
    }
    None
}
