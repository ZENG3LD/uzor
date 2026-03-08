//! Text input theme trait - Contract/Connector for text input colors and dimensions

/// Theme trait for text input colors and dimensions
pub trait TextInputTheme {
    fn input_height(&self) -> f64;
    fn padding(&self) -> f64;
    fn cursor_width(&self) -> f64;
    fn background_color(&self) -> [u8; 4];
    fn border_color(&self) -> [u8; 4];
    fn focused_border_color(&self) -> [u8; 4];
    fn text_color(&self) -> [u8; 4];
    fn placeholder_color(&self) -> [u8; 4];
    fn selection_color(&self) -> [u8; 4];
    fn disabled_opacity(&self) -> f64;
}

/// Default text input theme
pub struct DefaultTextInputTheme;

impl DefaultTextInputTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultTextInputTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputTheme for DefaultTextInputTheme {
    fn input_height(&self) -> f64 { 30.0 }
    fn padding(&self) -> f64 { 8.0 }
    fn cursor_width(&self) -> f64 { 1.0 }
    fn background_color(&self) -> [u8; 4] { [45, 45, 45, 255] }
    fn border_color(&self) -> [u8; 4] { [80, 80, 80, 255] }
    fn focused_border_color(&self) -> [u8; 4] { [0, 120, 215, 255] }
    fn text_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn placeholder_color(&self) -> [u8; 4] { [128, 128, 128, 255] }
    fn selection_color(&self) -> [u8; 4] { [0, 120, 215, 128] }
    fn disabled_opacity(&self) -> f64 { 0.5 }
}
