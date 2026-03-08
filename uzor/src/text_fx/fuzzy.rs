use rand::Rng;

/// Fuzzy text scanline distortion effect.
///
/// Per-row horizontal/vertical displacement creating a scanline-style glitch.
/// Intensity varies between base and hover states, with optional glitch spikes.
///
/// # Algorithm (from React source)
///
/// - For each row of text (pixel row):
///   - Calculate displacement: `intensity * (random - 0.5) * fuzz_range`
///   - Horizontal: dx displacement
///   - Vertical: dy displacement (scaled by 0.5)
/// - Intensity transitions:
///   - base_intensity: default state
///   - hover_intensity: on hover/interaction
///   - 1.0: during click or glitch spike
/// - Transition over time with configurable duration
/// - Glitch mode: periodic spikes to intensity=1.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyDirection {
    Horizontal,
    Vertical,
    Both,
}

#[derive(Debug, Clone)]
pub struct FuzzyTextConfig {
    /// Base intensity when not interacting (default: 0.18)
    pub base_intensity: f32,
    /// Intensity on hover (default: 0.5)
    pub hover_intensity: f32,
    /// Maximum displacement range in pixels (default: 30)
    pub fuzz_range: f32,
    /// Direction of distortion (default: Horizontal)
    pub direction: FuzzyDirection,
    /// Transition duration in seconds (default: 0.0 = instant)
    pub transition_duration: f64,
    /// Enable glitch mode (default: false)
    pub glitch_mode: bool,
    /// Glitch interval in seconds (default: 2.0)
    pub glitch_interval: f64,
    /// Glitch duration in seconds (default: 0.2)
    pub glitch_duration: f64,
}

