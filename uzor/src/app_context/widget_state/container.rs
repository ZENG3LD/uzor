//! Container state adapter - Contract/Connector for scroll state management
//!
//! **ContainerState is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - External state management systems (app state, Redux, ECS, etc.)
//!
//! Container state tracks scroll offset, scrollbar dragging, and hover state.
//! NOT content state — content lives in application domain.

use std::collections::HashMap;

/// State adapter for container scroll interaction
///
/// This trait defines the contract for tracking container scroll state.
/// External projects implement this trait to integrate with their state management systems.
pub trait ContainerState {
    // =========================================================================
    // Read State (Immutable)
    // =========================================================================

    /// Get current scroll offset for container (pixels, 0.0 = top)
    fn scroll_offset(&self, container_id: &str) -> f64;

    /// Check if scrollbar is currently being dragged
    fn is_scrollbar_dragging(&self, container_id: &str) -> bool;

    /// Check if scrollbar is currently hovered
    fn is_scrollbar_hovered(&self, container_id: &str) -> bool;

    // =========================================================================
    // Write State (Mutable)
    // =========================================================================

    /// Set scroll offset for container
    fn set_scroll_offset(&mut self, container_id: &str, offset: f64);

    /// Set scrollbar dragging state
    fn set_scrollbar_dragging(&mut self, container_id: &str, dragging: bool);

    /// Set scrollbar hover state
    fn set_scrollbar_hovered(&mut self, container_id: &str, hovered: bool);
}

// =============================================================================
// Default State Implementation
// =============================================================================

/// Simple implementation of ContainerState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleContainerState {
    /// Scroll offsets by container ID
    pub scroll_offsets: HashMap<String, f64>,

    /// Currently dragging scrollbar (container ID)
    pub dragging: Option<String>,

    /// Currently hovered scrollbar (container ID)
    pub hovered: Option<String>,
}

impl SimpleContainerState {
    /// Create new container state
    pub fn new() -> Self {
        Self {
            scroll_offsets: HashMap::new(),
            dragging: None,
            hovered: None,
        }
    }
}

impl ContainerState for SimpleContainerState {
    fn scroll_offset(&self, container_id: &str) -> f64 {
        *self.scroll_offsets.get(container_id).unwrap_or(&0.0)
    }

    fn is_scrollbar_dragging(&self, container_id: &str) -> bool {
        self.dragging.as_deref() == Some(container_id)
    }

    fn is_scrollbar_hovered(&self, container_id: &str) -> bool {
        self.hovered.as_deref() == Some(container_id)
    }

    fn set_scroll_offset(&mut self, container_id: &str, offset: f64) {
        self.scroll_offsets.insert(container_id.to_string(), offset);
    }

    fn set_scrollbar_dragging(&mut self, container_id: &str, dragging: bool) {
        if dragging {
            self.dragging = Some(container_id.to_string());
        } else if self.dragging.as_deref() == Some(container_id) {
            self.dragging = None;
        }
    }

    fn set_scrollbar_hovered(&mut self, container_id: &str, hovered: bool) {
        if hovered {
            self.hovered = Some(container_id.to_string());
        } else if self.hovered.as_deref() == Some(container_id) {
            self.hovered = None;
        }
    }
}
