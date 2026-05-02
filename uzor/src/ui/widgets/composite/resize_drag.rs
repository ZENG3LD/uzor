//! Shared resize-drag math for composites with `resizable` opt-in.
//!
//! Every composite that supports user-driven resize (toolbar, modal,
//! popup, sidebar...) reuses this:
//!
//! 1. Caller calls [`ResizeDrag::begin`] from the bridge drag-press arm
//!    when the dispatcher fires `ResizeHandleDragStarted { edge, .. }`,
//!    capturing the host rect + cursor position.
//! 2. Caller calls [`ResizeDrag::resolve`] each `on_mouse_move` while
//!    the drag is live; the helper returns the new rect (origin + size)
//!    already clamped to the supplied `min` and `cap`.
//!
//! Composites typically embed an `Option<ResizeDrag>` in their state and
//! expose tiny wrappers (`start_resize`, `update_resize`) so the app
//! never sees the math.

use crate::layout::ResizeEdge;
use crate::types::Rect;

/// Captured-at-mousedown info needed to compute every subsequent rect
/// while the drag is live. Cheap to clone / copy.
#[derive(Debug, Clone, Copy)]
pub struct ResizeDrag {
    /// Which edge / corner is being dragged.
    pub edge:        ResizeEdge,
    /// Host rect at the moment the drag started (top-left + size).
    pub start_rect:  Rect,
    /// Cursor position at the moment the drag started.
    pub start_pos:   (f64, f64),
    /// Minimum allowed (width, height).
    pub min:         (f64, f64),
    /// Maximum allowed (width, height). Pass `f64::INFINITY` to disable.
    pub cap:         (f64, f64),
}

impl ResizeDrag {
    /// Begin a resize drag.
    pub fn begin(
        edge:       ResizeEdge,
        start_rect: Rect,
        start_pos:  (f64, f64),
        min:        (f64, f64),
        cap:        (f64, f64),
    ) -> Self {
        Self { edge, start_rect, start_pos, min, cap }
    }

    /// Resolve the current rect from a fresh cursor position. Returns the
    /// new rect (top-left may shift when dragging from N / W). All values
    /// are clamped to `min` / `cap`.
    pub fn resolve(&self, cursor: (f64, f64)) -> Rect {
        let dx = cursor.0 - self.start_pos.0;
        let dy = cursor.1 - self.start_pos.1;

        let (mut x, mut y) = (self.start_rect.x, self.start_rect.y);
        let (mut w, mut h) = (self.start_rect.width, self.start_rect.height);
        let (min_w, min_h) = self.min;
        let (cap_w, cap_h) = self.cap;

        let clamp = |v: f64, lo: f64, hi: f64| v.max(lo).min(hi);

        match self.edge {
            ResizeEdge::E => {
                w = clamp(self.start_rect.width + dx, min_w, cap_w);
            }
            ResizeEdge::W => {
                let nw = clamp(self.start_rect.width - dx, min_w, cap_w);
                x = self.start_rect.x + (self.start_rect.width - nw);
                w = nw;
            }
            ResizeEdge::S => {
                h = clamp(self.start_rect.height + dy, min_h, cap_h);
            }
            ResizeEdge::N => {
                let nh = clamp(self.start_rect.height - dy, min_h, cap_h);
                y = self.start_rect.y + (self.start_rect.height - nh);
                h = nh;
            }
            ResizeEdge::SE => {
                w = clamp(self.start_rect.width  + dx, min_w, cap_w);
                h = clamp(self.start_rect.height + dy, min_h, cap_h);
            }
            ResizeEdge::NE => {
                w = clamp(self.start_rect.width - 0.0 + dx, min_w, cap_w);
                let nh = clamp(self.start_rect.height - dy, min_h, cap_h);
                y = self.start_rect.y + (self.start_rect.height - nh);
                h = nh;
            }
            ResizeEdge::SW => {
                let nw = clamp(self.start_rect.width - dx, min_w, cap_w);
                x = self.start_rect.x + (self.start_rect.width - nw);
                w = nw;
                h = clamp(self.start_rect.height + dy, min_h, cap_h);
            }
            ResizeEdge::NW => {
                let nw = clamp(self.start_rect.width - dx, min_w, cap_w);
                x = self.start_rect.x + (self.start_rect.width - nw);
                w = nw;
                let nh = clamp(self.start_rect.height - dy, min_h, cap_h);
                y = self.start_rect.y + (self.start_rect.height - nh);
                h = nh;
            }
        }
        Rect::new(x, y, w, h)
    }
}