impl Default for FuzzyTextConfig {
    fn default() -> Self {
        Self {
            base_intensity: 0.18,
            hover_intensity: 0.5,
            fuzz_range: 30.0,
            direction: FuzzyDirection::Horizontal,
            transition_duration: 0.0,
            glitch_mode: false,
            glitch_interval: 2.0,
            glitch_duration: 0.2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum InteractionState {
    Base,
    Hovering,
    Clicking,
    Glitching,
}

#[derive(Debug)]
pub struct FuzzyTextState {
    current_intensity: f32,
    target_intensity: f32,
    interaction_state: InteractionState,
    glitch_elapsed: f64,
    is_glitch_active: bool,
}

impl FuzzyTextState {
    pub fn new(base_intensity: f32) -> Self {
        Self {
            current_intensity: base_intensity,
            target_intensity: base_intensity,
            interaction_state: InteractionState::Base,
            glitch_elapsed: 0.0,
            is_glitch_active: false,
        }
    }

    /// Update intensity based on delta time and return current intensity.
    pub fn update(&mut self, delta_time: f64, config: &FuzzyTextConfig) -> f32 {
        // Update glitch state if enabled
        if config.glitch_mode {
            self.glitch_elapsed += delta_time;

            let cycle_duration = config.glitch_interval + config.glitch_duration;
            let cycle_time = self.glitch_elapsed % cycle_duration;

            self.is_glitch_active = cycle_time >= config.glitch_interval;
        }

        // Determine target intensity based on state
        self.target_intensity = match self.interaction_state {
            InteractionState::Clicking => 1.0,
            InteractionState::Glitching if self.is_glitch_active => 1.0,
            InteractionState::Hovering => config.hover_intensity,
            _ => config.base_intensity,
        };

        // Transition current intensity toward target
        if config.transition_duration > 0.0 {
            let step = (delta_time / config.transition_duration) as f32;

            if self.current_intensity < self.target_intensity {
                self.current_intensity =
                    (self.current_intensity + step).min(self.target_intensity);
            } else if self.current_intensity > self.target_intensity {
                self.current_intensity =
                    (self.current_intensity - step).max(self.target_intensity);
            }
        } else {
            self.current_intensity = self.target_intensity;
        }

        self.current_intensity
    }

    /// Calculate per-row displacement values for a given number of rows.
    ///
    /// Returns Vec of (dx, dy) displacement values.
    pub fn calculate_displacements(
        &self,
        num_rows: usize,
        config: &FuzzyTextConfig,
    ) -> Vec<(f32, f32)> {
        let mut rng = rand::thread_rng();
        let intensity = self.current_intensity;

        (0..num_rows)
            .map(|_| {
                let dx = if matches!(
                    config.direction,
                    FuzzyDirection::Horizontal | FuzzyDirection::Both
                ) {
                    intensity * (rng.gen::<f32>() - 0.5) * config.fuzz_range
                } else {
                    0.0
                };

                let dy = if matches!(
                    config.direction,
                    FuzzyDirection::Vertical | FuzzyDirection::Both
                ) {
                    intensity * (rng.gen::<f32>() - 0.5) * config.fuzz_range * 0.5
                } else {
                    0.0
                };

                (dx, dy)
            })
            .collect()
    }

    /// Set hover state.
    pub fn set_hovering(&mut self, hovering: bool) {
        if hovering {
            self.interaction_state = InteractionState::Hovering;
        } else {
            self.interaction_state = InteractionState::Base;
        }
    }

    /// Trigger click effect (duration handled externally).
    pub fn set_clicking(&mut self, clicking: bool) {
        if clicking {
            self.interaction_state = InteractionState::Clicking;
        } else {
            self.interaction_state = InteractionState::Base;
        }
    }

    /// Enable glitch mode state.
    pub fn set_glitching(&mut self, enabled: bool) {
        if enabled {
            self.interaction_state = InteractionState::Glitching;
        } else if matches!(self.interaction_state, InteractionState::Glitching) {
            self.interaction_state = InteractionState::Base;
        }
    }

    /// Get current intensity.
    pub fn intensity(&self) -> f32 {
        self.current_intensity
    }

    /// Reset state.
    pub fn reset(&mut self, base_intensity: f32) {
        self.current_intensity = base_intensity;
        self.target_intensity = base_intensity;
        self.interaction_state = InteractionState::Base;
        self.glitch_elapsed = 0.0;
        self.is_glitch_active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_intensity() {
        let config = FuzzyTextConfig::default();
        let mut state = FuzzyTextState::new(config.base_intensity);

        let intensity = state.update(0.0, &config);
        assert!((intensity - config.base_intensity).abs() < 0.01);
    }

    #[test]
    fn test_hover_intensity() {
        let config = FuzzyTextConfig {
            transition_duration: 0.0, // Instant transition
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(config.base_intensity);

        state.set_hovering(true);
        let intensity = state.update(0.0, &config);
        assert!((intensity - config.hover_intensity).abs() < 0.01);

        state.set_hovering(false);
        let intensity = state.update(0.0, &config);
        assert!((intensity - config.base_intensity).abs() < 0.01);
    }

    #[test]
    fn test_click_intensity() {
        let config = FuzzyTextConfig {
            transition_duration: 0.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(config.base_intensity);

        state.set_clicking(true);
        let intensity = state.update(0.0, &config);
        assert!((intensity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_transition_duration() {
        let config = FuzzyTextConfig {
            base_intensity: 0.0,
            hover_intensity: 1.0,
            transition_duration: 1.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(0.0);

        state.set_hovering(true);

        // After 0.5s: should be at ~0.5
        let i1 = state.update(0.5, &config);
        assert!(i1 > 0.4 && i1 < 0.6);

        // After 1.0s total: should be at 1.0
        let i2 = state.update(0.5, &config);
        assert!((i2 - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_displacements_horizontal() {
        let config = FuzzyTextConfig {
            direction: FuzzyDirection::Horizontal,
            fuzz_range: 30.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(1.0); // Max intensity
        state.update(0.0, &config);

        let displacements = state.calculate_displacements(10, &config);
        assert_eq!(displacements.len(), 10);

        // Should have horizontal displacement, no vertical
        for (dx, dy) in displacements {
            assert_ne!(dx, 0.0); // (very unlikely to be exactly 0)
            assert_eq!(dy, 0.0);
        }
    }

    #[test]
    fn test_displacements_vertical() {
        let config = FuzzyTextConfig {
            direction: FuzzyDirection::Vertical,
            fuzz_range: 30.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(1.0);
        state.update(0.0, &config);

        let displacements = state.calculate_displacements(10, &config);

        // Should have vertical displacement, no horizontal
        for (dx, dy) in displacements {
            assert_eq!(dx, 0.0);
            assert_ne!(dy, 0.0); // (very unlikely to be exactly 0)
        }
    }

    #[test]
    fn test_displacements_both() {
        let config = FuzzyTextConfig {
            direction: FuzzyDirection::Both,
            fuzz_range: 30.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(1.0);
        state.update(0.0, &config);

        let displacements = state.calculate_displacements(10, &config);

        // Should have both displacements
        let has_dx = displacements.iter().any(|(dx, _)| *dx != 0.0);
        let has_dy = displacements.iter().any(|(_, dy)| *dy != 0.0);
        assert!(has_dx);
        assert!(has_dy);
    }

    #[test]
    fn test_glitch_mode() {
        let config = FuzzyTextConfig {
            glitch_mode: true,
            glitch_interval: 1.0,
            glitch_duration: 0.5,
            transition_duration: 0.0,
            ..Default::default()
        };
        let mut state = FuzzyTextState::new(config.base_intensity);
        state.set_glitching(true);

        // Before glitch spike
        let i1 = state.update(0.5, &config);
        assert!((i1 - config.base_intensity).abs() < 0.1);

        // During glitch spike (after interval)
        let i2 = state.update(0.6, &config);
        assert!((i2 - 1.0).abs() < 0.1);

        // After glitch spike ends
        let i3 = state.update(0.5, &config);
        assert!((i3 - config.base_intensity).abs() < 0.1);
    }

    #[test]
    fn test_reset() {
        let config = FuzzyTextConfig::default();
        let mut state = FuzzyTextState::new(config.base_intensity);

        state.set_hovering(true);
        state.update(0.5, &config);

        state.reset(config.base_intensity);
        assert!((state.current_intensity - config.base_intensity).abs() < 0.01);
        assert!(matches!(
            state.interaction_state,
            InteractionState::Base
        ));
    }
}
