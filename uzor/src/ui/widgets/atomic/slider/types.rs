//! Slider variant catalog.
//!
//! Layout (rect) is layout-layer concern, not widget data.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Slider variants.
#[derive(Debug, Clone, PartialEq)]
pub enum SliderType {
    /// Single-handle slider (one value).
    Single { value: f64, min: f64, max: f64, step: f64 },
    /// Dual-handle range slider (min..max).
    Dual {
        min_value: f64,
        max_value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
}

impl WidgetCapabilities for SliderType {
    fn sense(&self) -> Sense {
        Sense::CLICK_AND_DRAG
    }
}

impl SliderType {
    pub fn single(value: f64, min: f64, max: f64) -> Self {
        Self::Single { value, min, max, step: 1.0 }
    }

    pub fn dual(min_value: f64, max_value: f64, min: f64, max: f64) -> Self {
        Self::Dual { min_value, max_value, min, max, step: 1.0 }
    }
}

/// Which handle is active during a drag of a `Dual` slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualSliderHandle {
    Min,
    Max,
}
