//! Animation coordinator - manages active animations

use std::collections::HashMap;

use crate::types::WidgetId;
use uzor_animation::{Decay, Easing, InterruptionStrategy, Spring};

use super::types::{ActiveAnimation, AnimationKey};

/// Central animation coordinator for uzor-core
///
/// Manages all active animations keyed by (WidgetId, property_name).
/// Integrates with uzor-animation primitives and the per-frame render loop.
pub struct AnimationCoordinator {
    /// Active animations: widget_id + property_key → ActiveAnimation
    active: HashMap<AnimationKey, ActiveAnimation>,
    /// Default interruption strategy
    default_interruption: InterruptionStrategy,
}

impl AnimationCoordinator {
    /// Create a new animation coordinator
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            default_interruption: InterruptionStrategy::default(),
        }
    }

    /// Tick all active animations. Call once per frame.
    ///
    /// Returns true if any animation is still active (needs repaint).
    pub fn update(&mut self, time_secs: f64) -> bool {
        // Update all animations
        for anim in self.active.values_mut() {
            anim.update(time_secs);
        }

        // Clean up completed animations
        self.cleanup_completed();

        // Return true if any animations are still active
        !self.active.is_empty()
    }

    /// Get current animated value for a property.
    ///
    /// Returns None if no animation is active for this property.
    pub fn get(&self, widget_id: &WidgetId, property: &str) -> Option<f64> {
        let key = AnimationKey::new(widget_id.clone(), property);
        self.active.get(&key).map(|anim| anim.current_value)
    }

    /// Get value or return default if no animation.
    pub fn get_or(&self, widget_id: &WidgetId, property: &str, default: f64) -> f64 {
        self.get(widget_id, property).unwrap_or(default)
    }

    /// Start a tween animation.
    ///
    /// If an animation is already active on this property, it will be replaced
    /// according to the interruption strategy.
    pub fn tween(
        &mut self,
        widget_id: WidgetId,
        property: impl Into<String>,
        from: f64,
        to: f64,
        duration_secs: f64,
        easing: Easing,
        time_secs: f64,
    ) {
        let key = AnimationKey::new(widget_id, property);
        let anim = ActiveAnimation::tween(from, to, time_secs, duration_secs, easing);
        self.insert_animation(key, anim);
    }

    /// Start a spring-driven animation.
    ///
    /// The spring animates from its initial displacement (1.0) toward the target.
    pub fn spring(
        &mut self,
        widget_id: WidgetId,
        property: impl Into<String>,
        spring: Spring,
        target: f64,
        time_secs: f64,
    ) {
        let key = AnimationKey::new(widget_id, property);
        let anim = ActiveAnimation::spring(spring, time_secs, target);
        self.insert_animation(key, anim);
    }

    /// Start a decay animation (e.g. flick scroll).
    pub fn decay(
        &mut self,
        widget_id: WidgetId,
        property: impl Into<String>,
        decay: Decay,
        initial_value: f64,
        time_secs: f64,
    ) {
        let key = AnimationKey::new(widget_id, property);
        let anim = ActiveAnimation::decay(decay, time_secs, initial_value);
        self.insert_animation(key, anim);
    }

    /// Cancel all animations for a widget.
    pub fn cancel_widget(&mut self, widget_id: &WidgetId) {
        self.active
            .retain(|key, _| &key.widget_id != widget_id);
    }

    /// Cancel a specific property animation.
    pub fn cancel(&mut self, widget_id: &WidgetId, property: &str) {
        let key = AnimationKey::new(widget_id.clone(), property);
        self.active.remove(&key);
    }

    /// Check if any animations are active.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// Check if a widget has active animations.
    pub fn is_animating(&self, widget_id: &WidgetId) -> bool {
        self.active.keys().any(|key| &key.widget_id == widget_id)
    }

    /// Set default interruption strategy.
    pub fn set_interruption_strategy(&mut self, strategy: InterruptionStrategy) {
        self.default_interruption = strategy;
    }

    /// Number of active animations (for diagnostics).
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Insert an animation, handling interruption if necessary
    fn insert_animation(&mut self, key: AnimationKey, anim: ActiveAnimation) {
        // For now, just replace any existing animation (Instant strategy)
        // TODO: Implement other interruption strategies (Blend, InheritVelocity, Queue)
        self.active.insert(key, anim);
    }

    /// Remove completed animations (called internally by update)
    fn cleanup_completed(&mut self) {
        self.active.retain(|_, anim| !anim.completed);
    }
}

