//! macOS traffic lights animation presets

/// Traffic light animation configuration
pub struct TrafficLightAnimation {
    pub duration_ms: f64,
    pub easing: &'static str,
}

/// Hover-in animation for traffic light icons
///
/// - 100ms duration
/// - Opacity: 0.0 → 1.0 (icons fade in)
/// - Easing: ease-in for crisp appearance
pub const HOVER_IN: TrafficLightAnimation = TrafficLightAnimation {
    duration_ms: 100.0,
    easing: "ease-in",
};

/// Hover-out animation for traffic light icons
///
/// - 150ms duration
/// - Opacity: 1.0 → 0.0 (icons fade out)
/// - Easing: ease-out for smooth disappearance
pub const HOVER_OUT: TrafficLightAnimation = TrafficLightAnimation {
    duration_ms: 150.0,
    easing: "ease-out",
};

/// Traffic light button colors
///
/// These are the standard macOS window control colors.
/// When pressed, colors are darkened (handled by renderer).
pub mod colors {
    /// Close button (red)
    pub const CLOSE: (f64, f64, f64) = (1.0, 0.38, 0.35);

    /// Minimize button (yellow/orange)
    pub const MINIMIZE: (f64, f64, f64) = (1.0, 0.75, 0.0);

    /// Maximize/Fullscreen button (green)
    pub const MAXIMIZE: (f64, f64, f64) = (0.16, 0.82, 0.35);

    /// Unfocused state (gray)
    pub const UNFOCUSED: (f64, f64, f64) = (0.85, 0.85, 0.85);
}

/// Darken factor when traffic light button is pressed
///
/// Multiply RGB values by this to get pressed state color.
pub const PRESS_DARKEN_FACTOR: f64 = 0.8;

/// Helper to darken a color for pressed state
#[inline]
pub fn darken_color(color: (f64, f64, f64), factor: f64) -> (f64, f64, f64) {
    (color.0 * factor, color.1 * factor, color.2 * factor)
}

/// Traffic light button press is instant (no transition)
///
/// macOS traffic lights change color immediately on press,
/// without animation.
pub const PRESS_INSTANT: bool = true;
