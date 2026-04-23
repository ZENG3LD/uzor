//! Container input adapter - Contract/Connector for scrollbar event handling
//!
//! **ContainerInputHandler is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory event handling logic (`factory/events.rs` - future)
//! - External input systems (event loops, input managers, etc.)

use crate::types::Rect;

/// Input handler adapter for container scrollbar events
///
/// This trait defines the contract for converting raw input events into scroll actions.
/// External projects implement this trait to customize scroll behavior (rare).
pub trait ContainerInputHandler {
    // =========================================================================
    // Hit Testing
    // =========================================================================

    /// Test if mouse position is inside scrollbar rect
    fn hit_test_scrollbar(&self, scrollbar_rect: &Rect, mouse_pos: (f64, f64)) -> bool {
        let (mouse_x, mouse_y) = mouse_pos;
        mouse_x >= scrollbar_rect.x
            && mouse_x <= scrollbar_rect.x + scrollbar_rect.width
            && mouse_y >= scrollbar_rect.y
            && mouse_y <= scrollbar_rect.y + scrollbar_rect.height
    }

    // =========================================================================
    // Scroll Calculations
    // =========================================================================

    /// Convert mouse Y position to scroll offset (for drag scrolling)
    fn mouse_to_scroll_offset(
        &self,
        mouse_y: f64,
        scrollbar_y: f64,
        scrollbar_height: f64,
        content_height: f64,
        viewport_height: f64,
    ) -> f64 {
        if scrollbar_height <= 0.0 {
            return 0.0;
        }

        let max_scroll = (content_height - viewport_height).max(0.0);
        let ratio = ((mouse_y - scrollbar_y) / scrollbar_height).clamp(0.0, 1.0);
        ratio * max_scroll
    }

    /// Convert scroll wheel delta to scroll offset change
    fn scroll_to_delta(&self, scroll_delta: f64, _viewport_height: f64) -> f64 {
        scroll_delta * 40.0
    }

    /// Clamp scroll offset to valid range [0, max_scroll]
    fn clamp_scroll_offset(
        &self,
        offset: f64,
        content_height: f64,
        viewport_height: f64,
    ) -> f64 {
        let max_scroll = (content_height - viewport_height).max(0.0);
        offset.clamp(0.0, max_scroll)
    }

    // =========================================================================
    // Scrollbar Thumb Sizing
    // =========================================================================

    /// Calculate scrollbar thumb size based on viewport/content ratio
    fn calculate_thumb_size(
        &self,
        viewport_height: f64,
        content_height: f64,
        scrollbar_height: f64,
        min_thumb_height: f64,
    ) -> f64 {
        if content_height <= 0.0 || viewport_height >= content_height {
            return scrollbar_height;
        }

        let ratio = viewport_height / content_height;
        let thumb_height = ratio * scrollbar_height;
        thumb_height.max(min_thumb_height)
    }

    /// Calculate scrollbar thumb Y position based on scroll offset
    fn calculate_thumb_position(
        &self,
        scroll_offset: f64,
        content_height: f64,
        viewport_height: f64,
        scrollbar_y: f64,
        scrollbar_height: f64,
        thumb_height: f64,
    ) -> f64 {
        let max_scroll = (content_height - viewport_height).max(0.0);
        if max_scroll <= 0.0 {
            return scrollbar_y;
        }

        let scroll_ratio = (scroll_offset / max_scroll).clamp(0.0, 1.0);
        let max_thumb_travel = scrollbar_height - thumb_height;
        scrollbar_y + scroll_ratio * max_thumb_travel
    }
}

// =============================================================================
// Default Input Handler Implementation
// =============================================================================

/// Default implementation of ContainerInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultContainerInputHandler;

impl ContainerInputHandler for DefaultContainerInputHandler {
    // All methods use trait defaults (no overrides needed)
}
