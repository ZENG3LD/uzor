//! Modal input-coordinator helpers.
//!
//! `register_input_coordinator_modal` is defined in `render.rs` (alongside
//! `register_context_manager_modal`) because both share the layout computation.
//! This module re-exports it and adds the drag helper.

pub use super::render::register_input_coordinator_modal;

use super::render::register_context_manager_modal;

use super::settings::ModalSettings;
use super::state::ModalState;
use super::types::{ModalRenderKind, ModalView};
use crate::input::text::store::TextFieldConfig;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::types::CompositeId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{CompositeKind, CompositeRegistration, DismissFrame, DispatchEvent, EventBuilder, LayoutManager, LayoutNodeId, ModalHandle, ModalNode, OverlayEntry, OverlayKind, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Return the widget id hovered by the pointer when the cursor is inside
/// `body_rect`, or `None` if the cursor is outside the body or no widget
/// is hovered.
///
/// Use this instead of a bespoke geometric hit-test to find which widget
/// inside a modal body the user is hovering.  The coordinator's retained
/// hover state is already up-to-date by the time rendering starts.
///
/// `body_rect` — screen-space content rect of the modal body (header
///               already subtracted, same rect you pass to the render helpers).
pub fn modal_body_hovered_widget<'a, P: DockPanel>(
    layout:    &'a LayoutManager<P>,
    body_rect: Rect,
) -> Option<&'a WidgetId> {
    let (mx, my) = layout.ctx().input.pointer_pos()?;
    if !body_rect.contains(mx, my) {
        return None;
    }
    layout.ctx().input.hovered_widget()
}

/// Cursor position and view metadata for events that need spatial context
/// (resize start, scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the modal this frame (post-drag, post-resize).
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this modal. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this modal; pass it through.
///
/// `host_id` is the modal composite's WidgetId (e.g. `"modal-widget"`). Only
/// events whose carried id starts with `{host_id}:` (or equals `host_id` for
/// resize) are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut ModalState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0)
                || chevron_id.0 == format!("{}:chevron_left", host_id.0)
                || chevron_id.0 == format!("{}:chevron_right", host_id.0);
            if is_own {
                state.body_chevron_step(direction);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, edge } => {
            if hid == host_id {
                let min = (200.0_f64, 120.0_f64);
                let cap = (f64::INFINITY, f64::INFINITY);
                state.start_resize(edge, ctx.frame_rect, ctx.cursor, min, cap);
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

/// Register + draw a modal in one call using a [`LayoutManager`].
///
/// Pushes the overlay entry onto the layout's overlay stack, then registers
/// the modal layer with the coordinator (so it blocks lower layers) and
/// forwards to [`register_context_manager_modal`].
///
/// State is taken from the layout manager's internal `modals` map (keyed by
/// `id`) and created with `Default` if absent — the caller no longer owns or
/// passes `&mut ModalState`.
///
/// `slot_id`      — stable overlay id (e.g. `"modal-overlay"`).  Used for
///                  dismiss-frame identity; must be unique per open overlay.
/// `overlay_rect` — screen-space rect of the modal frame this frame.
/// `anchor`       — optional anchor rect (e.g. trigger button) for
///                  repositioning logic.
pub fn register_layout_manager_modal<P: DockPanel>(
    layout:       &mut LayoutManager<P>,
    render:       &mut dyn RenderContext,
    parent:       LayoutNodeId,
    slot_id:      &str,
    handle:       &ModalHandle,
    overlay_rect: Rect,
    anchor:       Option<Rect>,
    view:         &mut ModalView<'_>,
    settings:     &ModalSettings,
    kind:         &ModalRenderKind,
) -> Option<ModalNode> {
    let id: WidgetId = handle.id.clone();

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.modals.remove(&id).unwrap_or_default();

    // Push the overlay entry so rect_for_overlay and dismiss resolution work.
    layout.push_overlay(OverlayEntry {
        id:   slot_id.to_string(),
        kind: OverlayKind::Modal,
        rect: overlay_rect,
        anchor,
    });
    let rect = overlay_rect;
    let layer = LayerId::modal();
    let z_order = layout.z_layers().modal as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect,
        overlay_id: WidgetId(slot_id.to_owned()),
    });
    // Push the modal layer so that the coordinator's hit-test blocks lower layers.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Modal, rect, sense: Sense::CLICK });

    // Register dispatcher patterns so the app gets semantic events instead of
    // raw "modal-widget:close" string matching.
    let dispatcher = layout.dispatcher_mut();
    dispatcher.on_exact(
        format!("{}:close", id.0),
        EventBuilder::ModalClose { handle: handle.clone() },
    );
    // Footer buttons close the modal by default — same semantics as the X.
    dispatcher.on_prefix(
        format!("{}:footer:", id.0),
        EventBuilder::ModalClose { handle: handle.clone() },
    );
    dispatcher.on_prefix(
        format!("{}:tab:", id.0),
        EventBuilder::ModalTabFromSuffix { handle: handle.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:next", id.0),
        EventBuilder::ModalWizardNext { handle: handle.clone() },
    );
    dispatcher.on_exact(
        format!("{}:wizard:back", id.0),
        EventBuilder::ModalWizardBack { handle: handle.clone() },
    );

    // Body overflow patterns (Scrollbar / Chevrons) and resize handles.
    if matches!(view.overflow, crate::types::OverflowMode::Scrollbar) {
        dispatcher.on_exact(
            format!("{}:scrollbar_track", id.0),
            EventBuilder::ScrollbarTrack { track_id: WidgetId::new(format!("{}:scrollbar_track", id.0)) },
        );
        dispatcher.on_exact(
            format!("{}:scrollbar_handle", id.0),
            EventBuilder::ScrollbarThumb { thumb_id: WidgetId::new(format!("{}:scrollbar_handle", id.0)) },
        );
    }
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        use crate::layout::ChevronStepDirection;
        for (suffix, dir) in [
            ("chevron_up",    ChevronStepDirection::Up),
            ("chevron_down",  ChevronStepDirection::Down),
            ("chevron_left",  ChevronStepDirection::Left),
            ("chevron_right", ChevronStepDirection::Right),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep {
                    chevron_id: WidgetId::new(format!("{}:{}", id.0, suffix)),
                    direction:  dir,
                },
            );
        }
    }
    // Resize handles (8): N S W E + NW NE SW SE.
    if view.resizable {
        use crate::layout::ResizeEdge;
        for (suffix, edge) in &[
            ("resize_n",  ResizeEdge::N),
            ("resize_s",  ResizeEdge::S),
            ("resize_w",  ResizeEdge::W),
            ("resize_e",  ResizeEdge::E),
            ("resize_nw", ResizeEdge::NW),
            ("resize_ne", ResizeEdge::NE),
            ("resize_sw", ResizeEdge::SW),
            ("resize_se", ResizeEdge::SE),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ResizeHandle { host_id: id.clone(), edge: *edge },
            );
        }
    }

    register_context_manager_modal(
        layout.ctx_mut(), render, id.clone(), rect, &mut state, view, settings, kind, &layer,
    );

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::Modal,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: rect,
    });

    // Return state to the map.
    layout.modals.insert(id, state);

    Some(ModalNode(node_id))
}

