//! Default parameter values for button animations
//!
//! These defaults are derived from Material Design 3, iOS HIG, and web animation best practices.

use crate::{Easing, Spring};

/// Default parameters for hover animations
#[derive(Debug, Clone)]
pub struct HoverDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for HoverDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 200,
            easing: Easing::EaseOutCubic,
            opacity_from: 0.0,
            opacity_to: 0.08,
        }
    }
}

/// Default parameters for press animations
#[derive(Debug, Clone)]
pub struct PressDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub scale: f64,
}

impl Default for PressDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 100,
            easing: Easing::EaseInCubic,
            scale: 0.95,
        }
    }
}

/// Default parameters for release (spring-back) animations
#[derive(Debug, Clone)]
pub struct ReleaseDefaults {
    pub spring: Spring,
}

impl Default for ReleaseDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(350.0).damping(28.0),
        }
    }
}

/// Default parameters for ripple effect
#[derive(Debug, Clone)]
pub struct RippleDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub scale_from: f64,
    pub scale_to: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
}

impl Default for RippleDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 400,
            easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
            scale_from: 0.0,
            scale_to: 4.0,
            opacity_from: 0.12,
            opacity_to: 0.0,
        }
    }
}

/// Default parameters for toggle animations
#[derive(Debug, Clone)]
pub struct ToggleDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for ToggleDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 200,
            easing: Easing::EASE_IN_OUT,
        }
    }
}

/// Default parameters for elastic scale hover
#[derive(Debug, Clone)]
pub struct ElasticScaleDefaults {
    pub spring: Spring,
    pub target_scale: f64,
}

impl Default for ElasticScaleDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(300.0).damping(15.0),
            target_scale: 1.1,
        }
    }
}

/// Default parameters for glow pulse
#[derive(Debug, Clone)]
pub struct GlowPulseDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub intensity_from: f64,
    pub intensity_to: f64,
}

impl Default for GlowPulseDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 2000,
            easing: Easing::EaseInOutSine,
            intensity_from: 1.0,
            intensity_to: 1.5,
        }
    }
}

/// Default parameters for underline slide
#[derive(Debug, Clone)]
pub struct UnderlineSlideDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for UnderlineSlideDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 250,
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for fill sweep
#[derive(Debug, Clone)]
pub struct FillSweepDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for FillSweepDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for border draw
#[derive(Debug, Clone)]
pub struct BorderDrawDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub stagger_delay_ms: u64,
}

impl Default for BorderDrawDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 600,
            easing: Easing::EaseInOutCubic,
            stagger_delay_ms: 100,
        }
    }
}

/// Default parameters for magnetic pull
#[derive(Debug, Clone)]
pub struct MagneticPullDefaults {
    pub spring: Spring,
    pub max_distance: f64,
    pub strength: f64,
}

impl Default for MagneticPullDefaults {
    fn default() -> Self {
        Self {
            spring: Spring::new().stiffness(150.0).damping(20.0),
            max_distance: 100.0,
            strength: 0.3,
        }
    }
}

/// Default parameters for lift shadow
#[derive(Debug, Clone)]
pub struct LiftShadowDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub shadow_y_from: f64,
    pub shadow_y_to: f64,
    pub shadow_blur_from: f64,
    pub shadow_blur_to: f64,
    pub lift_distance: f64,
}

impl Default for LiftShadowDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 250,
            easing: Easing::EaseOutCubic,
            shadow_y_from: 2.0,
            shadow_y_to: 8.0,
            shadow_blur_from: 4.0,
            shadow_blur_to: 16.0,
            lift_distance: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_creation() {
        let _ = HoverDefaults::default();
        let _ = PressDefaults::default();
        let _ = ReleaseDefaults::default();
        let _ = RippleDefaults::default();
        let _ = ToggleDefaults::default();
        let _ = ElasticScaleDefaults::default();
        let _ = GlowPulseDefaults::default();
        let _ = UnderlineSlideDefaults::default();
        let _ = FillSweepDefaults::default();
        let _ = BorderDrawDefaults::default();
        let _ = MagneticPullDefaults::default();
        let _ = LiftShadowDefaults::default();
    }

    #[test]
    fn test_hover_defaults_values() {
        let defaults = HoverDefaults::default();
        assert_eq!(defaults.duration_ms, 200);
        assert_eq!(defaults.opacity_from, 0.0);
        assert_eq!(defaults.opacity_to, 0.08);
    }

    #[test]
    fn test_press_defaults_values() {
        let defaults = PressDefaults::default();
        assert_eq!(defaults.duration_ms, 100);
        assert_eq!(defaults.scale, 0.95);
    }
}
