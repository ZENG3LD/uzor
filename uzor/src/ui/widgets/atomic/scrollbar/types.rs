//! Scrollbar type definitions

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScrollbarType {
    /// Standard scrollbar always visible
    Standard {
        orientation: ScrollbarOrientation,
    },
    /// Overlay scrollbar (fades in on scroll)
    Overlay {
        orientation: ScrollbarOrientation,
    },
}

impl WidgetCapabilities for ScrollbarType {
    fn sense(&self) -> Sense {
        // Scrollbar as a whole responds to click (track jump) + drag (thumb) + hover (show/hide)
        Sense::CLICK_AND_DRAG
    }
}

impl ScrollbarType {
    pub fn vertical() -> Self {
        Self::Standard { orientation: ScrollbarOrientation::Vertical }
    }

    pub fn horizontal() -> Self {
        Self::Standard { orientation: ScrollbarOrientation::Horizontal }
    }

    pub fn overlay_vertical() -> Self {
        Self::Overlay { orientation: ScrollbarOrientation::Vertical }
    }

    pub fn overlay_horizontal() -> Self {
        Self::Overlay { orientation: ScrollbarOrientation::Horizontal }
    }

    pub fn orientation(&self) -> ScrollbarOrientation {
        match self {
            Self::Standard { orientation } | Self::Overlay { orientation } => *orientation,
        }
    }

    pub fn is_overlay(&self) -> bool {
        matches!(self, Self::Overlay { .. })
    }

    pub fn thumb_sense(&self) -> Sense {
        Sense::DRAG
    }

    pub fn track_sense(&self) -> Sense {
        Sense::CLICK
    }
}
