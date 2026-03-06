//! Animation layers - priority-based system for combining multiple animations
//!
//! Extends the basic blending system with a managed layer stack that supports:
//! - Named layers for easy reference
//! - Automatic weight transitions (smooth fade in/out)
//! - Priority-based composition (like Unity animation layers)
//! - Enable/disable functionality
//!
//! This builds on top of blend.rs's AnimationLayer and CompositeMode.

use crate::blend::CompositeMode;
use crate::timeline::Animatable;

/// A named, managed animation layer with weight transitions
///
/// Unlike the basic AnimationLayer in blend.rs, this adds:
/// - Name for identification
/// - Target weight with smooth transitions
/// - Active/inactive state
/// - Weight transition speed
#[derive(Debug, Clone)]
pub struct ManagedLayer<T: Animatable> {
    /// Layer name (for identification)
    pub name: String,
    /// Current value
    pub value: T,
    /// Target weight (what we're transitioning to)
    pub target_weight: f64,
    /// Current weight (may be transitioning)
    pub current_weight: f64,
    /// Weight transition speed (weight units per second, e.g. 4.0 = full transition in 0.25s)
    pub weight_speed: f64,
    /// Composite mode
    pub mode: CompositeMode,
    /// Whether this layer is active
    pub active: bool,
}

impl<T: Animatable> ManagedLayer<T> {
    /// Create a new managed layer with default settings
    pub fn new(name: String, initial_value: T, mode: CompositeMode) -> Self {
        Self {
            name,
            value: initial_value,
            target_weight: 0.0,
            current_weight: 0.0,
            weight_speed: 4.0, // Default: full transition in 0.25s
            mode,
            active: true,
        }
    }

    /// Set weight transition speed (weight units per second)
    pub fn set_speed(&mut self, speed: f64) {
        self.weight_speed = speed;
    }

    /// Set initial weights (both current and target)
    pub fn set_weight(&mut self, weight: f64) {
        let clamped = weight.clamp(0.0, 1.0);
        self.target_weight = clamped;
        self.current_weight = clamped;
    }

    /// Is the weight transition complete?
    pub fn is_settled(&self) -> bool {
        (self.current_weight - self.target_weight).abs() < f64::EPSILON
    }
}

/// A stack of managed animation layers with automatic weight transitions
///
/// This is the main API for managing multiple animation layers on the same property.
/// Layers are processed in order (bottom-to-top), with each layer blending according
/// to its mode and weight.
#[derive(Debug, Clone)]
pub struct LayerStack<T: Animatable> {
    layers: Vec<ManagedLayer<T>>,
    base_value: T,
}

impl<T: Animatable> LayerStack<T> {
    /// Create a new layer stack with a base value
    pub fn new(base_value: T) -> Self {
        Self {
            layers: Vec::new(),
            base_value,
        }
    }

    /// Add a named layer
    /// Returns mutable reference for chaining setup
    pub fn add_layer(
        &mut self,
        name: &str,
        initial_value: T,
        mode: CompositeMode,
    ) -> &mut ManagedLayer<T> {
        self.layers.push(ManagedLayer::new(
            name.to_string(),
            initial_value,
            mode,
        ));
        self.layers.last_mut().unwrap()
    }

    /// Get layer by name
    pub fn layer(&self, name: &str) -> Option<&ManagedLayer<T>> {
        self.layers.iter().find(|layer| layer.name == name)
    }

    /// Get mutable layer by name
    pub fn layer_mut(&mut self, name: &str) -> Option<&mut ManagedLayer<T>> {
        self.layers.iter_mut().find(|layer| layer.name == name)
    }

    /// Set layer target weight (will smoothly transition to it)
    pub fn set_weight(&mut self, name: &str, weight: f64) {
        if let Some(layer) = self.layer_mut(name) {
            layer.target_weight = weight.clamp(0.0, 1.0);
        }
    }

    /// Enable layer (sets target_weight to 1.0)
    pub fn enable(&mut self, name: &str) {
        self.set_weight(name, 1.0);
    }

    /// Disable layer (sets target_weight to 0.0)
    pub fn disable(&mut self, name: &str) {
        self.set_weight(name, 0.0);
    }

    /// Update layer value
    pub fn set_value(&mut self, name: &str, value: T) {
        if let Some(layer) = self.layer_mut(name) {
            layer.value = value;
        }
    }

    /// Tick weight transitions (call each frame with dt in seconds)
    pub fn tick(&mut self, dt: f64) {
        for layer in &mut self.layers {
            if (layer.current_weight - layer.target_weight).abs() > f64::EPSILON {
                let direction = if layer.target_weight > layer.current_weight {
                    1.0
                } else {
                    -1.0
                };
                layer.current_weight += direction * layer.weight_speed * dt;

                // Clamp to not overshoot target
                if direction > 0.0 {
                    layer.current_weight = layer.current_weight.min(layer.target_weight);
                } else {
                    layer.current_weight = layer.current_weight.max(layer.target_weight);
                }
            }
        }
    }

    /// Resolve all layers to a single value
    ///
    /// Processes layers bottom-to-top:
    /// - Replace mode: lerp between current result and layer value by weight
    /// - Add/Accumulate mode: lerp (simplified additive for generic types)
    /// - Inactive layers or zero-weight layers are skipped
    pub fn resolve(&self) -> T {
        let mut result = self.base_value.clone();

        for layer in &self.layers {
            if !layer.active || layer.current_weight < f64::EPSILON {
                continue;
            }

            match layer.mode {
                CompositeMode::Replace => {
                    result = result.lerp(&layer.value, layer.current_weight);
                }
                CompositeMode::Add | CompositeMode::Accumulate => {
                    // For additive, lerp toward base+delta
                    // This is a simplified approach for generic Animatable types
                    result = result.lerp(&layer.value, layer.current_weight);
                }
            }
        }

        result
    }

