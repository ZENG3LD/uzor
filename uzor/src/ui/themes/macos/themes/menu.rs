//! macOS menu theme

use super::super::colors::{palette, AppearanceMode, WidgetState};

/// Menu item variant
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItemKind {
    /// Regular clickable item
    Action,
    /// Item with submenu indicator (>)
    Submenu,
    /// Toggle item with checkmark
    Toggle { checked: bool },
    /// Section header (non-clickable, styled differently)
    Header,
    /// Separator line
    Separator,
}

/// Menu container theme
pub struct MenuTheme {
    pub mode: AppearanceMode,
}

impl MenuTheme {
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    // Container styling

    /// Semi-transparent background for vibrancy effect
    pub fn container_bg(&self) -> &'static str {
        match self.mode {
            AppearanceMode::Dark
            | AppearanceMode::VibrantDark
            | AppearanceMode::AccessibleDark
            | AppearanceMode::AccessibleVibrantDark => "#242424D9", // rgba(36, 36, 36, 0.85)
            _ => "#F5F5F5E6", // rgba(245, 245, 245, 0.90)
        }
    }

    /// Border radius for the menu container
    pub fn container_border_radius(&self) -> f64 {
        6.0
    }

    /// Vertical and horizontal padding inside the menu
    pub fn container_padding(&self) -> (f64, f64) {
        (4.0, 0.0) // (vertical, horizontal)
    }

    /// Minimum width of the menu
    pub fn container_min_width(&self) -> f64 {
        200.0
    }

    /// Maximum width of the menu
    pub fn container_max_width(&self) -> f64 {
        300.0
    }

    // Menu item styling

    /// Height of a menu item
    pub fn item_height(&self) -> f64 {
        22.0
    }

    /// Padding inside a menu item (vertical, horizontal)
    pub fn item_padding(&self) -> (f64, f64) {
        (0.0, 12.0)
    }

    /// Background color for menu item based on state
    pub fn item_bg(&self, state: WidgetState) -> &'static str {
        let p = palette(self.mode);
        match state {
            WidgetState::Normal | WidgetState::Focused => "transparent",
            WidgetState::Hovered => p.selected_content_background, // Blue
            WidgetState::Pressed => {
                // Slightly darker than hovered
                match self.mode {
                    AppearanceMode::Dark
                    | AppearanceMode::VibrantDark
                    | AppearanceMode::AccessibleDark
                    | AppearanceMode::AccessibleVibrantDark => "#0055CC", // Darker blue
                    _ => "#005ACC", // Darker blue for light mode
                }
            }
            WidgetState::Disabled => "transparent",
        }
    }

    /// Text color for menu item based on state
    pub fn item_text_color(&self, state: WidgetState) -> &'static str {
        let p = palette(self.mode);
        match state {
            WidgetState::Normal | WidgetState::Focused => p.label,
            WidgetState::Hovered | WidgetState::Pressed => p.selected_menu_item_text, // White
            WidgetState::Disabled => p.disabled_control_text,
        }
    }

    /// Font specification for menu items
    pub fn item_font(&self) -> &'static str {
        "13px sans-serif"
    }

    /// Color for keyboard shortcut text
    pub fn item_shortcut_color(&self) -> &'static str {
        palette(self.mode).secondary_label
    }

    /// Color for submenu chevron (>)
    pub fn item_chevron_color(&self) -> &'static str {
        palette(self.mode).tertiary_label
    }

    // Separator styling

    /// Color of the separator line
    pub fn separator_color(&self) -> &'static str {
        palette(self.mode).separator
    }

    /// Height of the separator line
    pub fn separator_height(&self) -> f64 {
        1.0
    }

    /// Horizontal inset from the edges
    pub fn separator_inset(&self) -> f64 {
        8.0
    }

    // Header styling

    /// Text color for section headers
    pub fn header_text_color(&self) -> &'static str {
        palette(self.mode).secondary_label
    }

    /// Font specification for headers
    pub fn header_font(&self) -> &'static str {
        "bold 11px sans-serif"
    }
}

impl Default for MenuTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_theme_dark() {
        let theme = MenuTheme::new(AppearanceMode::Dark);
        assert_eq!(theme.container_bg(), "#242424D9");
        assert_eq!(theme.container_border_radius(), 6.0);
        assert_eq!(theme.item_height(), 22.0);
    }

    #[test]
    fn test_menu_theme_light() {
        let theme = MenuTheme::new(AppearanceMode::Light);
        assert_eq!(theme.container_bg(), "#F5F5F5E6");
        assert_eq!(theme.container_min_width(), 200.0);
        assert_eq!(theme.container_max_width(), 300.0);
    }

    #[test]
    fn test_menu_item_bg_states() {
        let theme = MenuTheme::new(AppearanceMode::Dark);
        assert_eq!(theme.item_bg(WidgetState::Normal), "transparent");
        assert_ne!(theme.item_bg(WidgetState::Hovered), "transparent");
    }

    #[test]
    fn test_menu_item_text_color_hovered() {
        let theme = MenuTheme::new(AppearanceMode::Dark);
        let p = palette(AppearanceMode::Dark);
        assert_eq!(
            theme.item_text_color(WidgetState::Hovered),
            p.selected_menu_item_text
        );
    }

    #[test]
    fn test_separator_styling() {
        let theme = MenuTheme::default();
        assert_eq!(theme.separator_height(), 1.0);
        assert_eq!(theme.separator_inset(), 8.0);
    }
}
