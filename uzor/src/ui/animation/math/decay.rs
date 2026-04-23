//! Inertia/Decay Animation
//!
//! Momentum-based deceleration using exponential decay. This implements
//! friction-based inertia for flick scrolling, swipe momentum, and drag release.
//!
//! ## Physics Model
//!
//! Pure decay (no bounds):
//! - velocity(t) = v0 * friction^(t * 60)  (friction applied 60x per second)
//! - position(t) = v0 * (friction^(t*60) - 1) / (60 * ln(friction))
//!
//! Bounded decay:
//! - When position exceeds bounds, apply spring force back to bound
//!
//! ## Usage
//!
//! ```rust
//! use uzor::animation::Decay;
//!
//! // iOS-like flick scrolling
//! let mut decay = Decay::new(500.0)  // Initial velocity: 500 units/sec
//!     .friction(0.998)
//!     .bounds(0.0, 1000.0);
//!
//! // Evaluate at 0.5 seconds
//! let (position, velocity) = decay.evaluate(0.5);
//!
//! // Check if at rest
//! if decay.is_at_rest(0.5) {
//!     println!("Animation complete");
//! }
//! ```

/// Inertia/decay animation — exponential deceleration
///
/// Used for flick scrolling, swipe momentum, drag release.
/// This is NOT spring physics — it's friction/inertia after gesture release.
#[derive(Debug, Clone, Copy)]
pub struct Decay {
    /// Initial velocity (units per second)
    pub velocity: f64,
    /// Friction coefficient per frame (0..1, lower = more friction)
    /// Applied 60 times per second. Default: 0.998 (iOS-like feel)
    pub friction: f64,
    /// Rest threshold — stop when velocity below this
    pub rest_threshold: f64,
    /// Optional min bound with spring bounce-back
    pub min_bound: Option<f64>,
    /// Optional max bound with spring bounce-back
    pub max_bound: Option<f64>,
    /// Spring stiffness for bounce-back at bounds (default: 400.0)
    pub bounce_stiffness: f64,
    /// Spring damping for bounce-back at bounds (default: 30.0)
    pub bounce_damping: f64,
}

impl Decay {
    /// Create a new decay animation with initial velocity
    ///
    /// # Arguments
    /// * `velocity` - Initial velocity in units per second
    ///
    /// # Example
    /// ```rust
    /// use uzor::animation::Decay;
    /// let decay = Decay::new(500.0); // 500 units/sec initial velocity
    /// ```
    pub fn new(velocity: f64) -> Self {
        Self {
            velocity,
            friction: 0.998,
            rest_threshold: 0.01,
            min_bound: None,
            max_bound: None,
            bounce_stiffness: 400.0,
            bounce_damping: 30.0,
        }
    }

    /// Set friction coefficient (0..1, lower = more friction)
    ///
    /// Default: 0.998 (iOS-like feel)
    /// - 0.99 = heavy friction (stops quickly)
    /// - 0.998 = iOS default (medium)
    /// - 0.999 = light friction (slides far)
    pub fn friction(mut self, f: f64) -> Self {
        self.friction = f.clamp(0.0, 1.0);
        self
    }

    /// Set rest threshold — animation stops when velocity falls below this
    ///
    /// Default: 0.01
    pub fn rest_threshold(mut self, t: f64) -> Self {
        self.rest_threshold = t.abs();
        self
    }

    /// Set min/max bounds with spring bounce-back
    ///
    /// When position exceeds bounds, a spring force pulls it back.
    pub fn bounds(mut self, min: f64, max: f64) -> Self {
        self.min_bound = Some(min);
        self.max_bound = Some(max);
        self
    }

    /// Set spring stiffness for bounce-back at bounds
    ///
    /// Default: 400.0
    pub fn bounce_stiffness(mut self, s: f64) -> Self {
        self.bounce_stiffness = s;
        self
    }

    /// Set spring damping for bounce-back at bounds
    ///
    /// Default: 30.0
    pub fn bounce_damping(mut self, d: f64) -> Self {
        self.bounce_damping = d;
        self
    }

    /// Evaluate position and velocity at time t (seconds)
    ///
    /// Returns (position, velocity) tuple.
    ///
    /// # Physics
    ///
    /// Without bounds: pure exponential decay
    /// - velocity(t) = v0 * friction^(t*60)
    /// - position(t) = v0 * (friction^(t*60) - 1) / (60 * ln(friction))
    ///
    /// With bounds: when position exceeds bounds, apply spring force
    pub fn evaluate(&self, t: f64) -> (f64, f64) {
        if self.friction >= 1.0 || t < 0.0 {
            return (0.0, self.velocity);
        }

        // Exponential decay: velocity(t) = v0 * friction^(t*60)
        let frames = t * 60.0;
        let velocity = self.velocity * self.friction.powf(frames);

        // Position: integrate velocity over time
        // position(t) = v0 * (friction^(t*60) - 1) / (60 * ln(friction))
        let ln_friction = self.friction.ln();
        let position = if ln_friction.abs() < 1e-10 {
            // Friction ≈ 1.0, avoid division by zero
            // Limit as friction → 1: position ≈ v0 * t
            self.velocity * t
        } else {
            self.velocity * (self.friction.powf(frames) - 1.0) / (60.0 * ln_friction)
        };

        // Apply bounds if set
        if self.min_bound.is_some() || self.max_bound.is_some() {
            self.apply_bounds(position, velocity, t)
        } else {
            (position, velocity)
        }
    }

