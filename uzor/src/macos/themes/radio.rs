//! macOS radio theme

use super::super::colors::{AppearanceMode, WidgetState, palette};

/// macOS radio button theme
pub struct RadioTheme {
    pub mode: AppearanceMode,
}

impl RadioTheme {
    /// Creates a new radio theme with the given appearance mode
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    /// Outer circle diameter (16x16 visual per macOS HIG)
    pub fn size(&self) -> f64 {
        16.0
    }

    /// Hit area size (20x20 for easier interaction)
    pub fn hit_size(&self) -> f64 {
        20.0
    }

    /// Inner selection dot diameter (when selected)
    pub fn inner_dot_size(&self) -> f64 {
        6.0
    }

    /// Background color based on selected state and widget state
    pub fn bg_color(&self, selected: bool, state: WidgetState) -> &'static str {
        let p = palette(self.mode);

        if selected {
            match state {
                WidgetState::Pressed => p.control_accent,
                _ => p.control_accent,
            }
        } else {
            // Unselected: use control background
            p.control
        }
    }

    /// Border color based on selected state and widget state
    pub fn border_color(&self, selected: bool, state: WidgetState) -> &'static str {
        if selected {
            // Transparent border when selected (background fills the circle)
            "#00000000"
        } else {
            // Gray border when unselected
            match self.mode {
                AppearanceMode::Light | AppearanceMode::VibrantLight
                | AppearanceMode::AccessibleLight | AppearanceMode::AccessibleVibrantLight => {
                    match state {
                        WidgetState::Hovered => "#A0A0A0",
                        WidgetState::Pressed => "#909090",
                        _ => "#C7C7C7",
                    }
                },
                _ => {
                    // Dark mode
                    match state {
                        WidgetState::Hovered => "#656569",
                        WidgetState::Pressed => "#707074",
                        _ => "#545458",
                    }
                }
            }
        }
    }

    /// Inner dot color (always white on accent background)
    pub fn dot_color(&self) -> &'static str {
        "#FFFFFFFF"
    }

    /// Focus ring color (accent with 50% alpha)
    pub fn focus_ring_color(&self) -> &'static str {
        match self.mode {
            AppearanceMode::Light | AppearanceMode::VibrantLight
            | AppearanceMode::AccessibleLight | AppearanceMode::AccessibleVibrantLight => {
                "#007AFF80" // Light mode accent at 50%
            },
            _ => {
                "#0A84FF80" // Dark mode accent at 50%
            }
        }
    }

    /// Focus ring width
    pub fn focus_ring_width(&self) -> f64 {
        3.0
    }

    /// Border width for radio circle
    pub fn border_width(&self) -> f64 {
        1.0
    }

    /// Disabled state opacity
    pub fn disabled_opacity(&self) -> f64 {
        0.5
    }

    /// Animation duration for selection change in milliseconds
    pub fn animation_duration_ms(&self) -> u64 {
        150
    }

    /// Animation easing curve identifier
    pub fn animation_easing(&self) -> &'static str {
        "ease-in-out"
    }

    /// Spacing between radio and label text
    pub fn label_spacing(&self) -> f64 {
        6.0
    }

    /// Vertical alignment offset for label text
    pub fn label_baseline_offset(&self) -> f64 {
        1.0
    }

    /// Inner dot shadow properties (for depth effect)
    pub fn dot_shadow(&self) -> (f64, f64, f64, &'static str) {
        // (offset_x, offset_y, blur_radius, color)
        (0.0, 0.5, 1.0, "#00000040") // Subtle shadow under dot
    }

    /// Scale animation for dot appearance
    pub fn dot_appear_scale(&self) -> (f64, f64) {
        // (from_scale, to_scale)
        (0.3, 1.0)
    }
}

impl Default for RadioTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_sizes() {
        let theme = RadioTheme::new(AppearanceMode::Light);
        assert_eq!(theme.size(), 16.0);
        assert_eq!(theme.hit_size(), 20.0);
        assert_eq!(theme.inner_dot_size(), 6.0);
    }

    #[test]
    fn test_radio_colors_light_mode() {
        let theme = RadioTheme::new(AppearanceMode::Light);

        // Unselected background should be control color
        let unselected_bg = theme.bg_color(false, WidgetState::Normal);
        assert_eq!(unselected_bg, "#FFFFFF");

        // Selected background should be accent
        let selected_bg = theme.bg_color(true, WidgetState::Normal);
        assert_eq!(selected_bg, "#007AFF");

        // Dot is always white
        assert_eq!(theme.dot_color(), "#FFFFFFFF");
    }

    #[test]
    fn test_radio_colors_dark_mode() {
        let theme = RadioTheme::new(AppearanceMode::Dark);

        // Unselected background
        let unselected_bg = theme.bg_color(false, WidgetState::Normal);
        assert_eq!(unselected_bg, "#3A3A3C");

        // Selected background should be dark mode accent
        let selected_bg = theme.bg_color(true, WidgetState::Normal);
        assert_eq!(selected_bg, "#0A84FF");
    }

    #[test]
    fn test_radio_border_colors() {
        let theme = RadioTheme::new(AppearanceMode::Light);

        // Selected border is transparent
        let selected_border = theme.border_color(true, WidgetState::Normal);
        assert_eq!(selected_border, "#00000000");

        // Unselected border is gray
        let unselected_border = theme.border_color(false, WidgetState::Normal);
        assert_eq!(unselected_border, "#C7C7C7");
    }

    #[test]
    fn test_focus_ring() {
        let theme = RadioTheme::new(AppearanceMode::Light);
        assert_eq!(theme.focus_ring_width(), 3.0);
        assert_eq!(theme.focus_ring_color(), "#007AFF80");
    }

    #[test]
    fn test_dot_animation() {
        let theme = RadioTheme::new(AppearanceMode::Light);
        let (from, to) = theme.dot_appear_scale();
        assert_eq!(from, 0.3);
        assert_eq!(to, 1.0);
    }
}
