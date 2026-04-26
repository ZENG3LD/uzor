//! Tooltip geometry.

pub trait TooltipStyle {
    fn radius(&self)         -> f64;
    fn padding_x(&self)      -> f64;
    fn padding_y(&self)      -> f64;
    fn font_size(&self)      -> f64;
    fn border_width(&self)   -> f64;
    fn anchor_gap(&self)     -> f64;
    fn fade_duration_ms(&self) -> f64;
}

pub struct DefaultTooltipStyle;

impl Default for DefaultTooltipStyle {
    fn default() -> Self {
        Self
    }
}

impl TooltipStyle for DefaultTooltipStyle {
    fn radius(&self)           -> f64 { 4.0 }
    fn padding_x(&self)        -> f64 { 8.0 }
    fn padding_y(&self)        -> f64 { 4.0 }
    fn font_size(&self)        -> f64 { 12.0 }
    fn border_width(&self)     -> f64 { 1.0 }
    fn anchor_gap(&self)       -> f64 { 6.0 }
    fn fade_duration_ms(&self) -> f64 { 150.0 }
}
