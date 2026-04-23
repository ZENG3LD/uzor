//! Container state adapter - Contract/Connector for scroll state management
//!
//! # Architecture Role
//!
//! **ContainerState is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - External state management systems (app state, Redux, ECS, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 1. External State Manager (e.g., AppState, UIState)        │
//! │    - Stores scroll positions and interaction state          │
//! │    - Implements ContainerState trait (mapping)              │
//! └─────────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 2. ContainerState trait (THIS MODULE)                       │
//! │    - Defines contract (which state containers need)         │
//! │    - Acts as connector interface                            │
//! └─────────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/render.rs)            │
//! │    - Accept &ContainerState or &mut ContainerState         │
//! │    - Call trait methods to get/update state                 │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # What is Container State?
//!
//! Container state tracks **scrollbar interaction state** - ephemeral data during scrolling:
//! - **Scroll offset** - Current vertical scroll position in pixels
//! - **Dragging state** - Is scrollbar thumb being dragged?
//! - **Hover state** - Is mouse over scrollbar?
//!
//! **NOT content state** - The container's content (children, data) lives in application domain.
//! ContainerState only tracks scroll/interaction state.
//!
//! # Implementation Example
//!
//! Each external project implements ContainerState for their state manager:
//!
//! ```rust,ignore
//! // In ui/app_state.rs (or your state module)
//! pub struct AppState {
//!     pub scroll_offsets: HashMap<String, f64>,
//!     pub scrollbar_dragging: Option<String>,  // container_id being dragged
//!     pub scrollbar_hovered: Option<String>,   // container_id being hovered
//!     // ... other app state
//! }
//!
//! impl ContainerState for AppState {
//!     fn scroll_offset(&self, container_id: &str) -> f64 {
//!         *self.scroll_offsets.get(container_id).unwrap_or(&0.0)
//!     }
//!
//!     fn is_scrollbar_dragging(&self, container_id: &str) -> bool {
//!         self.scrollbar_dragging.as_deref() == Some(container_id)
//!     }
//!
//!     fn is_scrollbar_hovered(&self, container_id: &str) -> bool {
//!         self.scrollbar_hovered.as_deref() == Some(container_id)
//!     }
//!
//!     fn set_scroll_offset(&mut self, container_id: &str, offset: f64) {
//!         self.scroll_offsets.insert(container_id.to_string(), offset);
//!     }
//!
//!     fn set_scrollbar_dragging(&mut self, container_id: &str, dragging: bool) {
//!         if dragging {
//!             self.scrollbar_dragging = Some(container_id.to_string());
//!         } else if self.scrollbar_dragging.as_deref() == Some(container_id) {
//!             self.scrollbar_dragging = None;
//!         }
//!     }
//!
//!     fn set_scrollbar_hovered(&mut self, container_id: &str, hovered: bool) {
//!         if hovered {
//!             self.scrollbar_hovered = Some(container_id.to_string());
//!         } else if self.scrollbar_hovered.as_deref() == Some(container_id) {
//!             self.scrollbar_hovered = None;
//!         }
//!     }
//! }
//! ```
//!
//! # Usage in Factory
//!
//! ```rust,ignore
//! use container::factory::render_default;
//!
//! let mut app_state = AppState::default();
//!
//! // Factory automatically uses ContainerState trait
//! render_default(
//!     ctx,
//!     &container,
//!     &theme,
//!     "chat_panel",  // container_id
//!     &mut app_state,  // ← Implements ContainerState
//!     &input_handler,
//!     rect
//! );
//!
//! // State changes during interaction
//! if mouse_over_scrollbar {
//!     app_state.set_scrollbar_hovered("chat_panel", true);
//! }
//! ```
//!
//! # Notes
//!
//! - **Simple containers need simple state** - Most only need scroll offset
//! - **Complex containers need more** - Dragging, hover for visual feedback
//! - **State lives in app** - ContainerState connects to app's state management
//! - **Factory reads state** - Uses `scroll_offset()` for rendering position
//! - **Factory writes state** - Uses `set_scroll_offset()` during drag events

use std::collections::HashMap;

