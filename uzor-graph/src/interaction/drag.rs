//! Node drag/pin lifecycle built on uzor's native `WidgetResponse` drag
//! model (`drag_started`/`dragged`/`drag_total`, `Sense`) — NOT a
//! bespoke `bool dragging; (f64, f64) drag_last; f64 drag_total` machine
//! (the rejected MVP's actual pattern).
//!
//! The graph canvas itself is driven by raw `PlatformEvent`s dispatched
//! through `App::on_event` — uzor's own documented escape hatch for
//! canvas bodies too free-form for the widget tree (hundreds of moving
//! node positions can't each be a registered `InputCoordinator` widget).
//! But the *drag bookkeeping* reuses uzor's real `InputState`/
//! `DragState`/`create_response` machinery instead of reinventing it:
//! [`DragController`] feeds raw pointer positions into a genuine
//! `InputState` and lets `create_response` derive a genuine
//! `WidgetResponse` from it, the same function every native uzor widget
//! uses.

use uzor::input::{create_response, InputState, MouseButton, PointerDragState, Sense, WidgetResponse};
use uzor::types::{Rect, WidgetId};

use crate::graph::NodeIndex;

/// Drag/pin lifecycle for at most one node at a time.
pub struct DragController {
    node: Option<NodeIndex>,
    input: InputState,
    response: WidgetResponse,
}

impl Default for DragController {
    fn default() -> Self {
        Self {
            node: None,
            input: InputState::new(),
            response: WidgetResponse::default(),
        }
    }
}

impl DragController {
    pub fn dragging_node(&self) -> Option<NodeIndex> {
        self.node
    }

    pub fn response(&self) -> &WidgetResponse {
        &self.response
    }

    /// Begin dragging `node`, pointer currently at `screen_pos`.
    pub fn start(&mut self, node: NodeIndex, screen_pos: (f64, f64)) {
        self.node = Some(node);
        self.input = InputState::new();
        self.input.pointer.pos = Some(screen_pos);
        self.input.pointer.button_down = Some(MouseButton::Left);
        self.input.drag = Some(PointerDragState::new(screen_pos, screen_pos, MouseButton::Left));

        self.response = create_response(
            WidgetId::from("graph-node-drag"),
            Rect::new(0.0, 0.0, 0.0, 0.0),
            Sense::CLICK_AND_DRAG,
            &self.input,
            false,
            false,
        );
        self.response.drag_started = true;
        self.response.dragged = true;
    }

    /// Update the in-progress drag with the pointer's new screen
    /// position. No-op if nothing is being dragged.
    pub fn update(&mut self, screen_pos: (f64, f64)) {
        if self.node.is_none() {
            return;
        }
        self.input.pointer.prev_pos = self.input.pointer.pos;
        self.input.pointer.pos = Some(screen_pos);
        if let Some(drag) = self.input.drag.as_mut() {
            drag.update(screen_pos.0, screen_pos.1);
        }

        self.response = create_response(
            WidgetId::from("graph-node-drag"),
            Rect::new(0.0, 0.0, 0.0, 0.0),
            Sense::CLICK_AND_DRAG,
            &self.input,
            true,
            false,
        );
        if let Some(drag) = &self.input.drag {
            self.response.dragged = true;
            self.response.drag_delta = drag.delta;
            self.response.drag_total = drag.total_delta;
        }
    }

    /// End the drag. Returns the released node and the final
    /// `WidgetResponse` (`drag_stopped = true`), or `None` if nothing
    /// was being dragged.
    pub fn stop(&mut self) -> Option<(NodeIndex, WidgetResponse)> {
        let node = self.node.take()?;
        self.response.dragged = false;
        self.response.drag_stopped = true;
        self.input.drag = None;
        Some((node, self.response.clone()))
    }
}
