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
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{DismissFrame, DispatchEvent, DropdownNode, EventBuilder, LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context.
///
/// Included for API uniformity with other composites; not used by dropdown
/// event handling today.
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the dropdown this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this dropdown. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this dropdown; pass it through.
///
/// `host_id` is the dropdown composite's WidgetId. Only events whose carried
/// `dropdown_id` equals `host_id` are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut DropdownState,
    host_id: &WidgetId,
    _ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::DropdownSubmenuToggle { ref dropdown_id, ref trigger_id } => {
            if dropdown_id == host_id {
                if state.submenu_open.as_deref() == Some(trigger_id.as_str()) {
                    state.submenu_open = None;
                } else {
                    state.submenu_open = Some(trigger_id.clone());
                }
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Register + draw a dropdown in one call using a [`LayoutManager`].
///
/// Resolves the rect from the overlay slot identified by `slot_id`, pushes the
/// dropdown layer onto the coordinator, then forwards to
/// [`register_context_manager_dropdown`].  Returns `None` if the slot is not
/// present in the overlay stack.
pub fn register_layout_manager_dropdown<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut DropdownState,
    view:     &mut DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
) -> Option<DropdownNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for_overlay(slot_id)?;
    let layer = LayerId::new("dropdown");
    let z_order = layout.z_layers().dropdown as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect,
        overlay_id: WidgetId::new(slot_id),
    });
    // Dropdown blocks lower layers — push the layer into the coordinator.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Dropdown, rect, sense: Sense::CLICK });

    // Register dispatch patterns: clicks on items + sub-items both surface
    // as DispatchEvent::DropdownItemClicked { dropdown_id, item_id }.
    layout.dispatcher_mut().on_prefix(
        format!("{}:item:", id.0),
        EventBuilder::DropdownItem { dropdown_id: id.clone() },
    );
    layout.dispatcher_mut().on_prefix(
        format!("{}:sub-item:", id.0),
        EventBuilder::DropdownItem { dropdown_id: id.clone() },
    );
    // Submenu chevron clicks (only used for SubmenuTrigger::ChevronClick).
    // Sticky chevron registers as `{dropdown}:chev:submenu:{row_id}`.
    layout.dispatcher_mut().on_prefix(
        format!("{}:chev:submenu:", id.0),
        EventBuilder::DropdownSubmenuToggleFromSuffix { dropdown_id: id.clone() },
    );

    // Auto-forward hovered_id (main panel) and submenu_hovered_id
    // (submenu panel) from the coordinator into the dropdown state.
    state.sync_flat_hover(&layout.ctx_mut().input, &id.0);

    // Auto-manage submenu open/close from coordinator hover state.
    //
    // - Hovering a `:submenu:{id}` row (trigger=Hover) opens it.
    // - Hovering inside the submenu panel keeps it open.
    // - Hovering a *non-submenu* main row closes the submenu.
    // - Hovering a `:submenu-chevron:` row keeps the submenu state alone
    //   (open is driven by *click* on the chevron, dispatched as
    //   DispatchEvent::DropdownSubmenuToggle).
    {
        let coord = &layout.ctx_mut().input;
        let main_prefix    = format!("{}:item:", id.0);
        let submenu_prefix = format!("{}:submenu:", id.0);
        let chev_prefix    = format!("{}:chev:submenu:", id.0);
        let sub_prefix     = format!("{}:sub-item:", id.0);
        let hovered = coord.hovered_widget().map(|w| w.0.clone());
        match hovered {
            Some(h) if h.starts_with(&submenu_prefix) && !h.starts_with(&chev_prefix) => {
                let rest = &h[submenu_prefix.len()..];
                state.submenu_open = Some(rest.to_string());
            }
            Some(h) if h.starts_with(&chev_prefix) => {
                // Chevron hover — leave submenu state untouched; click
                // toggles via dispatcher.
            }
            Some(h) if h.starts_with(&sub_prefix) => {
                // Inside the open submenu panel — keep it.
            }
            Some(h) if h.starts_with(&main_prefix) => {
                // Hovered a regular item — close any open submenu.
                state.submenu_open = None;
            }
            _ => {}
        }
    }

    register_context_manager_dropdown(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
    Some(DropdownNode(node_id))
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
