//! macOS traffic lights theme

use super::super::colors::{AppearanceMode, WidgetState};

/// Traffic light button type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficLightButton {
    Close,
    Minimize,
    Maximize,
}

/// Maximize button mode (changes icon)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaximizeMode {
    /// Standard zoom (+)
    Zoom,
    /// Fullscreen mode (⤢) — when Alt/Option is held
    Fullscreen,
}

/// State of the traffic light button group
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficLightGroupState {
    /// All buttons show default colors, no icons
    Default,
    /// Mouse is over the group — all buttons show icons
    Hovered,
    /// Window is unfocused — all buttons are gray
    Unfocused,
}

pub struct TrafficLightTheme {
    pub mode: AppearanceMode,
}

impl TrafficLightTheme {
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    /// Button diameter in pixels
    pub fn button_diameter(&self) -> f64 {
        12.0
    }

    /// Button radius in pixels (half of diameter)
    pub fn button_radius(&self) -> f64 {
        6.0
    }

    /// Gap between buttons in pixels
    pub fn button_gap(&self) -> f64 {
        8.0
    }

    /// Horizontal padding from container edge to first button
    pub fn container_padding_x(&self) -> f64 {
        12.0
    }

    /// Vertical padding (centered in title bar)
    pub fn container_padding_y(&self) -> f64 {
        0.0
    }

    /// Total width of traffic lights including padding
    /// = 12.0 * 3 (buttons) + 8.0 * 2 (gaps) + 12.0 * 2 (padding) = 76.0
    pub fn total_width(&self) -> f64 {
        self.button_diameter() * 3.0 + self.button_gap() * 2.0 + self.container_padding_x() * 2.0
    }

    /// Get button color based on button type, group state, and widget state
    pub fn button_color(
        &self,
        button: TrafficLightButton,
        group_state: TrafficLightGroupState,
        widget_state: WidgetState,
    ) -> &'static str {
        // Window unfocused — all buttons gray
        if group_state == TrafficLightGroupState::Unfocused {
            return "#80808080";
        }

        // Default colors (no hover on group)
        match button {
            TrafficLightButton::Close => match widget_state {
                WidgetState::Pressed => "#E0443E",
                WidgetState::Hovered => "#FF6F67",
                _ => "#FF5F57",
            },
            TrafficLightButton::Minimize => match widget_state {
                WidgetState::Pressed => "#DFA52A",
                WidgetState::Hovered => "#FFCC4D",
                _ => "#FEBC2E",
            },
            TrafficLightButton::Maximize => match widget_state {
                WidgetState::Pressed => "#1DAD36",
                WidgetState::Hovered => "#39D956",
                _ => "#28C840",
            },
        }
    }

    /// Whether to show icons (only when group is hovered)
    pub fn show_icon(&self, group_state: TrafficLightGroupState) -> bool {
        group_state == TrafficLightGroupState::Hovered
    }

    /// Icon color (dark color overlaid on button)
    pub fn icon_color(&self, button: TrafficLightButton) -> &'static str {
        match button {
            TrafficLightButton::Close => "#4C0002",     // dark red
            TrafficLightButton::Minimize => "#995700",  // dark yellow
            TrafficLightButton::Maximize => "#006500",  // dark green
        }
    }

    /// Border color around buttons
    pub fn border_color(&self) -> &'static str {
        match self.mode {
            AppearanceMode::Light
            | AppearanceMode::VibrantLight
            | AppearanceMode::AccessibleLight
            | AppearanceMode::AccessibleVibrantLight => "#00000020", // subtle border in light mode
            _ => "#00000000", // transparent (no border) in dark mode
        }
    }

    /// Hit test radius (slightly larger than visual for easier clicking)
    pub fn hit_test_radius(&self) -> f64 {
        7.0
    }

    /// Hover transition duration in milliseconds
    pub fn hover_transition_ms(&self) -> u64 {
        100
    }

    /// Hover scale factor (no scale in macOS)
    pub fn hover_scale(&self) -> f64 {
        1.0
    }

    /// Calculate center positions for each button
    /// Returns [(close_x, close_y), (minimize_x, minimize_y), (maximize_x, maximize_y)]
    pub fn button_positions(&self, container_x: f64, container_y: f64) -> [(f64, f64); 3] {
        let x_start = container_x + self.container_padding_x() + self.button_radius();
        let button_spacing = self.button_diameter() + self.button_gap();

        let close_x = x_start;
        let minimize_x = x_start + button_spacing;
        let maximize_x = x_start + button_spacing * 2.0;

        let y = container_y;

        [(close_x, y), (minimize_x, y), (maximize_x, y)]
    }

    /// Hit test to determine which button (if any) is under the cursor
    pub fn hit_test(
        &self,
        container_x: f64,
        container_y: f64,
        mouse_x: f64,
        mouse_y: f64,
    ) -> Option<TrafficLightButton> {
        let positions = self.button_positions(container_x, container_y);
        let hit_radius_sq = self.hit_test_radius() * self.hit_test_radius();

        for (i, &(cx, cy)) in positions.iter().enumerate() {
            let dx = mouse_x - cx;
            let dy = mouse_y - cy;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= hit_radius_sq {
                return Some(match i {
                    0 => TrafficLightButton::Close,
                    1 => TrafficLightButton::Minimize,
                    2 => TrafficLightButton::Maximize,
                    _ => unreachable!(),
                });
            }
        }

        None
    }

    /// Check if mouse is over the entire traffic light group
    pub fn is_group_hovered(
        &self,
        container_x: f64,
        container_y: f64,
        mouse_x: f64,
        mouse_y: f64,
    ) -> bool {
        let group_width = self.button_diameter() * 3.0 + self.button_gap() * 2.0;
        let group_height = self.button_diameter();

        let x_start = container_x + self.container_padding_x();
        let y_start = container_y - self.button_radius();

        mouse_x >= x_start
            && mouse_x <= x_start + group_width
            && mouse_y >= y_start
            && mouse_y <= y_start + group_height
    }
}

