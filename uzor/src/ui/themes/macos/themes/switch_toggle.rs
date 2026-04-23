//! macOS switch toggle theme

use super::super::colors::{AppearanceMode, WidgetState, palette};

/// macOS switch toggle (on/off slider) theme
pub struct SwitchTheme {
    pub mode: AppearanceMode,
}

/// Shadow configuration for switch components
#[derive(Clone, Copy, Debug)]
pub struct Shadow {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur_radius: f64,
    pub spread_radius: f64,
    pub color: &'static str,
}

impl SwitchTheme {
    /// Creates a new switch theme with the given appearance mode
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    /// Track width (38px per macOS HIG)
    pub fn width(&self) -> f64 {
        38.0
    }

    /// Track height (22px per macOS HIG)
    pub fn height(&self) -> f64 {
        22.0
    }

    /// Thumb (sliding circle) diameter
    pub fn thumb_size(&self) -> f64 {
        18.0
    }

    /// Margin between thumb and track edge
    pub fn thumb_margin(&self) -> f64 {
        2.0
    }

    /// Border radius for track (pill shape = height/2)
    pub fn border_radius(&self) -> f64 {
        11.0 // height / 2 = 22 / 2
    }

    /// Track background color based on on/off state and widget state
    pub fn track_bg(&self, on: bool, state: WidgetState) -> &'static str {
        let p = palette(self.mode);

        if on {
            // On state: use system green (or custom accent for switches)
            match state {
                WidgetState::Pressed => p.system_green, // Slightly darker on press
                _ => p.system_green,
            }
        } else {
            // Off state: use fill_secondary (subtle gray)
            p.fill_secondary
        }
    }

    /// Thumb (slider knob) background color
    pub fn thumb_bg(&self, _state: WidgetState) -> &'static str {
        // Thumb is always white regardless of on/off state
        "#FFFFFFFF"
    }

    /// Thumb shadow for depth effect
    pub fn thumb_shadow(&self) -> Shadow {
        match self.mode {
            AppearanceMode::Light | AppearanceMode::VibrantLight
            | AppearanceMode::AccessibleLight | AppearanceMode::AccessibleVibrantLight => {
                Shadow {
                    offset_x: 0.0,
                    offset_y: 2.0,
                    blur_radius: 4.0,
                    spread_radius: 0.0,
                    color: "#00000026", // rgba(0, 0, 0, 0.15)
                }
            },
            _ => {
                Shadow {
                    offset_x: 0.0,
                    offset_y: 2.0,
                    blur_radius: 4.0,
                    spread_radius: 0.0,
                    color: "#00000040", // Darker shadow in dark mode
                }
            }
        }
    }

    /// Track border color (subtle outline)
    pub fn track_border_color(&self, on: bool) -> &'static str {
        if on {
            // When on, border matches green or is transparent
            "#00000000"
        } else {
            // When off, subtle border for definition
            match self.mode {
                AppearanceMode::Light | AppearanceMode::VibrantLight
                | AppearanceMode::AccessibleLight | AppearanceMode::AccessibleVibrantLight => {
                    "#0000001A" // Very subtle
                },
                _ => {
                    "#FFFFFF1A"
                }
            }
        }
    }

    /// Track border width
    pub fn track_border_width(&self) -> f64 {
        0.5
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

    /// Disabled state opacity
    pub fn disabled_opacity(&self) -> f64 {
        0.5
    }

    /// Animation duration for on/off transition in milliseconds
    pub fn animation_duration_ms(&self) -> u64 {
        200
    }

    /// Animation easing curve identifier
    pub fn animation_easing(&self) -> &'static str {
        "ease-in-out"
    }

    /// Spacing between switch and label text
    pub fn label_spacing(&self) -> f64 {
        8.0
    }

    /// Vertical alignment offset for label text
    pub fn label_baseline_offset(&self) -> f64 {
        1.0
    }

    /// Calculate thumb X position based on switch state
    /// Returns the X offset from the track's left edge
    pub fn thumb_x_offset(&self, on: bool) -> f64 {
        let margin = self.thumb_margin();
        if on {
            // Right position: track_width - thumb_size - margin
            self.width() - self.thumb_size() - margin
        } else {
            // Left position: margin
            margin
        }
    }

    /// Calculate thumb Y position (vertically centered)
    /// Returns the Y offset from the track's top edge
    pub fn thumb_y_offset(&self) -> f64 {
        (self.height() - self.thumb_size()) / 2.0
    }

    /// Inner shadow for track (subtle depth)
    pub fn track_inner_shadow(&self, on: bool) -> Shadow {
        if on {
            // No inner shadow when on (flat green)
            Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: 0.0,
                spread_radius: 0.0,
                color: "#00000000",
            }
        } else {
            // Subtle inner shadow when off (recessed look)
            Shadow {
                offset_x: 0.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread_radius: 0.0,
                color: "#00000010",
            }
        }
    }
}

