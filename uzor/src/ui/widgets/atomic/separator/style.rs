//! Separator geometry.

pub trait SeparatorStyle {
    /// Line thickness in pixels.
    fn thickness(&self) -> f64;
    /// Margin (perpendicular padding from container edge).
    fn margin(&self) -> f64;
    /// Resize-handle hit area thickness (wider than visible line).
    fn handle_hit_thickness(&self) -> f64;
}

pub struct DefaultSeparatorStyle;

impl Default for DefaultSeparatorStyle {
    fn default() -> Self {
        Self
    }
}

impl SeparatorStyle for DefaultSeparatorStyle {
    fn thickness(&self)            -> f64 { 1.0 }
    fn margin(&self)               -> f64 { 0.0 }
    fn handle_hit_thickness(&self) -> f64 { 6.0 }
}
