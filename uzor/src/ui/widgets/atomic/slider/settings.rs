//! Bundled per-instance settings.

use super::style::{DefaultSliderStyle, SliderStyle};
use super::theme::{DefaultSliderTheme, SliderTheme};

pub struct SliderSettings {
    pub theme: Box<dyn SliderTheme>,
    pub style: Box<dyn SliderStyle>,
}

impl Default for SliderSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultSliderTheme),
            style: Box::new(DefaultSliderStyle),
        }
    }
}
