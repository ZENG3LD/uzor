//! Panel input helpers.
//!
//! Re-exports `register_input_coordinator_panel` and provides lightweight
//! helpers for column sort, scroll, and action click.

pub use super::render::register_input_coordinator_panel;

use super::render::register_context_manager_panel;

use super::settings::PanelSettings;
use super::state::PanelState;
use super::types::{PanelRenderKind, PanelView};
use crate::docking::panels::DockPanel;
use crate::input::{Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, PanelNode, WidgetNode};
use crate::render::RenderContext;
use crate::types::WidgetId;

/// Register + draw a panel in one call using a [`LayoutManager`].
///
/// Resolves the rect from the dock leaf identified by `slot_id`, then
/// forwards to [`register_context_manager_panel`].  Returns `None` if the leaf
/// is not present in the panel tree.
pub fn register_layout_manager_panel<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut PanelState,
    view:     &mut PanelView<'_>,
    settings: &PanelSettings,
    kind:     &PanelRenderKind,
) -> Option<PanelNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for(slot_id)?;
    let layer = layout.compute_layer_for(parent);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Panel, rect, sense: Sense::CLICK });

    // Body chevron routing (Chevrons explicitly OR Clip/Compress when
    // content overflows the body — post-resize fallback).
    if !matches!(view.overflow, crate::types::OverflowMode::Scrollbar) {
        use crate::layout::{ChevronStepDirection, EventBuilder};
        for (suffix, dir) in [
            ("chevron_up",    ChevronStepDirection::Up),
            ("chevron_down",  ChevronStepDirection::Down),
            ("chevron_left",  ChevronStepDirection::Left),
            ("chevron_right", ChevronStepDirection::Right),
        ] {
            let cid = WidgetId::new(format!("{}:{}", id.0, suffix));
            layout.dispatcher_mut().on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep { chevron_id: cid, direction: dir },
            );
        }
    }

    register_context_manager_panel(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
    Some(PanelNode(node_id))
}

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
