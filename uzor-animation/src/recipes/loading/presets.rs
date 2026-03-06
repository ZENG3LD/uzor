//! Loading animation presets
//!
//! Pre-configured loading animations based on popular design systems and patterns.

use crate::Easing;
use super::types::LoadingAnimation;

/// Material Design circular indeterminate spinner
///
/// Rotating partial arc with dynamic length changes.
/// Based on Material Design 3 spec.
pub fn material_circular() -> LoadingAnimation {
    LoadingAnimation::SpinningArc {
        rotation_duration_ms: 2000,
        arc_duration_ms: 1500,
        easing: Easing::EaseInOutCubic,
        dash_array_min: 1.0,
        dash_array_max: 90.0,
        path_length: 150.0,
    }
}

/// Material Design linear indeterminate progress
///
/// Sliding bar with growing/shrinking animation.
/// 2-second cycle matching Material spec.
pub fn material_linear() -> LoadingAnimation {
    LoadingAnimation::IndeterminateBar {
        duration_ms: 2000,
        easing: Easing::CubicBezier(0.65, 0.815, 0.735, 0.395),
        scale_x_from: 0.3,
        scale_x_mid: 1.0,
        scale_x_end: 0.3,
    }
}

/// Three bouncing dots (classic loading pattern)
///
/// Three dots that scale up in sequence, creating wave effect.
/// Total duration: 1.4s with 160ms stagger between dots.
pub fn three_bounce_dots() -> LoadingAnimation {
    LoadingAnimation::PulseDots {
        duration_ms: 1200,
        easing: Easing::EaseInOutQuad,
        count: 3,
        stagger_delay_ms: 160,
        scale_from: 0.0,
        scale_to: 1.0,
    }
}

/// Wave bars (audio equalizer style)
///
/// 5 vertical bars that stretch and compress in sequence.
/// Creates a wave/equalizer effect.
pub fn wave_bars() -> LoadingAnimation {
    LoadingAnimation::BarWave {
        duration_ms: 1200,
        easing: Easing::EaseInOutQuad,
        count: 5,
        stagger_delay_ms: 100,
        scale_from: 0.4,
        scale_to: 1.0,
    }
}

/// Pulse ring (expanding circle with fade)
///
/// Single ring that scales outward while fading.
/// Classic pulsing loader.
pub fn pulse_ring() -> LoadingAnimation {
    LoadingAnimation::PulseRing {
        duration_ms: 1500,
        easing: Easing::EaseInOutQuad,
        scale_from: 0.0,
        scale_to: 1.0,
        opacity_from: 1.0,
        opacity_to: 0.0,
    }
}

/// Shimmer gradient sweep
///
/// Skeleton loading shimmer that sweeps left to right.
/// 1.5s linear sweep for consistent speed.
pub fn shimmer() -> LoadingAnimation {
    LoadingAnimation::ShimmerSkeleton {
        duration_ms: 1500,
        easing: Easing::Linear,
        gradient_position_from: 200.0,
        gradient_position_to: -200.0,
    }
}

/// Fading dots in circle (iOS-style)
///
/// 8 dots arranged in a circle, fading in/out sequentially.
/// Creates rotating clock hand effect.
pub fn fading_dots_circle() -> LoadingAnimation {
    LoadingAnimation::FadingDots {
        duration_ms: 1200,
        easing: Easing::EaseInOutQuad,
        count: 8,
        stagger_delay_ms: 100,
        opacity_peak_percent: 40.0,
    }
}

/// iOS-style 12-segment spinner
///
/// Classic iOS loading spinner with 12 segments.
/// Segments fade in sequence creating rotation illusion.
pub fn ios_spinner() -> LoadingAnimation {
    LoadingAnimation::SegmentSpinner {
        duration_ms: 1200,
        easing: Easing::EaseInOutQuad,
        segment_count: 12,
    }
}

/// Determinate circular progress ring
///
/// SVG stroke-based progress ring for known completion percentage.
/// Updates with smooth easing when progress changes.
pub fn progress_ring_determinate() -> LoadingAnimation {
    LoadingAnimation::ProgressRing {
        duration_ms: 350,
        easing: Easing::EaseInOutQuad,
        radius: 52.0,
        stroke_width: 8.0,
    }
}

/// Determinate linear progress bar
///
/// Simple horizontal bar that fills to percentage.
/// Smooth width transition.
pub fn progress_bar_determinate() -> LoadingAnimation {
    LoadingAnimation::ProgressBar {
        duration_ms: 400,
        easing: Easing::EaseOutQuad,
    }
}

/// Skeleton pulse (subtle opacity fade)
///
/// Gentle pulsing opacity for skeleton placeholders.
/// 2s cycle matches skeleton shimmer patterns.
pub fn skeleton_pulse() -> LoadingAnimation {
    LoadingAnimation::ShimmerSkeleton {
        duration_ms: 2000,
        easing: Easing::EaseInOutQuad,
        gradient_position_from: 100.0,
        gradient_position_to: 0.0,
    }
}

