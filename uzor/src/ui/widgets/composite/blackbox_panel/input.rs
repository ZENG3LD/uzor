//! BlackboxPanel input helpers.
//!
//! Re-exports `register_input_coordinator_blackbox_panel` and provides
//! `dispatch_blackbox_event` for converting screen-space coordinates to
//! panel-local space before forwarding to the caller's handler closure.

pub use super::render::register_input_coordinator_blackbox_panel;

use crate::types::Rect;

use super::types::{BlackboxEvent, BlackboxEventResult, BlackboxView};

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
