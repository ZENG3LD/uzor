//! Scrollable container geometry and configuration
//!
//! High-level abstraction for scrollable content areas.
//! Handles offset calculation and geometry computation for headless architecture.

use crate::types::{Rect, ScrollState};
use serde::{Deserialize, Serialize};

/// Response from scrollable container geometry calculation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScrollableResponse {
    /// Total content size (as measured)
    pub content_size: f64,
    /// Viewport size
    pub viewport_size: f64,
    /// Whether scrollbar is visible
    pub has_scrollbar: bool,
    /// Viewport rectangle
    pub viewport: Rect,
    /// Content area rectangle (excluding scrollbar)
    pub content_area: Rect,
}

/// Configuration for scrollable container
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollableConfig {
    /// Width of scrollbar
    pub scrollbar_size: f64,
    /// Whether to always show scrollbar
    pub always_show_scrollbar: bool,
}

impl Default for ScrollableConfig {
    fn default() -> Self {
        Self {
            scrollbar_size: 8.0,
            always_show_scrollbar: false,
        }
    }
}

/// Scrollable container for managing scrollable content rendering
pub struct ScrollableContainer {
    viewport: Rect,
    scroll_offset: f64,
    is_dragging: bool,
    config: ScrollableConfig,
}

impl ScrollableContainer {
    pub fn new(viewport: Rect, scroll_state: &ScrollState, config: Option<ScrollableConfig>) -> Self {
        Self {
            viewport,
            scroll_offset: scroll_state.offset,
            is_dragging: scroll_state.is_dragging,
            config: config.unwrap_or_default(),
        }
    }

    /// Get content area rectangle (excludes scrollbar space)
    pub fn content_area(&self) -> Rect {
        Rect::new(
            self.viewport.x,
            self.viewport.y,
            self.viewport.width - self.config.scrollbar_size,
            self.viewport.height,
        )
    }

    /// Calculate scrollable area geometry
    pub fn calculate(self, content_height: f64) -> ScrollableResponse {
        let needs_scrollbar = content_height > self.viewport.height || self.config.always_show_scrollbar;

        let content_area = Rect::new(
            self.viewport.x,
            self.viewport.y,
            self.viewport.width - self.config.scrollbar_size,
            self.viewport.height,
        );

        ScrollableResponse {
            content_size: content_height,
            viewport_size: self.viewport.height,
            has_scrollbar: needs_scrollbar,
            viewport: self.viewport,
            content_area,
        }
    }

    pub fn content_y(&self) -> f64 {
        self.viewport.y - self.scroll_offset
    }

    pub fn content_width(&self) -> f64 {
        self.viewport.width - self.config.scrollbar_size
    }
}