/// Bouncing ball (single element)
///
/// Single element bouncing up and down with ease-in for gravity feel.
/// 600ms cycle.
pub fn bouncing_ball() -> LoadingAnimation {
    LoadingAnimation::BouncingBall {
        duration_ms: 600,
        easing: Easing::EaseInQuad,
        bounce_distance: 16.0,
    }
}

/// Multiple ripple rings
///
/// 4 concentric rings expanding outward in sequence.
/// Creates continuous ripple effect.
pub fn ripple_rings() -> LoadingAnimation {
    LoadingAnimation::RippleRings {
        duration_ms: 4000,
        easing: Easing::CubicBezier(0.455, 0.03, 0.515, 0.955),
        count: 4,
        stagger_delay_ms: 1000,
        scale_from: 0.33,
        scale_to: 1.0,
        opacity_from: 1.0,
        opacity_to: 0.0,
    }
}

/// Wave dots (vertical bounce with opacity)
///
/// 4 dots that bounce up/down with opacity fade.
/// Creates wave motion pattern.
pub fn wave_dots_vertical() -> LoadingAnimation {
    LoadingAnimation::WaveDots {
        duration_ms: 1400,
        easing: Easing::EaseInOutQuad,
        count: 4,
        stagger_delay_ms: 200,
        translate_y: -16.0,
        opacity_from: 0.9,
        opacity_to: 0.1,
    }
}

/// SVG path drawing loader
///
/// Draws an SVG path from start to finish, holds, then resets.
/// For logos and custom shapes.
pub fn path_draw(path_length: f64) -> LoadingAnimation {
    LoadingAnimation::PathDraw {
        duration_ms: 3000,
        easing: Easing::EaseInOutQuad,
        path_length,
        hold_duration_ms: 1000,
    }
}

/// Progress bar with shimmer overlay
///
/// Determinate progress bar with active shimmer sweep.
/// Indicates both progress and active processing.
pub fn progress_with_shimmer() -> LoadingAnimation {
    LoadingAnimation::ProgressWithShimmer {
        progress_duration_ms: 400,
        shimmer_duration_ms: 1500,
        progress_easing: Easing::EaseOutQuad,
        shimmer_easing: Easing::Linear,
    }
}

/// Simple circular spinner (basic rotation)
///
/// Single colored segment rotating at constant speed.
/// 2-second rotation.
pub fn basic_spinner() -> LoadingAnimation {
    LoadingAnimation::Spinner {
        duration_ms: 2000,
        easing: Easing::Linear,
    }
}

/// SpinKit-style chase
///
/// 6 dots in circle with pulsing scale.
/// Container rotates while dots pulse.
pub fn spinkit_chase() -> LoadingAnimation {
    LoadingAnimation::FadingDots {
        duration_ms: 2000,
        easing: Easing::EaseInOutQuad,
        count: 6,
        stagger_delay_ms: 166,
        opacity_peak_percent: 50.0,
    }
}

/// SpinKit-style double bounce
///
/// Two overlapping circles pulsing in sequence.
/// 2s cycle with 1s offset.
pub fn double_bounce() -> LoadingAnimation {
    LoadingAnimation::PulseDots {
        duration_ms: 2000,
        easing: Easing::EaseInOutQuad,
        count: 2,
        stagger_delay_ms: 1000,
        scale_from: 0.0,
        scale_to: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_circular() {
        let anim = material_circular();
        assert!(anim.is_infinite());
        assert_eq!(anim.duration_ms(), 2000);
    }

    #[test]
    fn test_material_linear() {
        let anim = material_linear();
        assert!(anim.is_infinite());
        assert_eq!(anim.duration_ms(), 2000);
    }

    #[test]
    fn test_three_bounce_dots() {
        let anim = three_bounce_dots();
        assert_eq!(anim.element_count(), 3);
        assert!(anim.is_infinite());
    }

    #[test]
    fn test_wave_bars() {
        let anim = wave_bars();
        assert_eq!(anim.element_count(), 5);
    }

    #[test]
    fn test_progress_ring_determinate() {
        let anim = progress_ring_determinate();
        assert!(!anim.is_infinite());
    }

    #[test]
    fn test_shimmer() {
        let anim = shimmer();
        assert!(anim.is_infinite());
        assert_eq!(anim.duration_ms(), 1500);
    }

    #[test]
    fn test_ios_spinner() {
        let anim = ios_spinner();
        assert_eq!(anim.element_count(), 12);
    }

    #[test]
    fn test_path_draw() {
        let anim = path_draw(500.0);
        assert!(anim.is_infinite());
    }
}
