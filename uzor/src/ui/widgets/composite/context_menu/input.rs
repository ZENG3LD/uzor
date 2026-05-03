//! ContextMenu input helpers.
//!
//! Re-exports `register_input_coordinator_context_menu` from `render.rs` and
//! adds click-outside dismiss and keyboard navigation.

pub use super::render::register_input_coordinator_context_menu;

use super::render::register_context_manager_context_menu;

use super::settings::ContextMenuSettings;
use super::state::ContextMenuState;
use super::types::{ContextMenuRenderKind, ContextMenuView};
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{CompositeKind, CompositeRegistration, ContextMenuNode, DismissFrame, EventBuilder, LayoutManager, LayoutNodeId, OverlayEntry, OverlayKind, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Register + draw a context menu in one call using a [`LayoutManager`].
///
/// Pushes the overlay entry, then registers the context-menu layer with the
/// coordinator (blocking lower layers) and forwards to
/// [`register_context_manager_context_menu`].  The menu positions itself from
/// `state.x`/`state.y`; `overlay_rect` is used for dismiss-frame resolution.
///
/// `slot_id`      — stable overlay id (e.g. `"ctx-menu-overlay"`).
/// `overlay_rect` — screen-space rect of the menu panel this frame.
/// `anchor`       — optional anchor rect for positioning.
pub fn register_layout_manager_context_menu<P: DockPanel>(
    layout:       &mut LayoutManager<P>,
    render:       &mut dyn RenderContext,
    parent:       LayoutNodeId,
    slot_id:      &str,
    id:           impl Into<WidgetId>,
    overlay_rect: Rect,
    anchor:       Option<Rect>,
    view:         &mut ContextMenuView<'_>,
    settings:     &ContextMenuSettings,
    kind:         &ContextMenuRenderKind<'_>,
) -> Option<ContextMenuNode> {
    let id: WidgetId = id.into();

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.context_menus.remove(&id).unwrap_or_default();

    layout.push_overlay(OverlayEntry {
        id:   slot_id.to_string(),
        kind: OverlayKind::ContextMenu,
        rect: overlay_rect,
        anchor,
    });
    let slot_rect = overlay_rect;
    let layer = LayerId::new("context_menu");
    let z_order = layout.z_layers().context_menu as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect: slot_rect,
        overlay_id: WidgetId::new(slot_id),
    });
    // Context menu blocks lower layers — push the layer into the coordinator.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    // Context menu positions itself from state.x/state.y; use a zero rect for tree metadata.
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::ContextMenu, rect: Rect::new(state.x, state.y, 0.0, 0.0), sense: Sense::CLICK });

    // Item ids are "{menu-id}:item:N" — surface as
    // DispatchEvent::ContextMenuItemClicked { menu_id, item_index }.
    layout.dispatcher_mut().on_prefix(
        format!("{}:item:", id.0),
        EventBuilder::ContextMenuItem { menu_id: id.clone() },
    );

    // Auto-forward hovered_index from the coordinator into menu state.
    let prefix = format!("{}:item:", id.0);
    state.sync_hover_from(&layout.ctx_mut().input, &prefix);

    register_context_manager_context_menu(
        layout.ctx_mut(), render, id.clone(), &mut state, view, settings, kind, &layer,
    );

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::ContextMenu,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: overlay_rect,
    });

    // Return state to the map.
    layout.context_menus.insert(id, state);

    Some(ContextMenuNode(node_id))
}

// ---------------------------------------------------------------------------
// Click-outside dismiss
// ---------------------------------------------------------------------------

/// Returns `true` if a click at `click_pos` is outside the open menu panel,
/// meaning the menu should be dismissed.
///
/// `menu_rect` — current screen rect of the menu panel.
pub fn handle_context_menu_dismiss(
    state:      &ContextMenuState,
    click_pos:  (f64, f64),
    menu_rect:  Rect,
) -> bool {
    if !state.is_open {
        return false;
    }
    !menu_rect.contains(click_pos.0, click_pos.1)
}

// ---------------------------------------------------------------------------
// Keyboard navigation
// ---------------------------------------------------------------------------

/// Key events relevant to context menu keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuKey {
    /// Move hover to the next enabled item.
    ArrowDown,
    /// Move hover to the previous enabled item.
    ArrowUp,
    /// Activate the currently hovered item.
    Enter,
    /// Close the menu without activating anything.
    Esc,
}

/// Result of `handle_context_menu_keyboard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuKeyResult {
    /// Menu should close.
    Close,
    /// Item at this index should be activated.
    Activate(usize),
    /// Hover moved to this item index.
    Hovered(usize),
    /// No change.
    None,
}

/// Handle a keyboard event on an open context menu.
///
/// `enabled_count` — number of rows that are enabled and navigable.
///   Rows are numbered `0..enabled_count` in display order.
///
/// Returns the action to take.  Caller should call `state.close()` when
/// `Close` is returned.
pub fn handle_context_menu_keyboard(
    state:         &mut ContextMenuState,
    key:           ContextMenuKey,
    enabled_count: usize,
) -> ContextMenuKeyResult {
    match key {
        ContextMenuKey::Esc => {
            state.close();
            ContextMenuKeyResult::Close
        }
        ContextMenuKey::Enter => {
            match state.hovered_index {
                Some(idx) => ContextMenuKeyResult::Activate(idx),
                None      => ContextMenuKeyResult::None,
            }
        }
        ContextMenuKey::ArrowDown => {
            if enabled_count == 0 {
                return ContextMenuKeyResult::None;
            }
            let next = match state.hovered_index {
                None      => 0,
                Some(cur) => (cur + 1).min(enabled_count.saturating_sub(1)),
            };
            state.hovered_index = Some(next);
            ContextMenuKeyResult::Hovered(next)
        }
        ContextMenuKey::ArrowUp => {
            if enabled_count == 0 {
                return ContextMenuKeyResult::None;
            }
            let next = match state.hovered_index {
                None      => enabled_count.saturating_sub(1),
                Some(cur) => cur.saturating_sub(1),
            };
            state.hovered_index = Some(next);
            ContextMenuKeyResult::Hovered(next)
        }
    }
}
