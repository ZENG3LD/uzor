//! Sidebar input helpers.
//!
//! Re-exports `register_input_coordinator_sidebar` and provides lightweight
//! helpers for common input operations (resize, scroll, collapse).

pub use super::render::register_input_coordinator_sidebar;

use super::state::{SidebarState, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};

// ---------------------------------------------------------------------------
// Resize
// ---------------------------------------------------------------------------

/// Clamp a new width and apply it to `state`.
///
/// Used by callers that compute the desired width from drag deltas outside the
/// composite (e.g. chart-app dragging the resize zone).
pub fn handle_sidebar_resize(state: &mut SidebarState, new_width: f64) {
    state.width = new_width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
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
