//! Parallax character float animation.
//!
//! Based on ReactBits ScrollFloat component. Characters float up from below
//! with per-character stagger, creating a cascading reveal effect.

/// Configuration for scroll float animation.
#[derive(Debug, Clone)]
pub struct ScrollFloatConfig {
    /// Stagger amount per character (default: 0.03)
    pub stagger: f32,
    /// Initial Y position as percentage (default: 120.0 = 120%)
    pub initial_y_percent: f32,
    /// Initial scale Y (default: 2.3)
    pub initial_scale_y: f32,
    /// Initial scale X (default: 0.7)
    pub initial_scale_x: f32,
}

impl Default for ScrollFloatConfig {
    fn default() -> Self {
        Self {
            stagger: 0.03,
            initial_y_percent: 120.0,
            initial_scale_y: 2.3,
            initial_scale_x: 0.7,
        }
    }
}

/// Visual state for a single character in the float animation.
#[derive(Debug, Clone, Copy)]
pub struct CharState {
    /// Character opacity (0.0..1.0)
    pub opacity: f32,
    /// Y position offset as percentage
    pub y_percent: f32,
    /// Scale Y factor
    pub scale_y: f32,
    /// Scale X factor
    pub scale_x: f32,
}

/// Scroll float animation state manager.
pub struct ScrollFloat {
    config: ScrollFloatConfig,
    char_count: usize,
}

impl ScrollFloat {
    /// Create a new scroll float animation.
    ///
    /// # Arguments
    /// * `char_count` - Number of characters in the text
    /// * `config` - Animation configuration
    pub fn new(char_count: usize, config: ScrollFloatConfig) -> Self {
        Self { config, char_count }
    }

    /// Create with default configuration.
    pub fn with_char_count(char_count: usize) -> Self {
        Self::new(char_count, ScrollFloatConfig::default())
    }

    /// Compute visual state for all characters based on scroll progress.
    ///
    /// # Arguments
    /// * `scroll_progress` - Scroll progress from 0.0 (start) to 1.0 (end)
    ///
    /// # Returns
    /// Vector of CharState for each character
    pub fn compute_char_states(&self, scroll_progress: f32) -> Vec<CharState> {
        let progress = scroll_progress.clamp(0.0, 1.0);

        (0..self.char_count)
            .map(|index| {
                // Each character has a staggered reveal based on its index
                let char_progress = self.compute_char_progress(progress, index);

                CharState {
                    opacity: char_progress,
                    y_percent: self.interpolate_y_percent(char_progress),
                    scale_y: self.interpolate_scale_y(char_progress),
                    scale_x: self.interpolate_scale_x(char_progress),
                }
            })
            .collect()
    }

    /// Compute individual character progress with stagger.
    fn compute_char_progress(&self, scroll_progress: f32, char_index: usize) -> f32 {
        // Stagger offset: each character starts revealing slightly later
        let stagger_offset = char_index as f32 * self.config.stagger;

        // Adjust progress range to account for stagger
        let total_stagger = (self.char_count - 1) as f32 * self.config.stagger;
        let adjusted_progress = scroll_progress * (1.0 + total_stagger);

        // Character-specific progress
        let char_progress = (adjusted_progress - stagger_offset).clamp(0.0, 1.0);

        // Apply easing (approximate back.inOut(2) from GSAP)
        self.ease_back_in_out(char_progress)
    }

    /// Approximate GSAP's back.inOut(2) easing function.
    fn ease_back_in_out(&self, t: f32) -> f32 {
        let overshoot = 1.70158 * 1.525; // Approximate back(2) overshoot

        let t = t * 2.0;
        if t < 1.0 {
            0.5 * (t * t * ((overshoot + 1.0) * t - overshoot))
        } else {
            let t = t - 2.0;
            0.5 * (t * t * ((overshoot + 1.0) * t + overshoot) + 2.0)
        }
    }

    /// Interpolate Y position from initial to 0.
    fn interpolate_y_percent(&self, progress: f32) -> f32 {
        self.config.initial_y_percent * (1.0 - progress)
    }

