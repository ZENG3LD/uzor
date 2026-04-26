//! Slider colour palette (3 tokens drive everything per research-C).

pub trait SliderTheme {
    /// Empty (right) portion of the track.
    fn track_empty(&self)  -> &str;
    /// Filled (left) portion of the track + hover halo around the handle.
    fn accent(&self)       -> &str;
    /// Handle fill colour + label text.
    fn text_normal(&self)  -> &str;
    /// Disabled overlay opacity / colour fallback.
    fn text_disabled(&self) -> &str;
}

pub struct DefaultSliderTheme;

impl Default for DefaultSliderTheme {
    fn default() -> Self {
        Self
    }
}

impl SliderTheme for DefaultSliderTheme {
    fn track_empty(&self)   -> &str { "#3a3a3a" }
    fn accent(&self)        -> &str { "#2962ff" }
    fn text_normal(&self)   -> &str { "#ffffff" }
    fn text_disabled(&self) -> &str { "#787b86" }
}