    /// Apply spring force when position exceeds bounds
    fn apply_bounds(&self, position: f64, velocity: f64, _t: f64) -> (f64, f64) {
        let min = self.min_bound.unwrap_or(f64::NEG_INFINITY);
        let max = self.max_bound.unwrap_or(f64::INFINITY);

        if position < min {
            // Exceeded lower bound - apply spring-like resistance
            let overshoot = min - position;
            let resistance = self.calculate_spring_resistance(overshoot, velocity);
            let bounded_pos = min - overshoot * resistance;
            let damped_vel = velocity * resistance;
            (bounded_pos, damped_vel)
        } else if position > max {
            // Exceeded upper bound - apply spring-like resistance
            let overshoot = position - max;
            let resistance = self.calculate_spring_resistance(overshoot, velocity);
            let bounded_pos = max + overshoot * resistance;
            let damped_vel = velocity * resistance;
            (bounded_pos, damped_vel)
        } else {
            // Within bounds
            (position, velocity)
        }
    }

    /// Calculate spring resistance factor (0..1)
    /// Higher overshoot = more resistance = less position/velocity
    fn calculate_spring_resistance(&self, overshoot: f64, _velocity: f64) -> f64 {
        // Exponential decay based on overshoot distance
        // Prevents unbounded overshoot while allowing some elasticity
        let spring_factor = self.bounce_stiffness / 1000.0; // Normalize stiffness
        (-overshoot * spring_factor).exp().clamp(0.0, 1.0)
    }

    /// Is the animation at rest?
    ///
    /// Returns true when velocity falls below rest threshold.
    pub fn is_at_rest(&self, t: f64) -> bool {
        let (_, velocity) = self.evaluate(t);
        velocity.abs() < self.rest_threshold
    }

    /// Estimated duration until rest (seconds)
    ///
    /// Calculates when velocity will fall below rest threshold.
    /// Returns infinity if friction >= 1.0 or velocity is zero.
    pub fn estimated_duration(&self) -> f64 {
        if self.velocity.abs() < self.rest_threshold {
            return 0.0;
        }

        if self.friction >= 1.0 || self.friction <= 0.0 {
            return f64::INFINITY;
        }

        // Solve: v0 * friction^(t*60) = rest_threshold
        // t*60 = log(rest_threshold / v0) / log(friction)
        // t = log(rest_threshold / v0) / (60 * log(friction))
        let ln_friction = self.friction.ln();
        if ln_friction.abs() < 1e-10 {
            return f64::INFINITY;
        }

        let frames = (self.rest_threshold / self.velocity.abs()).ln() / ln_friction;
        (frames / 60.0).max(0.0)
    }

    /// Final resting position (without bounds)
    ///
    /// Calculates the position when velocity reaches zero.
    /// With bounds, actual resting position may differ due to spring bounce.
    pub fn final_position(&self) -> f64 {
        if self.friction >= 1.0 || self.friction <= 0.0 {
            return f64::INFINITY * self.velocity.signum();
        }

        let ln_friction = self.friction.ln();
        if ln_friction.abs() < 1e-10 {
            return f64::INFINITY * self.velocity.signum();
        }

        // Limit as t → ∞: position = -v0 / (60 * ln(friction))
        -self.velocity / (60.0 * ln_friction)
    }

    /// iOS-like flick scrolling (friction: 0.998)
    pub fn ios_scroll(velocity: f64) -> Self {
        Self::new(velocity).friction(0.998)
    }

    /// Fast deceleration with heavy friction (friction: 0.99)
    pub fn heavy(velocity: f64) -> Self {
        Self::new(velocity).friction(0.99)
    }

    /// Light deceleration with low friction (friction: 0.999)
    pub fn light(velocity: f64) -> Self {
        Self::new(velocity).friction(0.999)
    }
}

impl Default for Decay {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_velocity_immediate_rest() {
        let decay = Decay::new(0.0);
        assert!(decay.is_at_rest(0.0));
        assert!(decay.is_at_rest(1.0));

        let (pos, vel) = decay.evaluate(0.0);
        assert_eq!(pos, 0.0);
        assert_eq!(vel, 0.0);
    }

