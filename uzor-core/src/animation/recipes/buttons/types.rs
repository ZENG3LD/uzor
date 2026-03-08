//! Button animation types catalog
//!
//! Defines all button animation variants with their parameters.

use crate::animation::{Easing, Spring};
use std::time::Duration;

/// Catalog of button animation patterns
#[derive(Debug, Clone)]
pub enum ButtonAnimation {
    /// Hover state transition (opacity, background color)
    Hover {
        duration_ms: u64,
        easing: Easing,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Press/tap feedback (scale down)
    Press {
        duration_ms: u64,
        easing: Easing,
        scale: f64,
    },

    /// Spring-back after press
    Release {
        spring: Spring,
    },

    /// Material Design ripple effect (expanding circle with fade)
    Ripple {
        duration_ms: u64,
        easing: Easing,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Toggle on/off state transition
    Toggle {
        duration_ms: u64,
        easing: Easing,
    },

    /// Bouncy scale on hover (spring-based)
    ElasticScale {
        spring: Spring,
        target_scale: f64,
    },

    /// Subtle glow pulsing on hover
    GlowPulse {
        duration_ms: u64,
        easing: Easing,
        intensity_from: f64,
        intensity_to: f64,
    },

    /// Underline sliding in from left
    UnderlineSlide {
        duration_ms: u64,
        easing: Easing,
        origin: SlideOrigin,
    },

    /// Background fills left-to-right
    FillSweep {
        duration_ms: u64,
        easing: Easing,
        direction: SweepDirection,
    },

    /// Border draws around button using stroke animation
    BorderDraw {
        duration_ms: u64,
        easing: Easing,
        stagger_delay_ms: u64,
    },

    /// Button subtly follows cursor (displacement)
    MagneticPull {
        spring: Spring,
        max_distance: f64,
        strength: f64,
    },

    /// Shadow grows + slight translateY on hover
    LiftShadow {
        duration_ms: u64,
        easing: Easing,
        shadow_y_from: f64,
        shadow_y_to: f64,
        shadow_blur_from: f64,
        shadow_blur_to: f64,
        lift_distance: f64,
    },
}

/// Origin point for slide animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideOrigin {
    Left,
    Right,
    Center,
}

/// Direction for sweep animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

impl ButtonAnimation {
    /// Get the duration of this animation in milliseconds
    pub fn duration_ms(&self) -> u64 {
        match self {
            ButtonAnimation::Hover { duration_ms, .. } => *duration_ms,
            ButtonAnimation::Press { duration_ms, .. } => *duration_ms,
            ButtonAnimation::Release { spring } => (spring.estimated_duration() * 1000.0) as u64,
            ButtonAnimation::Ripple { duration_ms, .. } => *duration_ms,
            ButtonAnimation::Toggle { duration_ms, .. } => *duration_ms,
            ButtonAnimation::ElasticScale { spring, .. } => {
                (spring.estimated_duration() * 1000.0) as u64
            }
            ButtonAnimation::GlowPulse { duration_ms, .. } => *duration_ms,
            ButtonAnimation::UnderlineSlide { duration_ms, .. } => *duration_ms,
            ButtonAnimation::FillSweep { duration_ms, .. } => *duration_ms,
            ButtonAnimation::BorderDraw { duration_ms, .. } => *duration_ms,
            ButtonAnimation::MagneticPull { spring, .. } => {
                (spring.estimated_duration() * 1000.0) as u64
            }
            ButtonAnimation::LiftShadow { duration_ms, .. } => *duration_ms,
        }
    }

    /// Get the duration as a std::time::Duration
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_duration() {
        let anim = ButtonAnimation::Hover {
            duration_ms: 200,
            easing: Easing::EaseOutCubic,
            opacity_from: 0.0,
            opacity_to: 0.08,
        };

        assert_eq!(anim.duration_ms(), 200);
        assert_eq!(anim.duration(), Duration::from_millis(200));
    }

    #[test]
    fn test_spring_based_duration() {
        let spring = Spring::new().stiffness(180.0).damping(12.0);
        let anim = ButtonAnimation::Release { spring };

        let duration = anim.duration_ms();
        assert!(duration > 0);
        assert!(duration < 10000); // Should be reasonable
    }
}
