//! Sidebar input helpers (level 1).
//!
//! Re-exports `register_input_coordinator_sidebar` and keeps resize, scroll,
//! and collapse helpers. Event consumption lives in [`super::consume`] and
//! is re-exported here.

pub use super::consume::{consume_event, drag_outcome_sidebar, ConsumeEventCtx};
pub use super::render::register_input_coordinator_sidebar;

use super::state::{SidebarState, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};

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
    state: &mut SidebarState,
    panel_id: &str,
    delta: f64,
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