impl Default for SwitchTheme {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_sizes() {
        let theme = SwitchTheme::new(AppearanceMode::Light);
        assert_eq!(theme.width(), 38.0);
        assert_eq!(theme.height(), 22.0);
        assert_eq!(theme.thumb_size(), 18.0);
        assert_eq!(theme.thumb_margin(), 2.0);
        assert_eq!(theme.border_radius(), 11.0);
    }

    #[test]
    fn test_switch_colors_light_mode() {
        let theme = SwitchTheme::new(AppearanceMode::Light);

        // Off state should use fill_secondary
        let off_bg = theme.track_bg(false, WidgetState::Normal);
        assert_eq!(off_bg, "#0000000F");

        // On state should use system green
        let on_bg = theme.track_bg(true, WidgetState::Normal);
        assert_eq!(on_bg, "#34C759");

        // Thumb is always white
        assert_eq!(theme.thumb_bg(WidgetState::Normal), "#FFFFFFFF");
    }

    #[test]
    fn test_switch_colors_dark_mode() {
        let theme = SwitchTheme::new(AppearanceMode::Dark);

        // Off state
        let off_bg = theme.track_bg(false, WidgetState::Normal);
        assert_eq!(off_bg, "#FFFFFF0F");

        // On state should use dark mode green
        let on_bg = theme.track_bg(true, WidgetState::Normal);
        assert_eq!(on_bg, "#32D74B");
    }

    #[test]
    fn test_thumb_positioning() {
        let theme = SwitchTheme::new(AppearanceMode::Light);

        // Off position (left)
        let off_x = theme.thumb_x_offset(false);
        assert_eq!(off_x, 2.0); // margin

        // On position (right)
        let on_x = theme.thumb_x_offset(true);
        assert_eq!(on_x, 18.0); // width - thumb_size - margin = 38 - 18 - 2

        // Vertical centering
        let y = theme.thumb_y_offset();
        assert_eq!(y, 2.0); // (height - thumb_size) / 2 = (22 - 18) / 2
    }

    #[test]
    fn test_animation_timing() {
        let theme = SwitchTheme::new(AppearanceMode::Light);
        assert_eq!(theme.animation_duration_ms(), 200);
        assert_eq!(theme.animation_easing(), "ease-in-out");
    }

    #[test]
    fn test_focus_ring() {
        let theme = SwitchTheme::new(AppearanceMode::Light);
        assert_eq!(theme.focus_ring_width(), 3.0);
        assert_eq!(theme.focus_ring_color(), "#007AFF80");
    }

    #[test]
    fn test_shadow_configuration() {
        let theme = SwitchTheme::new(AppearanceMode::Light);
        let shadow = theme.thumb_shadow();
        assert_eq!(shadow.offset_y, 2.0);
        assert_eq!(shadow.blur_radius, 4.0);
    }
}
