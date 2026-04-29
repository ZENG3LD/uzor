//! Panel input helpers.
//!
//! Re-exports `register_input_coordinator_panel` and provides lightweight
//! helpers for column sort, scroll, and action click.

pub use super::render::register_input_coordinator_panel;

use super::state::PanelState;

// ---------------------------------------------------------------------------
// Column sort
// ---------------------------------------------------------------------------

/// Handle a column-header click — toggle sort state.
///
/// - If `column_id` is the current sort column → flip `sort_ascending`.
/// - Otherwise → set `sort_column = Some(column_id)`, `sort_ascending = true`.
pub fn handle_panel_column_click(state: &mut PanelState, column_id: impl Into<String>) {
    state.toggle_sort(column_id);
}

// ---------------------------------------------------------------------------
// Scroll
// ---------------------------------------------------------------------------

/// Apply a scroll wheel delta (pixels) to the panel scroll state.
///
/// `delta` — positive scrolls down.
/// `content_height` / `viewport_height` — used to clamp the offset.
pub fn handle_panel_scroll(
    state:           &mut PanelState,
    delta:           f64,
    content_height:  f64,
    viewport_height: f64,
) {
    let max_scroll = (content_height - viewport_height).max(0.0);
    state.scroll.offset = (state.scroll.offset + delta).clamp(0.0, max_scroll);
}

// ---------------------------------------------------------------------------
// Action click
// ---------------------------------------------------------------------------

/// Handle a header action button click — returns the action id for the caller
/// to dispatch.  Clears `hovered_action` on the state.
pub fn handle_panel_action_click(state: &mut PanelState, action_id: &str) -> String {
    state.hovered_action = None;
    action_id.to_owned()
}
