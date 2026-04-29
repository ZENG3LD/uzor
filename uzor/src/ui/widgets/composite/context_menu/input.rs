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
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Register + draw a context menu in one call using a [`LayoutManager`].
///
/// Confirms the overlay slot identified by `slot_id` is registered, then
/// forwards to [`register_context_manager_context_menu`].  The menu positions
/// itself from `state.x`/`state.y`; the overlay rect is used only to verify
/// the slot exists.  Returns `None` if the slot is not present.
pub fn register_layout_manager_context_menu<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut ContextMenuState,
    view:     &mut ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
    layer:    &LayerId,
) -> Option<WidgetId> {
    let _rect: Rect = layout.rect_for_overlay(slot_id)?;
    Some(register_context_manager_context_menu(
        layout.ctx_mut(), render, id, state, view, settings, kind, layer,
    ))
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
