//! Icon button widget
//!
//! Minimal icon button implementation for toolbar actions (close, add, etc.)

use crate::types::WidgetState;

/// Configuration for icon button rendering
#[derive(Clone, Debug)]
pub struct IconButtonConfig {
    /// Icon identifier (e.g., "close", "plus", "settings")
    pub icon: String,
    /// Button size (width and height)
    pub size: f64,
}

impl IconButtonConfig {
    /// Create a new icon button configuration
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            size: 24.0, // Default size for toolbar buttons
        }
    }

    /// Set custom button size
    pub fn with_size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }
}

/// Response from icon button interaction
#[derive(Clone, Copy, Debug)]
pub struct IconButtonResponse {
    /// Whether the button was clicked this frame
    pub clicked: bool,
    /// Whether the button is hovered
    pub hovered: bool,
    /// Current interaction state (Normal, Hovered, Pressed)
    pub state: WidgetState,
}
