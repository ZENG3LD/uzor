//! macOS input theme

use crate::colors::{AppearanceMode, WidgetState, palette};

/// macOS input theme
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct InputTheme {
    pub appearance: AppearanceMode,
}

impl InputTheme {
    pub fn new(appearance: AppearanceMode) -> Self {
        Self { appearance }
    }

    /// Background color based on state
    pub fn bg_color(&self, _state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);
        pal.control_background
    }

    /// Text color based on state
    pub fn text_color(&self, state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);

        if state == WidgetState::Disabled {
            pal.disabled_control_text
        } else {
            pal.control_text
        }
    }

    /// Placeholder text color
    pub fn placeholder_color(&self) -> &'static str {
        let pal = palette(self.appearance);
        pal.placeholder_text
    }

    /// Border color based on state
    pub fn border_color(&self, state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);

        match state {
            WidgetState::Focused => pal.system_blue,
            WidgetState::Disabled => {
                match self.appearance {
                    AppearanceMode::Light => "#E0E0E0FF",
                    AppearanceMode::Dark => "#3A3A3AFF",
                    _ => pal.separator,
                }
            }
            _ => {
                match self.appearance {
                    AppearanceMode::Light => "#C7C7C7FF",
                    AppearanceMode::Dark => "#545458FF",
                    _ => pal.separator,
                }
            }
        }
    }

    /// Focus outline color (3px blue ring)
    pub fn focus_outline_color(&self) -> &'static str {
        let pal = palette(self.appearance);
        pal.system_blue
    }

    /// Border radius
    pub fn border_radius(&self) -> f64 {
        5.0
    }

    /// Border width
    pub fn border_width(&self) -> f64 {
        1.0
    }

    /// Focus outline width
    pub fn focus_outline_width(&self) -> f64 {
        3.0
    }

    /// Focus outline offset
    pub fn focus_outline_offset(&self) -> f64 {
        2.0
    }

    /// Padding (horizontal, vertical)
    pub fn padding(&self) -> (f64, f64) {
        (6.0, 4.0)
    }

    /// Height for single-line input
    pub fn height(&self) -> f64 {
        22.0
    }

    /// Opacity for disabled state
    pub fn disabled_opacity(&self) -> f64 {
        0.6
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_theme_default() {
        let theme = InputTheme::default();
        assert_eq!(theme.appearance, AppearanceMode::default());
    }

    #[test]
    fn test_input_theme_padding() {
        let theme = InputTheme::default();
        let (h, v) = theme.padding();
        assert_eq!(h, 6.0);
        assert_eq!(v, 4.0);
    }

    #[test]
    fn test_input_theme_border_radius() {
        let theme = InputTheme::default();
        assert_eq!(theme.border_radius(), 5.0);
    }

    #[test]
    fn test_input_theme_height() {
        let theme = InputTheme::default();
        assert_eq!(theme.height(), 22.0);
    }

    #[test]
    fn test_input_focus_outline_width() {
        let theme = InputTheme::default();
        assert_eq!(theme.focus_outline_width(), 3.0);
    }

    #[test]
    fn test_input_border_color_changes_on_focus() {
        let theme = InputTheme::new(AppearanceMode::Light);
        let normal_border = theme.border_color(WidgetState::Normal);
        let focus_border = theme.border_color(WidgetState::Focused);
        assert_ne!(normal_border, focus_border);
    }

    #[test]
    fn test_input_disabled_opacity() {
        let theme = InputTheme::default();
        assert_eq!(theme.disabled_opacity(), 0.6);
    }

    #[test]
    fn test_input_placeholder_color() {
        let theme = InputTheme::default();
        let placeholder = theme.placeholder_color();
        assert!(!placeholder.is_empty());
    }
}
