//! Spring physics with analytical solution
//!
//! Based on wobble/Framer Motion approach - uses closed-form solution
//! for damped harmonic oscillator rather than numerical integration.
//!
//! Advantages:
//! - Perfect accuracy, no drift
//! - Pure function of time: position = f(t), velocity = g(t)
//! - Frame-rate independent
//! - Faster than RK4 (no iterative solving)

use std::f64::consts::E;

/// Spring configuration with physics-based parameters
///
/// # Example
/// ```
/// use uzor_core::animation::Spring;
///
/// let spring = Spring::new()
///     .stiffness(180.0)
///     .damping(12.0)
///     .mass(1.0);
///
/// let (position, velocity) = spring.evaluate(0.1);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    /// Spring stiffness (rigidity). Higher = more sudden movement.
    /// Default: 100.0
    pub stiffness: f64,

    /// Damping coefficient (friction). Dissipates energy, slows oscillation.
    /// Default: 10.0
    pub damping: f64,

    /// Mass of the animated object. Higher = more lethargic movement.
    /// Default: 1.0
    pub mass: f64,

    /// Initial velocity (speed at t=0).
    /// Default: 0.0
    pub initial_velocity: f64,

    /// Rest threshold - animation stops when |displacement| + |velocity| < threshold.
    /// Default: 0.001
    pub rest_threshold: f64,
}

impl Default for Spring {
    fn default() -> Self {
        Self::new()
    }
}

impl Spring {
    /// Small epsilon to prevent division by zero
    const EPSILON: f64 = 1e-9;

