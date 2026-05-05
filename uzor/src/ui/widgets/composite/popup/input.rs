//! Popup input-coordinator helpers.

pub use super::render::register_input_coordinator_popup;

use super::render::register_context_manager_popup;

use super::settings::PopupSettings;
use super::state::PopupState;
use super::types::{PopupRenderKind, PopupView};
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{CompositeKind, CompositeRegistration, DismissFrame, DispatchEvent, EventBuilder, LayoutManager, LayoutNodeId, OverlayEntry, OverlayKind, PopupHandle, PopupNode, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Cursor position and view metadata for events that need spatial context
/// (scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the popup this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this popup. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this popup; pass it through.
///
/// `host_id` is the popup composite's WidgetId (e.g. `"popup-widget"`). Only
/// events whose carried id starts with `{host_id}:` are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut PopupState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0);
            if is_own {
                state.body_chevron_step(direction);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarTrackClicked { ref track_id } => {
            if track_id.0 == format!("{}:scrollbar_track", host_id.0) {
                state.body_scroll_track_click(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarThumbDragStarted { ref thumb_id } => {
            if thumb_id.0 == format!("{}:scrollbar_handle", host_id.0) {
                state.start_body_scroll_drag(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Inspect popup state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
pub fn drag_outcome_popup(state: &PopupState) -> Option<crate::layout::DragOutcome> {
    if state.scroll.is_dragging {
        return Some(crate::layout::DragOutcome::PopupBodyScroll);
    }
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::PopupResize);
    }
    None
}

/// Register + draw a popup in one call using a [`LayoutManager`].
///
/// Pushes the overlay entry, then registers the popup layer with the
/// coordinator and forwards to [`register_context_manager_popup`].
///
/// `slot_id`      — stable overlay id (e.g. `"demo-popup-overlay"`).
/// `overlay_rect` — screen-space rect of the popup frame this frame.
/// `anchor`       — optional anchor rect for repositioning logic.
pub fn register_layout_manager_popup<P: DockPanel>(
    layout:       &mut LayoutManager<P>,
    render:       &mut dyn RenderContext,
    parent:       LayoutNodeId,
    slot_id:      &str,
    handle:       &PopupHandle,
    overlay_rect: Rect,
    anchor:       Option<Rect>,
    view:         &mut PopupView<'_>,
    settings:     &PopupSettings,
    kind:         PopupRenderKind,
) -> Option<PopupNode> {
    let id: WidgetId = handle.id.clone();

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.popups_map_mut().remove(&id).unwrap_or_default();

    layout.push_overlay(OverlayEntry {
        id:   slot_id.to_string(),
        kind: OverlayKind::Popup,
        rect: overlay_rect,
        anchor,
    });
    let rect = overlay_rect;
    let layer = LayerId::popup();
    let z_order = layout.z_layers().popup as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect,
        overlay_id: WidgetId(slot_id.to_owned()),
    });
    // Popup blocks lower layers when open — push the layer so the coordinator
    // can apply the modal-blocking hit-test rule.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Popup, rect, sense: Sense::CLICK, label: None });

    // Popup overflow guard — chevrons only (popup auto-sizes; scrollbar /
    // compress are non-applicable). Chevron routing is unconditional so
    // Clip-content-overflow falls back without re-registration.
    {
        use crate::layout::ChevronStepDirection;
        let dispatcher = layout.dispatcher_mut();
        for (suffix, dir) in [
            ("chevron_up",    ChevronStepDirection::Up),
            ("chevron_down",  ChevronStepDirection::Down),
            ("chevron_left",  ChevronStepDirection::Left),
            ("chevron_right", ChevronStepDirection::Right),
        ] {
            let cid = WidgetId(format!("{}:{}", id.0, suffix));
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep { chevron_id: cid, direction: dir },
            );
        }
    }

    register_context_manager_popup(
        layout.ctx_mut(), render, id.clone(), rect, &mut state, view, settings, kind, &layer,
    );

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::Popup,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: rect,
    });

    // Return state to the map.
    layout.popups_map_mut().insert(id, state);

    Some(PopupNode(node_id))
}

/// Returns `true` if `click_pos` is outside the popup rect and the popup
/// should be dismissed.
///
/// Guards drag gestures: if any drag is in progress the popup stays open even
/// if the pointer leaves its bounds (the user may drag the opacity slider
/// outside the frame).
pub fn handle_popup_dismiss(state: &PopupState, click_pos: (f64, f64), popup_rect: Rect) -> bool {
    if state.is_dragging_any() {
        return false;
    }
    !popup_rect.contains(click_pos.0, click_pos.1)
}

// ---------------------------------------------------------------------------
// Grid cell helper
// ---------------------------------------------------------------------------

/// A single cell in a popup color/icon grid.
pub struct PopupGridCell<'a> {
    /// Stable widget id for this cell (e.g. `"demo-popup-cell-0"`).
    pub id: &'a str,
    /// Fill color string (e.g. `"#ef5350"`).
    pub color: &'a str,
}

/// Register and draw a grid of color/swatch cells inside an open popup body.
///
/// Cells are laid out in `cols` columns with `gap` pixels between cells.
/// Each cell is `cell_size × cell_size` pixels, filled with `cell.color` and
/// rounded with radius `4.0`. A white halo is drawn when the cell is hovered.
///
/// All cells are registered as `Button` children of `popup_id` on `layer`.
///
/// # Parameters
///
/// - `layout`    — layout manager (coordinator accessed via `ctx_mut()`).
/// - `render`    — render context.
/// - `popup_id`  — composite id of the parent popup (for `register_child`).
/// - `body_rect` — popup body rect in screen space.
/// - `cells`     — slice of cell descriptors.
/// - `cols`      — number of columns.
/// - `cell_size` — width and height of each cell in pixels.
/// - `gap`       — gap between cells in pixels.
/// - `layer`     — render layer for hit-test registration.
pub fn register_popup_grid<P: DockPanel>(
    layout:    &mut LayoutManager<P>,
    render:    &mut dyn RenderContext,
    popup_id:  &str,
    body_rect: Rect,
    cells:     &[PopupGridCell<'_>],
    cols:      usize,
    cell_size: f64,
    gap:       f64,
) {
    use crate::types::CompositeId;
    let composite_id = CompositeId(WidgetId::new(popup_id));
    let coord = &mut layout.ctx_mut().input;
    for (i, cell) in cells.iter().enumerate() {
        let col = i % cols.max(1);
        let row = i / cols.max(1);
        let cx = body_rect.x + col as f64 * (cell_size + gap);
        let cy = body_rect.y + row as f64 * (cell_size + gap);
        let cell_rect = Rect::new(cx, cy, cell_size, cell_size);
        coord.register_child(
            &composite_id,
            cell.id,
            WidgetKind::Button,
            cell_rect,
            Sense::CLICK | Sense::HOVER,
        );
        // Hover halo
        let hovered = coord.hovered_widget()
            .map(|id| id.0.as_str() == cell.id)
            .unwrap_or(false);
        if hovered {
            render.set_fill_color("#ffffff");
            render.fill_rounded_rect(cx - 2.0, cy - 2.0, cell_size + 4.0, cell_size + 4.0, 5.0);
        }
        render.set_fill_color(cell.color);
        render.fill_rounded_rect(cx, cy, cell_size, cell_size, 4.0);
    }
}
