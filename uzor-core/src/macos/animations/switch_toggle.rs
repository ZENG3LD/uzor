//! macOS switch toggle animation presets

/// Switch toggle animation configuration
pub struct SwitchAnimation {
    pub duration_ms: f64,
    pub easing: &'static str,
}

/// Toggle switch animation
/// - 200ms duration
/// - Thumb position: 0.0 → 1.0 (or reverse)
/// - Track color: interpolates between off/on colors (handled by renderer)
/// - Easing: ease-in-out for smooth, organic feel
pub const TOGGLE: SwitchAnimation = SwitchAnimation {
    duration_ms: 200.0,
    easing: "ease-in-out",
};

/// Helper function to calculate thumb position from progress
///
/// # Arguments
/// * `progress` - Animation progress (0.0 to 1.0)
/// * `track_width` - Total width available for thumb travel
/// * `thumb_radius` - Radius of the thumb circle
///
/// # Returns
/// X offset for thumb center from left edge of track
#[inline]
pub fn thumb_position(progress: f64, track_width: f64, thumb_radius: f64) -> f64 {
    let travel_distance = track_width - (thumb_radius * 2.0);
    thumb_radius + (travel_distance * progress)
}

/// Helper to interpolate colors between off and on states
///
/// # Arguments
/// * `progress` - Animation progress (0.0 to 1.0)
/// * `color_off` - Color when switch is off (RGBA tuple)
/// * `color_on` - Color when switch is on (RGBA tuple)
///
/// # Returns
/// Interpolated color at current progress
#[inline]
pub fn interpolate_track_color(
    progress: f64,
    color_off: (f64, f64, f64, f64),
    color_on: (f64, f64, f64, f64),
) -> (f64, f64, f64, f64) {
    (
        color_off.0 + (color_on.0 - color_off.0) * progress,
        color_off.1 + (color_on.1 - color_off.1) * progress,
        color_off.2 + (color_on.2 - color_off.2) * progress,
        color_off.3 + (color_on.3 - color_off.3) * progress,
    )
}
