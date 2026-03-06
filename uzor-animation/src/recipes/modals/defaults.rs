//! Default parameter structs for modal animations
//!
//! Provides default values for each animation variant, making it easier
//! to customize only specific parameters.

use crate::{Easing, Spring, Decay};

/// Default parameters for FadeScale animation
#[derive(Debug, Clone, Copy)]
pub struct FadeScaleDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub scale_from: f64,
    pub scale_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for FadeScaleDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 225,
            easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
            scale_from: 0.95,
            scale_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for SlideUp animation
#[derive(Debug, Clone, Copy)]
pub struct SlideUpDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub translate_from: f64,
    pub translate_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
    pub delay_ms: u64,
}

impl Default for SlideUpDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 450,
            easing: Easing::CubicBezier(0.32, 1.0, 0.23, 1.0),
            translate_from: 100.0,
            translate_to: 0.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
            delay_ms: 0,
        }
    }
}

/// Default parameters for SlideDown animation
#[derive(Debug, Clone, Copy)]
pub struct SlideDownDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub translate_from: f64,
    pub translate_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for SlideDownDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 400,
            easing: Easing::CubicBezier(0.32, 0.72, 0.0, 1.0),
            translate_from: -100.0,
            translate_to: 0.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for SlideRight animation
#[derive(Debug, Clone, Copy)]
pub struct SlideRightDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub translate_from: f64,
    pub translate_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for SlideRightDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::EaseOutCubic,
            translate_from: 100.0,
            translate_to: 0.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for SpringScale animation
#[derive(Debug, Clone)]
pub struct SpringScaleDefaults {
    pub spring: Spring,
    pub scale_from: f64,
    pub scale_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for SpringScaleDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(350.0).damping(28.0).mass(1.0),
            scale_from: 0.0,
            scale_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for Backdrop animation
#[derive(Debug, Clone, Copy)]
pub struct BackdropDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub opacity_from: f64,
    pub opacity_to: f64,
    pub blur_from: f64,
    pub blur_to: f64,
}

impl Default for BackdropDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 200,
            easing: Easing::Linear,
            opacity_from: 0.0,
            opacity_to: 0.5,
            blur_from: 0.0,
            blur_to: 0.0,
        }
    }
}

/// Default parameters for DrawerSnap animation
#[derive(Debug, Clone)]
pub struct DrawerSnapDefaults {
    pub spring: Spring,
    pub decay: Option<Decay>,
    pub snap_points: Vec<f64>,
    pub translate_from: f64,
    pub translate_to: f64,
    pub background_scale_from: f64,
    pub background_scale_to: f64,
}

impl Default for DrawerSnapDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(300.0).damping(30.0).mass(1.0),
            decay: Some(Decay::new(0.0).friction(0.998)),
            snap_points: vec![0.0, 40.0, 100.0],
            translate_from: 100.0,
            translate_to: 0.0,
            background_scale_from: 1.0,
            background_scale_to: 0.95,
        }
    }
}

/// Default parameters for DropIn animation
#[derive(Debug, Clone, Copy)]
pub struct DropInDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub translate_from: f64,
    pub translate_to: f64,
    pub scale_from: f64,
    pub scale_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for DropInDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 500,
            easing: Easing::EaseOutBounce,
            translate_from: -100.0,
            translate_to: 0.0,
            scale_from: 0.1,
            scale_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for ZoomFromOrigin animation
#[derive(Debug, Clone, Copy)]
pub struct ZoomFromOriginDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub origin_x: f64,
    pub origin_y: f64,
    pub scale_from: f64,
    pub scale_to: f64,
    pub translate_x_from: f64,
    pub translate_x_to: f64,
    pub translate_y_from: f64,
    pub translate_y_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for ZoomFromOriginDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            origin_x: 0.0,
            origin_y: 0.0,
            scale_from: 0.2,
            scale_to: 1.0,
            translate_x_from: 0.0,
            translate_x_to: 0.0,
            translate_y_from: 0.0,
            translate_y_to: 0.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

/// Default parameters for Curtain animation
#[derive(Debug, Clone, Copy)]
pub struct CurtainDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub clip_from: f64,
    pub clip_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for CurtainDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 400,
            easing: Easing::EASE_IN_OUT,
            clip_from: 0.0,
            clip_to: 1.0,
            opacity_from: 0.0,
            opacity_to: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_scale_defaults() {
        let defaults = FadeScaleDefaults::default();
        assert_eq!(defaults.duration_ms, 225);
        assert!((defaults.scale_from - 0.95).abs() < 0.01);
        assert!((defaults.scale_to - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_slide_up_defaults() {
        let defaults = SlideUpDefaults::default();
        assert_eq!(defaults.duration_ms, 450);
        assert!((defaults.translate_from - 100.0).abs() < 0.01);
        assert!((defaults.translate_to - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_spring_scale_defaults() {
        let defaults = SpringScaleDefaults::default();
        assert!((defaults.spring.stiffness - 350.0).abs() < 0.1);
        assert!((defaults.spring.damping - 28.0).abs() < 0.1);
        assert!((defaults.scale_from - 0.0).abs() < 0.01);
        assert!((defaults.scale_to - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_drawer_snap_defaults() {
        let defaults = DrawerSnapDefaults::default();
        assert_eq!(defaults.snap_points.len(), 3);
        assert!((defaults.snap_points[0] - 0.0).abs() < 0.01);
        assert!((defaults.snap_points[1] - 40.0).abs() < 0.01);
        assert!((defaults.snap_points[2] - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_backdrop_defaults() {
        let defaults = BackdropDefaults::default();
        assert_eq!(defaults.duration_ms, 200);
        assert!((defaults.opacity_to - 0.5).abs() < 0.01);
        assert!((defaults.blur_from - 0.0).abs() < 0.01);
    }
}
