//! Popup input-coordinator helpers.

pub use super::render::register_input_coordinator_popup;

use super::render::register_context_manager_popup;

use super::settings::PopupSettings;
use super::state::PopupState;
use super::types::{PopupRenderKind, PopupView};
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, PopupNode, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

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
    // Popup blocks lower layers when open — push the layer so the coordinator
    // can apply the modal-blocking hit-test rule.
    layout.ctx_mut().input.push_layer(layer.clone(), z_order, true);
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Popup, rect, sense: Sense::CLICK });
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