    /// Create a new spring with default parameters
    #[inline]
    pub fn new() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            initial_velocity: 0.0,
            rest_threshold: 0.001,
        }
    }

    /// Set stiffness (spring rigidity)
    #[inline]
    pub fn stiffness(mut self, s: f64) -> Self {
        self.stiffness = s.max(Self::EPSILON);
        self
    }

    /// Set damping coefficient
    #[inline]
    pub fn damping(mut self, d: f64) -> Self {
        self.damping = d.max(0.0);
        self
    }

    /// Set mass
    #[inline]
    pub fn mass(mut self, m: f64) -> Self {
        self.mass = m.max(Self::EPSILON);
        self
    }

    /// Set initial velocity
    #[inline]
    pub fn initial_velocity(mut self, v: f64) -> Self {
        self.initial_velocity = v;
        self
    }

    /// Set rest threshold
    #[inline]
    pub fn rest_threshold(mut self, t: f64) -> Self {
        self.rest_threshold = t.max(0.0);
        self
    }

    /// Calculate damping ratio: ζ = damping / (2 * sqrt(stiffness * mass))
    ///
    /// Determines oscillation behavior:
    /// - ζ < 1: Under-damped (bouncy, oscillates)
    /// - ζ = 1: Critically damped (fastest settling, no overshoot)
    /// - ζ > 1: Over-damped (slow, no oscillation)
    #[inline]
    pub fn damping_ratio(&self) -> f64 {
        self.damping / (2.0 * (self.stiffness * self.mass).sqrt())
    }

    /// Calculate angular frequency: ω₀ = sqrt(stiffness / mass)
    #[inline]
    pub fn angular_frequency(&self) -> f64 {
        (self.stiffness / self.mass).sqrt()
    }

    /// Evaluate spring position and velocity at time t (seconds from start)
    ///
    /// Returns (position, velocity) where:
    /// - position: displacement from target (1.0 at start, 0.0 at rest)
    /// - velocity: rate of change
    ///
    /// Uses analytical solution for damped harmonic oscillator.
    pub fn evaluate(&self, t: f64) -> (f64, f64) {
        if t <= 0.0 {
            return (1.0, self.initial_velocity);
        }

        let zeta = self.damping_ratio();
        let w0 = self.angular_frequency();

        // Initial conditions: x0 = 1.0 (displacement), v0 = initial_velocity
        let x0 = 1.0;
        let v0 = self.initial_velocity;

        if (zeta - 1.0).abs() < 1e-6 {
            // Critically damped (ζ = 1)
            self.evaluate_critical(t, w0, x0, v0)
        } else if zeta < 1.0 {
            // Under-damped (ζ < 1) - oscillates
            self.evaluate_underdamped(t, w0, zeta, x0, v0)
        } else {
            // Over-damped (ζ > 1) - slow approach
            self.evaluate_overdamped(t, w0, zeta, x0, v0)
        }
    }

    /// Under-damped case: x(t) = e^(-ζω₀t) * (A*cos(ωd*t) + B*sin(ωd*t))
    #[inline]
    fn evaluate_underdamped(&self, t: f64, w0: f64, zeta: f64, x0: f64, v0: f64) -> (f64, f64) {
        // Damped angular frequency: ωd = ω₀ * sqrt(1 - ζ²)
        let wd = w0 * (1.0 - zeta * zeta).sqrt();

        // Constants from initial conditions
        let a = x0;
        let b = (v0 + zeta * w0 * x0) / wd;

        // Decay envelope
        let envelope = E.powf(-zeta * w0 * t);

        // Oscillation
        let cos_term = (wd * t).cos();
        let sin_term = (wd * t).sin();

        // Position: x(t)
        let position = envelope * (a * cos_term + b * sin_term);

        // Velocity: x'(t) - derivative of position
        let velocity = envelope * (
            (-zeta * w0) * (a * cos_term + b * sin_term)
            + wd * (-a * sin_term + b * cos_term)
        );

        (position, velocity)
    }

    /// Critically damped case: x(t) = (A + B*t) * e^(-ω₀t)
    #[inline]
    fn evaluate_critical(&self, t: f64, w0: f64, x0: f64, v0: f64) -> (f64, f64) {
        let a = x0;
        let b = v0 + w0 * x0;

        let envelope = E.powf(-w0 * t);

        // Position
        let position = (a + b * t) * envelope;

        // Velocity: derivative of position
        let velocity = envelope * (b - w0 * (a + b * t));

        (position, velocity)
    }

    /// Over-damped case: x(t) = A*e^(r₁*t) + B*e^(r₂*t)
    #[inline]
    fn evaluate_overdamped(&self, t: f64, w0: f64, zeta: f64, x0: f64, v0: f64) -> (f64, f64) {
        // Characteristic roots
        let sqrt_term = (zeta * zeta - 1.0).sqrt();
        let r1 = -w0 * (zeta - sqrt_term);
        let r2 = -w0 * (zeta + sqrt_term);

        // Solve for constants A and B from initial conditions:
        // x(0) = A + B = x0
        // x'(0) = A*r1 + B*r2 = v0
        let a = (v0 - r2 * x0) / (r1 - r2);
        let b = x0 - a;

        let exp1 = E.powf(r1 * t);
        let exp2 = E.powf(r2 * t);

        // Position
        let position = a * exp1 + b * exp2;

        // Velocity
        let velocity = a * r1 * exp1 + b * r2 * exp2;

        (position, velocity)
    }

    /// Check if spring is at rest at time t
    #[inline]
    pub fn is_at_rest(&self, t: f64) -> bool {
        let (position, velocity) = self.evaluate(t);
        position.abs() + velocity.abs() < self.rest_threshold
    }

    /// Estimate duration until spring reaches rest
    ///
    /// Uses exponential decay envelope to estimate settling time.
    /// For under-damped and critically damped springs, the envelope
    /// decays as e^(-ζω₀t). We solve for when envelope < threshold.
    pub fn estimated_duration(&self) -> f64 {
        let zeta = self.damping_ratio();
        let w0 = self.angular_frequency();

        if w0 < Self::EPSILON {
            return 100.0; // Very weak spring, arbitrary large duration
        }

        // For under/critically damped: envelope = e^(-ζω₀t)
        // Solve: e^(-ζω₀t) < threshold
        // -ζω₀t < ln(threshold)
        // t > -ln(threshold) / (ζω₀)

        let decay_rate = zeta * w0;

        if decay_rate < Self::EPSILON {
            return 100.0; // No damping, never settles
        }

        // Use rest_threshold as target amplitude
        let duration = -self.rest_threshold.ln() / decay_rate;

        // Clamp to reasonable bounds
        duration.clamp(0.0, 100.0)
    }

    /// Gentle spring - iOS-style default
    ///
    /// Smooth, natural feel with subtle bounce.
    #[inline]
    pub fn gentle() -> Self {
        Self::new().stiffness(120.0).damping(14.0)
    }

    /// Bouncy spring - noticeable overshoot
    ///
    /// Playful animation with visible oscillation.
    #[inline]
    pub fn bouncy() -> Self {
        Self::new().stiffness(180.0).damping(12.0)
    }

    /// Stiff spring - fast, minimal overshoot
    ///
    /// Snappy response, crisp motion.
    #[inline]
    pub fn stiff() -> Self {
        Self::new().stiffness(300.0).damping(20.0)
    }

    /// Slow spring - deliberate, smooth
    ///
    /// Leisurely animation with soft motion.
    #[inline]
    pub fn slow() -> Self {
        Self::new().stiffness(60.0).damping(14.0)
    }

    /// Convert spring to a lookup table for use as an easing function
    ///
    /// Returns vector of position values sampled over estimated duration.
    /// Each sample represents spring position at t = i * (duration / samples).
    ///
    /// Can be used to create an easing curve from spring physics.
    pub fn as_easing(&self, samples: usize) -> Vec<f64> {
        if samples == 0 {
            return vec![];
        }

        let duration = self.estimated_duration();
        let dt = duration / samples.max(1) as f64;

        (0..samples)
            .map(|i| {
                let t = i as f64 * dt;
                let (position, _) = self.evaluate(t);
                // Invert: spring goes from 1.0 -> 0.0, easing wants 0.0 -> 1.0
                1.0 - position.clamp(0.0, 1.0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damping_ratio() {
        let spring = Spring::new();
        let ratio = spring.damping_ratio();

        // Default: damping=10, stiffness=100, mass=1
        // ratio = 10 / (2 * sqrt(100 * 1)) = 10 / 20 = 0.5
        assert!((ratio - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_angular_frequency() {
        let spring = Spring::new();
        let w0 = spring.angular_frequency();

        // w0 = sqrt(100 / 1) = 10
        assert!((w0 - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_initial_conditions() {
        let spring = Spring::new();
        let (pos, vel) = spring.evaluate(0.0);

        assert!((pos - 1.0).abs() < 1e-6);
        assert!(vel.abs() < 1e-6);
    }

    #[test]
    fn test_underdamped_oscillation() {
        // Under-damped spring should oscillate
        let spring = Spring::bouncy(); // zeta < 1

        let mut positions = Vec::new();
        for i in 0..100 {
            let t = i as f64 * 0.01;
            let (pos, _) = spring.evaluate(t);
            positions.push(pos);
        }

        // Should have oscillations (sign changes)
        let mut sign_changes = 0;
        for i in 1..positions.len() {
            if positions[i-1] * positions[i] < 0.0 {
                sign_changes += 1;
            }
        }

        assert!(sign_changes > 0, "Under-damped spring should oscillate");
    }

    #[test]
    fn test_overdamped_no_oscillation() {
        // Over-damped spring should not overshoot
        let spring = Spring::new().stiffness(50.0).damping(50.0); // zeta > 1

        for i in 0..100 {
            let t = i as f64 * 0.01;
            let (pos, _) = spring.evaluate(t);

            // Position should stay positive (no overshoot past target)
            assert!(pos >= -1e-6, "Over-damped spring should not overshoot");
        }
    }

    #[test]
    fn test_critically_damped_fast_settle() {
        // Critically damped should be fastest without overshoot
        let spring = Spring::new().stiffness(100.0).damping(20.0); // zeta = 1
        let ratio = spring.damping_ratio();

        assert!((ratio - 1.0).abs() < 1e-3);

        // Should settle quickly
        let duration = spring.estimated_duration();
        assert!(duration > 0.0 && duration < 10.0);
    }

    #[test]
    fn test_rest_detection() {
        let spring = Spring::stiff();

        // At t=0, should not be at rest
        assert!(!spring.is_at_rest(0.0));

        // After long time, should be at rest
        assert!(spring.is_at_rest(5.0));
    }

    #[test]
    fn test_presets() {
        let gentle = Spring::gentle();
        let bouncy = Spring::bouncy();
        let stiff = Spring::stiff();
        let slow = Spring::slow();

        // All should have valid parameters
        assert!(gentle.stiffness > 0.0);
        assert!(bouncy.stiffness > 0.0);
        assert!(stiff.stiffness > 0.0);
        assert!(slow.stiffness > 0.0);
    }

    #[test]
    fn test_as_easing() {
        let spring = Spring::gentle();
        let easing = spring.as_easing(100);

        assert_eq!(easing.len(), 100);

        // First value should be near 0 (inverted from position=1)
        assert!(easing[0] < 0.1);

        // Last value should be near 1 (inverted from position=0)
        assert!(easing[99] > 0.9);

        // Values should be monotonically increasing (for over/critically damped)
        // or mostly increasing (for under-damped)
        // Just check first and last are correct direction
        assert!(easing[99] > easing[0]);
    }

    #[test]
    fn test_initial_velocity() {
        let spring = Spring::new().initial_velocity(10.0);
        let (_, vel) = spring.evaluate(0.0);

        assert!((vel - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_edge_cases() {
        // Zero stiffness should clamp to epsilon
        let spring = Spring::new().stiffness(0.0);
        assert!(spring.stiffness > 0.0);

        // Zero mass should clamp to epsilon
        let spring = Spring::new().mass(0.0);
        assert!(spring.mass > 0.0);

        // Negative damping should clamp to 0
        let spring = Spring::new().damping(-5.0);
        assert!(spring.damping >= 0.0);
    }

    #[test]
    fn test_energy_conservation() {
        // Analytical solution should not drift over time
        let spring = Spring::bouncy();

        let duration = spring.estimated_duration();
        let (pos_early, _) = spring.evaluate(duration * 0.1);
        let (pos_late, _) = spring.evaluate(duration * 0.9);

        // Both should be within reasonable bounds
        assert!(pos_early.abs() <= 2.0); // Allow some overshoot
        assert!(pos_late.abs() < spring.rest_threshold * 10.0);
    }
}