/// Inspect modal state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
///
/// Call immediately after a successful consume. Returns `None` if no drag was
/// started (e.g. the event was a click, not a drag-start).
pub fn drag_outcome_modal(state: &ModalState) -> Option<crate::layout::DragOutcome> {
    if state.scroll.is_dragging {
        return Some(crate::layout::DragOutcome::ModalBodyScroll);
    }
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::ModalResize);
    }
    None
}

/// Apply a drag delta to modal state.
///
/// Call this in your pointer-move handler when the drag-handle widget reports
/// a drag gesture (`state.dragging` is `true`).
///
/// `cursor_pos`  — current pointer position in screen coordinates.
/// `screen_size` — `(width, height)` used to clamp the modal inside the viewport.
/// `modal_size`  — `(width, height)` of the modal frame.
pub fn handle_modal_drag(
    state:       &mut ModalState,
    cursor_pos:  (f64, f64),
    screen_size: (f64, f64),
    modal_size:  (f64, f64),
) {
    state.update_drag(cursor_pos, screen_size, modal_size);
}

/// Register one or more text fields inside a modal body.
///
/// For each `(id, local_rect, config)` entry, computes the screen-space rect
/// by adding the modal frame origin (accounting for drag) and the body header
/// height from `settings`, then registers the field with the input coordinator.
///
/// `body_rect` — rect of the modal body in screen space (header already
///               subtracted; this is the content area, not the full frame).
///
/// `fields`    — slice of `(field_id, local_rect, config)` where `local_rect`
///               is relative to `body_rect` origin.
pub fn register_modal_text_fields<P: DockPanel>(
    layout:    &mut LayoutManager<P>,
    body_rect: Rect,
    fields:    &[(&str, Rect, TextFieldConfig)],
) {
    let coord = &mut layout.ctx_mut().input;
    for (id, local_rect, config) in fields {
        let screen_rect = Rect::new(
            body_rect.x + local_rect.x,
            body_rect.y + local_rect.y,
            local_rect.width,
            local_rect.height,
        );
        coord.register_text_field(*id, screen_rect, config.clone());
    }
}

