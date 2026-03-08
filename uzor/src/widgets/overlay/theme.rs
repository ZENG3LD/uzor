//! Overlay theme trait - Contract/Connector for overlay styling

/// Theme trait for overlay styling
pub trait OverlayTheme {
    fn tooltip_padding(&self) -> f64;
    fn tooltip_max_width(&self) -> f64;
    fn background_color(&self) -> &str;
    fn text_color(&self) -> &str;
    fn border_color(&self) -> &str;
    fn shadow_color(&self) -> &str;
    fn border_width(&self) -> f64;
    fn shadow_blur(&self) -> f64;
    fn shadow_offset(&self) -> (f64, f64);
    fn fade_in_duration_ms(&self) -> u32;
    fn hover_delay_ms(&self) -> u32;
}

/// Default overlay theme using design specification values
pub struct DefaultOverlayTheme;

impl DefaultOverlayTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultOverlayTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayTheme for DefaultOverlayTheme {
    fn tooltip_padding(&self) -> f64 { 8.0 }
    fn tooltip_max_width(&self) -> f64 { 200.0 }
    fn background_color(&self) -> &str { "#323232" }
    fn text_color(&self) -> &str { "#ffffff" }
    fn border_color(&self) -> &str { "#505050" }
    fn shadow_color(&self) -> &str { "#00000080" }
    fn border_width(&self) -> f64 { 1.0 }
    fn shadow_blur(&self) -> f64 { 4.0 }
    fn shadow_offset(&self) -> (f64, f64) { (0.0, 2.0) }
    fn fade_in_duration_ms(&self) -> u32 { 200 }
    fn hover_delay_ms(&self) -> u32 { 300 }
}
