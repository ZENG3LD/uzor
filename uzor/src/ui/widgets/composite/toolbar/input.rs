//! Toolbar input helpers — re-exports `register_input_coordinator_toolbar`
//! plus overflow-scroll and keyboard navigation utilities.

pub use super::render::register_input_coordinator_toolbar;


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
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
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
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, edge } => {
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
pub fn drag_outcome_toolbar(state: &ToolbarState, which: &'static str) -> Option<crate::layout::DragOutcome> {
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::ToolbarResize { which });
    }
    None
}

// ---------------------------------------------------------------------------
// Overflow scroll
// ---------------------------------------------------------------------------

/// Apply a scroll delta to the toolbar, clamped to `[0, max_scroll]`.
///
/// `delta`      — signed pixel delta (positive = scroll forward / right).
/// `max_scroll` — maximum allowed scroll offset; compute as
///                `content_size - bar_size` (pass `0.0` to disable scrolling).
pub fn handle_toolbar_overflow_scroll(state: &mut ToolbarState, delta: f64, max_scroll: f64) {
    state.scroll(delta, 0.0, max_scroll.max(0.0));
}

// ---------------------------------------------------------------------------
// Keyboard navigation
// ---------------------------------------------------------------------------

/// Move toolbar keyboard focus forward (Tab) or backward (Shift+Tab).
///
/// `item_ids`  — ordered slice of focusable item ids in the toolbar.
/// `forward`   — `true` for Tab, `false` for Shift+Tab.
///
/// Wraps around at the ends of the list.
pub fn handle_toolbar_keyboard(
    state:    &mut ToolbarState,
    item_ids: &[&str],
    forward:  bool,
) {
    if item_ids.is_empty() {
        return;
    }

    let current_idx = state
        .hovered_item_id
        .as_deref()
        .and_then(|id| item_ids.iter().position(|&s| s == id));

    let next_idx = match current_idx {
        None => 0,
        Some(idx) => {
            if forward {
                (idx + 1) % item_ids.len()
            } else {
                idx.checked_sub(1).unwrap_or(item_ids.len().saturating_sub(1))
            }
        }
    };

    if let Some(&id) = item_ids.get(next_idx) {
        state.hovered_item_id = Some(id.to_string());
    }
}