/// State adapter for container scroll interaction
///
/// This trait defines the contract for tracking container scroll state.
/// External projects implement this trait to integrate with their state management systems.
///
/// # Responsibilities
///
/// - Track scroll offset (vertical position in pixels)
/// - Track scrollbar dragging state (for smooth drag scrolling)
/// - Track scrollbar hover state (for visual feedback)
/// - Provide state to rendering functions
/// - **NOT responsible for content state** (children, data)
///
/// # State Ownership
///
/// The external project owns the state. Factory borrows via trait:
/// - **Read state** - `&self` methods (`scroll_offset`, `is_*`)
/// - **Write state** - `&mut self` methods (`set_*`)
///
/// # Container Identity
///
/// All methods take `container_id: &str` to identify which container's state to check/update.
/// This enables a single state manager to track multiple containers.
pub trait ContainerState {
    // =========================================================================
    // Read State (Immutable)
    // =========================================================================

    /// Get current scroll offset for container
    ///
    /// # Parameters
    /// - `container_id` - Unique identifier for this container (e.g., "chat_panel", "order_list")
    ///
    /// # Returns
    /// Current scroll offset in pixels (0.0 = top, positive = scrolled down)
    ///
    /// # Usage
    /// Factory uses this to offset content rendering and position scrollbar thumb
    fn scroll_offset(&self, container_id: &str) -> f64;

    /// Check if scrollbar is currently being dragged
    ///
    /// # Parameters
    /// - `container_id` - Unique identifier for this container
    ///
    /// # Returns
    /// `true` if scrollbar thumb is being dragged, `false` otherwise
    ///
    /// # Usage
    /// Factory uses this to continue tracking mouse during drag (even outside scrollbar)
    fn is_scrollbar_dragging(&self, container_id: &str) -> bool;

    /// Check if scrollbar is currently hovered
    ///
    /// # Parameters
    /// - `container_id` - Unique identifier for this container
    ///
    /// # Returns
    /// `true` if mouse is over scrollbar, `false` otherwise
    ///
    /// # Usage
    /// Factory uses this to apply hover color to scrollbar thumb
    fn is_scrollbar_hovered(&self, container_id: &str) -> bool;

    // =========================================================================
    // Write State (Mutable)
    // =========================================================================

    /// Set scroll offset for container
    ///
    /// # Parameters
    /// - `container_id` - Which container to set scroll for
    /// - `offset` - New scroll offset in pixels (should be clamped by caller)
    ///
    /// # Usage
    /// Factory calls this during scroll wheel or scrollbar drag events
    fn set_scroll_offset(&mut self, container_id: &str, offset: f64);

    /// Set scrollbar dragging state
    ///
    /// # Parameters
    /// - `container_id` - Which container's scrollbar
    /// - `dragging` - `true` when drag starts, `false` when drag ends
    ///
    /// # Usage
    /// Factory calls this on mouse down (start drag) and mouse up (end drag)
    fn set_scrollbar_dragging(&mut self, container_id: &str, dragging: bool);

    /// Set scrollbar hover state
    ///
    /// # Parameters
    /// - `container_id` - Which container's scrollbar
    /// - `hovered` - `true` when mouse enters scrollbar, `false` when leaves
    ///
    /// # Usage
    /// Factory calls this during mouse move for hover visual feedback
    fn set_scrollbar_hovered(&mut self, container_id: &str, hovered: bool);
}

// =============================================================================
// Default State Implementation
// =============================================================================

/// Simple implementation of ContainerState for prototyping
///
/// This struct provides a minimal state implementation for external projects
/// that don't need complex state management integration.
///
/// Tracks state for multiple containers using container IDs and HashMaps.
///
/// # Usage
///
/// ```rust,ignore
/// use container::state::{ContainerState, SimpleContainerState};
///
/// let mut state = SimpleContainerState::new();
///
/// // Set scroll offset
/// state.set_scroll_offset("chat_panel", 150.0);
/// assert_eq!(state.scroll_offset("chat_panel"), 150.0);
///
/// // Simulate scrollbar hover
/// state.set_scrollbar_hovered("chat_panel", true);
/// assert!(state.is_scrollbar_hovered("chat_panel"));
/// assert!(!state.is_scrollbar_hovered("order_list"));
///
/// // Simulate drag start
/// state.set_scrollbar_dragging("chat_panel", true);
/// assert!(state.is_scrollbar_dragging("chat_panel"));
/// ```
///
/// For production, implement ContainerState for your app's state manager instead.
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
