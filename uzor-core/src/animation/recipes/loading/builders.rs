//! Builder pattern for loading animations
//!
//! Provides fluent API for customizing loading animations.

use crate::animation::Easing;
use super::types::LoadingAnimation;
use super::defaults::*;

/// Builder for Spinner animations
pub struct SpinnerBuilder {
    duration_ms: u64,
    easing: Easing,
}

impl SpinnerBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: SpinnerDefaults::DURATION_MS,
            easing: SpinnerDefaults::EASING,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::Spinner {
            duration_ms: self.duration_ms,
            easing: self.easing,
        }
    }
}

impl Default for SpinnerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for PulseDots animations
pub struct PulseDotsBuilder {
    duration_ms: u64,
    easing: Easing,
    count: usize,
    stagger_delay_ms: u64,
    scale_from: f64,
    scale_to: f64,
}

impl PulseDotsBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: PulseDotsDefaults::DURATION_MS,
            easing: PulseDotsDefaults::EASING,
            count: PulseDotsDefaults::COUNT,
            stagger_delay_ms: PulseDotsDefaults::STAGGER_DELAY_MS,
            scale_from: PulseDotsDefaults::SCALE_FROM,
            scale_to: PulseDotsDefaults::SCALE_TO,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn scale_range(mut self, from: f64, to: f64) -> Self {
        self.scale_from = from;
        self.scale_to = to;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::PulseDots {
            duration_ms: self.duration_ms,
            easing: self.easing,
            count: self.count,
            stagger_delay_ms: self.stagger_delay_ms,
            scale_from: self.scale_from,
            scale_to: self.scale_to,
        }
    }
}

impl Default for PulseDotsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for BarWave animations
pub struct BarWaveBuilder {
    duration_ms: u64,
    easing: Easing,
    count: usize,
    stagger_delay_ms: u64,
    scale_from: f64,
    scale_to: f64,
}

impl BarWaveBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: BarWaveDefaults::DURATION_MS,
            easing: BarWaveDefaults::EASING,
            count: BarWaveDefaults::COUNT,
            stagger_delay_ms: BarWaveDefaults::STAGGER_DELAY_MS,
            scale_from: BarWaveDefaults::SCALE_FROM,
            scale_to: BarWaveDefaults::SCALE_TO,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn scale_range(mut self, from: f64, to: f64) -> Self {
        self.scale_from = from;
        self.scale_to = to;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::BarWave {
            duration_ms: self.duration_ms,
            easing: self.easing,
            count: self.count,
            stagger_delay_ms: self.stagger_delay_ms,
            scale_from: self.scale_from,
            scale_to: self.scale_to,
        }
    }
}

impl Default for BarWaveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ProgressRing animations
pub struct ProgressRingBuilder {
    duration_ms: u64,
    easing: Easing,
    radius: f64,
    stroke_width: f64,
}

impl ProgressRingBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: ProgressRingDefaults::DURATION_MS,
            easing: ProgressRingDefaults::EASING,
            radius: ProgressRingDefaults::RADIUS,
            stroke_width: ProgressRingDefaults::STROKE_WIDTH,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = radius;
        self
    }

    pub fn stroke_width(mut self, width: f64) -> Self {
        self.stroke_width = width;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::ProgressRing {
            duration_ms: self.duration_ms,
            easing: self.easing,
            radius: self.radius,
            stroke_width: self.stroke_width,
        }
    }
}

impl Default for ProgressRingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ProgressBar animations
pub struct ProgressBarBuilder {
    duration_ms: u64,
    easing: Easing,
}

impl ProgressBarBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: ProgressBarDefaults::DURATION_MS,
            easing: ProgressBarDefaults::EASING,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::ProgressBar {
            duration_ms: self.duration_ms,
            easing: self.easing,
        }
    }
}

impl Default for ProgressBarBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ShimmerSkeleton animations
pub struct ShimmerBuilder {
    duration_ms: u64,
    easing: Easing,
    gradient_position_from: f64,
    gradient_position_to: f64,
}