impl Default for AnimationCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tween_lifecycle() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        // Start animation at t=0
        coord.tween(
            widget_id.clone(),
            "opacity",
            0.0,
            1.0,
            1.0,
            Easing::Linear,
            0.0,
        );

        assert!(coord.has_active());
        assert!(coord.is_animating(&widget_id));

        // At t=0, value should be 0.0 (initial value before any update)
        let val = coord.get(&widget_id, "opacity").unwrap();
        assert!((val - 0.0).abs() < 0.001);

        // Update to t=0.5 (halfway)
        coord.update(0.5);
        let val = coord.get(&widget_id, "opacity").unwrap();
        assert!((val - 0.5).abs() < 0.001);

        // Update to t=0.99 (almost at end, should still be active)
        coord.update(0.99);
        let val = coord.get(&widget_id, "opacity").unwrap();
        assert!((val - 0.99).abs() < 0.01);
        assert!(coord.has_active());

        // Update to t=1.0 (at end, should complete and be cleaned up)
        coord.update(1.0);
        assert!(!coord.has_active());
        assert_eq!(coord.get(&widget_id, "opacity"), None);
    }

    #[test]
    fn test_spring_animation() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        let spring = Spring::stiff();

        // Start spring animation at t=0, targeting value 100.0
        coord.spring(widget_id.clone(), "x", spring, 100.0, 0.0);

        assert!(coord.has_active());

        // At t=0, spring is at max displacement, so value = target - 1.0
        coord.update(0.0);
        let val = coord.get(&widget_id, "x").unwrap();
        assert!((val - 99.0).abs() < 0.1); // Should be near 99.0

        // Advance time - spring should approach target
        coord.update(0.5);
        let val = coord.get(&widget_id, "x").unwrap();
        assert!(val > 99.0 && val < 101.0); // Should be closer to 100.0

        // Eventually settles (stiff spring settles quickly)
        coord.update(5.0);
        assert!(!coord.has_active()); // Should be cleaned up when settled
    }

    #[test]
    fn test_decay_animation() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        let decay = Decay::new(500.0).friction(0.95); // Fast decay

        // Start decay at t=0, initial value 0.0
        coord.decay(widget_id.clone(), "scroll_y", decay, 0.0, 0.0);

        assert!(coord.has_active());

        // At t=0, should be at initial value
        coord.update(0.0);
        let val = coord.get(&widget_id, "scroll_y").unwrap();
        assert!((val - 0.0).abs() < 0.1);

        // Advance - position should increase
        coord.update(0.1);
        let val = coord.get(&widget_id, "scroll_y").unwrap();
        assert!(val > 0.0);

        // Eventually comes to rest and gets cleaned up
        coord.update(10.0);
        assert!(!coord.has_active());
    }

    #[test]
    fn test_multiple_widgets_simultaneously() {
        let mut coord = AnimationCoordinator::new();
        let widget1 = WidgetId::new("widget1");
        let widget2 = WidgetId::new("widget2");

        coord.tween(widget1.clone(), "x", 0.0, 100.0, 1.0, Easing::Linear, 0.0);
        coord.tween(widget2.clone(), "y", 0.0, 200.0, 1.0, Easing::Linear, 0.0);

        assert_eq!(coord.active_count(), 2);

        coord.update(0.5);

        let val1 = coord.get(&widget1, "x").unwrap();
        let val2 = coord.get(&widget2, "y").unwrap();

        assert!((val1 - 50.0).abs() < 0.1);
        assert!((val2 - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_cancel_widget() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        coord.tween(
            widget_id.clone(),
            "x",
            0.0,
            100.0,
            1.0,
            Easing::Linear,
            0.0,
        );
        coord.tween(
            widget_id.clone(),
            "y",
            0.0,
            100.0,
            1.0,
            Easing::Linear,
            0.0,
        );

        assert_eq!(coord.active_count(), 2);

        coord.cancel_widget(&widget_id);
        assert_eq!(coord.active_count(), 0);
    }

    #[test]
    fn test_cancel_property() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        coord.tween(
            widget_id.clone(),
            "x",
            0.0,
            100.0,
            1.0,
            Easing::Linear,
            0.0,
        );
        coord.tween(
            widget_id.clone(),
            "y",
            0.0,
            100.0,
            1.0,
            Easing::Linear,
            0.0,
        );

        assert_eq!(coord.active_count(), 2);

        coord.cancel(&widget_id, "x");
        assert_eq!(coord.active_count(), 1);
        assert_eq!(coord.get(&widget_id, "x"), None);
        assert!(coord.get(&widget_id, "y").is_some());
    }

    #[test]
    fn test_interruption_replaces_old() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        // Start first animation
        coord.tween(
            widget_id.clone(),
            "x",
            0.0,
            100.0,
            1.0,
            Easing::Linear,
            0.0,
        );

        coord.update(0.5);
        let val = coord.get(&widget_id, "x").unwrap();
        assert!((val - 50.0).abs() < 0.1);

        // Interrupt with new animation
        coord.tween(
            widget_id.clone(),
            "x",
            50.0,
            200.0,
            1.0,
            Easing::Linear,
            0.5,
        );

        // Old animation should be replaced
        coord.update(0.5);
        let val = coord.get(&widget_id, "x").unwrap();
        assert!((val - 50.0).abs() < 0.1); // New animation starts at 50.0
    }

    #[test]
    fn test_get_or_with_default() {
        let coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        let val = coord.get_or(&widget_id, "opacity", 1.0);
        assert_eq!(val, 1.0);
    }

    #[test]
    fn test_has_active_is_animating() {
        let mut coord = AnimationCoordinator::new();
        let widget1 = WidgetId::new("widget1");
        let widget2 = WidgetId::new("widget2");

        assert!(!coord.has_active());
        assert!(!coord.is_animating(&widget1));

        coord.tween(widget1.clone(), "x", 0.0, 100.0, 1.0, Easing::Linear, 0.0);

        assert!(coord.has_active());
        assert!(coord.is_animating(&widget1));
        assert!(!coord.is_animating(&widget2));
    }

    #[test]
    fn test_easing_functions() {
        let mut coord = AnimationCoordinator::new();
        let widget_id = WidgetId::new("widget1");

        // Test EaseInQuad
        coord.tween(
            widget_id.clone(),
            "x",
            0.0,
            100.0,
            1.0,
            Easing::EaseInQuad,
            0.0,
        );

        coord.update(0.5);
        let val = coord.get(&widget_id, "x").unwrap();
        // EaseInQuad at t=0.5 should give 0.25, so value = 0 + 100*0.25 = 25
        assert!((val - 25.0).abs() < 0.1);
    }
}
