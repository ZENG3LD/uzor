//! Infinite horizontal scroll with velocity-based speed.
//!
//! Based on ReactBits ScrollVelocity component. Text scrolls horizontally
//! at base velocity, with speed multiplied by scroll velocity. Direction
//! reverses based on scroll direction. Uses spring physics for smooth velocity.

/// Configuration for scroll velocity animation.
#[derive(Debug, Clone)]
pub struct ScrollVelocityConfig {
    /// Base velocity in pixels per second (default: 100.0)
    pub base_velocity: f32,
    /// Spring damping for velocity smoothing (default: 50.0)
    pub damping: f32,
    /// Spring stiffness for velocity smoothing (default: 400.0)
    pub stiffness: f32,
    /// Number of text copies for seamless wrap (default: 6)
    pub num_copies: usize,
    /// Velocity factor mapping input range (default: [0.0, 1000.0])
    pub velocity_input_range: [f32; 2],
    /// Velocity factor mapping output range (default: [0.0, 5.0])
    pub velocity_output_range: [f32; 2],
}

impl Default for ScrollVelocityConfig {
    fn default() -> Self {
        Self {
            base_velocity: 100.0,
            damping: 50.0,
            stiffness: 400.0,
            num_copies: 6,
            velocity_input_range: [0.0, 1000.0],
            velocity_output_range: [0.0, 5.0],
        }
    }
}

/// Scroll velocity animation state manager.
pub struct ScrollVelocity {
    config: ScrollVelocityConfig,
    base_x: f32,
    direction_factor: f32,
    velocity_spring: SpringState,
}

/// Simple spring state for velocity smoothing.
#[derive(Debug, Clone, Copy)]
struct SpringState {
    value: f32,
    velocity: f32,
}

impl SpringState {
    fn new() -> Self {
        Self {
            value: 0.0,
            velocity: 0.0,
        }
    }

    /// Update spring state using semi-implicit Euler integration.
    fn update(&mut self, target: f32, damping: f32, stiffness: f32, dt: f32) {
        let force = (target - self.value) * stiffness;
        let damping_force = self.velocity * damping;

        self.velocity += (force - damping_force) * dt;
        self.value += self.velocity * dt;
    }
}

impl ScrollVelocity {
    /// Create a new scroll velocity animation.
    pub fn new(config: ScrollVelocityConfig) -> Self {
        Self {
            config,
            base_x: 0.0,
            direction_factor: 1.0,
            velocity_spring: SpringState::new(),
        }
    }

    /// Create with default configuration.
    pub fn default() -> Self {
        Self::new(ScrollVelocityConfig::default())
    }

    /// Update animation state.
    ///
    /// # Arguments
    /// * `scroll_velocity` - Current scroll velocity (pixels/second)
    /// * `delta_time` - Time elapsed since last update (seconds)
    pub fn update(&mut self, scroll_velocity: f32, delta_time: f32) {
        // Update spring to smooth scroll velocity
        self.velocity_spring.update(
            scroll_velocity,
            self.config.damping,
            self.config.stiffness,
            delta_time,
        );

        // Map smoothed velocity to factor using linear interpolation
        let velocity_factor = self.map_velocity(self.velocity_spring.value);

        // Update direction based on velocity sign
        if velocity_factor < 0.0 {
            self.direction_factor = -1.0;
        } else if velocity_factor > 0.0 {
            self.direction_factor = 1.0;
        }

        // Calculate movement: base movement + velocity-based boost
        let base_move = self.direction_factor * self.config.base_velocity * delta_time;
        let velocity_boost = self.direction_factor * base_move * velocity_factor;
        let total_move = base_move + velocity_boost;

        self.base_x += total_move;
    }

    /// Get current X offset wrapped to text width.
    ///
    /// # Arguments
    /// * `text_width` - Width of a single text copy in pixels
    ///
    /// # Returns
    /// X offset in pixels for rendering
    pub fn x_offset(&self, text_width: f32) -> f32 {
        if text_width <= 0.0 {
            return 0.0;
        }

        // Wrap within [-text_width, 0]
        self.wrap(-text_width, 0.0, self.base_x)
    }

