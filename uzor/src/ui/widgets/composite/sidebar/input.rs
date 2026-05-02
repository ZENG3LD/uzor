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
use crate::layout::{ChevronStepDirection, DispatchEvent, LayoutManager, LayoutNodeId, SidebarNode, WidgetNode};
use crate::render::RenderContext;
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
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, .. } => {
            if hid == host_id {
                state.start_resize_drag(ctx.cursor.0);
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
