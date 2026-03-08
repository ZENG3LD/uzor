//! macOS button theme

use super::super::colors::{AppearanceMode, WidgetState, palette};

/// Button variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Default,
    Accent,
    Destructive,
}

/// Button size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Regular,
    Large,
}

/// macOS button theme
#[derive(Debug, Clone)]
pub struct ButtonTheme {
    pub variant: ButtonVariant,
    pub size: ButtonSize,
    pub appearance: AppearanceMode,
}

impl ButtonTheme {
    pub fn new(variant: ButtonVariant, appearance: AppearanceMode) -> Self {
        Self {
            variant,
            size: ButtonSize::Regular,
            appearance,
        }
    }

    pub fn with_size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Background color based on state
    pub fn bg_color(&self, state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);

        match (self.variant, state) {
            // Default variant
            (ButtonVariant::Default, WidgetState::Normal) => pal.control,
            (ButtonVariant::Default, WidgetState::Hovered) => {
                match self.appearance {
                    AppearanceMode::Light => "#E8E8E8FF",
                    AppearanceMode::Dark => "#3A3A3AFF",
                    _ => pal.control,
                }
            }
            (ButtonVariant::Default, WidgetState::Pressed) => {
                match self.appearance {
                    AppearanceMode::Light => "#D8D8D8FF",
                    AppearanceMode::Dark => "#4A4A4AFF",
                    _ => pal.control,
                }
            }
            (ButtonVariant::Default, WidgetState::Disabled) => pal.control,
            (ButtonVariant::Default, WidgetState::Focused) => pal.control,

            // Accent variant (blue primary)
            (ButtonVariant::Accent, WidgetState::Normal) => pal.system_blue,
            (ButtonVariant::Accent, WidgetState::Hovered) => {
                match self.appearance {
                    AppearanceMode::Light => "#0070E0FF",
                    AppearanceMode::Dark => "#0070E0FF",
                    _ => pal.system_blue,
                }
            }
            (ButtonVariant::Accent, WidgetState::Pressed) => {
                match self.appearance {
                    AppearanceMode::Light => "#0063C6FF",
                    AppearanceMode::Dark => "#0068D5FF",
                    _ => pal.system_blue,
                }
            }
            (ButtonVariant::Accent, WidgetState::Disabled) => pal.system_blue,
            (ButtonVariant::Accent, WidgetState::Focused) => pal.system_blue,

            // Destructive variant (red)
            (ButtonVariant::Destructive, WidgetState::Normal) => pal.system_red,
            (ButtonVariant::Destructive, WidgetState::Hovered) => {
                match self.appearance {
                    AppearanceMode::Light => "#FF3020FF",
                    AppearanceMode::Dark => "#FF4030FF",
                    _ => pal.system_red,
                }
            }
            (ButtonVariant::Destructive, WidgetState::Pressed) => {
                match self.appearance {
                    AppearanceMode::Light => "#E63020FF",
                    AppearanceMode::Dark => "#E63828FF",
                    _ => pal.system_red,
                }
            }
            (ButtonVariant::Destructive, WidgetState::Disabled) => pal.system_red,
            (ButtonVariant::Destructive, WidgetState::Focused) => pal.system_red,
        }
    }

    /// Text color based on state
    pub fn text_color(&self, state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);

        match self.variant {
            ButtonVariant::Default => {
                if state == WidgetState::Disabled {
                    pal.disabled_control_text
                } else {
                    pal.control_text
                }
            }
            ButtonVariant::Accent | ButtonVariant::Destructive => {
                if state == WidgetState::Disabled {
                    "#FFFFFF80"
                } else {
                    "#FFFFFFFF"
                }
            }
        }
    }

    /// Border color based on state
    pub fn border_color(&self, state: WidgetState) -> &'static str {
        let pal = palette(self.appearance);

        match self.variant {
            ButtonVariant::Default => {
                if state == WidgetState::Focused {
                    pal.system_blue
                } else if state == WidgetState::Disabled {
                    "#00000010"
                } else {
                    match self.appearance {
                        AppearanceMode::Light => "#C7C7C7FF",
                        AppearanceMode::Dark => "#545458FF",
                        _ => pal.separator,
                    }
                }
            }
            ButtonVariant::Accent | ButtonVariant::Destructive => {
                "#00000000" // Transparent - no visible border
            }
        }
    }

    /// Focus ring color
    pub fn focus_ring_color(&self) -> &'static str {
        let pal = palette(self.appearance);
        pal.system_blue
    }

    /// Padding (horizontal, vertical)
    pub fn padding(&self) -> (f64, f64) {
        match self.size {
            ButtonSize::Small => (8.0, 4.0),
            ButtonSize::Regular => (12.0, 6.0),
            ButtonSize::Large => (16.0, 8.0),
        }
    }

    /// Border radius
    pub fn border_radius(&self) -> f64 {
        6.0
    }

    /// Border width
    pub fn border_width(&self) -> f64 {
        match self.variant {
            ButtonVariant::Default => 0.5,
            ButtonVariant::Accent | ButtonVariant::Destructive => 0.0,
        }
    }

    /// Minimum height
    pub fn min_height(&self) -> f64 {
        match self.size {
            ButtonSize::Small => 22.0,
            ButtonSize::Regular => 28.0,
            ButtonSize::Large => 34.0,
        }
    }

    /// Focus ring offset from button edge
    pub fn focus_ring_offset(&self) -> f64 {
        2.0
    }

    /// Focus ring width
    pub fn focus_ring_width(&self) -> f64 {
        3.0
    }

    /// Opacity for disabled state
    pub fn disabled_opacity(&self) -> f64 {
        0.5
    }
}

impl Default for ButtonTheme {
    fn default() -> Self {
        Self {
            variant: ButtonVariant::Default,
            size: ButtonSize::Regular,
            appearance: AppearanceMode::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_theme_default() {
        let theme = ButtonTheme::default();
        assert_eq!(theme.variant, ButtonVariant::Default);
        assert_eq!(theme.size, ButtonSize::Regular);
    }

    #[test]
    fn test_button_theme_padding() {
        let theme = ButtonTheme::default();
        let (h, v) = theme.padding();
        assert_eq!(h, 12.0);
        assert_eq!(v, 6.0);
    }

    #[test]
    fn test_button_theme_sizes() {
        let small = ButtonTheme::default().with_size(ButtonSize::Small);
        let regular = ButtonTheme::default().with_size(ButtonSize::Regular);
        let large = ButtonTheme::default().with_size(ButtonSize::Large);

        assert_eq!(small.min_height(), 22.0);
        assert_eq!(regular.min_height(), 28.0);
        assert_eq!(large.min_height(), 34.0);
    }

    #[test]
    fn test_button_accent_no_border() {
        let theme = ButtonTheme::new(ButtonVariant::Accent, AppearanceMode::Light);
        assert_eq!(theme.border_width(), 0.0);
    }

    #[test]
    fn test_button_default_has_border() {
        let theme = ButtonTheme::new(ButtonVariant::Default, AppearanceMode::Light);
        assert_eq!(theme.border_width(), 0.5);
    }

    #[test]
    fn test_button_disabled_opacity() {
        let theme = ButtonTheme::default();
        assert_eq!(theme.disabled_opacity(), 0.5);
    }
}
