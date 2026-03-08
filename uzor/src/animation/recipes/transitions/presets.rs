//! Preset transition animations
//!
//! Ready-to-use presets based on Material Design, iOS, and web best practices.
//! All presets use standard timing and easing values from the research.

use super::types::{SlideDirection, TransitionAnimation};
use crate::animation::Easing;

/// Material Design Shared Axis X (horizontal)
///
/// Horizontal navigation with 300ms total duration.
/// Old page fades out (100ms) while translating left (-45px).
/// New page fades in (200ms, starts at 100ms) while translating from right (+45px to 0).
///
/// Uses Material standard curve: cubic-bezier(0.4, 0.0, 0.2, 1.0)
pub fn material_shared_axis_x() -> TransitionAnimation {
    TransitionAnimation::SharedAxisX {
        enter_duration_ms: 200,
        exit_duration_ms: 300,
        overlap_ms: 200, // Enter starts at 100ms (300-200=100)
        easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
        distance: 45.0,
    }
}

/// Material Design Shared Axis Y (vertical)
///
/// Vertical navigation with same timing as horizontal variant.
/// Old page slides up while fading, new slides down from above.
pub fn material_shared_axis_y() -> TransitionAnimation {
    TransitionAnimation::SharedAxisY {
        enter_duration_ms: 200,
        exit_duration_ms: 300,
        overlap_ms: 200,
        easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
        distance: 45.0,
    }
}

/// Material Design Fade Through
///
/// Sequential fade for unrelated content transitions.
/// Old page fades out linearly (100ms).
/// New page fades in with scale-up (200ms, starts after exit).
/// Total duration: 300ms.
pub fn material_fade_through() -> TransitionAnimation {
    TransitionAnimation::FadeThrough {
        exit_duration_ms: 100,
        enter_duration_ms: 200,
        exit_easing: Easing::Linear,
        enter_easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
        enter_scale_from: 0.92,
    }
}

/// Simple crossfade / dissolve
///
/// Pure opacity transition with simultaneous fade.
/// Old fades out, new fades in. 300ms duration, ease-in-out.
pub fn cross_fade() -> TransitionAnimation {
    TransitionAnimation::CrossFade {
        duration_ms: 300,
        easing: Easing::EaseInOutQuad,
    }
}

/// iOS Push Navigation
///
/// Classic iOS horizontal slide.
/// New page slides in from right (100% to 0).
/// Old page slides left partially (-100px).
/// 350ms duration with ease-in-out curve.
pub fn ios_push() -> TransitionAnimation {
    TransitionAnimation::PushLeft {
        duration_ms: 350,
        easing: Easing::EaseInOutQuad,
        old_page_offset: 100.0,
    }
}

/// Slide Over (modal-style)
///
/// New page slides over stationary old page.
/// New slides from right with slight scale-up (0.95 to 1.0).
/// 300ms with ease-out-cubic curve.
pub fn slide_over() -> TransitionAnimation {
    TransitionAnimation::SlideOver {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        direction: SlideDirection::Right,
        enter_scale: 0.95,
    }
}

/// Zoom In transition
///
/// Old page scales down (1.0 to 0.85) and fades out.
/// New page scales up (1.2 to 1.0) and fades in.
/// Creates depth effect. 400ms parallel animation.
pub fn zoom_in() -> TransitionAnimation {
    TransitionAnimation::ZoomIn {
        duration_ms: 400,
        old_scale_to: 0.85,
        new_scale_from: 1.2,
        easing: Easing::EaseOutQuad,
    }
}

/// Circle Reveal (center origin)
///
/// New page reveals from center in expanding circle.
/// 600ms with ease-out quadratic.
/// Origin at 50%, 50% (screen center).
pub fn circle_reveal() -> TransitionAnimation {
    TransitionAnimation::CircleReveal {
        duration_ms: 600,
        easing: Easing::EaseOutQuad,
        origin_x: 50.0,
        origin_y: 50.0,
    }
}

/// Circle Reveal with custom origin
///
/// Same as circle_reveal but allows specifying origin point.
/// Useful for button-triggered transitions.
pub fn circle_reveal_from(origin_x: f64, origin_y: f64) -> TransitionAnimation {
    TransitionAnimation::CircleReveal {
        duration_ms: 600,
        easing: Easing::EaseOutQuad,
        origin_x,
        origin_y,
    }
}

/// Stair Cascade transition
///
/// Old page fades quickly (200ms).
/// New page elements enter sequentially with stagger.
/// Each element: 400ms duration, 100ms stagger delay.
/// Elements slide up from -20px with opacity 0 to 1.
pub fn stair_cascade() -> TransitionAnimation {
    TransitionAnimation::StairCascade {
        element_duration_ms: 400,
        stagger_delay_ms: 100,
        easing: Easing::EaseOutQuad,
        old_page_fade_ms: 200,
        translate_distance: 20.0,
    }
}

