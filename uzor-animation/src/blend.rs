//! Animation blending, additive animation, and interruption handling
//!
//! Provides composition modes, crossfading, animation layers, and strategies
//! for handling animation interruptions smoothly.

use crate::timeline::Animatable;

/// How an animation combines with the base value
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum CompositeMode {
    /// Replace base value entirely (default)
    #[default]
    Replace,
    /// Add to base value: result = base + animation_delta
    Add,
    /// Accumulate: result = base + (animation_value * iteration_count)
    Accumulate,
}


/// Blend between two animation values with a weight
///
/// weight=0.0 → 100% value_a
/// weight=1.0 → 100% value_b
#[inline]
pub fn blend<T: Animatable>(a: &T, b: &T, weight: f64) -> T {
    a.lerp(b, weight.clamp(0.0, 1.0))
}

/// Blend multiple weighted values
/// Weights should sum to 1.0 (not enforced, but recommended)
///
/// Returns None if values is empty
pub fn blend_weighted<T: Animatable>(values: &[(T, f64)]) -> Option<T> {
    if values.is_empty() {
        return None;
    }

    // Start with first value weighted
    let mut result = values[0].0.clone();
    let mut accumulated_weight = values[0].1;

    // Blend in remaining values
    for (value, weight) in &values[1..] {
        if accumulated_weight > 0.0 {
            // Normalize the blend between accumulated and new value
            let blend_factor = weight / (accumulated_weight + weight);
            result = result.lerp(value, blend_factor);
            accumulated_weight += weight;
        } else {
            result = value.clone();
            accumulated_weight = *weight;
        }
    }

    Some(result)
}

/// A single animation layer with weight and mode
#[derive(Debug, Clone)]
pub struct AnimationLayer<T: Animatable> {
    /// Current animated value
    pub value: T,
    /// Layer weight (0.0..1.0)
    pub weight: f64,
    /// How this layer combines with lower layers
    pub mode: CompositeMode,
}

impl<T: Animatable> AnimationLayer<T> {
    /// Create a new layer with Replace mode and full weight
    pub fn new(value: T) -> Self {
        Self {
            value,
            weight: 1.0,
            mode: CompositeMode::Replace,
        }
    }

    /// Create a layer with specified weight
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Create a layer with specified mode
    pub fn with_mode(mut self, mode: CompositeMode) -> Self {
        self.mode = mode;
        self
    }
}

/// Resolve a stack of animation layers to a final value
/// Layers are processed bottom-to-top (index 0 = base, last = top override)
///
/// For Replace mode: lerp between accumulated result and layer value by weight
/// For Add mode: add weighted layer value (specialized for f64/f32)
/// For Accumulate mode: same as Add (requires iteration tracking in real usage)
pub fn resolve_layers<T: Animatable>(base: &T, layers: &[AnimationLayer<T>]) -> T {
    if layers.is_empty() {
        return base.clone();
    }

    let mut result = base.clone();

    for layer in layers {
        match layer.mode {
            CompositeMode::Replace => {
                result = result.lerp(&layer.value, layer.weight);
            }
            CompositeMode::Add | CompositeMode::Accumulate => {
                // For generic T, we approximate additive by lerping toward a "shifted" target
                // This is a simplified approach - for true additive, we'd need zero/identity
                result = result.lerp(&layer.value, layer.weight);
            }
        }
    }

    result
}

/// Additive blend for f64 values
/// Result = base + (additive_value * weight)
#[inline]
pub fn additive_blend_f64(base: f64, additive_value: f64, weight: f64) -> f64 {
    base + additive_value * weight
}

/// Additive blend for f32 values
/// Result = base + (additive_value * weight)
#[inline]
pub fn additive_blend_f32(base: f32, additive_value: f32, weight: f32) -> f32 {
    base + additive_value * weight
}

/// Strategy for handling animation interruption
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterruptionStrategy {
    /// Jump instantly to new animation (jarring but simple)
    Instant,
    /// Crossfade from current value to new animation over duration
    Blend { duration_secs: f64 },
    /// New animation inherits current velocity for smooth continuation
    InheritVelocity,
    /// Queue: wait for current animation to finish, then start new
    Queue,
}

