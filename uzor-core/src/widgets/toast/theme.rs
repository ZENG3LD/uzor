//! Toast theme trait - Contract/Connector for toast colors and dimensions

/// Theme trait for toast colors and dimensions
pub trait ToastTheme {
    fn toast_width(&self) -> f64;
    fn toast_padding(&self) -> f64;
    fn border_radius(&self) -> f64;
    fn icon_size(&self) -> f64;
    fn close_button_size(&self) -> f64;
    fn position_offset(&self) -> (f64, f64);
    fn stack_spacing(&self) -> f64;
    fn info_background(&self) -> [u8; 4];
    fn success_background(&self) -> [u8; 4];
    fn warning_background(&self) -> [u8; 4];
    fn error_background(&self) -> [u8; 4];
    fn text_color(&self) -> [u8; 4];
    fn fade_duration_ms(&self) -> u32;
    fn display_duration_ms(&self) -> u32;
    fn info_icon(&self) -> &str;
    fn success_icon(&self) -> &str;
    fn warning_icon(&self) -> &str;
    fn error_icon(&self) -> &str;
}

/// Default toast theme with designed colors and dimensions
pub struct DefaultToastTheme;

impl DefaultToastTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultToastTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastTheme for DefaultToastTheme {
    fn toast_width(&self) -> f64 { 300.0 }
    fn toast_padding(&self) -> f64 { 16.0 }
    fn border_radius(&self) -> f64 { 8.0 }
    fn icon_size(&self) -> f64 { 20.0 }
    fn close_button_size(&self) -> f64 { 16.0 }
    fn position_offset(&self) -> (f64, f64) { (16.0, 16.0) }
    fn stack_spacing(&self) -> f64 { 8.0 }
    fn info_background(&self) -> [u8; 4] { [30, 100, 200, 255] }
    fn success_background(&self) -> [u8; 4] { [40, 180, 40, 255] }
    fn warning_background(&self) -> [u8; 4] { [255, 180, 0, 255] }
    fn error_background(&self) -> [u8; 4] { [220, 50, 50, 255] }
    fn text_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn fade_duration_ms(&self) -> u32 { 300 }
    fn display_duration_ms(&self) -> u32 { 3000 }
    fn info_icon(&self) -> &str { "\u{2139}\u{FE0F}" }
    fn success_icon(&self) -> &str { "\u{2713}" }
    fn warning_icon(&self) -> &str { "\u{26A0}\u{FE0F}" }
    fn error_icon(&self) -> &str { "\u{2717}" }
}
