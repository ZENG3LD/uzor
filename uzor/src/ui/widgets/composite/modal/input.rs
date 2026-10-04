//! Modal input-coordinator helpers (level 1).
//!
//! `register_input_coordinator_modal` lives in `render.rs` next to the layout
//! math it shares with painting. This module re-exports it and keeps click
//! dispatch plus drag application. Event consumption lives in [`super::consume`]
//! and is re-exported here so existing `input::consume_event` paths keep compiling.

pub use super::consume::{consume_event, drag_outcome_modal, ConsumeEventCtx};
pub use super::render::register_input_coordinator_modal;

use super::state::ModalState;
use crate::layout::{ClickDispatcher, EventBuilder, ModalHandle};
use crate::types::{Rect, WidgetId};

/// Register a modal's click patterns (close, footer, tabs, wizard, body
/// scrollbar, chevrons and — when `resizable` — the eight resize handles)
/// into `dispatcher`, so clicks on its parts surface as typed
/// [`DispatchEvent`]s. An engine that owns the modal state (the framework
/// overlay stack, or any other caller) registers these patterns itself.
pub fn register_modal_dispatch(
    dispatcher: &mut ClickDispatcher,
    handle: &ModalHandle,
    resizable: bool,
) {
    let id: &WidgetId = &handle.id;
    dispatcher.on_exact(
        format!("{}:close", id.0),
        EventBuilder::ModalClose {
            handle: handle.clone(),
        },
    );
    // Footer buttons close the modal by default — same semantics as the X.
    dispatcher.on_prefix(
        format!("{}:footer:", id.0),
        EventBuilder::ModalClose {
            handle: handle.clone(),
        },
    );
    dispatcher.on_prefix(
        format!("{}:tab:", id.0),
        EventBuilder::ModalTabFromSuffix {
            handle: handle.clone(),
        },
    );
    dispatcher.on_exact(
        format!("{}:wizard:next", id.0),
        EventBuilder::ModalWizardNext {
            handle: handle.clone(),
        },
    );
    dispatcher.on_exact(
        format!("{}:wizard:back", id.0),
        EventBuilder::ModalWizardBack {
            handle: handle.clone(),
        },
    );

    // Body overflow dispatcher routing — both chevron and scrollbar routes
    // are registered unconditionally; the active guard is chosen per frame
    // inside register_body_overflow based on view.overflow.
    dispatcher.on_exact(
        format!("{}:scrollbar_track", id.0),
        EventBuilder::ScrollbarTrack {
            track_id: WidgetId::new(format!("{}:scrollbar_track", id.0)),
        },
    );
    dispatcher.on_exact(
        format!("{}:scrollbar_handle", id.0),
        EventBuilder::ScrollbarThumb {
            thumb_id: WidgetId::new(format!("{}:scrollbar_handle", id.0)),
        },
    );
    {
        use crate::layout::ChevronStepDirection;
        for (suffix, dir) in [
            ("chevron_up", ChevronStepDirection::Up),
            ("chevron_down", ChevronStepDirection::Down),
            ("chevron_left", ChevronStepDirection::Left),
            ("chevron_right", ChevronStepDirection::Right),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ChevronStep {
                    chevron_id: WidgetId::new(format!("{}:{}", id.0, suffix)),
                    direction: dir,
                },
            );
        }
    }
    // Resize handles (8): N S W E + NW NE SW SE.
    if resizable {
        use crate::layout::ResizeEdge;
        for (suffix, edge) in &[
            ("resize_n", ResizeEdge::N),
            ("resize_s", ResizeEdge::S),
            ("resize_w", ResizeEdge::W),
            ("resize_e", ResizeEdge::E),
            ("resize_nw", ResizeEdge::NW),
            ("resize_ne", ResizeEdge::NE),
            ("resize_sw", ResizeEdge::SW),
            ("resize_se", ResizeEdge::SE),
        ] {
            dispatcher.on_exact(
                format!("{}:{}", id.0, suffix),
                EventBuilder::ResizeHandle {
                    host_id: id.clone(),
                    edge: *edge,
                },
            );
        }
    }
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
    state: &mut ModalState,
    cursor_pos: (f64, f64),
    screen_size: (f64, f64),
    modal_size: (f64, f64),
) {
    state.update_drag(cursor_pos, screen_size, modal_size);
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
    modal_rect: Rect,
    px: f64,
    py: f64,
    header_height: f64,
    close_btn_width: f64,
) -> bool {
    px >= modal_rect.x
        && px <= modal_rect.x + modal_rect.width - close_btn_width
        && py >= modal_rect.y
        && py <= modal_rect.y + header_height
}
