//! RadioGroup widget layout and configuration
//!
//! Provides radio group configuration and response types for headless architecture.
//! Rendering is delegated to platform-specific implementations.

use crate::types::{WidgetState, Rect};
use serde::{Deserialize, Serialize};

/// A single option within a radio group
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadioOption {
    /// Unique key identifying this option
    pub key: String,
    /// Display label shown next to the radio circle
    pub label: String,
    /// Optional descriptive text shown below the label
    pub description: String,
}

impl RadioOption {
    pub fn new(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            description: description.to_string(),
        }
    }
}

/// Radio group configuration — exactly one option selected at a time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadioGroupConfig {
    /// All selectable options
    pub options: Vec<RadioOption>,
    /// Index of the currently selected option
    pub selected_index: usize,
    /// Whether the entire group is disabled
    pub disabled: bool,
    /// Height of each option row (includes label + description)
    pub item_height: f64,
    /// Gap between options
    pub gap: f64,
    /// Radio circle radius
    pub circle_radius: f64,
}

impl Default for RadioGroupConfig {
    fn default() -> Self {
        Self {
            options: Vec::new(),
            selected_index: 0,
            disabled: false,
            item_height: 52.0,
            gap: 8.0,
            circle_radius: 8.0,
        }
    }
}

impl RadioGroupConfig {
    pub fn new(options: Vec<RadioOption>) -> Self {
        Self { options, ..Default::default() }
    }

    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the key of the currently selected option, if any
    pub fn selected_key(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(|o| o.key.as_str())
    }

    /// Computes total height of the group including all items and gaps
    pub fn total_height(&self) -> f64 {
        let n = self.options.len() as f64;
        if n == 0.0 {
            return 0.0;
        }
        n * self.item_height + (n - 1.0) * self.gap
    }
}

/// Response from a radio group interaction
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RadioGroupResponse {
    /// Index of newly selected option, if selection changed this frame
    pub changed: Option<usize>,
    /// Currently hovered option index
    pub hovered_index: Option<usize>,
    /// Current widget state (Normal, Hovered, Pressed, etc.)
    pub state: WidgetState,
    /// Overall bounding rect of the entire group
    pub rect: Rect,
    /// Per-option rects for hit zone registration
    pub option_rects: Vec<Rect>,
}
