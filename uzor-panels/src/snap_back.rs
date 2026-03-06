//! Snap-back animation for separator constraint violations
//!
//! When a separator drag violates constraints (e.g., panel would go below minimum size),
//! a snap-back animation smoothly returns the separator to its valid position.
//!
//! # Feature Flags
//!
//! - `animation` (default): Uses analytical spring physics from `uzor-animation`
//! - Without feature: Falls back to Euler integration
//!
//! # Example
//!
//! ```rust,ignore
//! use uzor_panels::SnapBackAnimation;
//!
//! // Create animation for separator 0 with 50px violation offset
//! let mut anim = SnapBackAnimation::new(0, 50.0);
//!
//! loop {
//!     anim.update(dt);
//!     if anim.done {
//!         break;
//!     }
//!     let offset = anim.offset();
//!     // Render separator at offset...
//! }
//! ```

/// Snap-back animation state for separator
///
/// Animates a separator back to its constrained position when a drag
/// would violate minimum size constraints.
#[derive(Clone, Debug)]
pub struct SnapBackAnimation {
    /// Index of separator being animated
    pub separator_idx: usize,
    /// Whether animation is complete
    pub done: bool,

    // Internal state differs based on feature flag
    #[cfg(feature = "animation")]
    spring: SpringState,
    #[cfg(not(feature = "animation"))]
    euler: EulerState,
}

#[cfg(feature = "animation")]
#[derive(Clone, Debug)]
struct SpringState {
    /// Elapsed time since animation start (seconds)
    elapsed: f64,
    /// Initial offset (displacement from target)
    initial_offset: f64,
    /// Spring configuration
    spring: uzor_animation::Spring,
}

#[cfg(not(feature = "animation"))]
#[derive(Clone, Debug)]
struct EulerState {
    /// Current offset (displacement from target)
    offset: f32,
    /// Current velocity
    velocity: f32,
    /// Spring stiffness (K)
    stiffness: f32,
    /// Damping coefficient (C)
    damping: f32,
}

impl SnapBackAnimation {
    /// Create new snap-back animation
    ///
    /// # Arguments
    /// - `separator_idx`: Index of separator in separator list
    /// - `initial_offset`: Initial displacement from valid position (pixels)
    ///
    /// # Returns
    /// A new animation that will smoothly return to offset=0
    pub fn new(separator_idx: usize, initial_offset: f32) -> Self {
        // If initial offset is near-zero, mark as done immediately
        let done = initial_offset.abs() < 0.5;

        #[cfg(feature = "animation")]
        {
            // Use analytical spring solution
            let spring = uzor_animation::Spring::new()
                .stiffness(300.0)
                .damping(20.0)
                .mass(1.0)
                .initial_velocity(0.0);

            Self {
                separator_idx,
                done,
                spring: SpringState {
                    elapsed: 0.0,
                    initial_offset: initial_offset as f64,
                    spring,
                },
            }
        }

        #[cfg(not(feature = "animation"))]
        {
            // Fallback to Euler integration
            Self {
                separator_idx,
                done,
                euler: EulerState {
                    offset: initial_offset,
                    velocity: 0.0,
                    stiffness: 300.0,
                    damping: 20.0,
                },
            }
        }
    }

    /// Update animation by delta time
    ///
    /// # Arguments
    /// - `dt`: Delta time in seconds (typically 1/60 for 60 FPS)
    pub fn update(&mut self, dt: f32) {
        if self.done {
            return;
        }

        #[cfg(feature = "animation")]
        {
            // Analytical solution: evaluate spring at current time
            self.spring.elapsed += dt as f64;
            let (position, velocity) = self.spring.spring.evaluate(self.spring.elapsed);

            // Position is normalized (1.0 at start, 0.0 at rest)
            // Scale by initial_offset to get pixel displacement
            let pixel_offset = position * self.spring.initial_offset;

            // Check for rest: both position and velocity near zero
            if pixel_offset.abs() < 0.5 && velocity.abs() < 0.5 {
                self.done = true;
            }
        }

        #[cfg(not(feature = "animation"))]
        {
            // Euler integration fallback
            // F = -kx - cv (spring force + damping force)
            let spring_force = -self.euler.stiffness * self.euler.offset;
            let damping_force = -self.euler.damping * self.euler.velocity;
            let acceleration = spring_force + damping_force;

            self.euler.velocity += acceleration * dt;
            self.euler.offset += self.euler.velocity * dt;

            // Check for rest
            if self.euler.offset.abs() < 0.5 && self.euler.velocity.abs() < 0.5 {
                self.euler.offset = 0.0;
                self.euler.velocity = 0.0;
                self.done = true;
            }
        }
    }

    /// Get current offset (displacement from target position)
    ///
    /// # Returns
    /// Current pixel offset. When animation is done, this is 0.0.
    pub fn offset(&self) -> f32 {
        if self.done {
            return 0.0;
        }

        #[cfg(feature = "animation")]
        {
            // Evaluate spring at current elapsed time
            let (position, _velocity) = self.spring.spring.evaluate(self.spring.elapsed);
            (position * self.spring.initial_offset) as f32
        }

        #[cfg(not(feature = "animation"))]
        {
            self.euler.offset
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_back_creation() {
        let anim = SnapBackAnimation::new(0, 50.0);
        assert_eq!(anim.separator_idx, 0);
        assert!(!anim.done);

        // Initial offset should be ~50 (spring starts at position=1.0, scaled by 50)
        let offset = anim.offset();
        assert!((offset - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_snap_back_convergence() {
        let mut anim = SnapBackAnimation::new(0, 100.0);

        // Simulate 2 seconds at 60 FPS
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            anim.update(dt);
            if anim.done {
                break;
            }
        }

        // Should have completed by 2 seconds with stiffness=300, damping=20
        assert!(anim.done);
        assert_eq!(anim.offset(), 0.0);
    }

    #[test]
    fn test_snap_back_decreases() {
        let mut anim = SnapBackAnimation::new(0, 100.0);

        let initial_offset = anim.offset().abs();

        // Update several times
        let dt = 1.0 / 60.0;
        for _ in 0..10 {
            anim.update(dt);
        }

        let current_offset = anim.offset().abs();

        // Offset should have decreased (spring pulling back)
        assert!(current_offset < initial_offset);
    }

    #[test]
    fn test_snap_back_zero_offset() {
        let mut anim = SnapBackAnimation::new(0, 0.0);

        // With zero initial offset, should complete immediately
        anim.update(1.0 / 60.0);

        // Might take a frame or two to detect rest state
        for _ in 0..5 {
            if anim.done {
                break;
            }
            anim.update(1.0 / 60.0);
        }

        assert!(anim.done);
        assert_eq!(anim.offset(), 0.0);
    }

    #[test]
    fn test_snap_back_done_stays_zero() {
        let mut anim = SnapBackAnimation::new(0, 50.0);

        // Fast-forward to completion
        let dt = 1.0 / 60.0;
        for _ in 0..200 {
            anim.update(dt);
            if anim.done {
                break;
            }
        }

        assert!(anim.done);

        // Further updates should keep offset at 0
        for _ in 0..10 {
            anim.update(dt);
            assert_eq!(anim.offset(), 0.0);
        }
    }
}
