//! Default parameter values for toast animations
//!
//! These defaults are derived from Material Design 3, iOS HIG, Sonner,
//! React ecosystem libraries, and web animation best practices.

use super::types::{StackDirection, ToastDirection};
use crate::{Easing, Spring};

/// Default parameters for SlideFade animations
#[derive(Debug, Clone)]
pub struct SlideFadeDefaults {
    pub enter_duration_ms: u64,
    pub exit_duration_ms: u64,
    pub hold_duration_ms: u64,
    pub enter_easing: Easing,
    pub exit_easing: Easing,
    pub direction: ToastDirection,
    pub offset: f64,
}

impl Default for SlideFadeDefaults {
    fn default() -> Self {
        Self {
            enter_duration_ms: 300,
            exit_duration_ms: 200,
            hold_duration_ms: 3000,
            enter_easing: Easing::EASE_OUT,
            exit_easing: Easing::EASE_IN,
            direction: ToastDirection::Bottom,
            offset: 50.0,
        }
    }
}

/// Default parameters for SpringBounce animations
#[derive(Debug, Clone)]
pub struct SpringBounceDefaults {
    pub enter_spring: Spring,
    pub exit_duration_ms: u64,
    pub exit_easing: Easing,
    pub hold_duration_ms: u64,
    pub direction: ToastDirection,
}

impl Default for SpringBounceDefaults {
    fn default() -> Self {
        Self {
            enter_spring: Spring::new().stiffness(260.0).damping(20.0), // Framer Motion snappy
            exit_duration_ms: 300,
            exit_easing: Easing::EASE_OUT,
            hold_duration_ms: 3000,
            direction: ToastDirection::Bottom,
        }
    }
}

/// Default parameters for StackPush animations
#[derive(Debug, Clone)]
pub struct StackPushDefaults {
    pub push_duration_ms: u64,
    pub push_easing: Easing,
    pub gap: f64,
    pub scale_factor: f64,
    pub direction: StackDirection,
}

impl Default for StackPushDefaults {
    fn default() -> Self {
        Self {
            push_duration_ms: 400,
            push_easing: Easing::EASE,
            gap: 14.0,           // Sonner default
            scale_factor: 0.05,  // 5% scale reduction per position
            direction: StackDirection::Up,
        }
    }
}

/// Default parameters for SwipeDismiss animations
#[derive(Debug, Clone)]
pub struct SwipeDismissDefaults {
    pub velocity_threshold: f64,
    pub distance_threshold: f64,
    pub friction: f64,
    pub spring_back_spring: Spring,
}

impl Default for SwipeDismissDefaults {
    fn default() -> Self {
        Self {
            velocity_threshold: 0.11, // Sonner threshold (units/ms)
            distance_threshold: 100.0, // pixels
            friction: 0.998,           // iOS-like decay
            spring_back_spring: Spring::new().stiffness(400.0).damping(25.0),
        }
    }
}

/// Default parameters for ScaleFade animations
#[derive(Debug, Clone)]
pub struct ScaleFadeDefaults {
    pub enter_duration_ms: u64,
    pub exit_duration_ms: u64,
    pub hold_duration_ms: u64,
    pub enter_easing: Easing,
    pub exit_easing: Easing,
    pub scale_from: f64,
    pub scale_to: f64,
}

impl Default for ScaleFadeDefaults {
    fn default() -> Self {
        Self {
            enter_duration_ms: 200,
            exit_duration_ms: 150,
            hold_duration_ms: 3000,
            enter_easing: Easing::EASE_OUT,
            exit_easing: Easing::EASE_IN,
            scale_from: 0.8,
            scale_to: 1.0,
        }
    }
}

/// Default parameters for DropIn animations
#[derive(Debug, Clone)]
pub struct DropInDefaults {
    pub spring: Spring,
    pub exit_duration_ms: u64,
    pub exit_easing: Easing,
    pub hold_duration_ms: u64,
    pub drop_height: f64,
}

impl Default for DropInDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(300.0).damping(15.0), // Bouncy spring
            exit_duration_ms: 150,
            exit_easing: Easing::CubicBezier(0.5, 0.0, 1.0, 1.0),
            hold_duration_ms: 3000,
            drop_height: 120.0,
        }
    }
}

/// Default parameters for ProgressBar animations
#[derive(Debug, Clone)]
pub struct ProgressBarDefaults {
    pub enter_duration_ms: u64,
    pub enter_easing: Easing,
    pub exit_duration_ms: u64,
    pub exit_easing: Easing,
    pub hold_duration_ms: u64,
    pub progress_easing: Easing,
    pub direction: ToastDirection,
}

impl Default for ProgressBarDefaults {
    fn default() -> Self {
        Self {
            enter_duration_ms: 300,
            enter_easing: Easing::CubicBezier(0.68, -0.55, 0.265, 1.35), // Overshoot
            exit_duration_ms: 300,
            exit_easing: Easing::EASE_OUT,
            hold_duration_ms: 5000,
            progress_easing: Easing::Linear, // Countdown must be linear
            direction: ToastDirection::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_creation() {
        let _ = SlideFadeDefaults::default();
        let _ = SpringBounceDefaults::default();
        let _ = StackPushDefaults::default();
        let _ = SwipeDismissDefaults::default();
        let _ = ScaleFadeDefaults::default();
        let _ = DropInDefaults::default();
        let _ = ProgressBarDefaults::default();
    }

    #[test]
    fn test_slide_fade_defaults_values() {
        let defaults = SlideFadeDefaults::default();
        assert_eq!(defaults.enter_duration_ms, 300);
        assert_eq!(defaults.exit_duration_ms, 200);
        assert_eq!(defaults.hold_duration_ms, 3000);
        assert_eq!(defaults.offset, 50.0);
    }

    #[test]
    fn test_stack_push_defaults_values() {
        let defaults = StackPushDefaults::default();
        assert_eq!(defaults.push_duration_ms, 400);
        assert_eq!(defaults.gap, 14.0);
        assert_eq!(defaults.scale_factor, 0.05);
    }

    #[test]
    fn test_swipe_dismiss_defaults_values() {
        let defaults = SwipeDismissDefaults::default();
        assert_eq!(defaults.velocity_threshold, 0.11);
        assert_eq!(defaults.distance_threshold, 100.0);
        assert_eq!(defaults.friction, 0.998);
    }

    #[test]
    fn test_scale_fade_defaults_values() {
        let defaults = ScaleFadeDefaults::default();
        assert_eq!(defaults.scale_from, 0.8);
        assert_eq!(defaults.scale_to, 1.0);
    }

    #[test]
    fn test_progress_bar_defaults_values() {
        let defaults = ProgressBarDefaults::default();
        assert_eq!(defaults.hold_duration_ms, 5000);
        // Progress easing must be linear for accurate countdown
        matches!(defaults.progress_easing, Easing::Linear);
    }
}
