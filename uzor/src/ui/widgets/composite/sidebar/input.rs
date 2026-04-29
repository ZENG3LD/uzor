//! Sidebar input helpers.
//!
//! Re-exports `register_input_coordinator_sidebar` and provides lightweight
//! helpers for common input operations (resize, scroll, collapse).

pub use super::render::register_input_coordinator_sidebar;

use super::render::register_context_manager_sidebar;

use super::settings::SidebarSettings;
use super::state::{SidebarState, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};
use super::types::{SidebarRenderKind, SidebarView};
use crate::docking::panels::DockPanel;
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::WidgetId;

/// Register + draw a sidebar in one call using a [`LayoutManager`].
///
/// Resolves the rect from the edge slot identified by `slot_id`, then
/// forwards to [`register_context_manager_sidebar`].  Returns `None` if the
/// slot is not present in the edge panels.
pub fn register_layout_manager_sidebar<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
    layer:    &LayerId,
) -> Option<WidgetId> {
    let rect = layout.rect_for_edge_slot(slot_id)?;
    Some(register_context_manager_sidebar(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, layer,
    ))
}

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
