//! Loading animation types catalog
//!
//! Defines all loading animation variants with their parameters.

use crate::animation::Easing;
use std::time::Duration;

/// Catalog of loading animation patterns
#[derive(Debug, Clone)]
pub enum LoadingAnimation {
    /// Continuous rotation spinner (single element)
    Spinner {
        duration_ms: u64,
        easing: Easing,
    },

    /// Bouncing/pulsing dots in sequence (scale animation)
    PulseDots {
        duration_ms: u64,
        easing: Easing,
        count: usize,
        stagger_delay_ms: u64,
        scale_from: f64,
        scale_to: f64,
    },

    /// Bars oscillating like equalizer (scaleY animation)
    BarWave {
        duration_ms: u64,
        easing: Easing,
        count: usize,
        stagger_delay_ms: u64,
        scale_from: f64,
        scale_to: f64,
    },

    /// Circular progress ring (determinate)
    ProgressRing {
        duration_ms: u64,
        easing: Easing,
        radius: f64,
        stroke_width: f64,
    },

    /// Linear progress bar (determinate)
    ProgressBar {
        duration_ms: u64,
        easing: Easing,
    },

    /// Sliding bar animation (Material indeterminate)
    IndeterminateBar {
        duration_ms: u64,
        easing: Easing,
        scale_x_from: f64,
        scale_x_mid: f64,
        scale_x_end: f64,
    },

    /// Gradient sweep shimmer effect
    ShimmerSkeleton {
        duration_ms: u64,
        easing: Easing,
        gradient_position_from: f64,
        gradient_position_to: f64,
    },

    /// Dots fading in/out in sequence (opacity animation)
    FadingDots {
        duration_ms: u64,
        easing: Easing,
        count: usize,
        stagger_delay_ms: u64,
        opacity_peak_percent: f64,
    },

    /// Partial arc that rotates (Material circular)
    SpinningArc {
        rotation_duration_ms: u64,
        arc_duration_ms: u64,
        easing: Easing,
        dash_array_min: f64,
        dash_array_max: f64,
        path_length: f64,
    },

    /// Single element bouncing up/down (translateY animation)
    BouncingBall {
        duration_ms: u64,
        easing: Easing,
        bounce_distance: f64,
    },

    /// Multiple ripple rings expanding outward
    RippleRings {
        duration_ms: u64,
        easing: Easing,
        count: usize,
        stagger_delay_ms: u64,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// iOS-style 12-segment spinner (opacity rotation)
    SegmentSpinner {
        duration_ms: u64,
        easing: Easing,
        segment_count: usize,
    },

    /// Wave dots (vertical bounce with opacity)
    WaveDots {
        duration_ms: u64,
        easing: Easing,
        count: usize,
        stagger_delay_ms: u64,
        translate_y: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// SVG path drawing loader
    PathDraw {
        duration_ms: u64,
        easing: Easing,
        path_length: f64,
        hold_duration_ms: u64,
    },

    /// Progress bar with shimmer overlay
    ProgressWithShimmer {
        progress_duration_ms: u64,
        shimmer_duration_ms: u64,
        progress_easing: Easing,
        shimmer_easing: Easing,
    },

    /// Pulse ring (expanding + fading)
    PulseRing {
        duration_ms: u64,
        easing: Easing,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },
}

impl LoadingAnimation {
    /// Get the primary duration of this animation in milliseconds
    pub fn duration_ms(&self) -> u64 {
        match self {
            LoadingAnimation::Spinner { duration_ms, .. } => *duration_ms,
            LoadingAnimation::PulseDots { duration_ms, .. } => *duration_ms,
            LoadingAnimation::BarWave { duration_ms, .. } => *duration_ms,
            LoadingAnimation::ProgressRing { duration_ms, .. } => *duration_ms,
            LoadingAnimation::ProgressBar { duration_ms, .. } => *duration_ms,
            LoadingAnimation::IndeterminateBar { duration_ms, .. } => *duration_ms,
            LoadingAnimation::ShimmerSkeleton { duration_ms, .. } => *duration_ms,
            LoadingAnimation::FadingDots { duration_ms, .. } => *duration_ms,
            LoadingAnimation::SpinningArc {
                rotation_duration_ms,
                ..
            } => *rotation_duration_ms,
            LoadingAnimation::BouncingBall { duration_ms, .. } => *duration_ms,
            LoadingAnimation::RippleRings { duration_ms, .. } => *duration_ms,
            LoadingAnimation::SegmentSpinner { duration_ms, .. } => *duration_ms,
            LoadingAnimation::WaveDots { duration_ms, .. } => *duration_ms,
            LoadingAnimation::PathDraw { duration_ms, .. } => *duration_ms,
            LoadingAnimation::ProgressWithShimmer {
                progress_duration_ms,
                ..
            } => *progress_duration_ms,
            LoadingAnimation::PulseRing { duration_ms, .. } => *duration_ms,
        }
    }

    /// Get the duration as a std::time::Duration
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms())
    }

    /// Is this animation infinite/looping?
    pub fn is_infinite(&self) -> bool {
        match self {
            // Determinate progress indicators are not infinite
            LoadingAnimation::ProgressRing { .. } => false,
            LoadingAnimation::ProgressBar { .. } => false,
            LoadingAnimation::ProgressWithShimmer { .. } => false,
            // All other loading animations loop infinitely
            _ => true,
        }
    }

    /// Get element count (for multi-element animations)
    pub fn element_count(&self) -> usize {
        match self {
            LoadingAnimation::PulseDots { count, .. } => *count,
            LoadingAnimation::BarWave { count, .. } => *count,
            LoadingAnimation::FadingDots { count, .. } => *count,
            LoadingAnimation::RippleRings { count, .. } => *count,
            LoadingAnimation::SegmentSpinner { segment_count, .. } => *segment_count,
            LoadingAnimation::WaveDots { count, .. } => *count,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_duration() {
        let anim = LoadingAnimation::Spinner {
            duration_ms: 2000,
            easing: Easing::Linear,
        };

        assert_eq!(anim.duration_ms(), 2000);
        assert_eq!(anim.duration(), Duration::from_millis(2000));
        assert!(anim.is_infinite());
    }

    #[test]
    fn test_pulse_dots_count() {
        let anim = LoadingAnimation::PulseDots {
            duration_ms: 1200,
            easing: Easing::EaseInOutQuad,
            count: 3,
            stagger_delay_ms: 200,
            scale_from: 0.0,
            scale_to: 1.0,
        };

        assert_eq!(anim.element_count(), 3);
        assert!(anim.is_infinite());
    }

    #[test]
    fn test_progress_bar_not_infinite() {
        let anim = LoadingAnimation::ProgressBar {
            duration_ms: 400,
            easing: Easing::EaseOutQuad,
        };

        assert!(!anim.is_infinite());
    }

    #[test]
    fn test_spinning_arc_duration() {
        let anim = LoadingAnimation::SpinningArc {
            rotation_duration_ms: 2000,
            arc_duration_ms: 1500,
            easing: Easing::Linear,
            dash_array_min: 1.0,
            dash_array_max: 90.0,
            path_length: 150.0,
        };

        assert_eq!(anim.duration_ms(), 2000);
        assert!(anim.is_infinite());
    }
}
