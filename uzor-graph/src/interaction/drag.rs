//! Bare node drag/pin lifecycle — tracks at most one anchor node while a
//! drag gesture is in progress.
//!
//! **Graph-strengthening arc, Wave G4** — this module used to build a
//! genuine `WidgetResponse` via uzor's `InputState`/`create_response`
//! machinery on every single drag frame (`start`/`update`), framed as
//! "reusing uzor's real drag model instead of reinventing it." In
//! practice the response was computed and then thrown away: the real
//! drag math (`GraphEngine::apply_drag_shift`) always reprojected the
//! cursor's ABSOLUTE screen position through `Camera2D::screen_to_world`
//! directly, never read `response.drag_delta`/`drag_total`, and
//! `stop()`'s own returned `WidgetResponse` half was discarded at its one
//! call site (`if let Some((anchor, _response)) = self.drag.stop()`).
//!
//! Deleted rather than wired in — a delta-based `WidgetResponse` doesn't
//! actually serve this controller's need any better. Group-drag's
//! per-member `offset_from_anchor` (`engine.rs`'s `DragMember`) is a
//! FIXED offset added to the anchor's CURRENT absolute world position
//! every tick, not a running delta — so even if `drag_delta` were
//! consumed, it would have to be converted right back into an absolute
//! world-space reprojection to be useful here, at which point it adds a
//! second, redundant math path over calling `screen_to_world` once,
//! not a simplification. `DragController`/`create_response` had zero
//! outside callers (grep-confirmed across the workspace — `uzor-examples`
//! never references either), so nothing beyond this crate's own
//! `engine.rs` is affected.
//!
//! What's left is exactly the state `on_pointer_up` actually needs:
//! which node (if any) is the current drag's anchor, so it can be
//! selected/pinned on release. `PointerMode::DraggingNode` (`engine.rs`)
//! is a unit variant that carries no node of its own — this is the one
//! place that identity lives.

use crate::graph::NodeIndex;

/// Which node (if any) is the current drag's anchor.
#[derive(Default)]
pub struct DragController {
    node: Option<NodeIndex>,
}

impl DragController {
    pub fn dragging_node(&self) -> Option<NodeIndex> {
        self.node
    }

    /// Begin dragging `node`.
    pub fn start(&mut self, node: NodeIndex) {
        self.node = Some(node);
    }

    /// End the drag. Returns the released node, or `None` if nothing
    /// was being dragged.
    pub fn stop(&mut self) -> Option<NodeIndex> {
        self.node.take()
    }
}
