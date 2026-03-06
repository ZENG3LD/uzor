//! Click spark effect — particle burst on click
//!
//! Algorithm (from React source):
//! 1. On click: spawn N particles at cursor position
//! 2. Each particle has an angle: (2π * i) / N (evenly distributed in circle)
//! 3. Each frame:
//!    - Elapsed time → progress (0..1)
//!    - Apply easing function
//!    - Distance = eased * radius
//!    - Line length = size * (1 - eased)  [shrinks over time]
//!    - Position = origin + distance * (cos(angle), sin(angle))
//! 4. Remove particles when progress >= 1.0

use std::f32::consts::PI;

/// Easing function type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    /// Apply the easing function to a linear progress value (0.0..1.0)
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => t * (2.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// A single spark particle
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    /// Origin X position
    pub origin_x: f32,
    /// Origin Y position
    pub origin_y: f32,
    /// Angle in radians
    pub angle: f32,
    /// Start time in seconds
    pub start_time: f64,
}

impl Particle {
    /// Calculate the particle's current position and line length
    ///
    /// Returns: (x1, y1, x2, y2, opacity) or None if particle is expired
    pub fn calculate(
        &self,
        current_time: f64,
        duration: f64,
        easing: Easing,
        radius: f32,
        size: f32,
    ) -> Option<ParticleRender> {
        let elapsed = current_time - self.start_time;
        if elapsed >= duration {
            return None;
        }

        let progress = (elapsed / duration) as f32;
        let eased = easing.apply(progress);

        let distance = eased * radius;
        let line_length = size * (1.0 - eased);

        let x1 = self.origin_x + distance * self.angle.cos();
        let y1 = self.origin_y + distance * self.angle.sin();
        let x2 = self.origin_x + (distance + line_length) * self.angle.cos();
        let y2 = self.origin_y + (distance + line_length) * self.angle.sin();

        Some(ParticleRender {
            x1,
            y1,
            x2,
            y2,
            opacity: 1.0 - progress,
        })
    }
}

/// Rendering data for a particle (line segment)
#[derive(Debug, Clone, Copy)]
pub struct ParticleRender {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub opacity: f32,
}

/// Click spark effect state
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ClickSparkState {
    /// Active particles
    pub particles: Vec<Particle>,
}


/// Click spark effect configuration
pub struct ClickSpark {
    /// Number of particles per click (default: 8)
    pub count: usize,
    /// Particle travel radius in pixels (default: 15.0)
    pub radius: f32,
    /// Particle line size in pixels (default: 10.0)
    pub size: f32,
    /// Animation duration in seconds (default: 0.4)
    pub duration: f64,
    /// Easing function (default: EaseOut)
    pub easing: Easing,
}

impl Default for ClickSpark {
    fn default() -> Self {
        Self {
            count: 8,
            radius: 15.0,
            size: 10.0,
            duration: 0.4,
            easing: Easing::EaseOut,
        }
    }
}

impl ClickSpark {
    /// Create a new click spark with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the particle count
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Set the particle radius
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Set the particle size
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set the animation duration in seconds
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    /// Set the easing function
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Handle a click event at the given position
    ///
    /// # Arguments
    /// * `state` - Current state (will be mutated)
    /// * `x` - Click X position
    /// * `y` - Click Y position
    /// * `current_time` - Current time in seconds
    pub fn handle_click(&self, state: &mut ClickSparkState, x: f32, y: f32, current_time: f64) {
        let new_particles: Vec<Particle> = (0..self.count)
            .map(|i| {
                let angle = (2.0 * PI * i as f32) / self.count as f32;
                Particle {
                    origin_x: x,
                    origin_y: y,
                    angle,
                    start_time: current_time,
                }
            })
            .collect();

        state.particles.extend(new_particles);
    }

    /// Update the state and get particles to render
    ///
    /// # Arguments
    /// * `state` - Current state (will be mutated)
    /// * `current_time` - Current time in seconds
    ///
    /// # Returns
    /// Vector of particles to render
    pub fn update(&self, state: &mut ClickSparkState, current_time: f64) -> Vec<ParticleRender> {
        let mut rendered = Vec::new();

        // Calculate rendered particles and filter out expired ones
        state.particles.retain(|particle| {
            if let Some(render) =
                particle.calculate(current_time, self.duration, self.easing, self.radius, self.size)
            {
                rendered.push(render);
                true
            } else {
                false
            }
        });

        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        // Linear
        assert_eq!(Easing::Linear.apply(0.5), 0.5);

        // EaseIn (quadratic)
        assert_eq!(Easing::EaseIn.apply(0.5), 0.25);

        // EaseOut
        let ease_out = Easing::EaseOut.apply(0.5);
        assert!((ease_out - 0.75).abs() < 0.001);

        // Boundary checks
        assert_eq!(Easing::Linear.apply(0.0), 0.0);
        assert_eq!(Easing::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_click_spark_creation() {
        let spark = ClickSpark::new().with_count(4);
        let mut state = ClickSparkState::default();

        spark.handle_click(&mut state, 100.0, 100.0, 0.0);

        assert_eq!(state.particles.len(), 4);

        // Check angles are evenly distributed
        let expected_angles = [0.0, PI / 2.0, PI, 3.0 * PI / 2.0];
        for (i, particle) in state.particles.iter().enumerate() {
            assert!((particle.angle - expected_angles[i]).abs() < 0.001);
            assert_eq!(particle.origin_x, 100.0);
            assert_eq!(particle.origin_y, 100.0);
            assert_eq!(particle.start_time, 0.0);
        }
    }

    #[test]
    fn test_particle_calculation() {
        let particle = Particle {
            origin_x: 0.0,
            origin_y: 0.0,
            angle: 0.0, // Right direction
            start_time: 0.0,
        };

        // At 50% progress with ease-out
        let render = particle.calculate(0.2, 0.4, Easing::EaseOut, 100.0, 20.0);
        assert!(render.is_some());

        let render = render.unwrap();
        // Progress = 0.5, eased = 0.75
        // Distance = 0.75 * 100 = 75
        // Line length = 20 * 0.25 = 5
        assert!((render.x1 - 75.0).abs() < 0.1);
        assert_eq!(render.y1, 0.0);
        assert!((render.x2 - 80.0).abs() < 0.1);
        assert_eq!(render.y2, 0.0);
    }

    #[test]
    fn test_particle_expiration() {
        let particle = Particle {
            origin_x: 0.0,
            origin_y: 0.0,
            angle: 0.0,
            start_time: 0.0,
        };

        // After duration, particle should be None
        let render = particle.calculate(0.5, 0.4, Easing::Linear, 100.0, 20.0);
        assert!(render.is_none());
    }

    #[test]
    fn test_update_removes_expired() {
        let spark = ClickSpark::new().with_count(2).with_duration(0.1);
        let mut state = ClickSparkState::default();

        spark.handle_click(&mut state, 0.0, 0.0, 0.0);
        assert_eq!(state.particles.len(), 2);

        // Update at 0.05s - should have 2 particles
        let rendered = spark.update(&mut state, 0.05);
        assert_eq!(rendered.len(), 2);
        assert_eq!(state.particles.len(), 2);

        // Update at 0.2s - all particles expired
        let rendered = spark.update(&mut state, 0.2);
        assert_eq!(rendered.len(), 0);
        assert_eq!(state.particles.len(), 0);
    }
}
