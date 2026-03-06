//! Pre-configured button animation presets
//!
//! Factory functions returning ready-to-use ButtonAnimation instances
//! based on Material Design 3, iOS HIG, Framer Motion, and web best practices.

use super::types::{ButtonAnimation, SlideOrigin, SweepDirection};
use crate::{Easing, Spring};

// ============================================================================
// Hover Animations
// ============================================================================

/// Material Design 3 hover: 200ms ease-out, background opacity 0.08
///
/// Source: Material Design 3 specifications
pub fn material_hover() -> ButtonAnimation {
    ButtonAnimation::Hover {
        duration_ms: 200,
        easing: Easing::EaseOutCubic,
        opacity_from: 0.0,
        opacity_to: 0.08,
    }
}

/// Fast subtle hover: 150ms linear opacity change
///
/// For quick responsive feedback
pub fn fast_subtle_hover() -> ButtonAnimation {
    ButtonAnimation::Hover {
        duration_ms: 150,
        easing: Easing::Linear,
        opacity_from: 0.0,
        opacity_to: 0.05,
    }
}

// ============================================================================
// Press Animations
// ============================================================================

/// iOS press: 100ms scale to 0.97, cubic-bezier(0.4, 0, 0.2, 1)
///
/// Source: Apple Human Interface Guidelines
pub fn ios_press() -> ButtonAnimation {
    ButtonAnimation::Press {
        duration_ms: 100,
        easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
        scale: 0.97,
    }
}

/// Simple press: 100ms scale to 0.95
///
/// Universal tactile feedback
pub fn simple_press() -> ButtonAnimation {
    ButtonAnimation::Press {
        duration_ms: 100,
        easing: Easing::EaseOutCubic,
        scale: 0.95,
    }
}

/// Bounce press: scale 0.9 with elastic overshoot back
///
/// More pronounced press feedback
pub fn bounce_press() -> ButtonAnimation {
    ButtonAnimation::Press {
        duration_ms: 100,
        easing: Easing::EaseInCubic,
        scale: 0.9,
    }
}

// ============================================================================
// Release Animations
// ============================================================================

/// iOS release: spring back, stiffness=350, damping=28
///
/// Source: Apple Human Interface Guidelines motion curves
pub fn ios_release() -> ButtonAnimation {
    ButtonAnimation::Release {
        spring: Spring::new().stiffness(350.0).damping(28.0),
    }
}

/// Framer Motion style: spring stiffness=400, damping=17
///
/// Source: Framer Motion default spring configuration
pub fn framer_spring_press() -> ButtonAnimation {
    ButtonAnimation::Release {
        spring: Spring::new().stiffness(400.0).damping(17.0),
    }
}

// ============================================================================
// Ripple Effect
// ============================================================================

/// Material Design ripple: 400ms ease-out expand + fade
///
/// Source: Material Design 3 ripple specifications
/// Duration: 400ms (full expansion), 225ms (minimum press)
pub fn material_ripple() -> ButtonAnimation {
    ButtonAnimation::Ripple {
        duration_ms: 400,
        easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
        scale_from: 0.0,
        scale_to: 4.0,
        opacity_from: 0.12,
        opacity_to: 0.0,
    }
}

// ============================================================================
// Elastic Scale
// ============================================================================

/// Elastic scale hover: spring stiffness=300, damping=15
///
/// Bouncy hover effect with spring physics
pub fn elastic_scale_hover() -> ButtonAnimation {
    ButtonAnimation::ElasticScale {
        spring: Spring::new().stiffness(300.0).damping(15.0),
        target_scale: 1.1,
    }
}

/// Subtle elastic: less bouncy, more controlled
pub fn subtle_elastic_hover() -> ButtonAnimation {
    ButtonAnimation::ElasticScale {
        spring: Spring::new().stiffness(200.0).damping(20.0),
        target_scale: 1.05,
    }
}

// ============================================================================
// Glow Pulse
// ============================================================================

/// Subtle glow pulse on hover: 2s ease-in-out infinite
///
/// Breathing effect for call-to-action buttons
pub fn glow_pulse() -> ButtonAnimation {
    ButtonAnimation::GlowPulse {
        duration_ms: 2000,
        easing: Easing::EaseInOutSine,
        intensity_from: 1.0,
        intensity_to: 1.5,
    }
}

/// Fast pulse: shorter cycle for more energy
pub fn fast_glow_pulse() -> ButtonAnimation {
    ButtonAnimation::GlowPulse {
        duration_ms: 1000,
        easing: Easing::EaseInOutSine,
        intensity_from: 1.0,
        intensity_to: 1.3,
    }
}

// ============================================================================
// Underline Slide
// ============================================================================

/// Underline slides in from left: 250ms ease-out-cubic
///
/// Common navigation and link pattern
pub fn underline_slide() -> ButtonAnimation {
    ButtonAnimation::UnderlineSlide {
        duration_ms: 250,
        easing: Easing::EaseOutCubic,
        origin: SlideOrigin::Left,
    }
}