impl ShimmerBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: ShimmerDefaults::DURATION_MS,
            easing: ShimmerDefaults::EASING,
            gradient_position_from: ShimmerDefaults::POSITION_FROM,
            gradient_position_to: ShimmerDefaults::POSITION_TO,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn gradient_range(mut self, from: f64, to: f64) -> Self {
        self.gradient_position_from = from;
        self.gradient_position_to = to;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::ShimmerSkeleton {
            duration_ms: self.duration_ms,
            easing: self.easing,
            gradient_position_from: self.gradient_position_from,
            gradient_position_to: self.gradient_position_to,
        }
    }
}

impl Default for ShimmerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for PulseRing animations
pub struct PulseRingBuilder {
    duration_ms: u64,
    easing: Easing,
    scale_from: f64,
    scale_to: f64,
    opacity_from: f64,
    opacity_to: f64,
}

impl PulseRingBuilder {
    pub fn new() -> Self {
        Self {
            duration_ms: PulseRingDefaults::DURATION_MS,
            easing: PulseRingDefaults::EASING,
            scale_from: PulseRingDefaults::SCALE_FROM,
            scale_to: PulseRingDefaults::SCALE_TO,
            opacity_from: PulseRingDefaults::OPACITY_FROM,
            opacity_to: PulseRingDefaults::OPACITY_TO,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn scale_range(mut self, from: f64, to: f64) -> Self {
        self.scale_from = from;
        self.scale_to = to;
        self
    }

    pub fn opacity_range(mut self, from: f64, to: f64) -> Self {
        self.opacity_from = from;
        self.opacity_to = to;
        self
    }

    pub fn build(self) -> LoadingAnimation {
        LoadingAnimation::PulseRing {
            duration_ms: self.duration_ms,
            easing: self.easing,
            scale_from: self.scale_from,
            scale_to: self.scale_to,
            opacity_from: self.opacity_from,
            opacity_to: self.opacity_to,
        }
    }
}

impl Default for PulseRingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_builder() {
        let anim = SpinnerBuilder::new()
            .duration_ms(3000)
            .easing(Easing::EaseInOutQuad)
            .build();

        assert_eq!(anim.duration_ms(), 3000);
    }

    #[test]
    fn test_pulse_dots_builder() {
        let anim = PulseDotsBuilder::new()
            .count(5)
            .stagger_delay_ms(100)
            .build();

        assert_eq!(anim.element_count(), 5);
    }

    #[test]
    fn test_bar_wave_builder() {
        let anim = BarWaveBuilder::new()
            .count(7)
            .scale_range(0.2, 1.0)
            .build();

        assert_eq!(anim.element_count(), 7);
    }

    #[test]
    fn test_progress_ring_builder() {
        let anim = ProgressRingBuilder::new()
            .radius(60.0)
            .stroke_width(10.0)
            .build();

        assert_eq!(anim.duration_ms(), ProgressRingDefaults::DURATION_MS);
    }

    #[test]
    fn test_shimmer_builder() {
        let anim = ShimmerBuilder::new()
            .duration_ms(2000)
            .gradient_range(150.0, -150.0)
            .build();

        assert_eq!(anim.duration_ms(), 2000);
    }

    #[test]
    fn test_pulse_ring_builder() {
        let anim = PulseRingBuilder::new()
            .scale_range(0.5, 2.0)
            .opacity_range(0.8, 0.0)
            .build();

        assert!(anim.is_infinite());
    }

    #[test]
    fn test_builder_defaults() {
        let default_spinner = SpinnerBuilder::default().build();
        assert_eq!(default_spinner.duration_ms(), SpinnerDefaults::DURATION_MS);

        let default_dots = PulseDotsBuilder::default().build();
        assert_eq!(default_dots.element_count(), PulseDotsDefaults::COUNT);
    }
}
