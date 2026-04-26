//! Slider geometry — values from research cluster-C.

pub trait SliderStyle {
    /// Track height (mlc default 4.0).
    fn track_height(&self) -> f64;
    /// Track corner radius (mlc 2.0 — half height).
    fn track_radius(&self) -> f64;
    /// Handle (thumb) radius (mlc 7.0).
    fn handle_radius(&self) -> f64;
    /// Hover ring extra radius added to `handle_radius` (mlc +2.0).
    fn handle_hover_ring(&self) -> f64;
    /// Handle border thickness (mlc 1.0).
    fn handle_border_width(&self) -> f64;
    /// Gap between label and track (mlc 12.0).
    fn label_spacing(&self) -> f64;
    /// Gap between track and inline value input (mlc 12.0).
    fn track_input_spacing(&self) -> f64;
    /// Width reserved for inline value input (mlc ~60.0 typical).
    fn input_width(&self) -> f64;
    /// Inline input height (mlc 24.0).
    fn input_height(&self) -> f64;
    /// Font size for label / value text (mlc 12.0).
    fn font_size(&self) -> f64;
}

pub struct DefaultSliderStyle;

impl Default for DefaultSliderStyle {
    fn default() -> Self {
        Self
    }
}

impl SliderStyle for DefaultSliderStyle {
    fn track_height(&self)        -> f64 { 4.0 }
    fn track_radius(&self)        -> f64 { 2.0 }
    fn handle_radius(&self)       -> f64 { 7.0 }
    fn handle_hover_ring(&self)   -> f64 { 2.0 }
    fn handle_border_width(&self) -> f64 { 1.0 }
    fn label_spacing(&self)       -> f64 { 12.0 }
    fn track_input_spacing(&self) -> f64 { 12.0 }
    fn input_width(&self)         -> f64 { 60.0 }
    fn input_height(&self)        -> f64 { 24.0 }
    fn font_size(&self)           -> f64 { 12.0 }
}
