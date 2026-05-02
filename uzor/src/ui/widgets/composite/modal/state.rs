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

    // --- Body scroll (Scrollbar / Chevrons overflow modes) ---

    /// Total content size of the body (caller-set every frame).
    pub body_content_h: f64,
    pub body_content_w: f64,

    /// Track rect of the body scrollbar — set by the composite at
    /// register time. Used by `body_scroll_track_click` / drag.
    pub body_scroll_track: Option<Rect>,

    /// Body viewport size — set by the composite at register time.
    pub body_viewport_h: f64,
    pub body_viewport_w: f64,

    /// Horizontal scroll offset (Chevrons / Scrollbar overflow). Vertical
    /// offset lives on `scroll.offset` (legacy).
    pub body_scroll_x: f64,
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

    // --- Body scroll helpers (used by overflow Scrollbar / Chevrons) ---

    /// Begin a scrollbar-thumb drag. Cursor Y at mouse-down.
    pub fn start_body_scroll_drag(&mut self, cursor_y: f64) {
        self.scroll.start_drag(cursor_y);
    }

    /// Update the scrollbar drag from a fresh cursor Y. Uses the
    /// composite-recorded track rect / viewport / content height.
    pub fn update_body_scroll_drag(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_drag(cursor_y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }

    /// Apply a track click — jump scroll to the click position.
    pub fn body_scroll_track_click(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_track_click(cursor_y, track.y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }

    /// Step the body scroll by one viewport in the given direction.
    pub fn body_chevron_step(&mut self, direction: crate::layout::ChevronStepDirection) {
        use crate::layout::ChevronStepDirection as D;
        match direction {
            D::Up | D::Down => {
                let max = (self.body_content_h - self.body_viewport_h).max(0.0);
                let step = self.body_viewport_h.max(40.0);
                let signed = if matches!(direction, D::Up) { -step } else { step };
                let before = self.scroll.offset;
                self.scroll.offset = (self.scroll.offset + signed).clamp(0.0, max);
                eprintln!("[body_chevron_step] V dir={:?} ch={:.1} vh={:.1} max={:.1} step={:.1} {:.1}->{:.1}",
                    direction, self.body_content_h, self.body_viewport_h, max, step, before, self.scroll.offset);
            }
            D::Left | D::Right => {
                let max = (self.body_content_w - self.body_viewport_w).max(0.0);
                let step = self.body_viewport_w.max(40.0);
                let signed = if matches!(direction, D::Left) { -step } else { step };
                let before = self.body_scroll_x;
                self.body_scroll_x = (self.body_scroll_x + signed).clamp(0.0, max);
                eprintln!("[body_chevron_step] H dir={:?} cw={:.1} vw={:.1} max={:.1} step={:.1} {:.1}->{:.1}",
                    direction, self.body_content_w, self.body_viewport_w, max, step, before, self.body_scroll_x);
            }
        }
    }

    /// End any scrollbar drag (call from `on_mouse_up`).
    pub fn end_body_scroll_drag(&mut self) {
        self.scroll.end_drag();
    }
}