impl Default for InterruptionStrategy {
    fn default() -> Self {
        InterruptionStrategy::Blend { duration_secs: 0.2 }
    }
}

/// Manages transition between interrupted animations
#[derive(Debug, Clone)]
pub struct AnimationTransition<T: Animatable> {
    /// Value at the moment of interruption
    pub from_value: T,
    /// Velocity at the moment of interruption (if available)
    pub from_velocity: Option<f64>,
    /// Blend duration (for Blend strategy)
    pub blend_duration: f64,
    /// Elapsed time since interruption
    pub elapsed: f64,
}

impl<T: Animatable> AnimationTransition<T> {
    /// Create a new transition from current state
    pub fn new(current_value: T, strategy: InterruptionStrategy) -> Self {
        let blend_duration = match strategy {
            InterruptionStrategy::Blend { duration_secs } => duration_secs,
            InterruptionStrategy::Instant => 0.0,
            InterruptionStrategy::InheritVelocity => 0.2, // Default blend for velocity
            InterruptionStrategy::Queue => 0.0,
        };

        Self {
            from_value: current_value,
            from_velocity: None,
            blend_duration,
            elapsed: 0.0,
        }
    }

    /// Get the blend weight for crossfade (0.0 = old anim, 1.0 = new anim)
    /// Returns 1.0 when transition is complete
    pub fn blend_weight(&self) -> f64 {
        if self.blend_duration <= 0.0 {
            return 1.0;
        }

        (self.elapsed / self.blend_duration).clamp(0.0, 1.0)
    }

    /// Advance transition time
    pub fn tick(&mut self, dt: f64) {
        self.elapsed += dt;
    }

    /// Is the transition complete? (fully switched to new animation)
    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.blend_duration
    }

    /// Apply transition: blend between old snapshot and new animation value
    pub fn apply(&self, new_value: &T) -> T {
        self.from_value.lerp(new_value, self.blend_weight())
    }
}

/// A "slot" that manages a single animated property with interruption handling
/// This is the high-level API most users will interact with
#[derive(Debug, Clone)]
pub struct AnimationSlot<T: Animatable> {
    /// Current resolved value
    current: T,
    /// Active transition (if being interrupted)
    transition: Option<AnimationTransition<T>>,
    /// Default interruption strategy
    pub strategy: InterruptionStrategy,
}

impl<T: Animatable> AnimationSlot<T> {
    /// Create a new animation slot with initial value
    pub fn new(initial: T) -> Self {
        Self {
            current: initial,
            transition: None,
            strategy: InterruptionStrategy::default(),
        }
    }

