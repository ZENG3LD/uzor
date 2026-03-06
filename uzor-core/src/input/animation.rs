//! Animation helpers for smooth transitions and interpolations
//!
//! Provides easing functions, animated values, and state management for UI animations.

use super::widget_state::WidgetId;
use std::collections::HashMap;

/// Easing functions for smooth animations
pub mod easing {
    /// Linear interpolation (no easing)
    pub fn linear(t: f64) -> f64 {
        t
    }

    /// Ease in quadratic (slow start)
    pub fn ease_in_quad(t: f64) -> f64 {
        t * t
    }

    /// Ease out quadratic (slow end)
    pub fn ease_out_quad(t: f64) -> f64 {
        t * (2.0 - t)
    }

    /// Ease in-out quadratic (slow start and end)
    pub fn ease_in_out_quad(t: f64) -> f64 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        }
    }

    /// Ease out cubic (smoother slow end)
    pub fn ease_out_cubic(t: f64) -> f64 {
        1.0 - (1.0 - t).powi(3)
    }

    /// Ease in cubic (smoother slow start)
    pub fn ease_in_cubic(t: f64) -> f64 {
        t * t * t
    }

    /// Ease in-out cubic
    pub fn ease_in_out_cubic(t: f64) -> f64 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }

    /// Ease out elastic (bounce at end)
    pub fn ease_out_elastic(t: f64) -> f64 {
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let c4 = (2.0 * std::f64::consts::PI) / 3.0;
        2.0_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
    }

    /// Ease out back (slight overshoot)
    pub fn ease_out_back(t: f64) -> f64 {
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
    }
}

/// Type alias for easing function
pub type EasingFn = fn(f64) -> f64;

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
            easing: easing::ease_out_quad,
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

/// Manages multiple animated values by widget ID
#[derive(Clone, Debug)]
pub struct AnimationState {
    /// Animated float values
    values: HashMap<WidgetId, AnimatedValue>,
    /// Animated boolean values (as 0.0-1.0)
    bools: HashMap<WidgetId, AnimatedValue>,
    /// Default animation duration
    default_duration: f64,
    /// Default easing function
    default_easing: EasingFn,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationState {
    /// Create new animation state
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            bools: HashMap::new(),
            default_duration: 0.3,
            default_easing: easing::ease_out_quad,
        }
    }

    /// Set default animation duration
    pub fn set_default_duration(&mut self, duration: f64) {
        self.default_duration = duration;
    }

    /// Set default easing function
    pub fn set_default_easing(&mut self, easing: EasingFn) {
        self.default_easing = easing;
    }

    /// Animate a value and return current interpolated value
    pub fn animate_value(&mut self, id: &WidgetId, target: f64, time: f64) -> f64 {
        let entry = self.values.entry(id.clone()).or_insert_with(|| {
            // Start from 0.0 for new entries
            AnimatedValue::new(0.0)
                .with_duration(self.default_duration)
                .with_easing(self.default_easing)
        });
        entry.animate_to(target, time);
        entry.update(time)
    }

    /// Animate a boolean value (0.0 = false, 1.0 = true)
    pub fn animate_bool(&mut self, id: &WidgetId, target: bool, time: f64) -> f64 {
        let target_value = if target { 1.0 } else { 0.0 };
        let entry = self.bools.entry(id.clone()).or_insert_with(|| {
            // Start from 0.0 for new entries
            AnimatedValue::new(0.0)
                .with_duration(self.default_duration)
                .with_easing(self.default_easing)
        });
        entry.animate_to(target_value, time);
        entry.update(time)
    }

    /// Check if a specific widget has an active animation
    pub fn is_animating(&self, id: &WidgetId, time: f64) -> bool {
        self.values
            .get(id)
            .map(|v| v.is_animating(time))
            .unwrap_or(false)
            || self
                .bools
                .get(id)
                .map(|v| v.is_animating(time))
                .unwrap_or(false)
    }

    /// Check if any animation is in progress
    pub fn any_animating(&self, time: f64) -> bool {
        self.values.values().any(|v| v.is_animating(time))
            || self.bools.values().any(|v| v.is_animating(time))
    }

    /// Remove animation state for a widget
    pub fn remove(&mut self, id: &WidgetId) {
        self.values.remove(id);
        self.bools.remove(id);
    }

    /// Clear all animations
    pub fn clear(&mut self) {
        self.values.clear();
        self.bools.clear();
    }
}