    /// Set base value
    pub fn set_base(&mut self, value: T) {
        self.base_value = value;
    }

    /// Get base value
    pub fn base(&self) -> &T {
        &self.base_value
    }

    /// Remove layer by name
    pub fn remove_layer(&mut self, name: &str) {
        self.layers.retain(|layer| layer.name != name);
    }

    /// Number of layers
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Is the stack empty?
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Get all layer names
    pub fn layer_names(&self) -> Vec<&str> {
        self.layers.iter().map(|layer| layer.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_layer_and_resolve_single_replace() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace).set_weight(1.0);

        let result = stack.resolve();
        assert_eq!(result, 10.0);
    }

    #[test]
    fn test_two_layers_at_half_weight() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace).set_weight(0.5);

        let result = stack.resolve();
        // base=0, layer1=10 at weight 0.5 → 0 + (10-0)*0.5 = 5.0
        assert!((result - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_weight_transition_gradual_change() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace).set_weight(0.0);

        // Set target weight to 1.0
        stack.set_weight("layer1", 1.0);

        // Initially should be at 0.0
        let result = stack.resolve();
        assert_eq!(result, 0.0);

        // Tick forward by 0.125s (with speed=4.0, should move 0.5 weight units)
        stack.tick(0.125);
        let result = stack.resolve();
        assert!((result - 5.0).abs() < 1e-10); // weight should be ~0.5

        // Tick forward by another 0.125s (total 0.25s, weight should be 1.0)
        stack.tick(0.125);
        let result = stack.resolve();
        assert_eq!(result, 10.0);
    }

    #[test]
    fn test_enable_disable_toggles_target_weight() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace).set_weight(0.0);

        stack.enable("layer1");
        assert_eq!(stack.layer("layer1").unwrap().target_weight, 1.0);

        stack.disable("layer1");
        assert_eq!(stack.layer("layer1").unwrap().target_weight, 0.0);
    }

    #[test]
    fn test_inactive_layers_are_skipped() {
        let mut stack = LayerStack::new(0.0_f64);
        let layer = stack.add_layer("layer1", 10.0, CompositeMode::Replace);
        layer.current_weight = 1.0;
        layer.target_weight = 1.0;
        layer.active = false;

        let result = stack.resolve();
        // Layer is inactive, should return base value
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_remove_layer_works() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace);
        stack.add_layer("layer2", 20.0, CompositeMode::Replace);

        assert_eq!(stack.len(), 2);

        stack.remove_layer("layer1");
        assert_eq!(stack.len(), 1);
        assert!(stack.layer("layer1").is_none());
        assert!(stack.layer("layer2").is_some());
    }

    #[test]
    fn test_layer_mut_allows_value_updates() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace).set_weight(1.0);

        stack.set_value("layer1", 20.0);

        let result = stack.resolve();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_empty_stack_returns_base_value() {
        let stack = LayerStack::new(42.0_f64);
        let result = stack.resolve();
        assert_eq!(result, 42.0);
    }

    #[test]
    fn test_zero_weight_layer_skipped() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace)
            .set_weight(0.0);

        let result = stack.resolve();
        // Weight is 0.0, should return base value
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_multiple_layers_accumulate() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("base", 10.0, CompositeMode::Replace)
            .set_weight(1.0);
        stack.add_layer("detail", 5.0, CompositeMode::Replace)
            .set_weight(0.5);

        let result = stack.resolve();
        // base=0 -> 10 at 1.0 = 10
        // 10 -> 5 at 0.5 = 10 + (5-10)*0.5 = 7.5
        assert!((result - 7.5).abs() < 1e-10);
    }

    #[test]
    fn test_is_empty() {
        let mut stack = LayerStack::new(0.0_f64);
        assert!(stack.is_empty());

        stack.add_layer("layer1", 10.0, CompositeMode::Replace);
        assert!(!stack.is_empty());
    }

    #[test]
    fn test_layer_names() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace);
        stack.add_layer("layer2", 20.0, CompositeMode::Replace);

        let names = stack.layer_names();
        assert_eq!(names, vec!["layer1", "layer2"]);
    }

    #[test]
    fn test_set_base() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.set_base(100.0);
        assert_eq!(*stack.base(), 100.0);

        let result = stack.resolve();
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_weight_clamps_at_boundaries() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace)
            .set_weight(0.5);

        // Try to set weight above 1.0
        stack.set_weight("layer1", 1.5);
        assert_eq!(stack.layer("layer1").unwrap().target_weight, 1.0);

        // Try to set weight below 0.0
        stack.set_weight("layer1", -0.5);
        assert_eq!(stack.layer("layer1").unwrap().target_weight, 0.0);
    }

    #[test]
    fn test_weight_transition_downward() {
        let mut stack = LayerStack::new(0.0_f64);
        stack.add_layer("layer1", 10.0, CompositeMode::Replace)
            .set_weight(1.0);

        // Verify initial state
        let result = stack.resolve();
        assert_eq!(result, 10.0);

        // Set target weight to 0.0
        stack.set_weight("layer1", 0.0);

        // Tick forward by 0.125s (with speed=4.0, should move -0.5 weight units)
        stack.tick(0.125);
        let result = stack.resolve();
        assert!((result - 5.0).abs() < 1e-10); // weight should be ~0.5

        // Tick forward by another 0.125s
        stack.tick(0.125);
        let result = stack.resolve();
        assert_eq!(result, 0.0); // weight should be 0.0
    }
}
