//! macOS dock animation presets

/// Dock magnification spring configuration
///
/// Based on macos-web reverse-engineering:
/// - Damping: 0.38 (high damping, minimal overshoot)
/// - Stiffness: 0.1 (low stiffness, smooth motion)
/// - Mass: 1.0 (standard mass)
pub struct DockSpringConfig {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}

/// Spring config for dock icon magnification
///
/// This creates a smooth, fluid animation with slight bounce
/// that matches macOS's iconic dock behavior.
pub const MAGNIFICATION_SPRING: DockSpringConfig = DockSpringConfig {
    stiffness: 0.1,
    damping: 0.38,
    mass: 1.0,
};

/// Golden ratio used for peak magnification scale
///
/// macOS scales icons by approximately 2.618x at the cursor position,
/// then falls off with distance using a Gaussian-like curve.
pub const MAGNIFICATION_SCALE: f64 = 2.618;

/// Calculate dock icon scale based on distance from cursor
///
/// # Arguments
/// * `distance` - Distance from cursor to icon center (normalized, 0.0 = at cursor)
/// * `max_scale` - Peak scale at cursor (typically MAGNIFICATION_SCALE)
///
/// # Returns
/// Scale multiplier for the icon (1.0 = normal size)
#[inline]
pub fn icon_scale_from_distance(distance: f64, max_scale: f64) -> f64 {
    // Gaussian-like falloff: e^(-k*d²)
    // k controls how quickly the effect drops off
    const FALLOFF_RATE: f64 = 2.5;

    let normalized_distance = distance.abs();
    let scale_factor = (-FALLOFF_RATE * normalized_distance * normalized_distance).exp();

    1.0 + (max_scale - 1.0) * scale_factor
}

/// Bounce animation for app launch from dock
///
/// Uses spring with overshoot for playful launch effect.
pub const LAUNCH_BOUNCE_SPRING: DockSpringConfig = DockSpringConfig {
    stiffness: 0.15,  // Slightly stiffer for bouncier feel
    damping: 0.3,     // Lower damping = more bounce
    mass: 1.0,
};

/// Number of bounces during app launch animation
pub const LAUNCH_BOUNCE_COUNT: u32 = 3;

/// Vertical displacement for launch bounce (in icon heights)
///
/// Icon bounces up to 0.5x its height, then settles.
pub const LAUNCH_BOUNCE_HEIGHT: f64 = 0.5;
