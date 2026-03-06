//! Scrollbar widget geometry and configuration
//!
//! Handles the math of mapping scroll offsets to handle positions for headless architecture.

use crate::types::Rect;
use serde::{Deserialize, Serialize};

/// Scrollbar configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollbarConfig {
    /// Total content size (height or width)
    pub content_size: f64,
    /// Visible viewport size
    pub viewport_size: f64,
    /// Current scroll offset
    pub scroll_offset: f64,
    /// Minimum handle size
    pub min_handle_size: f64,
    /// Whether this is a horizontal scrollbar
    pub horizontal: bool,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            content_size: 0.0,
            viewport_size: 0.0,
            scroll_offset: 0.0,
            min_handle_size: 30.0,
            horizontal: false,
        }
    }
}

impl ScrollbarConfig {
    pub fn new(content_size: f64, viewport_size: f64, scroll_offset: f64) -> Self {
        Self {
            content_size,
            viewport_size,
            scroll_offset,
            ..Default::default()
        }
    }

    pub fn needs_scrollbar(&self) -> bool {
        self.content_size > self.viewport_size
    }

    fn visible_ratio(&self) -> f64 {
        if self.content_size <= 0.0 { 1.0 } else { (self.viewport_size / self.content_size).clamp(0.0, 1.0) }
    }

    fn scroll_ratio(&self) -> f64 {
        let max_scroll = (self.content_size - self.viewport_size).max(0.0);
        if max_scroll <= 0.0 { 0.0 } else { (self.scroll_offset / max_scroll).clamp(0.0, 1.0) }
    }

    pub fn max_scroll(&self) -> f64 {
        (self.content_size - self.viewport_size).max(0.0)
    }
}

/// Scrollbar geometry response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScrollbarResponse {
    /// Track rectangle
    pub track_rect: Rect,
    /// Handle rectangle
    pub handle_rect: Rect,
    /// New scroll offset (if dragging)
    pub scroll_offset: f64,
    /// Whether scrollbar was dragged
    pub dragged: bool,
}

impl ScrollbarConfig {
    /// Calculate scrollbar handle geometry
    pub fn calculate_geometry(&self, track_rect: Rect, drag_pos: Option<f64>) -> ScrollbarResponse {
        if !self.needs_scrollbar() {
            return ScrollbarResponse {
                track_rect,
                handle_rect: Rect::default(),
                scroll_offset: self.scroll_offset,
                dragged: false,
            };
        }

        let visible_ratio = self.visible_ratio();
        let scroll_ratio = self.scroll_ratio();

        let (handle_rect, new_scroll_offset) = if self.horizontal {
            let handle_width = (visible_ratio * track_rect.width).max(self.min_handle_size);
            let available_width = track_rect.width - handle_width;

            let mut offset = self.scroll_offset;
            let mut handle_x = track_rect.x + scroll_ratio * available_width;

            if let Some(x) = drag_pos {
                let new_ratio = ((x - track_rect.x - handle_width / 2.0) / available_width).clamp(0.0, 1.0);
                offset = new_ratio * self.max_scroll();
                handle_x = track_rect.x + new_ratio * available_width;
            }

            (Rect::new(handle_x, track_rect.y, handle_width, track_rect.height), offset)
        } else {
            let handle_height = (visible_ratio * track_rect.height).max(self.min_handle_size);
            let available_height = track_rect.height - handle_height;

            let mut offset = self.scroll_offset;
            let mut handle_y = track_rect.y + scroll_ratio * available_height;

            if let Some(y) = drag_pos {
                let new_ratio = ((y - track_rect.y - handle_height / 2.0) / available_height).clamp(0.0, 1.0);
                offset = new_ratio * self.max_scroll();
                handle_y = track_rect.y + new_ratio * available_height;
            }

            (Rect::new(track_rect.x, handle_y, track_rect.width, handle_height), offset)
        };

        ScrollbarResponse {
            track_rect,
            handle_rect,
            scroll_offset: new_scroll_offset,
            dragged: drag_pos.is_some(),
        }
    }
}