    /// Interpolate Y scale from initial to 1.0.
    fn interpolate_scale_y(&self, progress: f32) -> f32 {
        self.config.initial_scale_y + (1.0 - self.config.initial_scale_y) * progress
    }

    /// Interpolate X scale from initial to 1.0.
    fn interpolate_scale_x(&self, progress: f32) -> f32 {
        self.config.initial_scale_x + (1.0 - self.config.initial_scale_x) * progress
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: ScrollFloatConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &ScrollFloatConfig {
        &self.config
    }

    /// Update character count.
    pub fn set_char_count(&mut self, count: usize) {
        self.char_count = count;
    }

    /// Get character count.
    pub fn char_count(&self) -> usize {
        self.char_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_states_at_start() {
        let float = ScrollFloat::with_char_count(5);
        let states = float.compute_char_states(0.0);

        assert_eq!(states.len(), 5);

        // First character should be at initial state
        assert!(states[0].opacity < 0.1); // Easing starts slow
        assert!((states[0].y_percent - 120.0).abs() < 5.0);
        assert!((states[0].scale_y - 2.3).abs() < 0.1);
        assert!((states[0].scale_x - 0.7).abs() < 0.05);
    }

    #[test]
    fn test_char_states_at_end() {
        let float = ScrollFloat::with_char_count(5);
        let states = float.compute_char_states(1.0);

        assert_eq!(states.len(), 5);

        // All characters should be fully revealed
        for state in &states {
            assert!((state.opacity - 1.0).abs() < 0.1);
            assert!(state.y_percent < 1.0);
            assert!((state.scale_y - 1.0).abs() < 0.05);
            assert!((state.scale_x - 1.0).abs() < 0.05);
        }
    }

    #[test]
    fn test_stagger_effect() {
        let float = ScrollFloat::with_char_count(5);
        let states = float.compute_char_states(0.3);

        // Earlier characters should be more revealed than later ones
        assert!(states[0].opacity >= states[1].opacity);
        assert!(states[1].opacity >= states[2].opacity);

        assert!(states[0].y_percent <= states[1].y_percent);
        assert!(states[1].y_percent <= states[2].y_percent);
    }

    #[test]
    fn test_custom_config() {
        let config = ScrollFloatConfig {
            stagger: 0.05,
            initial_y_percent: 150.0,
            initial_scale_y: 3.0,
            initial_scale_x: 0.5,
        };
        let float = ScrollFloat::new(3, config);

        let states = float.compute_char_states(0.0);

        // Should use custom initial values
        assert!((states[0].y_percent - 150.0).abs() < 5.0);
        assert!((states[0].scale_y - 3.0).abs() < 0.2);
        assert!((states[0].scale_x - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_interpolation_midpoint() {
        let float = ScrollFloat::with_char_count(1);
        let states = float.compute_char_states(0.5);

        // At midpoint, values should be between initial and final
        assert!(states[0].opacity > 0.0 && states[0].opacity < 1.0);
        assert!(states[0].y_percent > 0.0 && states[0].y_percent < 120.0);
        assert!(states[0].scale_y > 1.0 && states[0].scale_y < 2.3);
        assert!(states[0].scale_x > 0.7 && states[0].scale_x < 1.0);
    }

    #[test]
    fn test_easing_function() {
        let float = ScrollFloat::with_char_count(1);

        // Easing should start slow, accelerate, then decelerate
        let t0 = float.ease_back_in_out(0.0);
        let t1 = float.ease_back_in_out(0.1);
        let t5 = float.ease_back_in_out(0.5);
        let t10 = float.ease_back_in_out(1.0);

        assert_eq!(t0, 0.0);
        assert_eq!(t10, 1.0);

        // Should have back ease overshoot characteristic
        let early_delta = t1 - t0;
        let mid_delta = t5 - t1;

        assert!(mid_delta > early_delta); // Accelerates
    }

    #[test]
    fn test_update_char_count() {
        let mut float = ScrollFloat::with_char_count(3);
        assert_eq!(float.char_count(), 3);

        float.set_char_count(5);
        assert_eq!(float.char_count(), 5);

        let states = float.compute_char_states(0.5);
        assert_eq!(states.len(), 5);
    }
}
