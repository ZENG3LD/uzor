//! Dropdown input-coordinator helpers.
//!
//! Re-exports `register_input_coordinator_dropdown` from `render.rs` and adds
//! click-outside dismiss + keyboard navigation helpers.

pub use super::render::register_input_coordinator_dropdown;

use super::render::register_context_manager_dropdown;

use super::settings::DropdownSettings;
use super::state::DropdownState;
use super::types::{DropdownRenderKind, DropdownView};
use crate::docking::panels::DockPanel;
use crate::input::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Register + draw a dropdown in one call using a [`LayoutManager`].
///
/// Resolves the rect from the overlay slot identified by `slot_id`, then
/// forwards to [`register_context_manager_dropdown`].  Returns `None` if the
/// slot is not present in the overlay stack.
pub fn register_layout_manager_dropdown<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut DropdownState,
    view:     &mut DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
    layer:    &LayerId,
) -> Option<WidgetId> {
    let rect = layout.rect_for_overlay(slot_id)?;
    Some(register_context_manager_dropdown(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, layer,
    ))
}

/// Returns `true` if a click at `click_pos` is outside both the main panel and
/// the open submenu panel, meaning the dropdown should be dismissed.
///
/// `main_rect`    — screen rect of the main dropdown panel.
/// `submenu_rect` — `Some(rect)` when a submenu panel is currently open.
pub fn handle_dropdown_dismiss(
    state:        &DropdownState,
    click_pos:    (f64, f64),
    main_rect:    Rect,
    submenu_rect: Option<Rect>,
) -> bool {
    if !state.open {
        return false;
    }
    let inside_main = main_rect.contains(click_pos.0, click_pos.1);
    let inside_sub  = submenu_rect
        .map(|r| r.contains(click_pos.0, click_pos.1))
        .unwrap_or(false);
    !inside_main && !inside_sub
}

/// Keyboard navigation for an open dropdown.
///
/// `items` — ordered list of item ids (headers / separators represented as
/// `""` so navigation skips them).
///
/// Returns the new `hovered_id` after applying the key action, or `None` if
/// the dropdown should close (Esc).
///
/// Callers should call `state.close()` when `None` is returned.
pub fn handle_dropdown_keyboard(
    state:  &mut DropdownState,
    key:    DropdownKey,
    items:  &[Option<&str>],
) -> DropdownKeyResult {
    match key {
        DropdownKey::Esc => {
            state.close();
            DropdownKeyResult::Close
        }
        DropdownKey::Enter => {
            if let Some(ref id) = state.hovered_id {
                DropdownKeyResult::Activate(id.clone())
            } else {
                DropdownKeyResult::None
            }
        }
        DropdownKey::ArrowDown => {
            let navigable: Vec<&str> = items.iter().filter_map(|o| *o).collect();
            if navigable.is_empty() {
                return DropdownKeyResult::None;
            }
            let next = match &state.hovered_id {
                None => navigable[0].to_owned(),
                Some(cur) => {
                    let pos = navigable.iter().position(|&s| s == cur.as_str());
                    let next_idx = pos.map(|i| (i + 1).min(navigable.len().saturating_sub(1))).unwrap_or(0);
                    navigable[next_idx].to_owned()
                }
            };
            state.hovered_id = Some(next.clone());
            DropdownKeyResult::Hovered(next)
        }
        DropdownKey::ArrowUp => {
            let navigable: Vec<&str> = items.iter().filter_map(|o| *o).collect();
            if navigable.is_empty() {
                return DropdownKeyResult::None;
            }
            let next = match &state.hovered_id {
                None => navigable[navigable.len().saturating_sub(1)].to_owned(),
                Some(cur) => {
                    let pos = navigable.iter().position(|&s| s == cur.as_str());
                    let next_idx = pos.map(|i| i.saturating_sub(1)).unwrap_or(0);
                    navigable[next_idx].to_owned()
                }
            };
            state.hovered_id = Some(next.clone());
            DropdownKeyResult::Hovered(next)
        }
        DropdownKey::Tab => {
            state.close();
            DropdownKeyResult::Close
        }
    }
}

// ---------------------------------------------------------------------------
// Key / result types
// ---------------------------------------------------------------------------

/// Key events relevant to dropdown keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownKey {
    /// Move hover to the next enabled item.
    ArrowDown,
    /// Move hover to the previous enabled item.
    ArrowUp,
    /// Activate the currently hovered item.
    Enter,
    /// Close the dropdown.
    Esc,
    /// Close the dropdown (optional; matches browser behaviour).
    Tab,
}

/// Result of `handle_dropdown_keyboard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownKeyResult {
    /// Dropdown should close.
    Close,
    /// Item with this id should be activated.
    Activate(String),
    /// Hover moved to this item id.
    Hovered(String),
    /// No change.
    None,
}
