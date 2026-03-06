//! Checkbox widget layout and configuration
//!
//! Provides checkbox configuration and response types for headless architecture.
//! Rendering is delegated to platform-specific implementations.

use crate::types::{WidgetState, Rect};
use serde::{Deserialize, Serialize};

/// Checkbox configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckboxConfig {
    /// Checkbox label
    pub label: String,
    /// Whether checkbox is checked
    pub checked: bool,
    /// Whether checkbox is disabled
    pub disabled: bool,
}

impl Default for CheckboxConfig {
    fn default() -> Self {
        Self {
            label: String::new(),
            checked: false,
            disabled: false,
        }
    }
}

impl CheckboxConfig {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            ..Default::default()
        }
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Checkbox interaction response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CheckboxResponse {
    /// Whether checkbox was toggled this frame
    pub toggled: bool,
    /// New checked state (if toggled)
    pub new_checked: bool,
    /// Whether checkbox is currently hovered
    pub hovered: bool,
    /// Current widget state (Normal, Hovered, Pressed, etc.)
    pub state: WidgetState,
    /// Checkbox rectangle (for platform rendering)
    pub rect: Rect,
}