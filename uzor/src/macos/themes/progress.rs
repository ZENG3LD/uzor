//! macOS progress theme

use super::super::colors::{AppearanceMode, palette};

/// Progress bar size
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressSize {
    Small,
    Regular,
    Large,
}

/// Progress style
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressStyle {
    Bar,
    Ring,
}

/// Progress theme configuration
pub struct ProgressTheme {
    pub mode: AppearanceMode,
}

impl ProgressTheme {
    /// Create a new progress theme
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    // --- Bar progress methods ---

    /// Bar height for specified size
    pub fn bar_height(&self, size: ProgressSize) -> f64 {
        match size {
            ProgressSize::Small => 2.0,
            ProgressSize::Regular => 4.0,
            ProgressSize::Large => 6.0,
        }
    }

    /// Bar border radius (pill shape = height/2)
    pub fn bar_border_radius(&self, size: ProgressSize) -> f64 {
        self.bar_height(size) / 2.0
    }

    /// Bar track (unfilled) color
    pub fn bar_track_color(&self) -> &'static str {
        palette(self.mode).fill_secondary
    }

    /// Bar fill (progress) color
    pub fn bar_fill_color(&self) -> &'static str {
        palette(self.mode).control_accent
    }

    /// Bar track background color (under the track)
    pub fn bar_track_bg(&self) -> &'static str {
        palette(self.mode).separator
    }

    // --- Ring progress methods ---

    /// Ring diameter for specified size
    pub fn ring_size(&self, size: ProgressSize) -> f64 {
        match size {
            ProgressSize::Small => 16.0,
            ProgressSize::Regular => 32.0,
            ProgressSize::Large => 64.0,
        }
    }

    /// Ring stroke width
    pub fn ring_stroke_width(&self, size: ProgressSize) -> f64 {
        match size {
            ProgressSize::Small => 2.0,
            ProgressSize::Regular => 3.0,
            ProgressSize::Large => 4.0,
        }
    }

    /// Ring track (unfilled) color
    pub fn ring_track_color(&self) -> &'static str {
        palette(self.mode).fill_secondary
    }

    /// Ring fill (progress) color
    pub fn ring_fill_color(&self) -> &'static str {
        palette(self.mode).control_accent
    }

    // --- Shared methods ---

    /// Indeterminate animation duration in milliseconds
    pub fn indeterminate_animation_duration_ms(&self) -> u32 {
        1500
    }

    /// Text color for progress labels
    pub fn text_color(&self) -> &'static str {
        palette(self.mode).secondary_label
    }

    /// Text font for progress labels
    pub fn text_font(&self) -> &'static str {
        "11px sans-serif"
    }
}

impl Default for ProgressTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_heights() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.bar_height(ProgressSize::Small), 2.0);
        assert_eq!(theme.bar_height(ProgressSize::Regular), 4.0);
        assert_eq!(theme.bar_height(ProgressSize::Large), 6.0);
    }

    #[test]
    fn test_bar_border_radius() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.bar_border_radius(ProgressSize::Small), 1.0);
        assert_eq!(theme.bar_border_radius(ProgressSize::Regular), 2.0);
        assert_eq!(theme.bar_border_radius(ProgressSize::Large), 3.0);
    }

    #[test]
    fn test_ring_sizes() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.ring_size(ProgressSize::Small), 16.0);
        assert_eq!(theme.ring_size(ProgressSize::Regular), 32.0);
        assert_eq!(theme.ring_size(ProgressSize::Large), 64.0);
    }

    #[test]
    fn test_ring_stroke_widths() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.ring_stroke_width(ProgressSize::Small), 2.0);
        assert_eq!(theme.ring_stroke_width(ProgressSize::Regular), 3.0);
        assert_eq!(theme.ring_stroke_width(ProgressSize::Large), 4.0);
    }

    #[test]
    fn test_indeterminate_animation() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.indeterminate_animation_duration_ms(), 1500);
    }

    #[test]
    fn test_text_settings() {
        let theme = ProgressTheme::default();
        assert_eq!(theme.text_font(), "11px sans-serif");
    }
}
