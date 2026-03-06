//! macOS checkbox theme

use crate::colors::{AppearanceMode, WidgetState, palette};

/// Checkbox state enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckboxState {
    /// Unchecked (empty box)
    Unchecked,
    /// Checked (checkmark visible)
    Checked,
    /// Mixed/Indeterminate (dash visible)
    Mixed,
}

/// macOS checkbox theme
pub struct CheckboxTheme {
    pub mode: AppearanceMode,
}

impl CheckboxTheme {
    /// Creates a new checkbox theme with the given appearance mode
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    /// Visual checkbox size (14x14 per macOS HIG)
    pub fn size(&self) -> f64 {
        14.0
    }

    /// Hit area size (20x20 for easier interaction)
    pub fn hit_size(&self) -> f64 {
        20.0
    }

    /// Border radius for checkbox corners
    pub fn border_radius(&self) -> f64 {
        3.0
    }

    /// Background color based on checked state and widget state
    pub fn bg_color(&self, checked: bool, state: WidgetState) -> &'static str {
        let p = palette(self.mode);

        if checked {
            match state {
                WidgetState::Pressed => p.control_accent, // Slightly darker on press
                _ => p.control_accent,
            }
        } else {
            // Unchecked: use control background
            p.control
        }
    }

    /// Border color based on checked state and widget state
    pub fn border_color(&self, checked: bool, state: WidgetState) -> &'static str {
        if checked {
            // Transparent border when checked (background fills the entire box)
            "#00000000"
        } else {
            // Gray border when unchecked
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

    /// Checkmark color (always white on accent background)
    pub fn checkmark_color(&self) -> &'static str {
        "#FFFFFFFF"
    }

    /// Focus ring color (accent with 50% alpha)
    pub fn focus_ring_color(&self) -> &'static str {
        // Return accent with 50% opacity
        // Note: This would need the color_with_alpha helper from colors/helpers.rs
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

    /// Border width for checkbox
    pub fn border_width(&self) -> f64 {
        1.0
    }

    /// Disabled state opacity
    pub fn disabled_opacity(&self) -> f64 {
        0.5
    }

    /// Checkmark path scale factor (relative to checkbox size)
    pub fn checkmark_scale(&self) -> f64 {
        0.7
    }

    /// Mixed state dash dimensions (relative to checkbox size)
    pub fn mixed_dash_scale(&self) -> (f64, f64) {
        (0.6, 0.15) // (width_scale, height_scale)
    }

    /// Animation duration for check/uncheck in milliseconds
    pub fn animation_duration_ms(&self) -> u64 {
        150
    }

    /// Animation easing curve identifier
    pub fn animation_easing(&self) -> &'static str {
        "ease-in-out"
    }

    /// Spacing between checkbox and label text
    pub fn label_spacing(&self) -> f64 {
        6.0
    }

    /// Vertical alignment offset for label text
    pub fn label_baseline_offset(&self) -> f64 {
        1.0
    }
}

impl Default for CheckboxTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkbox_sizes() {
        let theme = CheckboxTheme::new(AppearanceMode::Light);
        assert_eq!(theme.size(), 14.0);
        assert_eq!(theme.hit_size(), 20.0);
        assert_eq!(theme.border_radius(), 3.0);
    }

    #[test]
    fn test_checkbox_colors_light_mode() {
        let theme = CheckboxTheme::new(AppearanceMode::Light);

        // Unchecked background should be control color
        let unchecked_bg = theme.bg_color(false, WidgetState::Normal);
        assert_eq!(unchecked_bg, "#FFFFFF");

        // Checked background should be accent
        let checked_bg = theme.bg_color(true, WidgetState::Normal);
        assert_eq!(checked_bg, "#007AFF");

        // Checkmark is always white
        assert_eq!(theme.checkmark_color(), "#FFFFFFFF");
    }

    #[test]
    fn test_checkbox_colors_dark_mode() {
        let theme = CheckboxTheme::new(AppearanceMode::Dark);

        // Unchecked background
        let unchecked_bg = theme.bg_color(false, WidgetState::Normal);
        assert_eq!(unchecked_bg, "#3A3A3C");

        // Checked background should be dark mode accent
        let checked_bg = theme.bg_color(true, WidgetState::Normal);
        assert_eq!(checked_bg, "#0A84FF");
    }

    #[test]
    fn test_checkbox_border_colors() {
        let theme = CheckboxTheme::new(AppearanceMode::Light);

        // Checked border is transparent
        let checked_border = theme.border_color(true, WidgetState::Normal);
        assert_eq!(checked_border, "#00000000");

        // Unchecked border is gray
        let unchecked_border = theme.border_color(false, WidgetState::Normal);
        assert_eq!(unchecked_border, "#C7C7C7");
    }

    #[test]
    fn test_focus_ring() {
        let theme = CheckboxTheme::new(AppearanceMode::Light);
        assert_eq!(theme.focus_ring_width(), 3.0);
        assert_eq!(theme.focus_ring_color(), "#007AFF80");
    }
}
