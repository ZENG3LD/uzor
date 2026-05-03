//! Toolbar input helpers — re-exports `register_input_coordinator_toolbar`
//! plus overflow-scroll and keyboard navigation utilities.

pub use super::render::register_input_coordinator_toolbar;

use super::render::register_context_manager_toolbar;

use super::settings::ToolbarSettings;
use super::state::ToolbarState;
use super::types::{ToolbarRenderKind, ToolbarView};
use crate::docking::panels::DockPanel;
use crate::input::{Sense, WidgetKind};
use crate::layout::{ChevronStepDirection, CompositeKind, CompositeRegistration, DispatchEvent, EventBuilder, LayoutManager, LayoutNodeId, ToolbarHandle, ToolbarNode, WidgetNode};
use crate::render::RenderContext;
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

/// Register + draw a toolbar in one call using a [`LayoutManager`].
///
/// Resolves the rect from the edge slot identified by `slot_id`, then
/// forwards to [`register_context_manager_toolbar`].  Returns `None` if the
/// slot is not present in the edge panels.
pub fn register_layout_manager_toolbar<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    handle:   &ToolbarHandle,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind:     &ToolbarRenderKind,
) -> Option<ToolbarNode> {
    let id: WidgetId = handle.id.clone();
    let rect = layout.rect_for_edge_slot(slot_id)?;

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.toolbars.remove(&id).unwrap_or_default();

    let layer = layout.compute_layer_for(parent);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Toolbar, rect, sense: Sense::CLICK });

    // Toolbar item ids land as "{toolbar-widget-id}:tb-foo" in the coordinator;
    // register a prefix pattern so any item click surfaces as
    // DispatchEvent::ToolbarItemClicked { toolbar, item_id = "tb-foo" }.
    layout.dispatcher_mut().on_prefix(
        format!("{}:", id.0),
        EventBuilder::ToolbarItem { handle: handle.clone() },
    );

    // Overflow chevrons — register paging step events for the two strips.
    // Exact-pattern dispatch beats the per-item prefix above.
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        use crate::layout::ChevronStepDirection;
        let is_vertical = matches!(kind, ToolbarRenderKind::Vertical);
        let (back_dir, fwd_dir) = if is_vertical {
            (ChevronStepDirection::Up, ChevronStepDirection::Down)
        } else {
            (ChevronStepDirection::Left, ChevronStepDirection::Right)
        };
        let back_id = WidgetId(format!("{}:chevron_back", id.0));
        let fwd_id  = WidgetId(format!("{}:chevron_fwd",  id.0));
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_back", id.0),
            EventBuilder::ChevronStep { chevron_id: back_id, direction: back_dir },
        );
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_fwd", id.0),
            EventBuilder::ChevronStep { chevron_id: fwd_id, direction: fwd_dir },
        );
    }

    // Resize handle (opt-in) — fires ResizeHandleDragStarted on mouse-down.
    // Caller picks the edge so a Vertical toolbar on the right side reports
    // W (drag-left-edge → grow leftward) instead of always E.
    if let Some(edge) = view.resize_edge {
        layout.dispatcher_mut().on_exact(
            format!("{}:resize", id.0),
            EventBuilder::ResizeHandle { host_id: id.clone(), edge },
        );
    }

    // Auto-forward hovered_item_id from the coordinator into toolbar state.
    let prefix = format!("{}:", id.0);
    state.sync_hover_from(&layout.ctx_mut().input, &prefix);

    register_context_manager_toolbar(
        layout.ctx_mut(), render, id.clone(), rect, &mut state, view, settings, kind, &layer,
    );

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::Toolbar,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: rect,
    });

    // Return state to the map.
    layout.toolbars.insert(id, state);

    Some(ToolbarNode(node_id))
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