    /// Map scroll velocity to velocity factor.
    fn map_velocity(&self, velocity: f32) -> f32 {
        let [in_min, in_max] = self.config.velocity_input_range;
        let [out_min, out_max] = self.config.velocity_output_range;

        if in_max == in_min {
            return out_min;
        }

        // Linear interpolation without clamping
        let t = (velocity - in_min) / (in_max - in_min);
        out_min + (out_max - out_min) * t
    }

    /// Wrap value within range using modulo arithmetic.
    fn wrap(&self, min: f32, max: f32, v: f32) -> f32 {
        let range = max - min;
        if range <= 0.0 {
            return min;
        }

        let offset = v - min;
        let wrapped = ((offset % range) + range) % range;
        wrapped + min
    }

    /// Reset animation state.
    pub fn reset(&mut self) {
        self.base_x = 0.0;
        self.direction_factor = 1.0;
        self.velocity_spring = SpringState::new();
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: ScrollVelocityConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &ScrollVelocityConfig {
        &self.config
    }

    /// Get number of copies needed for seamless wrap.
    pub fn num_copies(&self) -> usize {
        self.config.num_copies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let velocity = ScrollVelocity::default();
        // Initial base_x is 0.0, wrapped to range [-100, 0] gives -100.0
        assert_eq!(velocity.x_offset(100.0), -100.0);
    }

    #[test]
    fn test_forward_movement() {
        let mut velocity = ScrollVelocity::default();

        // Update with no scroll velocity, should move by base velocity
        velocity.update(0.0, 0.1); // 0.1 seconds

        // Should have moved forward by base_velocity * time = 100 * 0.1 = 10 pixels
        let offset = velocity.x_offset(100.0);
        assert!(offset > -100.0 && offset <= 0.0);
    }

    #[test]
    fn test_direction_reversal() {
        let mut velocity = ScrollVelocity::default();

        // Positive scroll velocity
        velocity.update(100.0, 0.1);
        let dir1 = velocity.direction_factor;

        // Negative scroll velocity
        velocity.update(-100.0, 0.5); // More time for spring to catch up
        let dir2 = velocity.direction_factor;

        // Direction should have flipped
        assert_eq!(dir1, 1.0);
        assert_eq!(dir2, -1.0);
    }

    #[test]
    fn test_wrapping() {
        let mut velocity = ScrollVelocity::default();

        // Move past one full width
        for _ in 0..20 {
            velocity.update(0.0, 0.1);
        }

        let offset = velocity.x_offset(100.0);
        // Should be wrapped within [-100, 0]
        assert!(offset >= -100.0 && offset <= 0.0);
    }

    #[test]
    fn test_velocity_mapping() {
        let velocity = ScrollVelocity::default();

        // Test mapping
        assert_eq!(velocity.map_velocity(0.0), 0.0);
        assert_eq!(velocity.map_velocity(1000.0), 5.0);
        assert!((velocity.map_velocity(500.0) - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_custom_velocity() {
        let config = ScrollVelocityConfig {
            base_velocity: 200.0,
            ..Default::default()
        };
        let mut velocity = ScrollVelocity::new(config);

        velocity.update(0.0, 0.1);

        // Should move faster with higher base velocity
        let offset = velocity.base_x;
        assert!((offset - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_spring_smoothing() {
        let mut velocity = ScrollVelocity::default();

        // Sudden velocity change
        velocity.update(1000.0, 0.016);
        let velocity1 = velocity.velocity_spring.value;

        // Should not instantly reach target due to spring
        assert!(velocity1 < 1000.0);

        // Continue updating
        for _ in 0..10 {
            velocity.update(1000.0, 0.016);
        }
        let velocity2 = velocity.velocity_spring.value;

        // Should be closer to target
        assert!(velocity2 > velocity1);
    }

    #[test]
    fn test_reset() {
        let mut velocity = ScrollVelocity::default();

        velocity.update(100.0, 0.5);
        velocity.reset();

        assert_eq!(velocity.base_x, 0.0);
        assert_eq!(velocity.direction_factor, 1.0);
        assert_eq!(velocity.velocity_spring.value, 0.0);
    }
}
