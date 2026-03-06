//! macOS tabs theme

use crate::colors::{palette, AppearanceMode, WidgetState};

/// Tab container and item theme
pub struct TabTheme {
    pub mode: AppearanceMode,
}

impl TabTheme {
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    // Container styling

    /// Background color for the tab container
    pub fn container_bg(&self) -> &'static str {
        palette(self.mode).fill_secondary
    }

    /// Border radius for the tab container
    pub fn container_border_radius(&self) -> f64 {
        6.0
    }

    /// Height of the tab container
    pub fn container_height(&self) -> f64 {
        28.0
    }

    /// Padding inside the tab container
    pub fn container_padding(&self) -> f64 {
        2.0
    }

    // Tab item styling

    /// Background color for individual tab based on selection and state
    pub fn tab_bg(&self, selected: bool, state: WidgetState) -> &'static str {
        let p = palette(self.mode);

        if selected {
            p.control_background
        } else {
            match state {
                WidgetState::Hovered => p.fill_tertiary,
                _ => "transparent",
            }
        }
    }

    /// Text color for tab based on selection and state
    pub fn tab_text_color(&self, selected: bool, state: WidgetState) -> &'static str {
        let p = palette(self.mode);

        match state {
            WidgetState::Disabled => p.disabled_control_text,
            _ => {
                if selected {
                    p.label
                } else {
                    p.secondary_label
                }
            }
        }
    }

    /// Border radius for individual tabs
    pub fn tab_border_radius(&self) -> f64 {
        5.0
    }

    /// Padding inside a tab (vertical, horizontal)
    pub fn tab_padding(&self) -> (f64, f64) {
        (4.0, 12.0)
    }

    /// Font specification for tab text
    pub fn tab_font(&self) -> &'static str {
        "13px sans-serif"
    }

    /// Spacing between icon and label (if tab has an icon)
    pub fn tab_icon_spacing(&self) -> f64 {
        6.0
    }

    /// Height for small tabs
    pub fn container_height_small(&self) -> f64 {
        24.0
    }

    /// Height for large tabs
    pub fn container_height_large(&self) -> f64 {
        32.0
    }
}

impl Default for TabTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_theme_dark() {
        let theme = TabTheme::new(AppearanceMode::Dark);
        assert_eq!(theme.container_border_radius(), 6.0);
        assert_eq!(theme.container_height(), 28.0);
        assert_eq!(theme.tab_border_radius(), 5.0);
    }

    #[test]
    fn test_tab_theme_light() {
        let theme = TabTheme::new(AppearanceMode::Light);
        assert_eq!(theme.container_padding(), 2.0);
        assert_eq!(theme.tab_padding(), (4.0, 12.0));
    }

    #[test]
    fn test_tab_bg_selected() {
        let theme = TabTheme::new(AppearanceMode::Dark);
        let p = palette(AppearanceMode::Dark);

        assert_eq!(
            theme.tab_bg(true, WidgetState::Normal),
            p.control_background
        );
        assert_eq!(
            theme.tab_bg(false, WidgetState::Normal),
            "transparent"
        );
    }

    #[test]
    fn test_tab_bg_hover() {
        let theme = TabTheme::new(AppearanceMode::Light);
        let p = palette(AppearanceMode::Light);

        assert_eq!(
            theme.tab_bg(false, WidgetState::Hovered),
            p.fill_tertiary
        );
    }

    #[test]
    fn test_tab_text_color() {
        let theme = TabTheme::default();
        let p = palette(AppearanceMode::Dark);

        assert_eq!(
            theme.tab_text_color(true, WidgetState::Normal),
            p.label
        );
        assert_eq!(
            theme.tab_text_color(false, WidgetState::Normal),
            p.secondary_label
        );
    }

    #[test]
    fn test_tab_text_color_disabled() {
        let theme = TabTheme::new(AppearanceMode::Light);
        let p = palette(AppearanceMode::Light);

        assert_eq!(
            theme.tab_text_color(true, WidgetState::Disabled),
            p.disabled_control_text
        );
        assert_eq!(
            theme.tab_text_color(false, WidgetState::Disabled),
            p.disabled_control_text
        );
    }

    #[test]
    fn test_tab_font() {
        let theme = TabTheme::default();
        assert_eq!(theme.tab_font(), "13px sans-serif");
    }

    #[test]
    fn test_tab_sizing() {
        let theme = TabTheme::default();
        assert_eq!(theme.container_height_small(), 24.0);
        assert_eq!(theme.container_height(), 28.0);
        assert_eq!(theme.container_height_large(), 32.0);
    }
}
