//! Toast animation presets
//!
//! Production-ready toast animations from popular UI libraries and design systems.

use super::types::{StackDirection, ToastAnimation, ToastDirection};
use crate::animation::{Easing, Spring};

// ============================================================================
// Material Design
// ============================================================================

/// Material Design Snackbar — scale + fade + slide from bottom
///
/// Source: Android Material Components
/// - Enter: 250ms FastOutSlowIn, scale(0.8) + translateY(100%)
/// - Exit: 75ms FastOutLinear, scale down + fade
/// - Hold: 2000ms (short) / 3500ms (long)
pub fn material_snackbar() -> ToastAnimation {
    ToastAnimation::SlideFade {
        enter_duration_ms: 250,
        exit_duration_ms: 75,
        hold_duration_ms: 2000,
        enter_easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0), // FastOutSlowIn
        exit_easing: Easing::CubicBezier(0.4, 0.0, 1.0, 1.0),   // FastOutLinear
        direction: ToastDirection::Bottom,
        offset: 100.0,
    }
}

/// Material Design Snackbar (long duration variant)
pub fn material_snackbar_long() -> ToastAnimation {
    ToastAnimation::SlideFade {
        enter_duration_ms: 250,
        exit_duration_ms: 75,
        hold_duration_ms: 3500,
        enter_easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
        exit_easing: Easing::CubicBezier(0.4, 0.0, 1.0, 1.0),
        direction: ToastDirection::Bottom,
        offset: 100.0,
    }
}

// ============================================================================
// Sonner (Emil Kowalski)
// ============================================================================

/// Sonner spring toast with stack compression
///
/// Source: https://sonner.emilkowal.ski/
/// - Spring-based slide up from bottom-right
/// - Stack behavior: earlier toasts scale down 5% per position
/// - Hover expands all toasts
pub fn sonner_spring() -> ToastAnimation {
    ToastAnimation::StackPush {
        push_duration_ms: 400,
        push_easing: Easing::EASE, // CSS ease
        gap: 14.0,
        scale_factor: 0.05, // 5% scale reduction per stack position
        direction: StackDirection::Up,
    }
}

// ============================================================================
// iOS
// ============================================================================

/// iOS push notification banner — spring slide from top
///
/// Source: Apple HIG Notifications
/// - Non-bouncy spring: stiffness=350, damping=28
/// - Slides down from top edge
/// - Smooth iOS-like feel
pub fn ios_banner() -> ToastAnimation {
    ToastAnimation::SpringBounce {
        enter_spring: Spring::new().stiffness(350.0).damping(28.0),
        exit_duration_ms: 200,
        exit_easing: Easing::EASE_IN,
        hold_duration_ms: 5000,
        direction: ToastDirection::Top,
    }
}

// ============================================================================
// Slide + Fade Variants
// ============================================================================

/// Slide + fade from bottom — web.dev pattern
///
/// Source: https://web.dev/articles/building/a-toast-component
/// - 300ms fade-in + slide-in (concurrent)
/// - 300ms fade-out
/// - 3000ms hold
pub fn slide_fade_bottom() -> ToastAnimation {
    ToastAnimation::SlideFade {
        enter_duration_ms: 300,
        exit_duration_ms: 300,
        hold_duration_ms: 3000,
        enter_easing: Easing::EASE,
        exit_easing: Easing::EASE,
        direction: ToastDirection::Bottom,
        offset: 50.0, // 5vh ≈ 50px
    }
}

/// Slide + fade from top — React Hot Toast pattern
pub fn slide_fade_top() -> ToastAnimation {
    ToastAnimation::SlideFade {
        enter_duration_ms: 300,
        exit_duration_ms: 200,
        hold_duration_ms: 3000,
        enter_easing: Easing::EASE_OUT,
        exit_easing: Easing::EASE_IN,
        direction: ToastDirection::Top,
        offset: 50.0,
    }
}

// ============================================================================
// Bounce / Drop
// ============================================================================

/// Bounce drop from top — React-Toastify bounce variant
///
/// Source: https://fkhadra.github.io/react-toastify/
/// - Drops from -120px above with scaleY(0.8)
/// - Cubic bezier overshoot: (0, 1.5, 0.5, 1) bounces to 150%
/// - 500ms enter, 150ms exit
pub fn bounce_drop() -> ToastAnimation {
    ToastAnimation::DropIn {
        spring: Spring::new().stiffness(300.0).damping(15.0),
        exit_duration_ms: 150,
        exit_easing: Easing::CubicBezier(0.5, 0.0, 1.0, 1.0),
        hold_duration_ms: 3000,
        drop_height: 120.0,
    }
}

// ============================================================================
// Scale + Fade
// ============================================================================

/// Scale + fade — Angular Material / Chakra UI pattern
///
/// Source: Angular Material Snackbar, Chakra UI SlideFade
/// - Scales from 0.8 to 1.0 while fading in
/// - Clean, minimal animation
/// - 150ms enter, 75ms exit (Material timing)
pub fn scale_fade() -> ToastAnimation {
    ToastAnimation::ScaleFade {
        enter_duration_ms: 150,
        exit_duration_ms: 75,
        hold_duration_ms: 2000,
        enter_easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0), // LinearOutSlowIn
        exit_easing: Easing::CubicBezier(0.4, 0.0, 1.0, 1.0),   // FastOutLinear
        scale_from: 0.8,
        scale_to: 1.0,
    }
}

