//! Glare hover effect — shiny glare sweep across element on hover
//!
//! Algorithm (from React source using CSS):
//! 1. Linear gradient positioned off-element initially (-100%, -100%)
//! 2. On hover: gradient animates to (100%, 100%)
//! 3. Gradient has transparent regions on edges with bright center
//! 4. Configurable angle, duration, size
//! 5. Optional "play once" mode

use std::f32::consts::PI;

/// Glare hover effect state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlareHoverState {
    /// Current glare position (0.0 = start, 1.0 = end)
    pub position: f32,
    /// Whether currently hovering
    pub is_hovering: bool,
    /// Time when hover started (for animation)
    pub hover_start_time: Option<f64>,
    /// Whether the effect has played (for play_once mode)
    pub has_played: bool,
}

impl Default for GlareHoverState {
    fn default() -> Self {
        Self {
            position: 0.0,
            is_hovering: false,
            hover_start_time: None,
            has_played: false,
        }
    }
}

/// Glare hover effect configuration
pub struct GlareHover {
    /// Glare angle in degrees (default: -45.0)
    /// -45 = top-left to bottom-right
    pub angle: f32,
    /// Animation duration in seconds (default: 0.65)
    pub duration: f64,
    /// Glare size as percentage of element (default: 250.0)
    pub size: f32,
    /// Glare opacity (0.0..1.0, default: 0.5)
    pub opacity: f32,
    /// Only play animation once (default: false)
    pub play_once: bool,
}

impl Default for GlareHover {
    fn default() -> Self {
        Self {
            angle: -45.0,
            duration: 0.65,
            size: 250.0,
            opacity: 0.5,
            play_once: false,
        }
    }
}

impl GlareHover {
    /// Create a new glare hover with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the glare angle in degrees
    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    /// Set the animation duration in seconds
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    /// Set the glare size as percentage
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set the glare opacity
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Set whether to play only once
    pub fn with_play_once(mut self, play_once: bool) -> Self {
        self.play_once = play_once;
        self
    }

    /// Handle hover state change
    ///
    /// # Arguments
    /// * `state` - Current state (will be mutated)
    /// * `is_hovering` - Whether the element is currently being hovered
    /// * `current_time` - Current time in seconds
    pub fn set_hover(&self, state: &mut GlareHoverState, is_hovering: bool, current_time: f64) {
        if is_hovering && !state.is_hovering {
            // Started hovering
            if !self.play_once || !state.has_played {
                state.hover_start_time = Some(current_time);
                state.has_played = true;
            }
        } else if !is_hovering && state.is_hovering {
            // Stopped hovering - reset position
            state.position = 0.0;
            state.hover_start_time = None;
        }

        state.is_hovering = is_hovering;
    }

    /// Update the glare position based on time
    ///
    /// # Arguments
    /// * `state` - Current state (will be mutated)
    /// * `current_time` - Current time in seconds
    ///
    /// # Returns
    /// Current glare position (0.0 = off-screen left, 1.0 = off-screen right)
    pub fn update(&self, state: &mut GlareHoverState, current_time: f64) -> f32 {
        if let Some(start_time) = state.hover_start_time {
            let elapsed = current_time - start_time;
            let progress = (elapsed / self.duration).min(1.0) as f32;

            // Ease out function (same as CSS ease)
            // Cubic bezier approximation: ease = cubic-bezier(0.25, 0.1, 0.25, 1.0)
            // Simplified: ease-out ≈ t * (2 - t)
            let eased = progress * (2.0 - progress);

            state.position = eased;
        }

        state.position
    }

    /// Calculate the gradient position in element coordinates
    ///
    /// Maps position (0..1) to CSS background-position values
    /// Position 0.0 → (-100%, -100%)
    /// Position 1.0 → (100%, 100%)
    pub fn calculate_gradient_position(&self, position: f32) -> (f32, f32) {
        let x = -100.0 + position * 200.0;
        let y = -100.0 + position * 200.0;
        (x, y)
    }

