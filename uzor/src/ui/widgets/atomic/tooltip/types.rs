//! Tooltip type definitions.

use crate::types::Rect;

/// Where the tooltip appears relative to its anchor rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPosition {
    Above,
    Below,
    Left,
    Right,
}

/// Configuration for a tooltip popup.
#[derive(Debug, Clone)]
pub struct TooltipConfig {
    /// Text to display inside the tooltip.
    pub text: String,
    /// Rect of the widget that triggered the tooltip (screen coords).
    pub anchor: Rect,
    /// Preferred placement relative to the anchor.
    pub position: TooltipPosition,
}

impl TooltipConfig {
    pub fn new(text: impl Into<String>, anchor: Rect, position: TooltipPosition) -> Self {
        Self { text: text.into(), anchor, position }
    }

    pub fn above(text: impl Into<String>, anchor: Rect) -> Self {
        Self::new(text, anchor, TooltipPosition::Above)
    }

    pub fn below(text: impl Into<String>, anchor: Rect) -> Self {
        Self::new(text, anchor, TooltipPosition::Below)
    }
}

/// Output of a tooltip — no interactive events, provided for completeness.
#[derive(Debug, Clone, Default)]
pub struct TooltipResponse {
    /// Whether the tooltip is currently visible (past the show delay).
    pub visible: bool,
}
