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
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
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

// ---------------------------------------------------------------------------
// Stub panel registration
// ---------------------------------------------------------------------------

/// Register a non-blackbox (stub) panel in the coordinator as a `BlackboxPanel`
/// widget without providing a full `BlackboxView` render closure.
///
/// Use this for dock-leaf panels that have their own custom render function
/// and do not need the composite's body closure mechanism.  The panel is
/// registered for hit-testing with full pointer sense (`CLICK | HOVER | DRAG
/// | SCROLL`) so that overlays above it are still correctly shadowed.
///
/// Returns the registered [`WidgetId`] (same as `id`).
///
/// # Parameters
///
/// - `layout`   — the layout manager (coordinator accessed via `ctx_mut()`).
/// - `id`       — stable widget id for this panel.
/// - `rect`     — screen-space rect of the panel.
/// - `layer`    — render layer (typically `LayerId::main()`).
pub fn register_layout_manager_stub_panel<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    id:     impl Into<WidgetId>,
    rect:   Rect,
    layer:  &LayerId,
) -> WidgetId {
    let id: WidgetId = id.into();
    let coord = &mut layout.ctx_mut().input;
    coord.register_composite(
        id.clone(),
        WidgetKind::BlackboxPanel,
        rect,
        Sense::CLICK | Sense::HOVER | Sense::DRAG | Sense::SCROLL,
        layer,
    );
    id
}

// ---------------------------------------------------------------------------
// Sync-dispatch routing helpers
// ---------------------------------------------------------------------------

/// Route a pointer event to the `BlackboxPanel` under the cursor, if any.
///
/// Checks whether the currently-hovered widget (from the coordinator) is a
/// `BlackboxPanel`.  If so, calls `dispatch` with the hovered `WidgetId`,
/// screen coordinates, and event.  The `dispatch` closure is responsible for
/// finding the right handler and calling [`dispatch_to_handler`] itself.
/// Returns `true` when `dispatch` returns `true` (consumed).
///
/// Returns `false` when:
/// - No widget is hovered.
/// - The hovered widget is not a `BlackboxPanel`.
/// - `dispatch` returns `false` (panel not managed here).
///
/// ## Why this pattern?
///
/// Returning a `(&mut dyn BlackboxHandler, Rect)` from a closure causes
/// lifetime/borrow conflicts because the caller typically needs to borrow both
/// `layout` (for the coordinator check) and the handler (owned by `app`).
/// Inverting the control — passing `widget_id` into a closure that owns the
/// handler — avoids the conflict entirely.
///
/// ## Usage
///
/// ```rust,ignore
/// // Resolve panel info before the mutable layout borrow.
/// let watchlist_rect: Option<Rect> = ...;
///
/// let consumed = route_blackbox_pointer_down(
///     &mut app.layout, x, y,
///     BlackboxEvent::PointerDown { local_x: 0.0, local_y: 0.0, button: MouseButton::Left },
///     |widget_id, sx, sy, ev| {
///         if let Some(rect) = watchlist_rect {
///             if widget_id.0 == watchlist_widget_id {
///                 dispatch_to_handler(&mut app.watchlist, rect, sx, sy, ev);
///                 return true;
///             }
///         }
///         false
///     },
/// );
/// ```
pub fn route_blackbox_pointer_down<P, F>(
    layout:   &mut LayoutManager<P>,
    screen_x: f64,
    screen_y: f64,
    event:    BlackboxEvent,
    dispatch: F,
) -> bool
where
    P: DockPanel,
    F: FnOnce(&WidgetId, f64, f64, BlackboxEvent) -> bool,
{
    let top_id = layout.ctx_mut().input.hovered_widget().cloned();
    let top_id = match top_id {
        Some(id) => id,
        None     => return false,
    };
    if layout.ctx_mut().input.widget_kind(&top_id) != Some(WidgetKind::BlackboxPanel) {
        return false;
    }
    dispatch(&top_id, screen_x, screen_y, event)
}

/// Route a wheel event to the `BlackboxPanel` under the cursor, if any.
///
/// Mirrors [`route_blackbox_pointer_down`] but for wheel events.  The
/// `dispatch` closure receives `(widget_id, delta_x, delta_y)` and returns
/// `true` when the event was consumed.
///
/// `delta_x` / `delta_y` are passed directly — no coordinate conversion is
/// needed for wheel events (they carry no spatial origin).
pub fn route_blackbox_wheel<P, F>(
    layout:  &mut LayoutManager<P>,
    delta_x: f64,
    delta_y: f64,
    dispatch: F,
) -> bool
where
    P: DockPanel,
    F: FnOnce(&WidgetId, f64, f64) -> bool,
{
    let top_id = layout.ctx_mut().input.hovered_widget().cloned();
    let top_id = match top_id {
        Some(id) => id,
        None     => return false,
    };
    if layout.ctx_mut().input.widget_kind(&top_id) != Some(WidgetKind::BlackboxPanel) {
        return false;
    }
    dispatch(&top_id, delta_x, delta_y)
}
