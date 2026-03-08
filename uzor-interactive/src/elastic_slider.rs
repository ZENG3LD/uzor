//! Elastic slider with exponential decay overflow
//!
//! When dragging beyond min/max bounds, the slider applies exponential
//! decay to create a rubber-band effect. On release, it springs back.

#[cfg(feature = "animation")]
use uzor_core::animation::Spring;

/// Elastic slider state
///
/// Computes slider value with elastic overflow when dragging beyond bounds.
#[derive(Debug, Clone)]
pub struct ElasticSlider {
    /// Current slider value (clamped to min..max when not dragging)
    pub value: f32,

    /// Minimum value
    pub min: f32,

    /// Maximum value
    pub max: f32,

    /// Step size for discrete values (0.0 = continuous)
    pub step: f32,

    /// Maximum overflow distance in pixels before full saturation
    pub max_overflow: f32,

    /// Current overflow amount (positive = right overflow, negative = left)
    overflow: f32,

    /// Spring state for snap-back animation (displacement from 0)
    #[cfg(feature = "animation")]
    spring_state: Option<SpringState>,

    /// Time when spring started (for animation)
    #[cfg(feature = "animation")]
    spring_start_time: f64,
}

#[cfg(feature = "animation")]
#[derive(Debug, Clone)]
struct SpringState {
    spring: Spring,
}

impl Default for ElasticSlider {
    fn default() -> Self {
        Self::new(0.0, 100.0)
    }
}

impl ElasticSlider {
    /// Create a new elastic slider with given min and max values
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            value: (min + max) / 2.0,
            min,
            max,
            step: 0.0,
            max_overflow: 50.0,
            overflow: 0.0,
            #[cfg(feature = "animation")]
            spring_state: None,
            #[cfg(feature = "animation")]
            spring_start_time: 0.0,
        }
    }

    /// Set step size for discrete values
    pub fn with_step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Set maximum overflow distance
    pub fn with_max_overflow(mut self, max_overflow: f32) -> Self {
        self.max_overflow = max_overflow;
        self
    }

    /// Update slider value from pointer position
    ///
    /// # Arguments
    /// * `pointer_x` - Pointer X position relative to slider left edge
    /// * `slider_width` - Total width of slider in pixels
    pub fn update_from_pointer(&mut self, pointer_x: f32, slider_width: f32) {
        let range = self.max - self.min;

        // Calculate raw value from pointer position
        let mut new_value = self.min + (pointer_x / slider_width) * range;

        // Apply step quantization if enabled
        if self.step > 0.0 {
            new_value = (new_value / self.step).round() * self.step;
        }

        // Calculate overflow if beyond bounds
        let overflow_distance = if pointer_x < 0.0 {
            pointer_x
        } else if pointer_x > slider_width {
            pointer_x - slider_width
        } else {
            0.0
        };

        // Apply exponential decay to overflow
        self.overflow = Self::decay(overflow_distance, self.max_overflow);

        // Clamp value to bounds
        self.value = new_value.clamp(self.min, self.max);
    }

    /// Release pointer - start spring snap-back animation
    #[cfg(feature = "animation")]
    pub fn release(&mut self, current_time: f64) {
        if self.overflow.abs() > 0.01 {
            // Start spring animation from current overflow to 0
            self.spring_state = Some(SpringState {
                spring: Spring::new()
                    .stiffness(180.0)
                    .damping(12.0)
                    .mass(1.0),
            });
            self.spring_start_time = current_time;
        }
    }

    /// Release pointer (no-op without animation feature)
    #[cfg(not(feature = "animation"))]
    pub fn release(&mut self, _current_time: f64) {
        self.overflow = 0.0;
    }

    /// Update spring animation
    #[cfg(feature = "animation")]
    pub fn update(&mut self, current_time: f64) {
        if let Some(ref state) = self.spring_state {
            let elapsed = current_time - self.spring_start_time;
            let initial_overflow = self.overflow;

            let (displacement, _velocity) = state.spring.evaluate(elapsed);

            // Spring goes from 1.0 -> 0.0, scale by initial overflow
            self.overflow = initial_overflow * displacement as f32;

            // Stop animation when at rest
            if state.spring.is_at_rest(elapsed) {
                self.overflow = 0.0;
                self.spring_state = None;
            }
        }
    }

    /// Update spring animation (no-op without animation feature)
    #[cfg(not(feature = "animation"))]
    pub fn update(&mut self, _current_time: f64) {}

    /// Get current overflow amount
    pub fn overflow(&self) -> f32 {
        self.overflow
    }

    /// Get overflow region
    pub fn overflow_region(&self) -> OverflowRegion {
        if self.overflow < -0.01 {
            OverflowRegion::Left
        } else if self.overflow > 0.01 {
            OverflowRegion::Right
        } else {
            OverflowRegion::None
        }
    }

    /// Calculate fill percentage (0.0 to 1.0)
    pub fn fill_percentage(&self) -> f32 {
        let range = self.max - self.min;
        if range == 0.0 {
            0.0
        } else {
            ((self.value - self.min) / range).clamp(0.0, 1.0)
        }
    }

    /// Exponential decay function for overflow
    ///
    /// Uses sigmoid-like curve: 2 * (1 / (1 + e^(-x)) - 0.5)
    /// This creates elastic resistance that increases with distance.
    fn decay(value: f32, max: f32) -> f32 {
        if max == 0.0 {
            return 0.0;
        }

        let entry = value / max;
        let sigmoid = 2.0 * (1.0 / (1.0 + (-entry).exp()) - 0.5);
        sigmoid * max
    }
}

