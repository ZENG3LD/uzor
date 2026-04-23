//! Animated value helper for smooth single-value transitions.

use crate::core::animation::easing::Easing;

/// Type alias for easing function
pub type EasingFn = fn(f64) -> f64;

fn default_easing_fn(t: f64) -> f64 {
    Easing::EaseOutQuad.ease(t)
}

/// State for animating a single f64 value
#[derive(Clone, Debug)]
pub struct AnimatedValue {
    /// Current interpolated value
    current: f64,
    /// Target value to animate towards
    target: f64,
    /// Value when animation started
    start_value: f64,
    /// Time when animation started
    start_time: f64,
    /// Animation duration in seconds
    duration: f64,
    /// Easing function
    easing: EasingFn,
}

impl AnimatedValue {
    /// Create new animated value
    pub fn new(initial: f64) -> Self {
        Self {
            current: initial,
            target: initial,
            start_value: initial,
            start_time: 0.0,
            duration: 0.3,
            easing: default_easing_fn,
        }
    }

    /// Set the easing function
    pub fn with_easing(mut self, easing: EasingFn) -> Self {
        self.easing = easing;
        self
    }

    /// Set animation duration
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    /// Set a new target and start animation
    pub fn animate_to(&mut self, target: f64, time: f64) {
        if (self.target - target).abs() > 0.0001 {
            self.start_value = self.current;
            self.target = target;
            self.start_time = time;
        }
    }

    /// Update animation and return current value
    pub fn update(&mut self, time: f64) -> f64 {
        if self.duration <= 0.0 {
            self.current = self.target;
            return self.current;
        }

        let elapsed = time - self.start_time;
        let t = (elapsed / self.duration).clamp(0.0, 1.0);
        let eased_t = (self.easing)(t);

        self.current = self.start_value + (self.target - self.start_value) * eased_t;
        self.current
    }

    /// Check if animation is in progress
    pub fn is_animating(&self, time: f64) -> bool {
        let elapsed = time - self.start_time;
        elapsed < self.duration && (self.start_value - self.target).abs() > 0.0001
    }

    /// Get current value without updating
    pub fn get(&self) -> f64 {
        self.current
    }

    /// Get target value
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Instantly set value (no animation)
    pub fn set(&mut self, value: f64) {
        self.current = value;
        self.target = value;
        self.start_value = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animated_value_creation() {
        let value = AnimatedValue::new(5.0);
        assert_eq!(value.get(), 5.0);
        assert_eq!(value.target(), 5.0);
    }

    #[test]
    fn test_animated_value_instant_set() {
        let mut value = AnimatedValue::new(0.0);
        value.set(10.0);
        assert_eq!(value.get(), 10.0);
        assert_eq!(value.target(), 10.0);
        assert!(!value.is_animating(0.0));
    }

    #[test]
    fn test_animated_value_animation() {
        let mut value = AnimatedValue::new(0.0).with_duration(1.0);

        // Start animation to 10.0 at time 0
        value.animate_to(10.0, 0.0);

        // At time 0, should be at start
        assert!((value.update(0.0) - 0.0).abs() < 0.1);

        // At time 0.5 (halfway), should be somewhere between 0 and 10
        let mid = value.update(0.5);
        assert!(mid > 3.0 && mid < 10.0);

        // At time 1.0 (end), should be at target
        assert!((value.update(1.0) - 10.0).abs() < 0.1);

        // Should no longer be animating
        assert!(!value.is_animating(1.5));
    }

    #[test]
    fn test_animated_value_zero_duration() {
        let mut value = AnimatedValue::new(0.0).with_duration(0.0);
        value.animate_to(10.0, 0.0);

        // Should instantly reach target
        assert_eq!(value.update(0.0), 10.0);
    }
}
