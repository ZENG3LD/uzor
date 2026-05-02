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
use crate::input::{Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, SidebarNode, WidgetNode};
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
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) -> Option<SidebarNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for_edge_slot(slot_id)?;
    let layer = layout.compute_layer_for(parent);

    // Initialise size from viewport % on first registration. Top/Bottom use
    // viewport height, Left/Right/WithTypeSelector use viewport width. Once
    // sized, subsequent calls are no-ops so user resize stays sticky.
    if let Some(win) = layout.last_window() {
        let is_horizontal_kind = !matches!(kind, super::types::SidebarRenderKind::Top | super::types::SidebarRenderKind::Bottom);
        state.ensure_sized(win.width, win.height, is_horizontal_kind);
    }

    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Sidebar, rect, sense: Sense::CLICK });

    // Register dispatcher patterns so the inner scrollbar (when shown) gets
    // semantic events. Sidebar composite registers child rects as
    // "{id}:scrollbar_handle" (DRAG) and "{id}:scrollbar_track" (CLICK).
    if view.effective_show_scrollbar() {
        use crate::layout::EventBuilder;
        layout.dispatcher_mut().on_exact(
            format!("{}:scrollbar_track", id.0),
            EventBuilder::ScrollbarTrack { track_id: WidgetId::new(format!("{}:scrollbar_track", id.0)) },
        );
        layout.dispatcher_mut().on_exact(
            format!("{}:scrollbar_handle", id.0),
            EventBuilder::ScrollbarThumb { thumb_id: WidgetId::new(format!("{}:scrollbar_handle", id.0)) },
        );
    }

    // Chevrons mode — register paging step events on the two overlay strips.
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        use crate::layout::{ChevronStepDirection, EventBuilder};
        let chev_up_id = WidgetId::new(format!("{}:chevron_up", id.0));
        let chev_down_id = WidgetId::new(format!("{}:chevron_down", id.0));
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_up", id.0),
            EventBuilder::ChevronStep { chevron_id: chev_up_id, direction: ChevronStepDirection::Up },
        );
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_down", id.0),
            EventBuilder::ChevronStep { chevron_id: chev_down_id, direction: ChevronStepDirection::Down },
        );
    }

    register_context_manager_sidebar(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
    Some(SidebarNode(node_id))
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
