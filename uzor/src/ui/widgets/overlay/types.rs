//! Overlay type definitions - elements rendered outside UI layout

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Main overlay type enum covering all overlay variants
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayType {
    /// Generic hover-based tooltip
    Tooltip {
        text: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Informational text overlay
    InfoOverlay {
        text: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl WidgetCapabilities for OverlayType {
    fn sense(&self) -> Sense {
        Sense::HOVER
    }
}

impl OverlayType {
    /// Create a tooltip at position
    pub fn tooltip(text: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Tooltip {
            text: text.into(),
            position: (x, y),
            width,
            height,
        }
    }

    /// Create an info overlay with text
    pub fn info_overlay(text: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::InfoOverlay {
            text: text.into(),
            position: (x, y),
            width,
            height,
        }
    }

    /// Get overlay position
    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Tooltip { position, .. } => *position,
            Self::InfoOverlay { position, .. } => *position,
        }
    }

    /// Get overlay text
    pub fn text(&self) -> &str {
        match self {
            Self::Tooltip { text, .. } => text,
            Self::InfoOverlay { text, .. } => text,
        }
    }

    /// Get overlay width
    pub fn width(&self) -> f64 {
        match self {
            Self::Tooltip { width, .. } => *width,
            Self::InfoOverlay { width, .. } => *width,
        }
    }

    /// Get overlay height
    pub fn height(&self) -> f64 {
        match self {
            Self::Tooltip { height, .. } => *height,
            Self::InfoOverlay { height, .. } => *height,
        }
    }
}