/// Overflow region indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowRegion {
    Left,
    None,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slider() {
        let mut slider = ElasticSlider::new(0.0, 100.0);

        // Middle of slider (50px in 100px width)
        slider.update_from_pointer(50.0, 100.0);
        assert!((slider.value - 50.0).abs() < 0.1);
        assert_eq!(slider.overflow_region(), OverflowRegion::None);
    }

    #[test]
    fn test_stepped_slider() {
        let mut slider = ElasticSlider::new(0.0, 100.0).with_step(10.0);

        // 52px should snap to 50
        slider.update_from_pointer(52.0, 100.0);
        assert!((slider.value - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_overflow_left() {
        let mut slider = ElasticSlider::new(0.0, 100.0);

        // Drag -20px beyond left edge
        slider.update_from_pointer(-20.0, 100.0);
        assert_eq!(slider.overflow_region(), OverflowRegion::Left);
        assert!(slider.overflow() < 0.0);
        assert_eq!(slider.value, 0.0); // Value stays clamped
    }

    #[test]
    fn test_overflow_right() {
        let mut slider = ElasticSlider::new(0.0, 100.0);

        // Drag 20px beyond right edge
        slider.update_from_pointer(120.0, 100.0);
        assert_eq!(slider.overflow_region(), OverflowRegion::Right);
        assert!(slider.overflow() > 0.0);
        assert_eq!(slider.value, 100.0); // Value stays clamped
    }

    #[test]
    fn test_decay_function() {
        // At zero, no decay
        assert_eq!(ElasticSlider::decay(0.0, 50.0), 0.0);

        // At max, sigmoid produces ~46% of max (due to exponential curve)
        let result = ElasticSlider::decay(50.0, 50.0);
        assert!(result > 20.0 && result < 30.0);

        // Small overflow should be proportionally small
        let small = ElasticSlider::decay(5.0, 50.0);
        let large = ElasticSlider::decay(25.0, 50.0);
        assert!(small < large);
    }

    #[test]
    fn test_fill_percentage() {
        let mut slider = ElasticSlider::new(0.0, 100.0);

        slider.update_from_pointer(0.0, 100.0);
        assert!((slider.fill_percentage() - 0.0).abs() < 0.01);

        slider.update_from_pointer(50.0, 100.0);
        assert!((slider.fill_percentage() - 0.5).abs() < 0.01);

        slider.update_from_pointer(100.0, 100.0);
        assert!((slider.fill_percentage() - 1.0).abs() < 0.01);
    }

    #[cfg(feature = "animation")]
    #[test]
    fn test_spring_release() {
        let mut slider = ElasticSlider::new(0.0, 100.0);

        // Create overflow
        slider.update_from_pointer(-20.0, 100.0);
        let initial_overflow = slider.overflow();
        assert!(initial_overflow < 0.0);

        // Release at t=0
        slider.release(0.0);

        // Update at t=0.1s - should be moving toward 0
        slider.update(0.1);
        assert!(slider.overflow().abs() < initial_overflow.abs());

        // Update at t=2.0s - should be at rest
        slider.update(2.0);
        assert!(slider.overflow().abs() < 0.01);
    }
}