    #[test]
    fn positive_velocity_decelerates() {
        let decay = Decay::new(100.0).friction(0.95);

        let (pos1, vel1) = decay.evaluate(0.0);
        let (pos2, vel2) = decay.evaluate(0.5);
        let (pos3, vel3) = decay.evaluate(1.0);

        // Velocity should decrease over time
        assert!(vel1 > vel2);
        assert!(vel2 > vel3);

        // Position should increase (moving forward)
        assert!(pos1 < pos2);
        assert!(pos2 < pos3);

        // Velocity at t=0 should equal initial velocity
        assert!((vel1 - 100.0).abs() < 0.1);
    }

    #[test]
    fn final_position_is_finite() {
        let decay = Decay::new(500.0).friction(0.998);
        let final_pos = decay.final_position();

        assert!(final_pos.is_finite());
        assert!(final_pos > 0.0);

        // Should be reachable within reasonable time
        let duration = decay.estimated_duration();
        assert!(duration.is_finite());
        assert!(duration > 0.0);
    }

    #[test]
    fn higher_friction_shorter_distance() {
        let light = Decay::new(500.0).friction(0.999);
        let heavy = Decay::new(500.0).friction(0.99);

        let light_final = light.final_position();
        let heavy_final = heavy.final_position();

        // Lower friction (0.99) = more friction = shorter distance
        assert!(heavy_final < light_final);

        // Check durations too
        let light_duration = light.estimated_duration();
        let heavy_duration = heavy.estimated_duration();
        assert!(heavy_duration < light_duration);
    }

    #[test]
    fn bounds_contain_position() {
        let decay = Decay::new(500.0)  // More moderate velocity
            .friction(0.998)
            .bounds(0.0, 100.0);

        // Sample several time points
        for i in 0..20 {
            let t = i as f64 * 0.1;
            let (pos, _vel) = decay.evaluate(t);

            // With spring resistance, position should be constrained
            // Allow some overshoot due to momentum, but limited by spring
            assert!(
                pos >= -10.0 && pos <= 110.0,
                "Position {} out of range at t={}",
                pos, t
            );
        }

        // Final position should be within or very close to bounds
        let final_pos = decay.final_position();
        if final_pos.is_finite() {
            // Without spring physics in final_position calculation,
            // just verify it's calculated correctly
            assert!(final_pos > 100.0); // Would exceed bound without spring
        }
    }

    #[test]
    fn estimated_duration_positive() {
        let decay = Decay::new(100.0).friction(0.998);
        let duration = decay.estimated_duration();

        assert!(duration > 0.0);
        assert!(duration.is_finite());

        // Should be at rest after estimated duration
        let (_, vel) = decay.evaluate(duration);
        assert!(vel.abs() < decay.rest_threshold * 2.0);
    }

    #[test]
    fn rest_detection_works() {
        let decay = Decay::new(100.0)
            .friction(0.95)
            .rest_threshold(0.1);

        // Not at rest initially
        assert!(!decay.is_at_rest(0.0));

        // Should reach rest eventually
        let duration = decay.estimated_duration();

        // Check at a point well past estimated duration to ensure rest
        // (estimated_duration is when velocity reaches threshold, but we add buffer)
        assert!(decay.is_at_rest(duration * 1.5));
        assert!(decay.is_at_rest(duration * 2.0));
    }

    #[test]
    fn presets_have_different_behaviors() {
        let velocity = 500.0;
        let heavy = Decay::heavy(velocity);
        let ios = Decay::ios_scroll(velocity);
        let light = Decay::light(velocity);

        let heavy_dist = heavy.final_position();
        let ios_dist = ios.final_position();
        let light_dist = light.final_position();

        // Heavy friction (0.99) < iOS (0.998) < Light (0.999)
        assert!(heavy_dist < ios_dist);
        assert!(ios_dist < light_dist);
    }

    #[test]
    fn negative_velocity_decelerates_backward() {
        let decay = Decay::new(-200.0).friction(0.98);

        let (pos1, vel1) = decay.evaluate(0.5);
        let (pos2, vel2) = decay.evaluate(1.0);

        // Position should decrease (moving backward)
        assert!(pos1 < 0.0);
        assert!(pos2 < pos1);

        // Velocity should approach zero from negative
        assert!(vel1 < 0.0);
        assert!(vel2 < 0.0);
        assert!(vel1 < vel2); // Becoming less negative (approaching 0)
    }

    #[test]
    fn friction_near_one_behaves_reasonably() {
        // Test edge case where friction ≈ 1.0
        let decay = Decay::new(100.0).friction(0.9999);

        let (pos, vel) = decay.evaluate(0.1);
        assert!(pos.is_finite());
        assert!(vel.is_finite());
        assert!(pos > 0.0);
        assert!(vel > 0.0);
    }
}
