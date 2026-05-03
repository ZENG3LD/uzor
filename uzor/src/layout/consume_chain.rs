//! Composite event consume chain helper.
//!
//! [`consume_event_chain`] replaces the per-frame `try_consume!` macro chain
//! in L3 host code with a single call.  The caller builds a `Vec<Box<dyn
//! CompositeConsumer>>` of closures — each closure captures a mutable borrow
//! of one composite's state AND its own frame_rect — and the function drives
//! the event through them in z-order until one composite consumes it.
//!
//! ## L3 usage
//!
//! ```rust,ignore
//! use uzor::layout::{consume_event_chain, CompositeConsumer};
//!
//! let modal_rect   = app.layout.rect_for_overlay("modal-overlay");
//! let sidebar_rect = app.layout.rect_for_edge_slot("sidebar");
//!
//! let consumed = consume_event_chain(event, cursor, viewport, &mut [
//!     &mut |ev: DispatchEvent| modal_input::consume_event(ev, &mut app.modal_state,
//!         &WidgetId::new("modal-widget"),
//!         modal_input::ConsumeEventCtx { cursor, frame_rect: modal_rect.unwrap_or_default(), viewport }),
//!     &mut |ev: DispatchEvent| sidebar_input::consume_event(ev, &mut app.sidebar_state,
//!         &WidgetId::new("sidebar-widget"),
//!         sidebar_input::ConsumeEventCtx { cursor, frame_rect: sidebar_rect.unwrap_or_default(), viewport }),
//! ]);
//! ```
//!
//! Each closure captures its own `frame_rect` (resolved before the chain call).

use super::dispatcher::DispatchEvent;

/// Drive `event` through `consumers` in order, stopping at the first consumer
/// that handles (returns `None`).
///
/// Returns:
/// - `None`        — event was consumed by a consumer.
/// - `Some(event)` — no consumer handled the event; passes it back.
///
/// `consumers` is a slice of mutable references to closures, each of which
/// takes a `DispatchEvent` and returns `Option<DispatchEvent>`.  Closures
/// capture their own `frame_rect`, `cursor`, and `viewport` from the
/// enclosing scope.
///
/// # Why not a trait object Vec?
///
/// Using `&mut [&mut dyn FnMut(DispatchEvent) -> Option<DispatchEvent>]`
/// avoids heap allocation and works with Rust's borrow checker: each closure
/// captures a mutable borrow of one composite's state, and all closures
/// coexist in the same borrow scope because they each capture *different*
/// state fields.
pub fn consume_event_chain(
    event:     DispatchEvent,
    consumers: &mut [&mut dyn FnMut(DispatchEvent) -> Option<DispatchEvent>],
) -> Option<DispatchEvent> {
    let mut opt_ev = Some(event);
    for consumer in consumers.iter_mut() {
        let ev = match opt_ev.take() {
            Some(e) => e,
            None    => return None,
        };
        opt_ev = consumer(ev);
    }
    opt_ev
}