impl Default for TrafficLightTheme {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::Dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry() {
        let theme = TrafficLightTheme::default();
        assert_eq!(theme.button_diameter(), 12.0);
        assert_eq!(theme.button_radius(), 6.0);
        assert_eq!(theme.button_gap(), 8.0);
        assert_eq!(theme.total_width(), 76.0);
    }

    #[test]
    fn test_button_positions() {
        let theme = TrafficLightTheme::default();
        let positions = theme.button_positions(0.0, 20.0);

        // First button: padding_x + radius = 12 + 6 = 18
        assert_eq!(positions[0], (18.0, 20.0));

        // Second button: 18 + 12 + 8 = 38
        assert_eq!(positions[1], (38.0, 20.0));

        // Third button: 38 + 12 + 8 = 58
        assert_eq!(positions[2], (58.0, 20.0));
    }

    #[test]
    fn test_hit_test() {
        let theme = TrafficLightTheme::default();

        // Test close button hit
        let result = theme.hit_test(0.0, 20.0, 18.0, 20.0);
        assert_eq!(result, Some(TrafficLightButton::Close));

        // Test minimize button hit
        let result = theme.hit_test(0.0, 20.0, 38.0, 20.0);
        assert_eq!(result, Some(TrafficLightButton::Minimize));

        // Test maximize button hit
        let result = theme.hit_test(0.0, 20.0, 58.0, 20.0);
        assert_eq!(result, Some(TrafficLightButton::Maximize));

        // Test miss
        let result = theme.hit_test(0.0, 20.0, 100.0, 100.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_group_hover() {
        let theme = TrafficLightTheme::default();

        // Inside group
        assert!(theme.is_group_hovered(0.0, 20.0, 30.0, 20.0));

        // Outside group
        assert!(!theme.is_group_hovered(0.0, 20.0, 100.0, 20.0));
    }

    #[test]
    fn test_button_colors() {
        let theme = TrafficLightTheme::default();

        // Unfocused state
        let color = theme.button_color(
            TrafficLightButton::Close,
            TrafficLightGroupState::Unfocused,
            WidgetState::Normal,
        );
        assert_eq!(color, "#80808080");

        // Normal close button
        let color = theme.button_color(
            TrafficLightButton::Close,
            TrafficLightGroupState::Default,
            WidgetState::Normal,
        );
        assert_eq!(color, "#FF5F57");

        // Pressed close button
        let color = theme.button_color(
            TrafficLightButton::Close,
            TrafficLightGroupState::Hovered,
            WidgetState::Pressed,
        );
        assert_eq!(color, "#E0443E");
    }

    #[test]
    fn test_show_icon() {
        let theme = TrafficLightTheme::default();

        // Icons only show when hovered
        assert!(!theme.show_icon(TrafficLightGroupState::Default));
        assert!(theme.show_icon(TrafficLightGroupState::Hovered));
        assert!(!theme.show_icon(TrafficLightGroupState::Unfocused));
    }

    #[test]
    fn test_icon_colors() {
        let theme = TrafficLightTheme::default();

        assert_eq!(
            theme.icon_color(TrafficLightButton::Close),
            "#4C0002"
        );
        assert_eq!(
            theme.icon_color(TrafficLightButton::Minimize),
            "#995700"
        );
        assert_eq!(
            theme.icon_color(TrafficLightButton::Maximize),
            "#006500"
        );
    }

    #[test]
    fn test_border_color() {
        let light_theme = TrafficLightTheme::new(AppearanceMode::Light);
        assert_eq!(light_theme.border_color(), "#00000020");

        let dark_theme = TrafficLightTheme::new(AppearanceMode::Dark);
        assert_eq!(dark_theme.border_color(), "#00000000");
    }
}
