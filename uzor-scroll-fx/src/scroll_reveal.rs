//! Word-by-word scroll reveal animation.
//!
//! Based on ReactBits ScrollReveal component. As the user scrolls, words
//! progressively reveal with:
//! - Opacity: baseOpacity → 1.0
//! - Blur: blurStrength → 0
//! - Container rotation: baseRotation → 0
//!
//! Each word has a stagger offset for cascading effect.

/// Configuration for scroll reveal animation.
#[derive(Debug, Clone)]
pub struct ScrollRevealConfig {
    /// Enable blur effect (default: true)
    pub enable_blur: bool,
    /// Base opacity for unrevealed words (default: 0.1)
    pub base_opacity: f32,
    /// Base rotation in degrees for the container (default: 3.0)
    pub base_rotation: f32,
    /// Blur strength in pixels for unrevealed words (default: 4.0)
    pub blur_strength: f32,
    /// Stagger amount per word (default: 0.05)
    pub stagger: f32,
}

impl Default for ScrollRevealConfig {
    fn default() -> Self {
        Self {
            enable_blur: true,
            base_opacity: 0.1,
            base_rotation: 3.0,
            blur_strength: 4.0,
            stagger: 0.05,
        }
    }
}

/// Visual state for a single word in the reveal animation.
#[derive(Debug, Clone, Copy)]
pub struct WordState {
    /// Word opacity (0.0..1.0)
    pub opacity: f32,
    /// Blur amount in pixels
    pub blur: f32,
}

/// Scroll reveal animation state manager.
pub struct ScrollReveal {
    config: ScrollRevealConfig,
    word_count: usize,
}

impl ScrollReveal {
    /// Create a new scroll reveal animation.
    ///
    /// # Arguments
    /// * `word_count` - Number of words in the text
    /// * `config` - Animation configuration
    pub fn new(word_count: usize, config: ScrollRevealConfig) -> Self {
        Self { config, word_count }
    }

    /// Create with default configuration.
    pub fn with_word_count(word_count: usize) -> Self {
        Self::new(word_count, ScrollRevealConfig::default())
    }

    /// Compute container rotation based on scroll progress.
    ///
    /// # Arguments
    /// * `scroll_progress` - Scroll progress from 0.0 (start) to 1.0 (end)
    ///
    /// # Returns
    /// Rotation in degrees
    pub fn compute_rotation(&self, scroll_progress: f32) -> f32 {
        let progress = scroll_progress.clamp(0.0, 1.0);
        // Linear interpolation: baseRotation → 0
        self.config.base_rotation * (1.0 - progress)
    }

    /// Compute visual state for all words based on scroll progress.
    ///
    /// # Arguments
    /// * `scroll_progress` - Scroll progress from 0.0 (start) to 1.0 (end)
    ///
    /// # Returns
    /// Vector of WordState for each word
    pub fn compute_word_states(&self, scroll_progress: f32) -> Vec<WordState> {
        let progress = scroll_progress.clamp(0.0, 1.0);

        (0..self.word_count)
            .map(|index| {
                // Each word has a staggered reveal based on its index
                let word_progress = self.compute_word_progress(progress, index);

                WordState {
                    opacity: self.interpolate_opacity(word_progress),
                    blur: if self.config.enable_blur {
                        self.interpolate_blur(word_progress)
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }

    /// Compute individual word progress with stagger.
    fn compute_word_progress(&self, scroll_progress: f32, word_index: usize) -> f32 {
        // Stagger offset: each word starts revealing slightly later
        let stagger_offset = word_index as f32 * self.config.stagger;

        // Adjust progress range to account for stagger
        // Total range needs to be (1.0 + total_stagger)
        let total_stagger = (self.word_count - 1) as f32 * self.config.stagger;
        let adjusted_progress = scroll_progress * (1.0 + total_stagger);

        // Word-specific progress
        let word_progress = (adjusted_progress - stagger_offset).clamp(0.0, 1.0);

        word_progress
    }

    /// Interpolate opacity from base to 1.0.
    fn interpolate_opacity(&self, progress: f32) -> f32 {
        self.config.base_opacity + (1.0 - self.config.base_opacity) * progress
    }

    /// Interpolate blur from strength to 0.
    fn interpolate_blur(&self, progress: f32) -> f32 {
        self.config.blur_strength * (1.0 - progress)
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: ScrollRevealConfig) {
        self.config = config;
    }

    /// Get current configuration.
    pub fn config(&self) -> &ScrollRevealConfig {
        &self.config
    }

    /// Update word count.
    pub fn set_word_count(&mut self, count: usize) {
        self.word_count = count;
    }

    /// Get word count.
    pub fn word_count(&self) -> usize {
        self.word_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_interpolation() {
        let reveal = ScrollReveal::with_word_count(5);

        assert_eq!(reveal.compute_rotation(0.0), 3.0);
        assert_eq!(reveal.compute_rotation(1.0), 0.0);
        assert!((reveal.compute_rotation(0.5) - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_word_states_at_start() {
        let reveal = ScrollReveal::with_word_count(3);
        let states = reveal.compute_word_states(0.0);

        assert_eq!(states.len(), 3);

        // First word should be at base state
        assert!((states[0].opacity - 0.1).abs() < 0.001);
        assert!((states[0].blur - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_word_states_at_end() {
        let reveal = ScrollReveal::with_word_count(3);
        let states = reveal.compute_word_states(1.0);

        assert_eq!(states.len(), 3);

        // All words should be fully revealed
        for state in &states {
            assert!((state.opacity - 1.0).abs() < 0.001);
            assert!(state.blur < 0.001);
        }
    }

    #[test]
    fn test_stagger_effect() {
        let reveal = ScrollReveal::with_word_count(3);
        let states = reveal.compute_word_states(0.3);

        // First word should be more revealed than second
        assert!(states[0].opacity > states[1].opacity);
        assert!(states[1].opacity > states[2].opacity);

        assert!(states[0].blur < states[1].blur);
        assert!(states[1].blur < states[2].blur);
    }

    #[test]
    fn test_blur_disabled() {
        let config = ScrollRevealConfig {
            enable_blur: false,
            ..Default::default()
        };
        let reveal = ScrollReveal::new(3, config);
        let states = reveal.compute_word_states(0.0);

        for state in &states {
            assert_eq!(state.blur, 0.0);
        }
    }

    #[test]
    fn test_custom_config() {
        let config = ScrollRevealConfig {
            enable_blur: true,
            base_opacity: 0.3,
            base_rotation: 5.0,
            blur_strength: 10.0,
            stagger: 0.1,
        };
        let reveal = ScrollReveal::new(2, config);

        assert_eq!(reveal.compute_rotation(0.0), 5.0);

        let states = reveal.compute_word_states(0.0);
        assert!((states[0].opacity - 0.3).abs() < 0.001);
        assert!((states[0].blur - 10.0).abs() < 0.001);
    }
}
