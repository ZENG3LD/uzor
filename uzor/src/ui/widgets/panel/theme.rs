//! Panel theme trait - Contract/Connector for panel colors and dimensions

/// Theme trait for panel colors and dimensions
pub trait PanelTheme {
    // Toolbar dimensions
    fn toolbar_height(&self) -> f64;
    fn toolbar_width(&self) -> f64;
    fn toolbar_blur(&self) -> bool;

    // Sidebar dimensions
    fn sidebar_width(&self) -> f64;
    fn sidebar_header_height(&self) -> f64;
    fn sidebar_item_height(&self) -> f64;
    fn sidebar_section_height(&self) -> f64;
    fn sidebar_logo_size(&self) -> f64;

    // Modal dimensions
    fn modal_max_width(&self) -> f64;
    fn modal_padding(&self) -> f64;
    fn modal_backdrop_color(&self) -> [u8; 4];
    fn modal_header_height(&self) -> f64;
    fn modal_tab_height(&self) -> f64;
    fn modal_content_row_height(&self) -> f64;
    fn modal_content_padding(&self) -> f64;

    // Hideable dimensions
    fn hideable_chevron_size(&self) -> f64;
    fn hideable_button_height(&self) -> f64;
    fn hideable_row_height(&self) -> f64;
    fn hideable_row_gap(&self) -> f64;
    fn hideable_icon_size(&self) -> f64;
    fn hideable_border_width(&self) -> f64;
    fn hideable_border_opacity(&self) -> f64;
    fn hideable_button_padding(&self) -> f64;

    // Common colors
    fn background_color(&self) -> [u8; 4];
    fn border_color(&self) -> [u8; 4];
    fn text_color(&self) -> [u8; 4];
    fn text_secondary_color(&self) -> [u8; 4];
    fn hover_color(&self) -> [u8; 4];
}

/// Default panel theme using values from inline specs
pub struct DefaultPanelTheme;

impl DefaultPanelTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultPanelTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelTheme for DefaultPanelTheme {
    fn toolbar_height(&self) -> f64 { 40.0 }
    fn toolbar_width(&self) -> f64 { 40.0 }
    fn toolbar_blur(&self) -> bool { true }

    fn sidebar_width(&self) -> f64 { 250.0 }
    fn sidebar_header_height(&self) -> f64 { 56.0 }
    fn sidebar_item_height(&self) -> f64 { 40.0 }
    fn sidebar_section_height(&self) -> f64 { 32.0 }
    fn sidebar_logo_size(&self) -> f64 { 40.0 }

    fn modal_max_width(&self) -> f64 { 600.0 }
    fn modal_padding(&self) -> f64 { 24.0 }
    fn modal_backdrop_color(&self) -> [u8; 4] { [0, 0, 0, 128] }
    fn modal_header_height(&self) -> f64 { 36.0 }
    fn modal_tab_height(&self) -> f64 { 32.0 }
    fn modal_content_row_height(&self) -> f64 { 28.0 }
    fn modal_content_padding(&self) -> f64 { 12.0 }

    fn hideable_chevron_size(&self) -> f64 { 12.0 }
    fn hideable_button_height(&self) -> f64 { 18.0 }
    fn hideable_row_height(&self) -> f64 { 20.0 }
    fn hideable_row_gap(&self) -> f64 { 2.0 }
    fn hideable_icon_size(&self) -> f64 { 14.0 }
    fn hideable_border_width(&self) -> f64 { 0.5 }
    fn hideable_border_opacity(&self) -> f64 { 0.8 }
    fn hideable_button_padding(&self) -> f64 { 6.0 }

    fn background_color(&self) -> [u8; 4] { [30, 30, 30, 255] }
    fn border_color(&self) -> [u8; 4] { [80, 80, 80, 255] }
    fn text_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn text_secondary_color(&self) -> [u8; 4] { [128, 128, 128, 255] }
    fn hover_color(&self) -> [u8; 4] { [50, 50, 50, 255] }
}