/// Register a button inside a modal body as a composite Panel + atomic Button child.
///
/// Some callers need the button to act as a composite host so that sticky
/// chevrons or other child widgets can attach to it.  This helper registers
/// a `Panel` composite (with `Sense::NONE`) and immediately adds the visual
/// `Button` as an atomic child, returning the composite `CompositeId` so the
/// caller can attach further children (e.g. `register_sticky_chevron`).
///
/// `host_id`  — stable id for the composite Panel host (e.g. `"l2-btn-connect-host"`).
/// `child_id` — stable id for the atomic Button child (e.g. `"l2-btn-connect"`).
/// `rect`     — screen-space rect for both host and child (they share the same rect).
/// `sense`    — sense flags for the Button child (typically `CLICK | HOVER`).
/// `layer`    — current render layer.
pub fn register_modal_button<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    host_id:  impl Into<WidgetId>,
    child_id: impl Into<WidgetId>,
    rect:     Rect,
    sense:    Sense,
    layer:    &LayerId,
) -> CompositeId {
    let coord = &mut layout.ctx_mut().input;
    let host = coord.register_composite(host_id, WidgetKind::Panel, rect, Sense::NONE, layer);
    coord.register_child(&host, child_id, WidgetKind::Button, rect, sense);
    host
}

/// Complete the modal two-pass body rendering: paint overflow overlays then
/// re-register overflow hit-zones after app body-content has been drawn.
///
/// Call this AFTER drawing all body widgets (and after calling `render.restore()`
/// to close the body clip). It replaces the explicit
/// `draw_body_overflow_chevrons` + `register_body_overflow` pair.
///
/// The `modal_id` defaults to `"modal-widget"` which is the standard id used
/// by `register_layout_manager_modal`.
pub fn modal_body_finish<P: DockPanel>(
    layout:     &mut LayoutManager<P>,
    render:     &mut dyn RenderContext,
    frame_rect: Rect,
    state:      &mut ModalState,
    view:       &ModalView<'_>,
    settings:   &ModalSettings,
    kind:       &ModalRenderKind,
) {
    use super::render::{draw_body_overflow_chevrons, register_body_overflow};

    // Paint chevron/scrollbar overlays on top of body content.
    draw_body_overflow_chevrons(render, frame_rect, state, view, settings, kind);

    // Re-register overflow hit-zones last so they outrank body widgets.
    let modal_id = CompositeId(WidgetId::new("modal-widget"));
    register_body_overflow(
        &mut layout.ctx_mut().input,
        &modal_id,
        frame_rect,
        view,
        settings,
        kind,
        state,
    );
}

/// Draw the modal body content and register overflow (scrollbar / chevrons) in one call.
///
/// This replaces the two-pass pattern in app code where:
/// 1. `body_fn` draws body widgets and registers them with the coordinator.
/// 2. `draw_body_overflow_chevrons` paints chevron/scrollbar overlays on top.
/// 3. `register_body_overflow` registers the overflow hit zones last so they
///    sit above body widgets in the coordinator's hit-test.
///
/// Using this helper eliminates the need to call steps 2 and 3 manually.
///
/// # Arguments
/// - `layout`    — the LayoutManager (for coord access).
/// - `render`    — the render context.
/// - `frame_rect`— the modal's full screen-space rect (from `rect_for_overlay`).
/// - `state`     — mutable modal state.
/// - `view`      — modal view description.
/// - `settings`  — modal settings.
/// - `kind`      — render kind.
/// - `body_fn`   — closure that draws body widgets. Receives `(render, body_rect)`.
pub fn modal_body_scope<P: DockPanel, F>(
    layout:     &mut LayoutManager<P>,
    render:     &mut dyn RenderContext,
    frame_rect: Rect,
    state:      &mut ModalState,
    view:       &ModalView<'_>,
    settings:   &ModalSettings,
    kind:       &ModalRenderKind,
    body_fn:    F,
) where
    F: FnOnce(&mut dyn RenderContext, Rect),
{
    use super::render::{body_rect as modal_body_rect, draw_body_overflow_chevrons, register_body_overflow};

    let br = modal_body_rect(frame_rect, view, settings, kind);

    // Step 1: let caller draw body content.
    body_fn(render, br);

    // Step 2: paint overflow overlays on top of body content.
    draw_body_overflow_chevrons(render, frame_rect, state, view, settings, kind);

    // Step 3: register overflow hit zones last (outruns body widgets in coord).
    let modal_id = CompositeId(WidgetId::new("modal-widget"));
    register_body_overflow(
        &mut layout.ctx_mut().input,
        &modal_id,
        frame_rect,
        view,
        settings,
        kind,
        state,
    );
}

/// Hit-test whether a pointer position is inside the modal header drag zone.
///
/// Returns `true` when `(px, py)` falls within the header rect:
/// - x: `[modal_rect.x, modal_rect.x + modal_rect.width - close_btn_width]`
/// - y: `[modal_rect.y, modal_rect.y + header_height]`
///
/// `header_height` — height of the header strip in pixels (default: `44.0`).
/// `close_btn_width` — width reserved for the close button on the right
///                     (default: `34.0` = 24 px button + 10 px padding).
pub fn modal_header_hit(
    modal_rect:      Rect,
    px:              f64,
    py:              f64,
    header_height:   f64,
    close_btn_width: f64,
) -> bool {
    px >= modal_rect.x
        && px <= modal_rect.x + modal_rect.width - close_btn_width
        && py >= modal_rect.y
        && py <= modal_rect.y + header_height
}
