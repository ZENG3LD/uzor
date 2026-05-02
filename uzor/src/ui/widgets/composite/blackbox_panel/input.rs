//! BlackboxPanel input helpers.
//!
//! Re-exports `register_input_coordinator_blackbox_panel` and provides
//! `dispatch_blackbox_event` for converting screen-space coordinates to
//! panel-local space before forwarding to the caller's handler closure.

pub use super::render::register_input_coordinator_blackbox_panel;

use super::render::register_context_manager_blackbox_panel;

use super::settings::BlackboxPanelSettings;
use super::state::BlackboxState;
use super::types::{BlackboxEvent, BlackboxEventResult, BlackboxHandler, BlackboxRenderKind, BlackboxView};
use crate::docking::panels::DockPanel;
use crate::input::WidgetKind;
use crate::layout::{BlackboxPanelNode, LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

/// Register + draw a blackbox panel in one call using a [`LayoutManager`].
///
/// Resolves the rect from the dock leaf identified by `slot_id`, then
/// forwards to [`register_context_manager_blackbox_panel`].  Returns `None` if
/// the leaf is not present in the panel tree.
pub fn register_layout_manager_blackbox_panel<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    id:       impl Into<WidgetId>,
    state:    &mut BlackboxState,
    view:     &mut BlackboxView<'_>,
    settings: &BlackboxPanelSettings,
    kind:     &BlackboxRenderKind,
) -> Option<BlackboxPanelNode> {
    let id: WidgetId = id.into();
    let rect = layout.rect_for(slot_id)?;
    let layer = layout.compute_layer_for(parent);
    let sense = view.sense;
    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::BlackboxPanel, rect, sense });

    // Auto-forward pointer events into the blackbox view ONLY when this
    // blackbox is the top-most widget under the cursor. Higher layers
    // (dropdown / popup / modal) shadow this and forwarding is
    // suppressed — events can't bleed through to the panel underneath.
    {
        use super::types::BlackboxEvent;
        let coord = &layout.ctx_mut().input;
        let is_top = coord.hovered_widget() == Some(&id);
        let pos    = coord.pointer_pos();
        if is_top {
            if let Some((mx, my)) = pos {
                let _ = dispatch_blackbox_event(
                    view, rect, mx, my,
                    BlackboxEvent::PointerMove { local_x: 0.0, local_y: 0.0 },
                );
            }
        }
    }

    register_context_manager_blackbox_panel(
        layout.ctx_mut(), render, id, rect, state, view, settings, kind, &layer,
    );
    Some(BlackboxPanelNode(node_id))
}

// ---------------------------------------------------------------------------
// Event dispatch
// ---------------------------------------------------------------------------

/// Convert a screen-space pointer position to panel-local coordinates and
/// forward the event to `view.handle_event`.
///
/// # Parameters
///
/// - `view`        — mutable view (handle_event is FnMut)
/// - `body_rect`   — the body rect in screen space (origin used for local conversion)
/// - `screen_x`    — pointer x in screen space
/// - `screen_y`    — pointer y in screen space
/// - `event`       — the event to dispatch (coordinates already converted to local)
///
/// For events that carry coordinates (`PointerMove`, `PointerDown`, `PointerUp`),
/// use the `screen_x`/`screen_y` parameters — the function subtracts `body_rect`
/// origin and injects local coordinates into the event variant before dispatching.
///
/// For non-coordinate events (`Wheel`, `KeyPress`, `Focus`, `PointerEnter`,
/// `PointerLeave`), pass `screen_x = 0.0, screen_y = 0.0` — they are ignored.
pub fn dispatch_blackbox_event(
    view:      &mut BlackboxView<'_>,
    body_rect: Rect,
    screen_x:  f64,
    screen_y:  f64,
    event:     BlackboxEvent,
) -> BlackboxEventResult {
    let local_x = screen_x - body_rect.x;
    let local_y = screen_y - body_rect.y;

    // Inject local coordinates into pointer events.
    let local_event = match event {
        BlackboxEvent::PointerMove { .. } =>
            BlackboxEvent::PointerMove { local_x, local_y },
        BlackboxEvent::PointerDown { button, .. } =>
            BlackboxEvent::PointerDown { local_x, local_y, button },
        BlackboxEvent::PointerUp { button, .. } =>
            BlackboxEvent::PointerUp { local_x, local_y, button },
        other => other,
    };

    (view.handle_event)(local_event)
}

/// Convert a screen-space pointer position to panel-local coordinates and
/// dispatch the event to the handler trait object directly. Use this on
/// the bridge side (winit event callback) for zero-lag sync dispatch:
/// the host routes `(widget_id, event)` to the right `&mut dyn BlackboxHandler`
/// (typically by looking up the panel id in a host-owned registry) and
/// calls this helper.
///
/// Mirrors the local-coordinate conversion logic of `dispatch_blackbox_event`
/// but dispatches via the trait instead of through a `BlackboxView` closure.
pub fn dispatch_to_handler(
    handler:   &mut dyn BlackboxHandler,
    body_rect: Rect,
    screen_x:  f64,
    screen_y:  f64,
    event:     BlackboxEvent,
) -> BlackboxEventResult {
    let local_x = screen_x - body_rect.x;
    let local_y = screen_y - body_rect.y;
    let local_event = match event {
        BlackboxEvent::PointerMove { .. } =>
            BlackboxEvent::PointerMove { local_x, local_y },
        BlackboxEvent::PointerDown { button, .. } =>
            BlackboxEvent::PointerDown { local_x, local_y, button },
        BlackboxEvent::PointerUp { button, .. } =>
            BlackboxEvent::PointerUp { local_x, local_y, button },
        other => other,
    };
    handler.handle_event(local_event)
}
