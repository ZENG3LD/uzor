//! Toast geometry.

pub trait ToastStyle {
    fn padding(&self)     -> f64;
    fn radius(&self)      -> f64;
    fn icon_size(&self)   -> f64;
    fn font_size(&self)   -> f64;
    fn fade_duration_ms(&self) -> u32;
}

pub struct DefaultToastStyle;

impl Default for DefaultToastStyle {
    fn default() -> Self {
        Self
    }
}

impl ToastStyle for DefaultToastStyle {
    fn padding(&self)          -> f64 { 16.0 }
    fn radius(&self)           -> f64 { 8.0 }
    fn icon_size(&self)        -> f64 { 20.0 }
    fn font_size(&self)        -> f64 { 13.0 }
    fn fade_duration_ms(&self) -> u32 { 300 }
}
