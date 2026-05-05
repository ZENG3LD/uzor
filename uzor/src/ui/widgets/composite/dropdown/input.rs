//! Dropdown input-coordinator helpers.
//!
//! Re-exports `register_input_coordinator_dropdown` from `render.rs` and adds
//! click-outside dismiss + keyboard navigation helpers.

pub use super::render::register_input_coordinator_dropdown;

use super::render::{measure_flat, register_context_manager_dropdown};

use super::settings::DropdownSettings;
use super::state::DropdownState;
use super::types::{DropdownItem, DropdownRenderKind, DropdownView, DropdownViewKind, SubmenuWidth};
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{CompositeKind, CompositeRegistration, DismissFrame, DispatchEvent, DropdownHandle, DropdownNode, EventBuilder, LayoutManager, LayoutNodeId, OverlayEntry, OverlayKind, WidgetNode};
use crate::render::RenderContext;
use crate::types::{OverflowMode, Rect, SizeMode, WidgetId};

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
        DispatchEvent::DropdownSubmenuToggle { ref dropdown, ref trigger_id } => {
            if dropdown.id == *host_id {
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
/// Pushes the overlay entry, then registers the dropdown layer with the
/// coordinator and forwards to [`register_context_manager_dropdown`].
///
/// `slot_id`      — stable overlay id (e.g. `"dd-file-overlay"`).
/// `overlay_rect` — screen-space rect of the dropdown panel this frame.
/// `anchor`       — optional anchor rect (trigger button) for positioning.
pub fn register_layout_manager_dropdown<P: DockPanel>(
    layout:       &mut LayoutManager<P>,
    render:       &mut dyn RenderContext,
    parent:       LayoutNodeId,
    slot_id:      &str,
    handle:       &DropdownHandle,
    overlay_rect: Rect,
    anchor:       Option<Rect>,
    view:         &mut DropdownView<'_>,
    settings:     &DropdownSettings,
    kind:         DropdownRenderKind,
) -> Option<DropdownNode> {
    let id: WidgetId = handle.id.clone();

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.dropdowns_map_mut().remove(&id).unwrap_or_default();

    layout.push_overlay(OverlayEntry {
        id:   slot_id.to_string(),
        kind: OverlayKind::Dropdown,
        rect: overlay_rect,
        anchor,
    });
    let rect = overlay_rect;
    let layer = LayerId::new("dropdown");
    let z_order = layout.z_layers().dropdown as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect,
        overlay_id: WidgetId(slot_id.to_owned()),
    });
    // Dropdown blocks lower layers — push the layer into the coordinator.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Dropdown, rect, sense: Sense::CLICK, label: None });

    // Register dispatch patterns: clicks on items + sub-items both surface
    // as DispatchEvent::DropdownItemClicked { dropdown, item_id }.
    layout.dispatcher_mut().on_prefix(
        format!("{}:item:", id.0),
        EventBuilder::DropdownItem { handle: handle.clone() },
    );
    layout.dispatcher_mut().on_prefix(
        format!("{}:sub-item:", id.0),
        EventBuilder::DropdownItem { handle: handle.clone() },
    );
    // Submenu chevron clicks (only used for SubmenuTrigger::ChevronClick).
    layout.dispatcher_mut().on_prefix(
        format!("{}:chev:submenu:", id.0),
        EventBuilder::DropdownSubmenuToggleFromSuffix { handle: handle.clone() },
    );
    // Body-overflow chevron pager — fires only when the dropdown panel was
    // clipped by the window edge (window guard).  Routes are registered
    // unconditionally; the strips themselves are registered later in
    // register_input_coordinator_dropdown only when the panel is clipped.
    {
        use crate::layout::ChevronStepDirection;
        for (suffix, dir) in [
            ("chevron_up",    ChevronStepDirection::Up),
            ("chevron_down",  ChevronStepDirection::Down),
        ] {
            let cid = WidgetId(format!("{}:{}", id.0, suffix));
            layout.dispatcher_mut().on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep { chevron_id: cid, direction: dir },
            );
        }
    }

    // Auto-forward hovered_id (main panel) and submenu_hovered_id
    // (submenu panel) from the layout manager (L3 authoritative hover source).
    state.sync_flat_hover_from_layout(layout, &id.0);

    // Auto-manage submenu open/close from layout hover state.
    //
    // - Hovering a `:submenu:{id}` row (trigger=Hover) opens it.
    // - Hovering inside the submenu panel keeps it open.
    // - Hovering a *non-submenu* main row closes the submenu.
    // - Hovering a `:submenu-chevron:` row keeps the submenu state alone
    //   (open is driven by *click* on the chevron, dispatched as
    //   DispatchEvent::DropdownSubmenuToggle).
    {
        let main_prefix    = format!("{}:item:", id.0);
        let submenu_prefix = format!("{}:submenu:", id.0);
        let chev_prefix    = format!("{}:chev:submenu:", id.0);
        let sub_prefix     = format!("{}:sub-item:", id.0);
        let hovered = layout.hovered_widget().map(|w| w.0.clone());
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
        layout.ctx_mut(), render, id.clone(), rect, &mut state, view, settings, kind, &layer,
    );

    // Window-edge guard: if the dropdown frame extends past the bottom of
    // the window viewport, register chevron-strip overlays so the user
    // can page through the clipped portion.  Pure overlay — no space
    // reservation, no scroll state needed yet (paging is L4-driven).
    if let Some(win) = layout.last_window() {
        let frame_top    = rect.y;
        let frame_bottom = rect.y + rect.height;
        let win_bottom   = win.y + win.height;
        let win_top      = win.y;
        let clipped_top    = frame_top    < win_top;
        let clipped_bottom = frame_bottom > win_bottom;
        if clipped_top || clipped_bottom {
            use crate::types::CompositeId;
            let composite_id = CompositeId(id.clone());
            let strip_h = 16.0_f64;
            let visible_top    = frame_top.max(win_top);
            let visible_bottom = frame_bottom.min(win_bottom);
            let up_rect = crate::types::Rect::new(rect.x, visible_top, rect.width, strip_h);
            let dn_rect = crate::types::Rect::new(rect.x, visible_bottom - strip_h, rect.width, strip_h);
            // Register hit zones.
            {
                let coord = &mut layout.ctx_mut().input;
                if clipped_top {
                    coord.register_child(
                        &composite_id,
                        format!("{}:chevron_up", id.0),
                        crate::input::WidgetKind::Button,
                        up_rect,
                        crate::input::Sense::CLICK | crate::input::Sense::HOVER,
                    );
                }
                if clipped_bottom {
                    coord.register_child(
                        &composite_id,
                        format!("{}:chevron_down", id.0),
                        crate::input::WidgetKind::Button,
                        dn_rect,
                        crate::input::Sense::CLICK | crate::input::Sense::HOVER,
                    );
                }
            }
            // Paint overlay arrows on top of the dropdown content.
            use crate::ui::widgets::atomic::chevron::{
                draw_chevron, settings::ChevronSettings,
                types::{ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
                        HitAreaPolicy, PlacementPolicy, VisibilityPolicy},
            };
            let bg = settings.theme.as_ref().bg();
            let chev_settings = ChevronSettings::default();
            if clipped_top {
                render.set_fill_color(bg);
                render.fill_rect(up_rect.x, up_rect.y, up_rect.width, up_rect.height);
                let v = ChevronView {
                    direction:   ChevronDirection::Up,
                    use_case:    ChevronUseCase::PixelScrollStep,
                    visibility:  VisibilityPolicy::WhenOverflow { has_more: true },
                    placement:   PlacementPolicy::Overlay,
                    hit_area:    HitAreaPolicy::Visual,
                    visual_kind: ChevronVisualKind::Stroked,
                    ..Default::default()
                };
                draw_chevron(render, up_rect, &v, &chev_settings);
            }
            if clipped_bottom {
                render.set_fill_color(bg);
                render.fill_rect(dn_rect.x, dn_rect.y, dn_rect.width, dn_rect.height);
                let v = ChevronView {
                    direction:   ChevronDirection::Down,
                    use_case:    ChevronUseCase::PixelScrollStep,
                    visibility:  VisibilityPolicy::WhenOverflow { has_more: true },
                    placement:   PlacementPolicy::Overlay,
                    hit_area:    HitAreaPolicy::Visual,
                    visual_kind: ChevronVisualKind::Stroked,
                    ..Default::default()
                };
                draw_chevron(render, dn_rect, &v, &chev_settings);
            }
        }
    }

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::Dropdown,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: rect,
    });

    // Return state to the map.
    layout.dropdowns_map_mut().insert(id, state);

    Some(DropdownNode(node_id))
}

