//! Drag handle persistent state.

/// State for a drag handle widget.
///
/// Tracks whether a drag is currently in progress and the cursor offset
/// recorded at drag start so the caller can compute deltas.
#[derive(Debug, Default, Clone)]
pub struct DragHandleState {
    /// Whether a drag gesture is currently active.
    pub dragging: bool,

    /// Cursor position relative to the drag origin recorded at `start`.
    pub drag_offset: (f64, f64),
}

impl DragHandleState {
    /// Begin a drag gesture at `cursor_pos`.
    pub fn start(&mut self, cursor_pos: (f64, f64)) {
        self.dragging = true;
        self.drag_offset = cursor_pos;
    }

    /// Compute the delta from the last recorded position to `cursor_pos` and
    /// update the internal offset.
    ///
    /// Returns `(dx, dy)` that the caller should apply to the dragged item.
    pub fn update(&mut self, cursor_pos: (f64, f64)) -> (f64, f64) {
        if !self.dragging {
            return (0.0, 0.0);
        }
        let dx = cursor_pos.0 - self.drag_offset.0;
        let dy = cursor_pos.1 - self.drag_offset.1;
        self.drag_offset = cursor_pos;
        (dx, dy)
    }

    /// End the current drag gesture.
    pub fn end(&mut self) {
        self.dragging = false;
        self.drag_offset = (0.0, 0.0);
    }

    /// Returns `true` while a drag is in progress.
    pub fn is_active(&self) -> bool {
        self.dragging
    }
}