/// Underline from center: expands outward
pub fn underline_slide_center() -> ButtonAnimation {
    ButtonAnimation::UnderlineSlide {
        duration_ms: 250,
        easing: Easing::EaseOutCubic,
        origin: SlideOrigin::Center,
    }
}

// ============================================================================
// Fill Sweep
// ============================================================================

/// Background fills left-to-right: 300ms ease-out
///
/// Popular hover effect for outlined buttons
pub fn fill_sweep() -> ButtonAnimation {
    ButtonAnimation::FillSweep {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        direction: SweepDirection::LeftToRight,
    }
}

/// Vertical fill: bottom to top
pub fn fill_sweep_vertical() -> ButtonAnimation {
    ButtonAnimation::FillSweep {
        duration_ms: 350,
        easing: Easing::EaseOutCubic,
        direction: SweepDirection::BottomToTop,
    }
}

// ============================================================================
// Border Draw
// ============================================================================

/// Border draws around button: 600ms ease-in-out (stroke animation)
///
/// Sequential border drawing effect
pub fn border_draw() -> ButtonAnimation {
    ButtonAnimation::BorderDraw {
        duration_ms: 600,
        easing: Easing::EaseInOutCubic,
        stagger_delay_ms: 100,
    }
}

/// Fast border draw: quicker response
pub fn fast_border_draw() -> ButtonAnimation {
    ButtonAnimation::BorderDraw {
        duration_ms: 400,
        easing: Easing::EaseOutCubic,
        stagger_delay_ms: 50,
    }
}

// ============================================================================
// Magnetic Pull
// ============================================================================

/// Button follows cursor slightly: spring damping=20, stiffness=150
///
/// Advanced interactive effect
pub fn magnetic_pull() -> ButtonAnimation {
    ButtonAnimation::MagneticPull {
        spring: Spring::new().stiffness(150.0).damping(20.0),
        max_distance: 100.0,
        strength: 0.3,
    }
}

/// Strong magnetic: more pronounced pull
pub fn strong_magnetic_pull() -> ButtonAnimation {
    ButtonAnimation::MagneticPull {
        spring: Spring::new().stiffness(200.0).damping(15.0),
        max_distance: 150.0,
        strength: 0.5,
    }
}

// ============================================================================
// Lift Shadow
// ============================================================================

/// Shadow grows + slight lift: 250ms ease-out
///
/// Material Design elevation change
pub fn lift_shadow() -> ButtonAnimation {
    ButtonAnimation::LiftShadow {
        duration_ms: 250,
        easing: Easing::EaseOutCubic,
        shadow_y_from: 2.0,
        shadow_y_to: 8.0,
        shadow_blur_from: 4.0,
        shadow_blur_to: 16.0,
        lift_distance: 2.0,
    }
}

/// Dramatic lift: larger shadow and movement
pub fn dramatic_lift() -> ButtonAnimation {
    ButtonAnimation::LiftShadow {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        shadow_y_from: 2.0,
        shadow_y_to: 16.0,
        shadow_blur_from: 4.0,
        shadow_blur_to: 24.0,
        lift_distance: 4.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_hover() {
        let anim = material_hover();
        assert_eq!(anim.duration_ms(), 200);
    }

    #[test]
    fn test_ios_press() {
        let anim = ios_press();
        assert_eq!(anim.duration_ms(), 100);
    }

    #[test]
    fn test_ios_release() {
        let anim = ios_release();
        let duration = anim.duration_ms();
        assert!(duration > 0);
        assert!(duration < 5000);
    }

    #[test]
    fn test_material_ripple() {
        let anim = material_ripple();
        assert_eq!(anim.duration_ms(), 400);
    }

    #[test]
    fn test_elastic_scale_hover() {
        let anim = elastic_scale_hover();
        let duration = anim.duration_ms();
        assert!(duration > 0);
    }

    #[test]
    fn test_all_presets_compile() {
        // Verify all presets can be instantiated
        let _ = material_hover();
        let _ = fast_subtle_hover();
        let _ = ios_press();
        let _ = simple_press();
        let _ = bounce_press();
        let _ = ios_release();
        let _ = framer_spring_press();
        let _ = material_ripple();
        let _ = elastic_scale_hover();
        let _ = subtle_elastic_hover();
        let _ = glow_pulse();
        let _ = fast_glow_pulse();
        let _ = underline_slide();
        let _ = underline_slide_center();
        let _ = fill_sweep();
        let _ = fill_sweep_vertical();
        let _ = border_draw();
        let _ = fast_border_draw();
        let _ = magnetic_pull();
        let _ = strong_magnetic_pull();
        let _ = lift_shadow();
        let _ = dramatic_lift();
    }
}