/// Open and draw a simple Flat dropdown in one call.
///
/// This is the common-case helper that covers 5 of the 7 dropdowns in a
/// typical app: File, View, Help, Sidebar, Toolbar, Theme.  It does:
///   1. Peeks at dropdown state in the layout map to see if open.
///   2. Measures panel size via `measure_flat`.
///   3. Builds a `DropdownView` with `DropdownViewKind::Flat` (no submenu).
///   4. Calls `register_layout_manager_dropdown`.
///
/// Only use this for simple flat lists.  Dropdowns with submenu items (e.g.
/// the Popup template picker) need to build `DropdownView` manually.
///
/// Returns `None` when the dropdown state is closed (no-op call).
///
/// # Parameters
///
/// - `slot_id`    — stable overlay id (e.g. `"dd-file-overlay"`).
/// - `widget_id`  — stable widget id (e.g. `"dd-file-widget"`).
/// - `items`      — flat item list; must outlive this call frame.
/// - `settings`   — pass `&DropdownSettings::default()` for stock look.
pub fn open_dropdown_flat<'items, P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    handle:   &DropdownHandle,
    items:    &'items [DropdownItem<'items>],
    settings: &DropdownSettings,
) -> Option<DropdownNode> {
    let widget_id = &handle.id;
    // Peek at state to decide whether to open (do not take it out yet —
    // register_layout_manager_dropdown will do the remove/insert dance).
    let (open, hovered_id, origin, anchor_rect, position_override) = {
        let st = layout.dropdowns_map_mut().get(widget_id).map(|s| (
            s.open,
            s.hovered_id.clone(),
            s.effective_origin(),
            s.anchor_rect,
            s.open_position_override,
        ));
        match st {
            Some(v) => v,
            None    => return None, // no state → not open
        }
    };
    if !open {
        return None;
    }
    let (w, h) = measure_flat(items, settings);
    let mut view = DropdownView {
        anchor:             anchor_rect,
        position_override,
        open:               true,
        kind:               DropdownViewKind::Flat {
            items,
            hovered_id:         hovered_id.as_deref(),
            submenu_items:      None,
            submenu_hovered_id: None,
        },
        size_mode:     SizeMode::AutoFit,
        overflow:      OverflowMode::Clip,
        submenu_width: SubmenuWidth::Auto,
    };
    register_layout_manager_dropdown(
        layout, render, parent,
        slot_id, handle,
        Rect::new(origin.0, origin.1, w, h),
        None,
        &mut view, settings,
        DropdownRenderKind::Flat,
    )
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
