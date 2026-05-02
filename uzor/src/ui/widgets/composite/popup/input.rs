//! Popup input-coordinator helpers.

pub use super::render::register_input_coordinator_popup;

use super::render::register_context_manager_popup;

use super::settings::PopupSettings;
use super::state::PopupState;
use super::types::{PopupRenderKind, PopupView};
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{DismissFrame, DispatchEvent, EventBuilder, LayoutManager, LayoutNodeId, PopupNode, WidgetNode};
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

/// Register + draw a popup in one call using a [`LayoutManager`].
///
/// Resolves the rect from the overlay slot identified by `slot_id`, then
/// pushes the popup layer onto the coordinator and forwards to
/// [`register_context_manager_popup`].  Returns `None` if the slot is not
/// present in the overlay stack.
pub fn register_layout_manager_popup<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut PopupState,
    view:     &mut PopupView<'_>,
    settings: &PopupSettings,
    kind:     PopupRenderKind,
) -> Option<PopupNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for_overlay(slot_id)?;
    let layer = LayerId::popup();
    let z_order = layout.z_layers().popup as u32;
    // Register this overlay for outside-click dismiss resolution.
    layout.push_dismiss_frame(DismissFrame {
        z: z_order,
        rect,
        overlay_id: WidgetId::new(slot_id),
    });
    // Popup blocks lower layers when open — push the layer so the coordinator
    // can apply the modal-blocking hit-test rule.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Popup, rect, sense: Sense::CLICK });

    // Body overflow patterns + opt-in resize handles.
    let dispatcher = layout.dispatcher_mut();
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
        dispatcher.on_exact(
            format!("{}:chevron_up", id.0),
            EventBuilder::ChevronStep {
                chevron_id: WidgetId::new(format!("{}:chevron_up", id.0)),
                direction:  ChevronStepDirection::Up,
            },
        );
        dispatcher.on_exact(
            format!("{}:chevron_down", id.0),
            EventBuilder::ChevronStep {
                chevron_id: WidgetId::new(format!("{}:chevron_down", id.0)),
                direction:  ChevronStepDirection::Down,
            },
        );
    }

    register_context_manager_popup(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
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