/// Zoom toast — React-Toastify zoom variant
///
/// Zooms from center (scale 0.3 to 1.0)
/// 500ms enter, 600ms exit
pub fn zoom_toast() -> ToastAnimation {
    ToastAnimation::ScaleFade {
        enter_duration_ms: 500,
        exit_duration_ms: 600,
        hold_duration_ms: 3000,
        enter_easing: Easing::EASE_OUT,
        exit_easing: Easing::EASE_IN,
        scale_from: 0.3,
        scale_to: 1.0,
    }
}

// ============================================================================
// Framer Motion Spring
// ============================================================================

/// Framer Motion spring toast — physics-based with presets
///
/// Source: https://motion.dev/docs/react-use-spring
/// - Stiffness: 260, Damping: 20 (snappy preset)
/// - Natural spring feel
pub fn framer_spring_toast() -> ToastAnimation {
    ToastAnimation::SpringBounce {
        enter_spring: Spring::new().stiffness(260.0).damping(20.0),
        exit_duration_ms: 300,
        exit_easing: Easing::EASE_OUT,
        hold_duration_ms: 3000,
        direction: ToastDirection::Bottom,
    }
}

/// Framer Motion gentle spring — lower stiffness
pub fn framer_spring_gentle() -> ToastAnimation {
    ToastAnimation::SpringBounce {
        enter_spring: Spring::new().stiffness(100.0).damping(10.0),
        exit_duration_ms: 300,
        exit_easing: Easing::EASE_OUT,
        hold_duration_ms: 3000,
        direction: ToastDirection::Bottom,
    }
}

/// Framer Motion bouncy spring — higher stiffness, lower damping
pub fn framer_spring_bouncy() -> ToastAnimation {
    ToastAnimation::SpringBounce {
        enter_spring: Spring::new().stiffness(300.0).damping(15.0),
        exit_duration_ms: 300,
        exit_easing: Easing::EASE_OUT,
        hold_duration_ms: 3000,
        direction: ToastDirection::Bottom,
    }
}

// ============================================================================
// Swipe-to-Dismiss
// ============================================================================

/// Swipe-to-dismiss toast — Sonner / Radix UI pattern
///
/// Source: https://emilkowal.ski/ui/building-a-toast-component
/// - Velocity threshold: 0.11 units/ms
/// - Distance threshold: 100px
/// - Friction: 0.5 for upward swipes
/// - Spring back if canceled: stiffness=400, damping=25
pub fn swipe_dismiss() -> ToastAnimation {
    ToastAnimation::SwipeDismiss {
        velocity_threshold: 0.11,
        distance_threshold: 100.0,
        friction: 0.998, // iOS-like decay
        spring_back_spring: Spring::new().stiffness(400.0).damping(25.0),
    }
}

// ============================================================================
// Progress Bar Countdown
// ============================================================================

/// Progress bar countdown toast
///
/// Source: https://www.codingnepalweb.com/toast-notification-progress-bar-html-css-javascript/
/// - Toast slides in with progress bar
/// - Progress bar animates linearly from 100% to 0%
/// - 5000ms hold time (customizable)
/// - Pause on hover supported via timeline control
pub fn progress_countdown() -> ToastAnimation {
    ToastAnimation::ProgressBar {
        enter_duration_ms: 300,
        enter_easing: Easing::CubicBezier(0.68, -0.55, 0.265, 1.35), // Overshoot
        exit_duration_ms: 300,
        exit_easing: Easing::EASE_OUT,
        hold_duration_ms: 5000,
        progress_easing: Easing::Linear,
        direction: ToastDirection::Right,
    }
}

/// Progress countdown with shorter hold time (3s)
pub fn progress_countdown_short() -> ToastAnimation {
    ToastAnimation::ProgressBar {
        enter_duration_ms: 300,
        enter_easing: Easing::CubicBezier(0.68, -0.55, 0.265, 1.35),
        exit_duration_ms: 300,
        exit_easing: Easing::EASE_OUT,
        hold_duration_ms: 3000,
        progress_easing: Easing::Linear,
        direction: ToastDirection::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_snackbar() {
        let anim = material_snackbar();
        assert_eq!(anim.total_duration_ms(), 250 + 2000 + 75);
    }

    #[test]
    fn test_sonner_spring() {
        let anim = sonner_spring();
        // Stack push has single duration
        assert_eq!(anim.total_duration_ms(), 400);
    }

    #[test]
    fn test_ios_banner() {
        let anim = ios_banner();
        let duration = anim.total_duration_ms();
        // Should include spring settle time + hold + exit
        assert!(duration > 5000);
        assert!(duration < 10000);
    }

    #[test]
    fn test_slide_fade_bottom() {
        let anim = slide_fade_bottom();
        assert_eq!(anim.total_duration_ms(), 300 + 3000 + 300);
    }

    #[test]
    fn test_scale_fade() {
        let anim = scale_fade();
        assert_eq!(anim.total_duration_ms(), 150 + 2000 + 75);
    }

    #[test]
    fn test_framer_spring_variants() {
        let snappy = framer_spring_toast();
        let gentle = framer_spring_gentle();
        let bouncy = framer_spring_bouncy();

        // All should have reasonable durations
        assert!(snappy.total_duration_ms() > 3000);
        assert!(gentle.total_duration_ms() > 3000);
        assert!(bouncy.total_duration_ms() > 3000);
    }

    #[test]
    fn test_progress_countdown() {
        let anim = progress_countdown();
        assert_eq!(anim.total_duration_ms(), 300 + 5000 + 300);

        let short = progress_countdown_short();
        assert_eq!(short.total_duration_ms(), 300 + 3000 + 300);
    }

    #[test]
    fn test_swipe_dismiss() {
        let anim = swipe_dismiss();
        // Swipe is gesture-driven, total duration is 0
        assert_eq!(anim.total_duration_ms(), 0);
    }
}
