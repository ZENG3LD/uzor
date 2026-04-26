//! Separator/resize handle type definitions

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeparatorType {
    /// Visual divider, no interaction
    Divider {
        orientation: SeparatorOrientation,
    },
    /// Draggable resize handle between panels
    ResizeHandle {
        orientation: SeparatorOrientation,
    },
}

impl WidgetCapabilities for SeparatorType {
    fn sense(&self) -> Sense {
        match self {
            SeparatorType::Divider { .. } => Sense::NONE,
            SeparatorType::ResizeHandle { .. } => Sense::DRAG,
        }
    }
}

impl SeparatorType {
    pub fn horizontal_divider() -> Self {
        Self::Divider { orientation: SeparatorOrientation::Horizontal }
    }

    pub fn vertical_divider() -> Self {
        Self::Divider { orientation: SeparatorOrientation::Vertical }
    }

    pub fn horizontal_resize() -> Self {
        Self::ResizeHandle { orientation: SeparatorOrientation::Horizontal }
    }

    pub fn vertical_resize() -> Self {
        Self::ResizeHandle { orientation: SeparatorOrientation::Vertical }
    }

    pub fn orientation(&self) -> SeparatorOrientation {
        match self {
            Self::Divider { orientation } | Self::ResizeHandle { orientation } => *orientation,
        }
    }

    pub fn is_interactive(&self) -> bool {
        matches!(self, Self::ResizeHandle { .. })
    }
}
