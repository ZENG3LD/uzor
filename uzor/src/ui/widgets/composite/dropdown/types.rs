//! Dropdown type definitions - semantic dropdown variants

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Main dropdown type enum covering all dropdown variants
#[derive(Debug, Clone, PartialEq)]
pub enum DropdownType {
    /// Standard text-based dropdown
    Standard {
        selected_index: Option<usize>,
        placeholder: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Grid-based dropdown with visual items
    Grid {
        selected_index: Option<usize>,
        columns: usize,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Layout dropdown with preview icons
    Layout {
        selected_index: Option<usize>,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl WidgetCapabilities for DropdownType {
    fn sense(&self) -> Sense {
        Sense::CLICK
    }
}

impl DropdownType {
    pub fn standard(placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Standard {
            selected_index: None,
            placeholder: placeholder.into(),
            position: (x, y),
            width,
            height,
        }
    }

    pub fn standard_with_selection(
        selected_index: usize,
        placeholder: impl Into<String>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self::Standard {
            selected_index: Some(selected_index),
            placeholder: placeholder.into(),
            position: (x, y),
            width,
            height,
        }
    }

    pub fn grid(columns: usize, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Grid {
            selected_index: None,
            columns,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn grid_with_selection(
        selected_index: usize,
        columns: usize,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self::Grid {
            selected_index: Some(selected_index),
            columns,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn layout(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Layout {
            selected_index: None,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn layout_with_selection(selected_index: usize, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Layout {
            selected_index: Some(selected_index),
            position: (x, y),
            width,
            height,
        }
    }

    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Standard { position, .. } => *position,
            Self::Grid { position, .. } => *position,
            Self::Layout { position, .. } => *position,
        }
    }

    pub fn width(&self) -> f64 {
        match self {
            Self::Standard { width, .. } => *width,
            Self::Grid { width, .. } => *width,
            Self::Layout { width, .. } => *width,
        }
    }

    pub fn height(&self) -> f64 {
        match self {
            Self::Standard { height, .. } => *height,
            Self::Grid { height, .. } => *height,
            Self::Layout { height, .. } => *height,
        }
    }
}