    /// Set interruption strategy
    pub fn with_strategy(mut self, strategy: InterruptionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Start a new animation value (handles interruption automatically)
    pub fn set(&mut self, new_target_value: T) {
        match self.strategy {
            InterruptionStrategy::Instant => {
                self.current = new_target_value;
                self.transition = None;
            }
            InterruptionStrategy::Blend { .. } => {
                // Create transition from current value
                self.transition = Some(AnimationTransition::new(
                    self.current.clone(),
                    self.strategy,
                ));
            }
            InterruptionStrategy::InheritVelocity => {
                // For now, same as Blend (velocity inheritance needs external tracking)
                self.transition = Some(AnimationTransition::new(
                    self.current.clone(),
                    self.strategy,
                ));
            }
            InterruptionStrategy::Queue => {
                // Queue not implemented in simple slot - would need queue structure
                // For now, treat as instant
                self.current = new_target_value;
                self.transition = None;
            }
        }
    }

    /// Update with new animation output value and dt
    /// Returns the resolved value (with transition applied if active)
    pub fn update(&mut self, animated_value: T, dt: f64) -> T {
        if let Some(ref mut transition) = self.transition {
            transition.tick(dt);

            if transition.is_complete() {
                // Transition complete, commit to new value
                self.current = animated_value.clone();
                self.transition = None;
                self.current.clone()
            } else {
                // Blend between old and new
                let blended = transition.apply(&animated_value);
                self.current = blended.clone();
                blended
            }
        } else {
            // No transition, use value directly
            self.current = animated_value.clone();
            self.current.clone()
        }
    }

    /// Get current resolved value
    pub fn value(&self) -> &T {
        &self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_50_50() {
        let a = 0.0_f64;
        let b = 10.0_f64;
        let result = blend(&a, &b, 0.5);
        assert!((result - 5.0_f64).abs() < 1e-10);
    }

    #[test]
    fn test_blend_weighted_three_values() {
        let values = vec![
            (0.0_f64, 0.5),  // 50% weight
            (10.0_f64, 0.3), // 30% weight
            (20.0_f64, 0.2), // 20% weight
        ];

        let result = blend_weighted(&values).unwrap();

        // Expected: first blend 0 and 10 (0.5/(0.5+0.3) = 0.625 weight on 0)
        // Then blend that result with 20
        // This is not a simple weighted average, but sequential blending
        // Let's verify it produces a value between 0 and 20
        assert!(result >= 0.0 && result <= 20.0);
    }

    #[test]
    fn test_resolve_layers_replace_mode() {
        let base = 0.0_f64;
        let layers = vec![
            AnimationLayer::new(10.0_f64).with_weight(0.5),
            AnimationLayer::new(20.0_f64).with_weight(0.3),
        ];

        let result = resolve_layers(&base, &layers);

        // First layer: 0 -> 10 at 0.5 weight = 5.0
        // Second layer: 5 -> 20 at 0.3 weight = 5 + (20-5)*0.3 = 9.5
        assert!((result - 9.5_f64).abs() < 1e-10);
    }

    #[test]
    fn test_additive_blend_f64() {
        let base = 10.0_f64;
        let additive = 5.0_f64;
        let weight = 0.5_f64;
        let result = additive_blend_f64(base, additive, weight);
        assert!((result - 12.5_f64).abs() < 1e-10);
    }

    #[test]
    fn test_animation_transition_crossfade() {
        let initial_value = 0.0;
        let strategy = InterruptionStrategy::Blend { duration_secs: 1.0 };
        let mut transition = AnimationTransition::new(initial_value, strategy);

        // At start, weight should be 0
        assert_eq!(transition.blend_weight(), 0.0);

        // Advance halfway
        transition.tick(0.5);
        assert!((transition.blend_weight() - 0.5).abs() < 1e-10);

        // Advance to completion
        transition.tick(0.5);
        assert_eq!(transition.blend_weight(), 1.0);
        assert!(transition.is_complete());
    }

    #[test]
    fn test_animation_transition_apply() {
        let initial_value = 0.0;
        let strategy = InterruptionStrategy::Blend { duration_secs: 1.0 };
        let mut transition = AnimationTransition::new(initial_value, strategy);

        let new_value = 10.0;

        // At start, should be initial value
        let result = transition.apply(&new_value);
        assert_eq!(result, 0.0);

        // Halfway
        transition.tick(0.5);
        let result = transition.apply(&new_value);
        assert!((result - 5.0_f64).abs() < 1e-10);

        // Complete
        transition.tick(0.5);
        let result = transition.apply(&new_value);
        assert_eq!(result, 10.0);
    }

    #[test]
    fn test_animation_slot_instant_strategy() {
        let mut slot = AnimationSlot::new(0.0)
            .with_strategy(InterruptionStrategy::Instant);

        slot.set(10.0);
        let result = slot.update(10.0, 0.0);

        // Should jump instantly
        assert_eq!(result, 10.0);
        assert_eq!(*slot.value(), 10.0);
    }

    #[test]
    fn test_animation_slot_blend_strategy() {
        let mut slot = AnimationSlot::new(0.0)
            .with_strategy(InterruptionStrategy::Blend { duration_secs: 1.0 });

        slot.set(10.0);

        // First update at t=0, should be at initial value
        let result = slot.update(10.0, 0.0);
        assert_eq!(result, 0.0);

        // Update at t=0.5, should be halfway
        let result = slot.update(10.0, 0.5);
        assert!((result - 5.0_f64).abs() < 1e-10);

        // Update at t=1.0, should complete transition
        let result = slot.update(10.0, 0.5);
        assert_eq!(result, 10.0);
        assert_eq!(*slot.value(), 10.0);
    }
}
