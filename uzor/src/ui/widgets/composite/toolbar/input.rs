//! Toolbar input helpers — re-exports `register_input_coordinator_toolbar`
//! plus overflow-scroll and keyboard navigation utilities.

pub use super::render::register_input_coordinator_toolbar;

use super::render::register_context_manager_toolbar;

use super::settings::ToolbarSettings;
use super::state::ToolbarState;
use super::types::{ToolbarRenderKind, ToolbarView};
use crate::docking::panels::DockPanel;
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::WidgetId;

/// Register + draw a toolbar in one call using a [`LayoutManager`].
///
/// Resolves the rect from the edge slot identified by `slot_id`, then
/// forwards to [`register_context_manager_toolbar`].  Returns `None` if the
/// slot is not present in the edge panels.
pub fn register_layout_manager_toolbar<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut ToolbarState,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind:     &ToolbarRenderKind,
    layer:    &LayerId,
) -> Option<WidgetId> {
    let rect = layout.rect_for_edge_slot(slot_id)?;
    Some(register_context_manager_toolbar(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, layer,
    ))
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
