//! Slider theme trait - Contract/Connector for slider colors and dimensions

/// Theme trait for slider colors and dimensions
pub trait SliderTheme {
    fn track_height(&self) -> f64;
    fn handle_radius(&self) -> f64;
    fn handle_border_width(&self) -> f64;
    fn track_color(&self) -> [u8; 4];
    fn handle_color(&self) -> [u8; 4];
    fn handle_border_color(&self) -> [u8; 4];
    fn active_handle_color(&self) -> [u8; 4];
    fn hover_handle_color(&self) -> [u8; 4];
}

/// Default slider theme using prototype colors
pub struct DefaultSliderTheme;

impl DefaultSliderTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultSliderTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl SliderTheme for DefaultSliderTheme {
    fn track_height(&self) -> f64 { 4.0 }
    fn handle_radius(&self) -> f64 { 6.0 }
    fn handle_border_width(&self) -> f64 { 2.0 }
    fn track_color(&self) -> [u8; 4] { [80, 80, 80, 255] }
    fn handle_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn handle_border_color(&self) -> [u8; 4] { [100, 100, 100, 255] }
    fn active_handle_color(&self) -> [u8; 4] { [0, 120, 215, 255] }
    fn hover_handle_color(&self) -> [u8; 4] { [200, 200, 200, 255] }
}
