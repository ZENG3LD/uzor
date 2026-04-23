/// Animated gradient text effect.
///
/// Multi-stop gradient that animates by moving background position or rotating hue.
/// Gradient is 300% size and position animates from 0% to 100%.
///
/// # Algorithm (from React source)
///
/// - Progress animates from 0 to 100 over animation_speed seconds
/// - With yoyo: forward (0→100), reverse (100→0), repeat
/// - Without yoyo: continuous forward (0→∞) for seamless looping
/// - Background position moves based on direction:
///   - horizontal: X position varies, Y = 50%
///   - vertical: X = 50%, Y position varies
///   - diagonal: X position varies, Y = 50%
/// - Gradient colors duplicated (first color added at end) for seamless loop

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    Diagonal,
}

#[derive(Debug, Clone)]
pub struct GradientTextConfig {
    /// Colors for the gradient (minimum 2)
    pub colors: Vec<[u8; 3]>, // RGB colors
    /// Animation speed in seconds (default: 8.0)
    pub animation_speed: f64,
    /// Direction of gradient movement (default: Horizontal)
    pub direction: GradientDirection,
    /// Yoyo mode: reverse animation after completion (default: true)
    pub yoyo: bool,
}

impl Default for GradientTextConfig {
    fn default() -> Self {
        Self {
            colors: vec![[82, 39, 255], [255, 159, 252], [177, 158, 239]],
            animation_speed: 8.0,
            direction: GradientDirection::Horizontal,
            yoyo: true,
        }
    }
}

#[derive(Debug)]
pub struct GradientTextState {
    elapsed: f64,
}

impl GradientTextState {
    pub fn new() -> Self {
        Self { elapsed: 0.0 }
    }

    /// Update animation state with delta time and return current progress (0..100).
    pub fn update(&mut self, delta_time: f64, config: &GradientTextConfig) -> f32 {
        self.elapsed += delta_time;

        let animation_duration = config.animation_speed;

        if config.yoyo {
            // Full cycle: forward + reverse
            let full_cycle = animation_duration * 2.0;
            let cycle_time = self.elapsed % full_cycle;

            if cycle_time < animation_duration {
                // Forward: 0 -> 100
                ((cycle_time / animation_duration) * 100.0) as f32
            } else {
                // Reverse: 100 -> 0
                let reverse_time = cycle_time - animation_duration;
                (100.0 - (reverse_time / animation_duration) * 100.0) as f32
            }
        } else {
            // Continuous forward for seamless looping
            ((self.elapsed / animation_duration) * 100.0) as f32
        }
    }

    /// Get background position from progress value.
    ///
    /// Returns (x_percent, y_percent) as 0.0..1.0 values.
    pub fn background_position(progress: f32, direction: GradientDirection) -> (f32, f32) {
        let p = progress / 100.0; // Normalize to 0..1

        match direction {
            GradientDirection::Horizontal => (p, 0.5),
            GradientDirection::Vertical => (0.5, p),
            GradientDirection::Diagonal => (p, 0.5),
        }
    }

    /// Reset animation state.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }
}

impl Default for GradientTextState {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create seamless looping gradient by duplicating first color at end.
pub fn seamless_gradient_colors(colors: &[[u8; 3]]) -> Vec<[u8; 3]> {
    if colors.is_empty() {
        return vec![];
    }

    let mut result = colors.to_vec();
    result.push(colors[0]);
    result
}

/// Get gradient angle for CSS-style gradient direction.
pub fn gradient_angle(direction: GradientDirection) -> f32 {
    match direction {
        GradientDirection::Horizontal => 90.0,  // to right
        GradientDirection::Vertical => 180.0,   // to bottom
        GradientDirection::Diagonal => 135.0,   // to bottom right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_yoyo() {
        let config = GradientTextConfig {
            animation_speed: 1.0,
            yoyo: true,
            ..Default::default()
        };
        let mut state = GradientTextState::new();

        // At 0s: progress = 0
        let p0 = state.update(0.0, &config);
        assert_eq!(p0, 0.0);

        // At 0.5s: progress = 50
        state.update(0.5, &config);
        let p1 = state.update(0.0, &config);
        assert!((p1 - 50.0).abs() < 1.0);

        // At 1.0s: progress = 100
        state.update(0.5, &config);
        let p2 = state.update(0.0, &config);
        assert!((p2 - 100.0).abs() < 1.0);

        // At 1.5s: progress = 50 (reverse)
        state.update(0.5, &config);
        let p3 = state.update(0.0, &config);
        assert!((p3 - 50.0).abs() < 1.0);

        // At 2.0s: progress = 0 (back to start)
        state.update(0.5, &config);
        let p4 = state.update(0.0, &config);
        assert!((p4 - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_progress_continuous() {
        let config = GradientTextConfig {
            animation_speed: 1.0,
            yoyo: false,
            ..Default::default()
        };
        let mut state = GradientTextState::new();

        // At 0.5s: progress = 50
        state.update(0.5, &config);
        let p1 = state.update(0.0, &config);
        assert!((p1 - 50.0).abs() < 1.0);

        // At 1.0s: progress = 100
        state.update(0.5, &config);
        let p2 = state.update(0.0, &config);
        assert!((p2 - 100.0).abs() < 1.0);

        // At 1.5s: progress = 150 (continues)
        state.update(0.5, &config);
        let p3 = state.update(0.0, &config);
        assert!((p3 - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_background_position_horizontal() {
        let (x, y) = GradientTextState::background_position(0.0, GradientDirection::Horizontal);
        assert_eq!((x, y), (0.0, 0.5));

        let (x, y) = GradientTextState::background_position(50.0, GradientDirection::Horizontal);
        assert!((x - 0.5).abs() < 0.01);
        assert_eq!(y, 0.5);

        let (x, y) = GradientTextState::background_position(100.0, GradientDirection::Horizontal);
        assert_eq!((x, y), (1.0, 0.5));
    }

    #[test]
    fn test_background_position_vertical() {
        let (x, y) = GradientTextState::background_position(0.0, GradientDirection::Vertical);
        assert_eq!((x, y), (0.5, 0.0));

        let (x, y) = GradientTextState::background_position(100.0, GradientDirection::Vertical);
        assert_eq!((x, y), (0.5, 1.0));
    }

    #[test]
    fn test_seamless_gradient() {
        let colors = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        let seamless = seamless_gradient_colors(&colors);

        assert_eq!(seamless.len(), 4);
        assert_eq!(seamless[0], [255, 0, 0]);
        assert_eq!(seamless[3], [255, 0, 0]); // First color duplicated
    }

    #[test]
    fn test_gradient_angle() {
        assert_eq!(gradient_angle(GradientDirection::Horizontal), 90.0);
        assert_eq!(gradient_angle(GradientDirection::Vertical), 180.0);
        assert_eq!(gradient_angle(GradientDirection::Diagonal), 135.0);
    }

    #[test]
    fn test_reset() {
        let config = GradientTextConfig::default();
        let mut state = GradientTextState::new();

        state.update(1.0, &config);
        assert!(state.elapsed > 0.0);

        state.reset();
        assert_eq!(state.elapsed, 0.0);
    }
}
