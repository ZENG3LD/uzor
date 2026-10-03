//! Sidebar input helpers.
//!
//! Re-exports `register_input_coordinator_sidebar` and provides lightweight
//! helpers for common input operations (resize, scroll, collapse).

pub use super::render::register_input_coordinator_sidebar;


use super::state::{SidebarState, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};

use crate::layout::{ChevronStepDirection, DispatchEvent};
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context
/// (resize start, scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the sidebar this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this sidebar. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this sidebar; pass it through.
///
/// `host_id` is the sidebar composite's WidgetId (e.g. `"sidebar-widget"`).
/// Only events whose carried id starts with `{host_id}:` (or equals `host_id`
/// for resize) are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut SidebarState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0);
            if is_own {
                let step = 40.0_f64;
                let signed = match direction {
                    ChevronStepDirection::Up | ChevronStepDirection::Left => -step,
                    _ => step,
                };
                let scroll = state.get_or_insert_scroll("default");
                scroll.offset = (scroll.offset + signed).max(0.0);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, edge } => {
            if hid == host_id {
                let min_size = MIN_SIDEBAR_WIDTH;
                let cap_size = (ctx.viewport.0.max(ctx.viewport.1)).max(MAX_SIDEBAR_WIDTH);
                state.start_resize(edge, ctx.frame_rect, ctx.cursor, min_size, cap_size);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarTrackClicked { ref track_id } => {
            if track_id.0 == format!("{}:scrollbar_track", host_id.0) {
                // TODO: body_y / body_h / content_h / viewport_h not available
                // on SidebarState — pass through until dimensions are wired.
                Some(event)
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarThumbDragStarted { ref thumb_id } => {
            if thumb_id.0 == format!("{}:scrollbar_handle", host_id.0) {
                state.get_or_insert_scroll("default").start_drag(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Inspect sidebar state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
///
/// `which`       — app-supplied tag for the sidebar (e.g. `"main"`, `"right"`).
/// `sidebar_rect`— the sidebar frame rect this frame (used for scrollbar track geometry).
/// `est_content_h` — estimated content height in pixels (used for scrollbar math).
pub fn drag_outcome_sidebar(
    state:       &SidebarState,
    which:       &'static str,
    sidebar_rect: crate::types::Rect,
    est_content_h: f64,
) -> Option<crate::layout::DragOutcome> {
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::SidebarResize { which });
    }
    if let Some(scroll) = state.scroll_per_panel.get("default") {
        if scroll.is_dragging {
            let track_rect   = SidebarState::scrollbar_track_rect(sidebar_rect);
            let viewport_h   = track_rect.height;
            return Some(crate::layout::DragOutcome::SidebarScrollbar {
                track_rect,
                content_h:  est_content_h,
                viewport_h,
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Resize
// ---------------------------------------------------------------------------

/// Clamp a new size and apply it to `state.width` using the global pixel
/// limits `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
///
/// Use this when the sidebar lives on a vertical edge (Left/Right) — the
/// default min/max are sized for typical sidebar widths.
pub fn handle_sidebar_resize(state: &mut SidebarState, new_width: f64) {
    state.width = new_width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
}

/// Like [`handle_sidebar_resize`] but with explicit min/max bounds.
///
/// Top / Bottom sidebars want different limits than Left / Right because the
/// dimension being resized is height, not width. Caller passes whatever range
/// is appropriate (e.g. 60..viewport_height/2).
pub fn handle_sidebar_resize_clamped(state: &mut SidebarState, new_size: f64, min: f64, max: f64) {
    state.width = new_size.clamp(min, max);
}

// ---------------------------------------------------------------------------
// Scroll
// ---------------------------------------------------------------------------

/// Apply a scroll wheel delta to the per-panel scroll state.
///
/// `panel_id` — matches the key used in `state.scroll_per_panel`.
/// `delta`    — pixels; positive scrolls down.
/// `content_height` / `viewport_height` — needed to clamp the offset.
pub fn handle_sidebar_scroll(
    state:          &mut SidebarState,
    panel_id:       &str,
    delta:          f64,
    content_height: f64,
    viewport_height: f64,
) {
    let scroll = state.get_or_insert_scroll(panel_id);
    let max_scroll = (content_height - viewport_height).max(0.0);
    scroll.offset = (scroll.offset + delta).clamp(0.0, max_scroll);
}

// ---------------------------------------------------------------------------
// Collapse
// ---------------------------------------------------------------------------

/// Toggle the sidebar between collapsed and expanded.
pub fn handle_sidebar_collapse_toggle(state: &mut SidebarState) {
    state.toggle_collapse();
}

// ---------------------------------------------------------------------------
// SidebarBodyBuilder
// ---------------------------------------------------------------------------

/// A row entry for [`SidebarBodyBuilder::add_radio_group`].
pub struct SidebarRadioItem<'a> {
    /// Stable widget id for this radio button.
    pub id: &'a str,
    /// Display label.
    pub label: &'a str,
    /// Whether this item is currently selected.
    pub selected: bool,
}

/// A row entry for [`SidebarBodyBuilder::add_panel_list`].
pub struct SidebarPanelEntry<'a> {
    /// Widget id for the panel row's close button (e.g. `"dock-leaf-close-0"`).
    pub close_id: &'a str,
    /// Display title.
    pub title: &'a str,
    /// Whether this panel is the active leaf.
    pub active: bool,
}

