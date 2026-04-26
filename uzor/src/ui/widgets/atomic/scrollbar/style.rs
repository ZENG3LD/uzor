//! Scrollbar geometry.

pub trait ScrollbarStyle {
    fn track_thickness(&self) -> f64;
    fn thumb_min_length(&self) -> f64;
    fn thumb_radius(&self) -> f64;
    fn track_padding(&self) -> f64;
    fn overlay_fade_ms(&self) -> u64;
}

pub struct DefaultScrollbarStyle;

impl Default for DefaultScrollbarStyle {
    fn default() -> Self {
        Self
    }
}

impl ScrollbarStyle for DefaultScrollbarStyle {
    fn track_thickness(&self)  -> f64 { 8.0 }
    fn thumb_min_length(&self) -> f64 { 24.0 }
    fn thumb_radius(&self)     -> f64 { 4.0 }
    fn track_padding(&self)    -> f64 { 2.0 }
    fn overlay_fade_ms(&self)  -> u64 { 600 }
}
