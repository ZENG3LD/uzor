//! Slider type definitions - semantic slider variants

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Main slider type enum covering all slider variants
#[derive(Debug, Clone, PartialEq)]
pub enum SliderType {
    /// Single-point slider with one handle
    Single {
        value: f64,
        min: f64,
        max: f64,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Dual-point slider with two handles for range selection
    Dual {
        min_value: f64,
        max_value: f64,
        min: f64,
        max: f64,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl WidgetCapabilities for SliderType {
    fn sense(&self) -> Sense {
        Sense::CLICK_AND_DRAG
    }
}

impl SliderType {
    pub fn single(value: f64, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Single {
            value,
            min: 0.0,
            max: 1.0,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn single_with_range(
        value: f64,
        min: f64,
        max: f64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self::Single {
            value,
            min,
            max,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn dual(
        min_value: f64,
        max_value: f64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self::Dual {
            min_value,
            max_value,
            min: 0.0,
            max: 1.0,
            position: (x, y),
            width,
            height,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dual_with_range(
        min_value: f64,
        max_value: f64,
        min: f64,
        max: f64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self::Dual {
            min_value,
            max_value,
            min,
            max,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Single { position, .. } => *position,
            Self::Dual { position, .. } => *position,
        }
    }

    pub fn width(&self) -> f64 {
        match self {
            Self::Single { width, .. } => *width,
            Self::Dual { width, .. } => *width,
        }
    }

    pub fn height(&self) -> f64 {
        match self {
            Self::Single { height, .. } => *height,
            Self::Dual { height, .. } => *height,
        }
    }
}
