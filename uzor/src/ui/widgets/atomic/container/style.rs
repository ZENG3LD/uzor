//! Container geometry.

pub trait ContainerStyle {
    fn radius(&self)         -> f64;
    fn padding(&self)        -> f64;
    fn border_width(&self)   -> f64;
    fn shadow_offset(&self)  -> (f64, f64);
    fn shadow_alpha(&self)   -> f64;
}

pub struct DefaultContainerStyle;

impl Default for DefaultContainerStyle {
    fn default() -> Self {
        Self
    }
}

impl ContainerStyle for DefaultContainerStyle {
    fn radius(&self)        -> f64 { 4.0 }
    fn padding(&self)       -> f64 { 8.0 }
    fn border_width(&self)  -> f64 { 1.0 }
    fn shadow_offset(&self) -> (f64, f64) { (0.0, 2.0) }
    fn shadow_alpha(&self)  -> f64 { 0.25 }
}
