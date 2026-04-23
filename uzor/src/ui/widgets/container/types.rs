//! Container type definitions - semantic container variants
//!
//! This module defines the container taxonomy through enum types.
//! NO colors, NO rendering logic - only semantic classification.

/// Main container type enum covering all container variants in the terminal
///
/// Containers are wrapper widgets that hold and display content.
///
/// # Variants
///
/// - **Scrollable**: Container with scrollbar for overflow content
/// - **Plain**: Simple container without scrolling
///
/// # L1 Standard Signature
///
/// All variants include position (x, y), width, and height for consistent geometry access.
///
/// # Usage in Terminal (Documentation + Inline Rendering)
///
/// ```rust,ignore
/// // Enum instance for DOCUMENTATION
/// let _chat_container = ContainerType::Scrollable {
///     scroll_offset: 0.0,
///     content_height: 1500.0,
///     viewport_height: 400.0,
///     position: (10.0, 20.0),
///     width: 300.0,
///     height: 400.0,
/// };
///
/// // INLINE RENDERING (production)
/// let (x, y) = container.position();
/// let width = container.width();
/// let height = container.height();
/// ctx.fill_rect(x, y, width, height);
/// // Render content with clipping
/// // Render scrollbar if needed
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerType {
    /// Scrollable container with vertical scrollbar
    ///
    /// Used for: chat messages, order lists, long content areas
    Scrollable {
        /// Current scroll offset in pixels
        scroll_offset: f64,
        /// Total height of content
        content_height: f64,
        /// Height of visible viewport (same as height conceptually)
        viewport_height: f64,
        /// Position (x, y) of container
        position: (f64, f64),
        /// Width of container
        width: f64,
        /// Height of container
        height: f64,
    },

    /// Plain container without scrolling
    ///
    /// Used for: fixed-size content areas, simple wrappers
    Plain {
        /// Position (x, y) of container
        position: (f64, f64),
        /// Width of container
        width: f64,
        /// Height of container
        height: f64,
    },
}

impl ContainerType {
    /// Create a scrollable container
    pub fn scrollable(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        content_height: f64,
    ) -> Self {
        Self::Scrollable {
            scroll_offset: 0.0,
            content_height,
            viewport_height: height,
            position: (x, y),
            width,
            height,
        }
    }

    /// Create a plain container
    pub fn plain(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Plain {
            position: (x, y),
            width,
            height,
        }
    }

    /// Check if container needs scrollbar
    pub fn needs_scrollbar(&self) -> bool {
        match self {
            Self::Scrollable { content_height, viewport_height, .. } => {
                content_height > viewport_height
            }
            Self::Plain { .. } => false,
        }
    }

    /// Get container position
    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Scrollable { position, .. } => *position,
            Self::Plain { position, .. } => *position,
        }
    }

    /// Get container width
    pub fn width(&self) -> f64 {
        match self {
            Self::Scrollable { width, .. } => *width,
            Self::Plain { width, .. } => *width,
        }
    }

    /// Get container height
    pub fn height(&self) -> f64 {
        match self {
            Self::Scrollable { height, .. } => *height,
            Self::Plain { height, .. } => *height,
        }
    }
}
