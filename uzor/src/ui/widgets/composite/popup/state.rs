//! Popup persistent state.

use crate::types::{Rect, ScrollState};
use super::super::resize_drag::ResizeDrag;

/// All per-popup frame state.
#[derive(Debug, Clone, Default)]
pub struct PopupState {
    // --- Lifecycle ---

    /// Whether the popup is currently open.
    pub open: bool,

    // --- Position ---

    /// Top-left corner of the popup in screen coordinates.
    pub position: (f64, f64),

    /// Anchor trigger-rect origin for smart re-positioning on resize.
    pub anchor: Option<(f64, f64)>,

    // --- Resize (kept on state for parity with modal — popups themselves
    // don't expose resize handles, but the field is harmless and lets
    // hosts share dispatch handlers across modal/popup) ---

    pub resize_drag: Option<ResizeDrag>,
    pub resized_rect: Option<Rect>,

    // --- Body scroll (Scrollbar / Chevrons overflow modes) ---

    pub scroll: ScrollState,
    pub body_content_h: f64,
    pub body_scroll_track: Option<Rect>,
    pub body_viewport_h: f64,
}

impl PopupState {
    /// Open the popup at the given screen-space origin.
    pub fn open_at(&mut self, origin: (f64, f64)) {
        self.open = true;
        self.position = origin;
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Returns `true` while any drag gesture (resize / scrollbar) is live —
    /// guards click-outside dismiss.
    pub fn is_dragging_any(&self) -> bool {
        self.resize_drag.is_some() || self.scroll.is_dragging
    }

    // --- Resize helpers ---

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

    pub fn update_resize(&mut self, cursor: (f64, f64)) {
        if let Some(drag) = self.resize_drag {
            let rect = drag.resolve(cursor);
            self.resized_rect = Some(rect);
            self.position = (rect.x, rect.y);
        }
    }

    pub fn end_resize(&mut self) {
        self.resize_drag = None;
    }

    // --- Body scroll helpers ---

    pub fn start_body_scroll_drag(&mut self, cursor_y: f64) {
        self.scroll.start_drag(cursor_y);
    }

    pub fn update_body_scroll_drag(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_drag(cursor_y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }

    pub fn body_scroll_track_click(&mut self, cursor_y: f64) {
        if let Some(track) = self.body_scroll_track {
            self.scroll.handle_track_click(cursor_y, track.y, track.height,
                self.body_content_h, self.body_viewport_h);
        }
    }

    pub fn body_chevron_step(&mut self, direction: crate::layout::ChevronStepDirection) {
        let max = (self.body_content_h - self.body_viewport_h).max(0.0);
        let step = self.body_viewport_h.max(40.0);
        let signed = match direction {
            crate::layout::ChevronStepDirection::Up
            | crate::layout::ChevronStepDirection::Left => -step,
            _ => step,
        };
        self.scroll.offset = (self.scroll.offset + signed).clamp(0.0, max);
    }

    pub fn end_body_scroll_drag(&mut self) {
        self.scroll.end_drag();
    }
}
