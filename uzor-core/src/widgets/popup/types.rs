//! Popup type definitions - semantic popup variants
//!
//! This module defines the popup taxonomy through enum types.
//! NO colors, NO rendering logic - only semantic classification.

/// Main popup type enum covering all popup variants in the terminal
///
/// Popups are click-driven overlays that appear on top of other UI elements.
///
/// # Distinction from Overlays
///
/// - **Popup**: Requires user click to appear, must be explicitly closed
/// - **Overlay**: Appears automatically on hover, auto-dismisses
///
/// # Variants
///
/// - **ContextMenu**: Right-click menu with action items
/// - **ColorPicker**: Color selection popup with palette
/// - **Custom**: Generic popup with custom content
#[derive(Debug, Clone, PartialEq)]
pub enum PopupType {
    /// Context menu triggered by right-click
    ///
    /// Used for: chart context menu, primitive actions
    ContextMenu {
        /// Position where popup was triggered (x, y)
        position: (f64, f64),
        /// Currently highlighted item index
        selected_index: Option<usize>,
        /// Width of popup
        width: f64,
        /// Height of popup
        height: f64,
    },

    /// Color picker popup for color selection
    ///
    /// Used for: chart colors, indicator colors, primitive colors
    ColorPicker {
        /// Position where popup was triggered (x, y)
        position: (f64, f64),
        /// Currently selected color (hex string)
        selected_color: Option<String>,
        /// Whether showing custom color input
        custom_mode: bool,
        /// Width of popup
        width: f64,
        /// Height of popup
        height: f64,
    },

    /// Generic custom popup
    ///
    /// Used for: custom dialogs, confirmation popups
    Custom {
        /// Position where popup was triggered (x, y)
        position: (f64, f64),
        /// Width of popup
        width: f64,
        /// Height of popup
        height: f64,
    },
}

impl PopupType {
    /// Create a context menu at position
    pub fn context_menu(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::ContextMenu {
            position: (x, y),
            selected_index: None,
            width,
            height,
        }
    }

    /// Create a color picker at position
    pub fn color_picker(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::ColorPicker {
            position: (x, y),
            selected_color: None,
            custom_mode: false,
            width,
            height,
        }
    }

    /// Create a custom popup at position
    pub fn custom(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Custom {
            position: (x, y),
            width,
            height,
        }
    }

    /// Get popup position
    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::ContextMenu { position, .. } => *position,
            Self::ColorPicker { position, .. } => *position,
            Self::Custom { position, .. } => *position,
        }
    }

    /// Get popup width
    pub fn width(&self) -> f64 {
        match self {
            Self::ContextMenu { width, .. } => *width,
            Self::ColorPicker { width, .. } => *width,
            Self::Custom { width, .. } => *width,
        }
    }

    /// Get popup height
    pub fn height(&self) -> f64 {
        match self {
            Self::ContextMenu { height, .. } => *height,
            Self::ColorPicker { height, .. } => *height,
            Self::Custom { height, .. } => *height,
        }
    }
}