    /// Calculate the gradient angle in radians
    pub fn angle_radians(&self) -> f32 {
        self.angle * PI / 180.0
    }
}

/// Gradient stop for rendering
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    pub position: f32, // 0.0..1.0
    pub opacity: f32,  // 0.0..1.0
}

impl GlareHover {
    /// Get the gradient stops for the glare effect
    ///
    /// Creates a gradient with:
    /// - Transparent edges (0% opacity at 0% and 100%)
    /// - Bright center (full opacity at 70%)
    pub fn gradient_stops(&self) -> Vec<GradientStop> {
        vec![
            GradientStop {
                position: 0.0,
                opacity: 0.0,
            },
            GradientStop {
                position: 0.6,
                opacity: 0.0,
            },
            GradientStop {
                position: 0.7,
                opacity: self.opacity,
            },
            GradientStop {
                position: 0.8,
                opacity: 0.0,
            },
            GradientStop {
                position: 1.0,
                opacity: 0.0,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glare_hover_init() {
        let state = GlareHoverState::default();
        assert_eq!(state.position, 0.0);
        assert!(!state.is_hovering);
        assert!(state.hover_start_time.is_none());
        assert!(!state.has_played);
    }

    #[test]
    fn test_glare_hover_activation() {
        let glare = GlareHover::new();
        let mut state = GlareHoverState::default();

        // Start hovering
        glare.set_hover(&mut state, true, 0.0);
        assert!(state.is_hovering);
        assert_eq!(state.hover_start_time, Some(0.0));
        assert!(state.has_played);

        // Update after half duration
        let pos = glare.update(&mut state, 0.325); // Half of 0.65s
        assert!(pos > 0.0 && pos < 1.0);
        assert_eq!(state.position, pos);

        // Update after full duration
        glare.update(&mut state, 1.0);
        assert_eq!(state.position, 1.0);
    }

    #[test]
    fn test_glare_hover_reset_on_unhover() {
        let glare = GlareHover::new();
        let mut state = GlareHoverState::default();

        glare.set_hover(&mut state, true, 0.0);
        glare.update(&mut state, 0.5);
        assert!(state.position > 0.0);

        // Stop hovering
        glare.set_hover(&mut state, false, 0.5);
        assert!(!state.is_hovering);
        assert_eq!(state.position, 0.0);
        assert!(state.hover_start_time.is_none());
    }

    #[test]
    fn test_glare_play_once() {
        let glare = GlareHover::new().with_play_once(true);
        let mut state = GlareHoverState::default();

        // First hover
        glare.set_hover(&mut state, true, 0.0);
        assert!(state.hover_start_time.is_some());
        assert!(state.has_played);

        // Unhover
        glare.set_hover(&mut state, false, 1.0);

        // Second hover - should NOT restart
        glare.set_hover(&mut state, true, 2.0);
        assert!(state.hover_start_time.is_none()); // Not set again
    }

    #[test]
    fn test_gradient_position_calculation() {
        let glare = GlareHover::new();

        let (x, y) = glare.calculate_gradient_position(0.0);
        assert_eq!(x, -100.0);
        assert_eq!(y, -100.0);

        let (x, y) = glare.calculate_gradient_position(0.5);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);

        let (x, y) = glare.calculate_gradient_position(1.0);
        assert_eq!(x, 100.0);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn test_gradient_stops() {
        let glare = GlareHover::new().with_opacity(0.8);
        let stops = glare.gradient_stops();

        assert_eq!(stops.len(), 5);
        assert_eq!(stops[0].opacity, 0.0); // Start transparent
        assert_eq!(stops[2].opacity, 0.8); // Center bright
        assert_eq!(stops[4].opacity, 0.0); // End transparent
    }

    #[test]
    fn test_angle_conversion() {
        let glare = GlareHover::new().with_angle(90.0);
        let radians = glare.angle_radians();
        assert!((radians - PI / 2.0).abs() < 0.001);

        let glare = GlareHover::new().with_angle(-45.0);
        let radians = glare.angle_radians();
        assert!((radians + PI / 4.0).abs() < 0.001);
    }
}