/// Linear interpolation between two values
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Linear interpolation for colors (r, g, b, a as 0.0-1.0)
pub fn lerp_color(
    a: (f64, f64, f64, f64),
    b: (f64, f64, f64, f64),
    t: f64,
) -> (f64, f64, f64, f64) {
    (
        lerp(a.0, b.0, t),
        lerp(a.1, b.1, t),
        lerp(a.2, b.2, t),
        lerp(a.3, b.3, t),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions_bounds() {
        // Test all easing functions at boundaries
        let easing_fns = [
            easing::linear,
            easing::ease_in_quad,
            easing::ease_out_quad,
            easing::ease_in_out_quad,
            easing::ease_out_cubic,
            easing::ease_in_cubic,
            easing::ease_in_out_cubic,
            easing::ease_out_elastic,
            easing::ease_out_back,
        ];

        for easing_fn in &easing_fns {
            // All functions should return 0 at t=0 (except elastic/back might overshoot)
            let result_0 = easing_fn(0.0);
            assert!(
                (result_0 - 0.0).abs() < 0.1,
                "Easing function should be near 0 at t=0"
            );

            // All functions should return 1 at t=1
            let result_1 = easing_fn(1.0);
            assert!(
                (result_1 - 1.0).abs() < 0.1,
                "Easing function should be near 1 at t=1"
            );
        }
    }

    #[test]
    fn test_easing_linear() {
        assert_eq!(easing::linear(0.0), 0.0);
        assert_eq!(easing::linear(0.5), 0.5);
        assert_eq!(easing::linear(1.0), 1.0);
    }

    #[test]
    fn test_easing_quad() {
        assert_eq!(easing::ease_in_quad(0.0), 0.0);
        assert_eq!(easing::ease_in_quad(0.5), 0.25);
        assert_eq!(easing::ease_in_quad(1.0), 1.0);

        assert_eq!(easing::ease_out_quad(0.0), 0.0);
        assert_eq!(easing::ease_out_quad(0.5), 0.75);
        assert_eq!(easing::ease_out_quad(1.0), 1.0);
    }

    #[test]
    fn test_easing_cubic() {
        assert_eq!(easing::ease_in_cubic(0.0), 0.0);
        assert_eq!(easing::ease_in_cubic(0.5), 0.125);
        assert_eq!(easing::ease_in_cubic(1.0), 1.0);

        assert_eq!(easing::ease_out_cubic(0.0), 0.0);
        assert_eq!(easing::ease_out_cubic(0.5), 0.875);
        assert_eq!(easing::ease_out_cubic(1.0), 1.0);
    }

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

    #[test]
    fn test_animation_state_value() {
        let mut state = AnimationState::new();
        let id = WidgetId::new("widget1");

        // Animate to 5.0, starting from 0.0 at time 0.0
        let result = state.animate_value(&id, 5.0, 0.0);
        // At start time, should be close to start value (0.0)
        assert!(result < 1.0);

        // Later in the animation
        let result = state.animate_value(&id, 5.0, 0.2);
        assert!(result > 1.0 && result < 5.0);

        // Change target to 10.0
        state.animate_value(&id, 10.0, 0.5);
        let result = state.animate_value(&id, 10.0, 0.7);
        assert!(result > 5.0 && result <= 10.0);
    }

    #[test]
    fn test_animation_state_bool() {
        let mut state = AnimationState::new();
        let id = WidgetId::new("widget1");

        // Animate to true (1.0), starting from 0.0 at time 0.0
        let result = state.animate_bool(&id, true, 0.0);
        // At start time, should be close to start value (0.0)
        assert!(result < 0.3);

        // Later in the animation
        let result = state.animate_bool(&id, true, 0.2);
        assert!(result > 0.3 && result <= 1.0);

        // Animate to false (0.0)
        state.animate_bool(&id, false, 1.0);
        let result = state.animate_bool(&id, false, 1.2);
        assert!(result >= 0.0 && result < 0.7);
    }

    #[test]
    fn test_animation_state_is_animating() {
        let mut state = AnimationState::new();
        state.set_default_duration(1.0);
        let id = WidgetId::new("widget1");

        // Start animation
        state.animate_value(&id, 10.0, 0.0);

        // Should be animating
        assert!(state.is_animating(&id, 0.5));

        // After duration, should not be animating
        state.animate_value(&id, 10.0, 2.0);
        assert!(!state.is_animating(&id, 2.0));
    }

    #[test]
    fn test_animation_state_any_animating() {
        let mut state = AnimationState::new();
        state.set_default_duration(1.0);
        let id1 = WidgetId::new("widget1");
        let id2 = WidgetId::new("widget2");

        // No animations
        assert!(!state.any_animating(0.0));

        // Start animation on id1
        state.animate_value(&id1, 10.0, 0.0);
        assert!(state.any_animating(0.5));

        // Start animation on id2
        state.animate_bool(&id2, true, 0.5);
        assert!(state.any_animating(1.0));

        // All done
        state.animate_value(&id1, 10.0, 2.0);
        state.animate_bool(&id2, true, 2.0);
        assert!(!state.any_animating(2.0));
    }

    #[test]
    fn test_animation_state_remove() {
        let mut state = AnimationState::new();
        let id = WidgetId::new("widget1");

        state.animate_value(&id, 10.0, 0.0);
        assert!(state.is_animating(&id, 0.0));

        state.remove(&id);
        assert!(!state.is_animating(&id, 0.0));
    }

    #[test]
    fn test_animation_state_clear() {
        let mut state = AnimationState::new();
        let id1 = WidgetId::new("widget1");
        let id2 = WidgetId::new("widget2");

        state.animate_value(&id1, 10.0, 0.0);
        state.animate_bool(&id2, true, 0.0);
        assert!(state.any_animating(0.0));

        state.clear();
        assert!(!state.any_animating(0.0));
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);

        assert_eq!(lerp(5.0, 15.0, 0.5), 10.0);
    }

    #[test]
    fn test_lerp_color() {
        let black = (0.0, 0.0, 0.0, 1.0);
        let white = (1.0, 1.0, 1.0, 1.0);

        let result = lerp_color(black, white, 0.0);
        assert_eq!(result, black);

        let result = lerp_color(black, white, 1.0);
        assert_eq!(result, white);

        let result = lerp_color(black, white, 0.5);
        assert_eq!(result, (0.5, 0.5, 0.5, 1.0));

        // Test with alpha blending
        let transparent_black = (0.0, 0.0, 0.0, 0.0);
        let opaque_white = (1.0, 1.0, 1.0, 1.0);

        let result = lerp_color(transparent_black, opaque_white, 0.5);
        assert_eq!(result, (0.5, 0.5, 0.5, 0.5));
    }

    #[test]
    fn test_custom_easing_and_duration() {
        let mut state = AnimationState::new();
        state.set_default_duration(2.0);
        state.set_default_easing(easing::linear);

        let id = WidgetId::new("widget1");

        // First access should use defaults
        state.animate_value(&id, 10.0, 0.0);
        let mid = state.animate_value(&id, 10.0, 1.0);

        // With linear easing and 2.0 duration, at t=1.0 should be at 0.5 progress
        assert!((mid - 5.0).abs() < 0.1);
    }
}