/// Stair Cascade with custom bounce
///
/// Same as stair_cascade but with bounce easing for playful effect.
pub fn stair_cascade_bounce() -> TransitionAnimation {
    TransitionAnimation::StairCascade {
        element_duration_ms: 400,
        stagger_delay_ms: 80,
        easing: Easing::CubicBezier(0.6, -0.05, 0.01, 0.99),
        old_page_fade_ms: 200,
        translate_distance: 20.0,
    }
}

/// Parallax Slide transition
///
/// Old page moves at 50% speed, new page at 100% speed.
/// Creates depth through motion parallax.
/// 500ms with ease-in-out.
pub fn parallax_slide() -> TransitionAnimation {
    TransitionAnimation::ParallaxSlide {
        duration_ms: 500,
        easing: Easing::EaseInOutQuad,
        old_speed: 0.5,
        new_speed: 1.0,
    }
}

/// Slow crossfade (1 second)
///
/// Longer crossfade for dramatic effect or high-quality imagery.
/// 1000ms with ease-in-out.
pub fn cross_fade_slow() -> TransitionAnimation {
    TransitionAnimation::CrossFade {
        duration_ms: 1000,
        easing: Easing::EaseInOutQuad,
    }
}

/// Fast crossfade (200ms)
///
/// Quick fade for snappy UI transitions.
pub fn cross_fade_fast() -> TransitionAnimation {
    TransitionAnimation::CrossFade {
        duration_ms: 200,
        easing: Easing::EaseInOutQuad,
    }
}

/// Slide Over from Left
///
/// Variant of slide_over that enters from left side.
pub fn slide_over_left() -> TransitionAnimation {
    TransitionAnimation::SlideOver {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        direction: SlideDirection::Left,
        enter_scale: 0.95,
    }
}

/// Slide Over from Top
///
/// Variant of slide_over that enters from top (modal-like).
pub fn slide_over_top() -> TransitionAnimation {
    TransitionAnimation::SlideOver {
        duration_ms: 400,
        easing: Easing::EaseOutCubic,
        direction: SlideDirection::Up,
        enter_scale: 0.95,
    }
}

/// Zoom Out transition
///
/// Inverse of zoom_in.
/// Old page scales up and fades, new page scales down into view.
pub fn zoom_out() -> TransitionAnimation {
    TransitionAnimation::ZoomIn {
        duration_ms: 400,
        old_scale_to: 1.15,
        new_scale_from: 0.85,
        easing: Easing::EaseInQuad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_shared_axis_x() {
        let transition = material_shared_axis_x();
        // Should have 300ms total duration
        assert_eq!(transition.combined_duration_ms(), 300);
    }

    #[test]
    fn test_material_fade_through() {
        let transition = material_fade_through();
        // 100ms exit + 200ms enter = 300ms
        assert_eq!(transition.combined_duration_ms(), 300);
    }

    #[test]
    fn test_cross_fade() {
        let transition = cross_fade();
        assert_eq!(transition.combined_duration_ms(), 300);
    }

    #[test]
    fn test_ios_push() {
        let transition = ios_push();
        assert_eq!(transition.combined_duration_ms(), 350);
    }

    #[test]
    fn test_zoom_in() {
        let transition = zoom_in();
        assert_eq!(transition.combined_duration_ms(), 400);
    }

    #[test]
    fn test_circle_reveal() {
        let transition = circle_reveal();
        assert_eq!(transition.combined_duration_ms(), 600);
    }

    #[test]
    fn test_circle_reveal_custom_origin() {
        let transition = circle_reveal_from(100.0, 200.0);
        if let TransitionAnimation::CircleReveal {
            origin_x, origin_y, ..
        } = transition
        {
            assert_eq!(origin_x, 100.0);
            assert_eq!(origin_y, 200.0);
        } else {
            panic!("Expected CircleReveal variant");
        }
    }

    #[test]
    fn test_stair_cascade() {
        let transition = stair_cascade();
        // Should have reasonable duration (old fade + stagger elements)
        let duration = transition.combined_duration_ms();
        assert!(duration >= 200); // At least the old page fade
        assert!(duration <= 1500); // Reasonable upper bound
    }

    #[test]
    fn test_parallax_slide() {
        let transition = parallax_slide();
        assert_eq!(transition.combined_duration_ms(), 500);
    }

    #[test]
    fn test_crossfade_variants() {
        assert_eq!(cross_fade_slow().combined_duration_ms(), 1000);
        assert_eq!(cross_fade_fast().combined_duration_ms(), 200);
    }

    #[test]
    fn test_slide_over_directions() {
        let left = slide_over_left();
        let top = slide_over_top();

        if let TransitionAnimation::SlideOver { direction, .. } = left {
            assert_eq!(direction, SlideDirection::Left);
        }

        if let TransitionAnimation::SlideOver { direction, .. } = top {
            assert_eq!(direction, SlideDirection::Up);
        }
    }
}
