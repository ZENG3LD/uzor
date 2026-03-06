//! Default parameters for transition animations
//!
//! Defines standard parameter sets for each transition variant.

use super::types::SlideDirection;
use crate::Easing;

/// Default parameters for Shared Axis transitions
#[derive(Debug, Clone)]
pub struct SharedAxisDefaults {
    pub enter_duration_ms: u64,
    pub exit_duration_ms: u64,
    pub overlap_ms: u64,
    pub easing: Easing,
    pub distance: f64,
}

impl Default for SharedAxisDefaults {
    fn default() -> Self {
        Self {
            enter_duration_ms: 200,
            exit_duration_ms: 300,
            overlap_ms: 200,
            easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            distance: 45.0,
        }
    }
}

/// Default parameters for Fade Through transition
#[derive(Debug, Clone)]
pub struct FadeThroughDefaults {
    pub exit_duration_ms: u64,
    pub enter_duration_ms: u64,
    pub exit_easing: Easing,
    pub enter_easing: Easing,
    pub enter_scale_from: f64,
}

impl Default for FadeThroughDefaults {
    fn default() -> Self {
        Self {
            exit_duration_ms: 100,
            enter_duration_ms: 200,
            exit_easing: Easing::Linear,
            enter_easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            enter_scale_from: 0.92,
        }
    }
}

/// Default parameters for CrossFade transition
#[derive(Debug, Clone)]
pub struct CrossFadeDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for CrossFadeDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::EaseInOutQuad,
        }
    }
}

/// Default parameters for Push navigation
#[derive(Debug, Clone)]
pub struct PushDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub old_page_offset: f64,
}

impl Default for PushDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 350,
            easing: Easing::EaseInOutQuad,
            old_page_offset: 100.0,
        }
    }
}

/// Default parameters for SlideOver transition
#[derive(Debug, Clone)]
pub struct SlideOverDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub direction: SlideDirection,
    pub enter_scale: f64,
}

impl Default for SlideOverDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::EaseOutCubic,
            direction: SlideDirection::Right,
            enter_scale: 0.95,
        }
    }
}

/// Default parameters for Zoom transition
#[derive(Debug, Clone)]
pub struct ZoomDefaults {
    pub duration_ms: u64,
    pub old_scale_to: f64,
    pub new_scale_from: f64,
    pub easing: Easing,
}

impl Default for ZoomDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 400,
            old_scale_to: 0.85,
            new_scale_from: 1.2,
            easing: Easing::EaseOutQuad,
        }
    }
}

/// Default parameters for CircleReveal transition
#[derive(Debug, Clone)]
pub struct CircleRevealDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub origin_x: f64,
    pub origin_y: f64,
}

impl Default for CircleRevealDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 600,
            easing: Easing::EaseOutQuad,
            origin_x: 50.0,
            origin_y: 50.0,
        }
    }
}

/// Default parameters for StairCascade transition
#[derive(Debug, Clone)]
pub struct StairCascadeDefaults {
    pub element_duration_ms: u64,
    pub stagger_delay_ms: u64,
    pub easing: Easing,
    pub old_page_fade_ms: u64,
    pub translate_distance: f64,
}

impl Default for StairCascadeDefaults {
    fn default() -> Self {
        Self {
            element_duration_ms: 400,
            stagger_delay_ms: 100,
            easing: Easing::EaseOutQuad,
            old_page_fade_ms: 200,
            translate_distance: 20.0,
        }
    }
}

/// Default parameters for ParallaxSlide transition
#[derive(Debug, Clone)]
pub struct ParallaxSlideDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub old_speed: f64,
    pub new_speed: f64,
}

impl Default for ParallaxSlideDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 500,
            easing: Easing::EaseInOutQuad,
            old_speed: 0.5,
            new_speed: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_axis_defaults() {
        let defaults = SharedAxisDefaults::default();
        assert_eq!(defaults.enter_duration_ms, 200);
        assert_eq!(defaults.exit_duration_ms, 300);
        assert_eq!(defaults.distance, 45.0);
    }

    #[test]
    fn test_fade_through_defaults() {
        let defaults = FadeThroughDefaults::default();
        assert_eq!(defaults.exit_duration_ms, 100);
        assert_eq!(defaults.enter_duration_ms, 200);
        assert_eq!(defaults.enter_scale_from, 0.92);
    }

    #[test]
    fn test_crossfade_defaults() {
        let defaults = CrossFadeDefaults::default();
        assert_eq!(defaults.duration_ms, 300);
    }

    #[test]
    fn test_push_defaults() {
        let defaults = PushDefaults::default();
        assert_eq!(defaults.duration_ms, 350);
        assert_eq!(defaults.old_page_offset, 100.0);
    }

    #[test]
    fn test_slide_over_defaults() {
        let defaults = SlideOverDefaults::default();
        assert_eq!(defaults.duration_ms, 300);
        assert_eq!(defaults.direction, SlideDirection::Right);
        assert_eq!(defaults.enter_scale, 0.95);
    }

    #[test]
    fn test_zoom_defaults() {
        let defaults = ZoomDefaults::default();
        assert_eq!(defaults.duration_ms, 400);
        assert_eq!(defaults.old_scale_to, 0.85);
        assert_eq!(defaults.new_scale_from, 1.2);
    }

    #[test]
    fn test_circle_reveal_defaults() {
        let defaults = CircleRevealDefaults::default();
        assert_eq!(defaults.duration_ms, 600);
        assert_eq!(defaults.origin_x, 50.0);
        assert_eq!(defaults.origin_y, 50.0);
    }

    #[test]
    fn test_stair_cascade_defaults() {
        let defaults = StairCascadeDefaults::default();
        assert_eq!(defaults.element_duration_ms, 400);
        assert_eq!(defaults.stagger_delay_ms, 100);
        assert_eq!(defaults.old_page_fade_ms, 200);
        assert_eq!(defaults.translate_distance, 20.0);
    }

    #[test]
    fn test_parallax_slide_defaults() {
        let defaults = ParallaxSlideDefaults::default();
        assert_eq!(defaults.duration_ms, 500);
        assert_eq!(defaults.old_speed, 0.5);
        assert_eq!(defaults.new_speed, 1.0);
    }
}
