//! Modal persistent state.

use crate::types::{Rect, ScrollState};
use super::super::resize_drag::ResizeDrag;

/// All per-modal frame state.
///
/// Fields are stored flat (no per-template sub-enums) so the caller can hold
/// a single `ModalState` regardless of which `ModalRenderKind` is in use.
/// Fields irrelevant to the active kind are simply never touched.
#[derive(Debug, Default, Clone)]
pub struct ModalState {
    // --- Position & drag (all draggable modals) ---

    /// Modal top-left position.  `(0.0, 0.0)` = use caller-supplied `rect` as-is.
    ///
    /// Set to a non-zero value when the user drags the modal.
    pub position: (f64, f64),

    /// Whether a drag gesture is currently in progress.
    pub dragging: bool,

    /// Cursor position relative to the modal origin recorded at drag start.
    pub drag_offset: (f64, f64),

    // --- Tab navigation (TopTabs, SideTabs) ---

    /// Index of the currently selected tab (0-based).
    pub active_tab: usize,

    /// Index of the tab the pointer is currently hovering over.
    pub hovered_tab: Option<usize>,

    // --- Wizard pagination ---

    /// Index of the currently displayed wizard page (0-based).
    pub current_page: usize,

    // --- Scroll ---

    /// Shared scroll state for single-body modals.
    pub scroll: ScrollState,

    // --- Close button hover ---

    /// Whether the pointer is over the close-X button this frame.
    pub hovered_close: bool,

    /// Index of the footer button the pointer is hovering over (0-based).
    pub footer_hovered: Option<usize>,

    // --- Resize ---

    /// Resize drag in progress. Set by `start_resize`, consumed by
    /// `update_resize`, cleared on `end_resize` (called from
    /// `on_mouse_up` / `end_drag`).
    pub resize_drag: Option<ResizeDrag>,

    /// User-resized override for the modal frame. `None` = use the
    /// caller-supplied measured rect. `Some(rect)` = composite returns
    /// this rect from `effective_rect()`.
    pub resized_rect: Option<Rect>,
}

impl ModalState {
    /// Begin a drag gesture at `cursor_pos` with the modal currently at `modal_origin`.
    pub fn start_drag(&mut self, cursor_pos: (f64, f64), modal_origin: (f64, f64)) {
        self.dragging = true;
        self.drag_offset = (
            cursor_pos.0 - modal_origin.0,
            cursor_pos.1 - modal_origin.1,
        );
    }

    /// Update modal position while dragging.
    ///
    /// `cursor_pos` — current pointer position.
    /// `screen_size` — `(width, height)` used to clamp the modal inside the viewport.
    /// `modal_size` — `(width, height)` of the modal frame.
    pub fn update_drag(
        &mut self,
        cursor_pos: (f64, f64),
        screen_size: (f64, f64),
        modal_size: (f64, f64),
    ) {
        if !self.dragging {
            return;
        }
        let x = (cursor_pos.0 - self.drag_offset.0)
            .clamp(0.0, (screen_size.0 - modal_size.0).max(0.0));
        let y = (cursor_pos.1 - self.drag_offset.1)
            .clamp(0.0, (screen_size.1 - modal_size.1).max(0.0));
        self.position = (x, y);
    }

    /// End the current drag gesture.
    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// Switch the active tab and reset per-tab scroll.
    pub fn set_active_tab(&mut self, index: usize) {
        self.active_tab = index;
        self.scroll.reset();
    }

    /// Advance to the next wizard page (saturates at `page_count - 1`).
    pub fn next_page(&mut self, page_count: usize) {
        if page_count > 0 {
            self.current_page = (self.current_page + 1).min(page_count.saturating_sub(1));
        }
    }

    /// Go back to the previous wizard page (saturates at 0).
    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }

    /// Begin a resize drag from a `ResizeHandleDragStarted` event.
    /// `start_rect` — the current frame rect, `cursor` — pointer at mouse-down.
    /// `min` / `cap` — width/height bounds.
    pub fn start_resize(
        &mut self,
        edge:       crate::layout::ResizeEdge,
        start_rect: Rect,
        cursor:     (f64, f64),
        min:        (f64, f64),
        cap:        (f64, f64),
    ) {
        self.resize_drag = Some(ResizeDrag::begin(edge, start_rect, cursor, min, cap));
    }

    /// Update the resized rect from a fresh cursor position. No-op when
    /// no resize drag is in progress. Writes the resolved rect to
    /// `resized_rect` and the resolved origin to `position` so the modal
    /// composite picks them up next frame.
    pub fn update_resize(&mut self, cursor: (f64, f64)) {
        if let Some(drag) = self.resize_drag {
            let rect = drag.resolve(cursor);
            self.resized_rect = Some(rect);
            self.position = (rect.x, rect.y);
        }
    }

    /// End any active resize drag (call from `on_mouse_up`).
    pub fn end_resize(&mut self) {
        self.resize_drag = None;
    }
}
