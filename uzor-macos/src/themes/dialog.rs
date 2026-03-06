//! macOS dialog theme

use crate::colors::{AppearanceMode, palette};

/// Dialog size variant
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogSize {
    Small,    // 320px wide
    Regular,  // 448px wide
    Large,    // 540px wide
}

impl DialogSize {
    /// Width in pixels for this size variant
    pub fn width(&self) -> f64 {
        match self {
            Self::Small => 320.0,
            Self::Regular => 448.0,
            Self::Large => 540.0,
        }
    }
}

/// Dialog theme configuration
pub struct DialogTheme {
    pub mode: AppearanceMode,
}

impl DialogTheme {
    /// Create a new dialog theme
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    /// Get dialog width for specified size
    pub fn width(&self, size: DialogSize) -> f64 {
        size.width()
    }

    /// Border radius for dialog
    pub fn border_radius(&self) -> f64 {
        10.0
    }

    /// Content padding
    pub fn padding(&self) -> f64 {
        20.0
    }

    /// Background color
    pub fn bg_color(&self) -> &'static str {
        palette(self.mode).window_background
    }

    /// Title font string
    pub fn title_font(&self) -> &'static str {
        "bold 17px sans-serif" // Headline style
    }

    /// Title text color
    pub fn title_color(&self) -> &'static str {
        palette(self.mode).label
    }

    /// Body font string
    pub fn body_font(&self) -> &'static str {
        "13px sans-serif" // Body style
    }

    /// Body text color
    pub fn body_color(&self) -> &'static str {
        palette(self.mode).secondary_label
    }

    /// Overlay backdrop color (40% black)
    pub fn overlay_color(&self) -> &'static str {
        "#00000066"
    }

    /// Shadow color and size (reference to elevation 3)
    /// Returns a tuple of (color, blur_radius, offset_x, offset_y)
    pub fn shadow(&self) -> (&'static str, f64, f64, f64) {
        // ELEVATION_3 equivalent: shadow with moderate depth
        ("#00000040", 20.0, 0.0, 8.0)
    }

    /// Spacing between buttons in dialog footer
    pub fn button_spacing(&self) -> f64 {
        12.0
    }

    /// Animation duration in milliseconds
    pub fn animation_duration_ms(&self) -> u32 {
        300
    }
}

impl Default for DialogTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_sizes() {
        assert_eq!(DialogSize::Small.width(), 320.0);
        assert_eq!(DialogSize::Regular.width(), 448.0);
        assert_eq!(DialogSize::Large.width(), 540.0);
    }

    #[test]
    fn test_dialog_theme_defaults() {
        let theme = DialogTheme::default();
        assert_eq!(theme.border_radius(), 10.0);
        assert_eq!(theme.padding(), 20.0);
        assert_eq!(theme.button_spacing(), 12.0);
        assert_eq!(theme.animation_duration_ms(), 300);
    }

    #[test]
    fn test_dialog_shadow() {
        let theme = DialogTheme::default();
        let (color, blur, x, y) = theme.shadow();
        assert_eq!(color, "#00000040");
        assert_eq!(blur, 20.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 8.0);
    }

    #[test]
    fn test_dialog_fonts() {
        let theme = DialogTheme::default();
        assert_eq!(theme.title_font(), "bold 17px sans-serif");
        assert_eq!(theme.body_font(), "13px sans-serif");
    }
}
