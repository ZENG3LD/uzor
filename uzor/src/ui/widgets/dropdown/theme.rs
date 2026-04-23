//! Dropdown Theme Trait - 5-Level Architecture (Level 2)

/// Dropdown theme trait providing colors and dimensions for dropdown rendering.
pub trait DropdownTheme {
    fn dropdown_height(&self) -> f64;
    fn arrow_icon_size(&self) -> f64;
    fn popup_padding(&self) -> f64;
    fn item_height(&self) -> f64;
    fn grid_item_size(&self) -> f64;
    fn background_color(&self) -> [u8; 4];
    fn background_hover_color(&self) -> [u8; 4];
    fn popup_background_color(&self) -> [u8; 4];
    fn border_color(&self) -> [u8; 4];
    fn border_hover_color(&self) -> [u8; 4];
    fn border_focus_color(&self) -> [u8; 4];
    fn selected_item_color(&self) -> [u8; 4];
    fn hover_item_color(&self) -> [u8; 4];
    fn text_color(&self) -> [u8; 4];
    fn text_selected_color(&self) -> [u8; 4];
    fn text_disabled_color(&self) -> [u8; 4];
    fn arrow_color(&self) -> [u8; 4];
    fn arrow_hover_color(&self) -> [u8; 4];
}

/// Default dropdown theme for prototyping and external projects.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultDropdownTheme;

impl DropdownTheme for DefaultDropdownTheme {
    fn dropdown_height(&self) -> f64 { 30.0 }
    fn arrow_icon_size(&self) -> f64 { 8.0 }
    fn popup_padding(&self) -> f64 { 8.0 }
    fn item_height(&self) -> f64 { 28.0 }
    fn grid_item_size(&self) -> f64 { 48.0 }
    fn background_color(&self) -> [u8; 4] { [45, 45, 45, 255] }
    fn background_hover_color(&self) -> [u8; 4] { [55, 55, 55, 255] }
    fn popup_background_color(&self) -> [u8; 4] { [45, 45, 45, 255] }
    fn border_color(&self) -> [u8; 4] { [80, 80, 80, 255] }
    fn border_hover_color(&self) -> [u8; 4] { [100, 100, 100, 255] }
    fn border_focus_color(&self) -> [u8; 4] { [0, 120, 215, 255] }
    fn selected_item_color(&self) -> [u8; 4] { [0, 120, 215, 255] }
    fn hover_item_color(&self) -> [u8; 4] { [60, 60, 60, 255] }
    fn text_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn text_selected_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn text_disabled_color(&self) -> [u8; 4] { [128, 128, 128, 255] }
    fn arrow_color(&self) -> [u8; 4] { [180, 180, 180, 255] }
    fn arrow_hover_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
}
