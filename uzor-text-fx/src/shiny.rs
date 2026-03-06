/// Shiny text gradient sweep effect.
///
/// Animates a metallic shine that sweeps across text using a linear gradient.
/// The gradient is 200% width and animates from 150% to -50% position.
///
/// # Algorithm (from React source)
///
/// - Progress value (p) animates from 0 to 100
/// - Background position: `150 - p * 2` (so p=0 → 150%, p=100 → -50%)
/// - With yoyo: forward (0→100), delay, reverse (100→0), delay, repeat
/// - Without yoyo: forward (0→100), delay, repeat
/// - Direction: left (1) sweeps left→right, right (-1) sweeps right→left

#[derive(Debug, Clone)]
pub struct ShinyTextConfig {
    /// Animation speed in seconds for one full sweep (default: 2.0)
    pub speed: f64,
    /// Direction: true = left to right, false = right to left (default: true)
    pub direction_left: bool,
    /// Yoyo mode: reverse animation after completion (default: false)
    pub yoyo: bool,
    /// Delay after each sweep in seconds (default: 0.0)
    pub delay: f64,
    /// Gradient angle in degrees (default: 120)
    pub spread: f32,
}

impl Default for ShinyTextConfig {
    fn default() -> Self {
        Self {
            speed: 2.0,
            direction_left: true,
            yoyo: false,
            delay: 0.0,
            spread: 120.0,
        }
    }
}

#[derive(Debug)]
pub struct ShinyTextState {
    elapsed: f64,
    direction: f32, // 1.0 or -1.0
}

impl ShinyTextState {
    pub fn new(direction_left: bool) -> Self {
        Self {
            elapsed: 0.0,
            direction: if direction_left { 1.0 } else { -1.0 },
        }
    }

    /// Update animation state with delta time and return current progress (0..100).
    pub fn update(&mut self, delta_time: f64, config: &ShinyTextConfig) -> f32 {
        self.elapsed += delta_time;

        let animation_duration = config.speed;
        let delay_duration = config.delay;

        if config.yoyo {
            // Full cycle: forward + delay + reverse + delay
            let cycle_duration = animation_duration + delay_duration;
            let full_cycle = cycle_duration * 2.0;
            let cycle_time = self.elapsed % full_cycle;

            if cycle_time < animation_duration {
                // Forward animation: 0 -> 100
                let p = (cycle_time / animation_duration) * 100.0;
                if self.direction > 0.0 {
                    p as f32
                } else {
                    100.0 - p as f32
                }
            } else if cycle_time < cycle_duration {
                // Delay at end
                if self.direction > 0.0 {
                    100.0
                } else {
                    0.0
                }
            } else if cycle_time < cycle_duration + animation_duration {
                // Reverse animation: 100 -> 0
                let reverse_time = cycle_time - cycle_duration;
                let p = 100.0 - (reverse_time / animation_duration) * 100.0;
                if self.direction > 0.0 {
                    p as f32
                } else {
                    100.0 - p as f32
                }
            } else {
                // Delay at start
                if self.direction > 0.0 {
                    0.0
                } else {
                    100.0
                }
            }
        } else {
            // Simple loop: forward + delay
            let cycle_duration = animation_duration + delay_duration;
            let cycle_time = self.elapsed % cycle_duration;

            if cycle_time < animation_duration {
                // Animation phase: 0 -> 100
                let p = (cycle_time / animation_duration) * 100.0;
                if self.direction > 0.0 {
                    p as f32
                } else {
                    100.0 - p as f32
                }
            } else {
                // Delay phase - hold at end
                if self.direction > 0.0 {
                    100.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Get background position percentage from progress value.
    ///
    /// Returns position as 0.0..1.0 normalized value (0.0 = -50%, 1.0 = 150%).
    pub fn background_position(progress: f32) -> f32 {
        let pos_percent = 150.0 - progress * 2.0;
        // Normalize to 0..1 range: -50 → 0.0, 150 → 1.0
        (pos_percent + 50.0) / 200.0
    }

    /// Reset animation state, optionally changing direction.
    pub fn reset(&mut self, direction_left: bool) {
        self.elapsed = 0.0;
        self.direction = if direction_left { 1.0 } else { -1.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_forward() {
        let config = ShinyTextConfig {
            speed: 1.0,
            delay: 0.0,
            yoyo: false,
            ..Default::default()
        };
        let mut state = ShinyTextState::new(true);

        // At 0.5s: progress = 50
        let p1 = state.update(0.5, &config);
        assert!((p1 - 50.0).abs() < 1.0);

        // At 0.99s total: progress ~= 99 (avoid modulo wrap at exactly 1.0)
        let p2 = state.update(0.49, &config);
        assert!((p2 - 99.0).abs() < 2.0);
    }

    #[test]
    fn test_background_position() {
        // progress=0 → pos=150% → normalized 1.0
        let pos0 = ShinyTextState::background_position(0.0);
        assert!((pos0 - 1.0).abs() < 0.01);

        // progress=100 → pos=-50% → normalized 0.0
        let pos100 = ShinyTextState::background_position(100.0);
        assert!((pos100 - 0.0).abs() < 0.01);

        // progress=50 → pos=50% → normalized 0.5
        let pos50 = ShinyTextState::background_position(50.0);
        assert!((pos50 - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_yoyo_mode() {
        let config = ShinyTextConfig {
            speed: 1.0,
            delay: 0.0,
            yoyo: true,
            ..Default::default()
        };
        let mut state = ShinyTextState::new(true);

        // Forward phase
        state.update(0.5, &config);
        let p_forward = state.update(0.0, &config);
        assert!((p_forward - 50.0).abs() < 1.0);

        // After 1s: at end
        state.update(0.5, &config);
        let p_end = state.update(0.0, &config);
        assert!((p_end - 100.0).abs() < 1.0);

        // Reverse phase
        state.update(0.5, &config);
        let p_reverse = state.update(0.0, &config);
        assert!((p_reverse - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_direction_right() {
        let config = ShinyTextConfig {
            speed: 1.0,
            delay: 0.0,
            yoyo: false,
            direction_left: false,
            ..Default::default()
        };
        let mut state = ShinyTextState::new(false);

        // direction=-1: progress inverted
        let p0 = state.update(0.0, &config);
        assert_eq!(p0, 100.0);

        state.update(0.5, &config);
        let p1 = state.update(0.0, &config);
        assert!((p1 - 50.0).abs() < 1.0);
    }
}
