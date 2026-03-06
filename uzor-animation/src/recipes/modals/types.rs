//! Modal animation types catalog
//!
//! Defines all modal/dialog/overlay animation variants with their parameters.

use crate::{Easing, Spring, Decay, Timeline};
use std::time::Duration;

/// Catalog of modal animation patterns
#[derive(Debug, Clone)]
pub enum ModalAnimation {
    /// Material Design dialog: fade + scale from 0.95 to 1.0
    FadeScale {
        duration_ms: u64,
        easing: Easing,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Slide up from bottom (bottom sheet)
    SlideUp {
        duration_ms: u64,
        easing: Easing,
        translate_from: f64, // percentage: 100.0 = 100%
        translate_to: f64,
        opacity_from: f64,
        opacity_to: f64,
        delay_ms: u64,
    },

    /// Slide down from top (notification panel)
    SlideDown {
        duration_ms: u64,
        easing: Easing,
        translate_from: f64, // percentage: -100.0 = -100%
        translate_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Slide from right (side drawer)
    SlideRight {
        duration_ms: u64,
        easing: Easing,
        translate_from: f64, // percentage
        translate_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// iOS-style spring scale (alert dialog)
    SpringScale {
        spring: Spring,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Backdrop overlay fade/blur
    Backdrop {
        duration_ms: u64,
        easing: Easing,
        opacity_from: f64,
        opacity_to: f64,
        blur_from: f64, // pixels
        blur_to: f64,
    },

    /// Bottom drawer with snap points and momentum
    DrawerSnap {
        spring: Spring,
        decay: Option<Decay>,
        snap_points: Vec<f64>, // percentage positions
        translate_from: f64,
        translate_to: f64,
        background_scale_from: f64,
        background_scale_to: f64,
    },

    /// Drop in from above with bounce
    DropIn {
        duration_ms: u64,
        easing: Easing,
        translate_from: f64, // percentage: -100.0 = above screen
        translate_to: f64,
        scale_from: f64,
        scale_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Zoom from trigger element position
    ZoomFromOrigin {
        duration_ms: u64,
        easing: Easing,
        origin_x: f64, // percentage or pixel
        origin_y: f64,
        scale_from: f64,
        scale_to: f64,
        translate_x_from: f64,
        translate_x_to: f64,
        translate_y_from: f64,
        translate_y_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },

    /// Clip-path reveal from center (curtain effect)
    Curtain {
        duration_ms: u64,
        easing: Easing,
        clip_from: f64, // 0.0 = fully clipped, 1.0 = fully visible
        clip_to: f64,
        opacity_from: f64,
        opacity_to: f64,
    },
}

impl ModalAnimation {
    /// Get the duration of this animation in milliseconds
    pub fn duration_ms(&self) -> u64 {
        match self {
            ModalAnimation::FadeScale { duration_ms, .. } => *duration_ms,
            ModalAnimation::SlideUp { duration_ms, .. } => *duration_ms,
            ModalAnimation::SlideDown { duration_ms, .. } => *duration_ms,
            ModalAnimation::SlideRight { duration_ms, .. } => *duration_ms,
            ModalAnimation::SpringScale { spring, .. } => {
                (spring.estimated_duration() * 1000.0) as u64
            }
            ModalAnimation::Backdrop { duration_ms, .. } => *duration_ms,
            ModalAnimation::DrawerSnap { spring, .. } => {
                (spring.estimated_duration() * 1000.0) as u64
            }
            ModalAnimation::DropIn { duration_ms, .. } => *duration_ms,
            ModalAnimation::ZoomFromOrigin { duration_ms, .. } => *duration_ms,
            ModalAnimation::Curtain { duration_ms, .. } => *duration_ms,
        }
    }

    /// Get the duration as a std::time::Duration
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms())
    }

    /// Create an enter timeline for this modal animation
    pub fn enter_timeline(&self) -> Timeline {
        Timeline::new()
        // Implementation will be completed by preset functions
    }

    /// Create an exit timeline for this modal animation
    pub fn exit_timeline(&self) -> Timeline {
        Timeline::new()
        // Implementation will be completed by preset functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_scale_duration() {
        let anim = ModalAnimation::FadeScale {
            duration_ms: 225,
            easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
            scale_from: 0.95,
            scale_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        };

        assert_eq!(anim.duration_ms(), 225);
        assert_eq!(anim.duration(), Duration::from_millis(225));
    }

    #[test]
    fn test_spring_based_duration() {
        let spring = Spring::new().stiffness(350.0).damping(28.0);
        let anim = ModalAnimation::SpringScale {
            spring,
            scale_from: 0.0,
            scale_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        };

        let duration = anim.duration_ms();
        assert!(duration > 0);
        assert!(duration < 10000); // Should be reasonable
    }

    #[test]
    fn test_slide_up_with_delay() {
        let anim = ModalAnimation::SlideUp {
            duration_ms: 450,
            easing: Easing::CubicBezier(0.32, 1.0, 0.23, 1.0),
            translate_from: 100.0,
            translate_to: 0.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
            delay_ms: 100,
        };

        assert_eq!(anim.duration_ms(), 450);
    }
}
