//! Scrollbar scroll state — ported 1:1 from mlc ScrollState.
//!
//! No velocity, no inertia, no opacity animation, no timestamp.
//! Offset is in pixels from top (absolute, not 0..1 normalised).

/// Per-scrollable scroll state.  Cheap to clone; store one per scrollable widget.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Current scroll offset in pixels from top.
    pub offset: f64,
    /// Whether the scrollbar thumb is being dragged.
    pub is_dragging: bool,
    /// Absolute Y position where the drag started (screen coordinates).
    pub drag_start_y: Option<f64>,
    /// Scroll offset at drag start — used to compute delta.
    pub drag_start_offset: Option<f64>,
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset to initial state — zero offset, no drag.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Begin a thumb drag.  Captures current Y and current offset.
    pub fn start_drag(&mut self, y: f64) {
        self.is_dragging = true;
        self.drag_start_y = Some(y);
        self.drag_start_offset = Some(self.offset);
    }

    /// End the thumb drag — clears drag state, offset is preserved.
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_start_y = None;
        self.drag_start_offset = None;
    }

    /// Handle one mouse-wheel notch.
    ///
    /// `delta_y` — signed direction after caller sign-flip.  Positive = scroll
    /// down (offset increases).  Per-notch movement: `|delta_y| * 10.0` pixels.
    ///
    /// Returns `true` if the event was consumed (content is taller than viewport).
    pub fn handle_wheel(&mut self, delta_y: f64, content_height: f64, viewport_height: f64) -> bool {
        if content_height <= viewport_height {
            return false;
        }
        let max_scroll = (content_height - viewport_height).max(0.0);
        let scroll_step = 10.0;
        self.offset = (self.offset + delta_y * scroll_step).clamp(0.0, max_scroll);
        true
    }

    /// Continue an in-progress thumb drag to absolute position `y`.
    ///
    /// Uses a linear mapping from track-space delta to content-space delta,
    /// anchored at the drag-start position so the thumb follows the cursor
    /// precisely.
    pub fn handle_drag(
        &mut self,
        y: f64,
        track_height: f64,
        content_height: f64,
        viewport_height: f64,
    ) {
        if !self.is_dragging {
            return;
        }
        let Some(start_y) = self.drag_start_y else { return };
        let Some(start_offset) = self.drag_start_offset else { return };

        let max_scroll = (content_height - viewport_height).max(0.0);
        if max_scroll <= 0.0 {
            return;
        }
        // Drag math minimum (20 px) is independent of the render minimum (varies
        // per style preset).  Matches mlc handle_drag exactly.
        let handle_height = (viewport_height / content_height * track_height).max(20.0);
        let scroll_range = track_height - handle_height;
        if scroll_range <= 0.0 {
            return;
        }
        let dy = y - start_y;
        let scroll_delta = dy / scroll_range * max_scroll;
        self.offset = (start_offset + scroll_delta).clamp(0.0, max_scroll);
    }

    /// Proportional-jump on track click — snaps offset to the clicked position
    /// within the track.  NOT a page jump.
    pub fn handle_track_click(
        &mut self,
        click_y: f64,
        track_y: f64,
        track_height: f64,
        content_height: f64,
        viewport_height: f64,
    ) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        if max_scroll <= 0.0 {
            return;
        }
        let relative_y = (click_y - track_y) / track_height;
        self.offset = (relative_y * max_scroll).clamp(0.0, max_scroll);
    }

    /// Clamp the stored offset to the valid range.  Call after content height
    /// shrinks (e.g. filter narrowed a list) to avoid stale out-of-range offset.
    pub fn clamp(&mut self, content_height: f64, viewport_height: f64) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        self.offset = self.offset.clamp(0.0, max_scroll);
    }

    /// Absolute programmatic scroll — instant, no animation.
    pub fn scroll_to(&mut self, offset: f64, content_height: f64, viewport_height: f64) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        self.offset = offset.clamp(0.0, max_scroll);
    }

    /// Scroll just enough to make the item at `item_y` with `item_height`
    /// fully visible.  No-op if item is already in view.
    pub fn ensure_visible(
        &mut self,
        item_y: f64,
        item_height: f64,
        viewport_height: f64,
        content_height: f64,
    ) {
        let max_scroll = (content_height - viewport_height).max(0.0);
        if item_y < self.offset {
            self.offset = item_y.max(0.0);
        } else if item_y + item_height > self.offset + viewport_height {
            self.offset = (item_y + item_height - viewport_height).clamp(0.0, max_scroll);
        }
    }

    /// Whether the content overflows the viewport (scrollbar should be drawn).
    pub fn is_scrollable(&self, content_height: f64, viewport_height: f64) -> bool {
        content_height > viewport_height
    }

    /// Compute thumb geometry: `(thumb_y_offset_within_track, thumb_length)`.
    ///
    /// `track_height` — rendered track pixel height.
    /// `min_thumb_length` — style-defined minimum.
    pub fn thumb_geometry(
        &self,
        content_height: f64,
        viewport_height: f64,
        track_height: f64,
        min_thumb_length: f64,
    ) -> (f64, f64) {
        if content_height <= 0.0 {
            return (0.0, track_height);
        }
        let visible_ratio = (viewport_height / content_height).clamp(0.0, 1.0);
        let thumb_len = (track_height * visible_ratio).max(min_thumb_length).min(track_height);
        let max_scroll = (content_height - viewport_height).max(0.0);
        let scroll_ratio = if max_scroll > 0.0 {
            (self.offset / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let available = (track_height - thumb_len).max(0.0);
        let thumb_y = available * scroll_ratio;
        (thumb_y, thumb_len)
    }
}
