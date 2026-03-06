//! Default parameters for loading animations

use crate::Easing;

/// Default parameters for spinner animations
pub struct SpinnerDefaults;

impl SpinnerDefaults {
    /// Standard rotation duration (2s)
    pub const DURATION_MS: u64 = 2000;

    /// Linear easing for constant rotation speed
    pub const EASING: Easing = Easing::Linear;
}

/// Default parameters for pulse dot animations
pub struct PulseDotsDefaults;

impl PulseDotsDefaults {
    /// Animation cycle duration
    pub const DURATION_MS: u64 = 1200;

    /// Number of dots
    pub const COUNT: usize = 3;

    /// Delay between dot animations
    pub const STAGGER_DELAY_MS: u64 = 160;

    /// Starting scale (invisible)
    pub const SCALE_FROM: f64 = 0.0;

    /// Peak scale (full size)
    pub const SCALE_TO: f64 = 1.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for bar wave animations
pub struct BarWaveDefaults;

impl BarWaveDefaults {
    /// Animation cycle duration
    pub const DURATION_MS: u64 = 1200;

    /// Number of bars
    pub const COUNT: usize = 5;

    /// Delay between bars
    pub const STAGGER_DELAY_MS: u64 = 100;

    /// Minimum bar height (40% of full)
    pub const SCALE_FROM: f64 = 0.4;

    /// Maximum bar height (100%)
    pub const SCALE_TO: f64 = 1.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for progress ring (determinate)
pub struct ProgressRingDefaults;

impl ProgressRingDefaults {
    /// Update transition duration
    pub const DURATION_MS: u64 = 350;

    /// Ring radius (pixels)
    pub const RADIUS: f64 = 52.0;

    /// Stroke width (pixels)
    pub const STROKE_WIDTH: f64 = 8.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for progress bar (determinate)
pub struct ProgressBarDefaults;

impl ProgressBarDefaults {
    /// Update transition duration
    pub const DURATION_MS: u64 = 400;

    /// Deceleration easing (feels natural)
    pub const EASING: Easing = Easing::EaseOutQuad;
}

/// Default parameters for indeterminate bar
pub struct IndeterminateBarDefaults;

impl IndeterminateBarDefaults {
    /// Full cycle duration (Material spec: 2s)
    pub const DURATION_MS: u64 = 2000;

    /// Starting scale (30% width)
    pub const SCALE_X_FROM: f64 = 0.3;

    /// Mid-point scale (100% width)
    pub const SCALE_X_MID: f64 = 1.0;

    /// Ending scale (30% width)
    pub const SCALE_X_END: f64 = 0.3;

    /// Material Design cubic-bezier
    pub const EASING: Easing = Easing::CubicBezier(0.65, 0.815, 0.735, 0.395);
}

/// Default parameters for shimmer skeleton
pub struct ShimmerDefaults;

impl ShimmerDefaults {
    /// Sweep duration
    pub const DURATION_MS: u64 = 1500;

    /// Starting gradient position (200% offscreen right)
    pub const POSITION_FROM: f64 = 200.0;

    /// Ending gradient position (-200% offscreen left)
    pub const POSITION_TO: f64 = -200.0;

    /// Linear easing for constant sweep speed
    pub const EASING: Easing = Easing::Linear;
}

/// Default parameters for fading dots
pub struct FadingDotsDefaults;

impl FadingDotsDefaults {
    /// Animation cycle duration
    pub const DURATION_MS: u64 = 1200;

    /// Number of dots
    pub const COUNT: usize = 8;

    /// Delay between dot fades
    pub const STAGGER_DELAY_MS: u64 = 100;

    /// Percentage of cycle where opacity peaks (40%)
    pub const OPACITY_PEAK_PERCENT: f64 = 40.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for spinning arc
pub struct SpinningArcDefaults;

impl SpinningArcDefaults {
    /// Container rotation duration
    pub const ROTATION_DURATION_MS: u64 = 2000;

    /// Arc animation duration
    pub const ARC_DURATION_MS: u64 = 1500;

    /// Minimum arc length (degrees)
    pub const DASH_ARRAY_MIN: f64 = 1.0;

    /// Maximum arc length (degrees)
    pub const DASH_ARRAY_MAX: f64 = 90.0;

    /// Path circumference
    pub const PATH_LENGTH: f64 = 150.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutCubic;
}

/// Default parameters for bouncing ball
pub struct BouncingBallDefaults;

impl BouncingBallDefaults {
    /// Bounce cycle duration
    pub const DURATION_MS: u64 = 600;

    /// Distance to bounce (pixels)
    pub const BOUNCE_DISTANCE: f64 = 16.0;

    /// Ease-in for gravity feel
    pub const EASING: Easing = Easing::EaseInQuad;
}

/// Default parameters for ripple rings
pub struct RippleRingsDefaults;

impl RippleRingsDefaults {
    /// Full animation duration
    pub const DURATION_MS: u64 = 4000;

    /// Number of rings
    pub const COUNT: usize = 4;

    /// Delay between ring expansions
    pub const STAGGER_DELAY_MS: u64 = 1000;

    /// Starting scale (33%)
    pub const SCALE_FROM: f64 = 0.33;

    /// Ending scale (100%)
    pub const SCALE_TO: f64 = 1.0;

    /// Starting opacity (solid)
    pub const OPACITY_FROM: f64 = 1.0;

    /// Ending opacity (transparent)
    pub const OPACITY_TO: f64 = 0.0;

    /// Custom cubic-bezier for natural expansion
    pub const EASING: Easing = Easing::CubicBezier(0.455, 0.03, 0.515, 0.955);
}

/// Default parameters for wave dots
pub struct WaveDotsDefaults;

impl WaveDotsDefaults {
    /// Animation cycle duration
    pub const DURATION_MS: u64 = 1400;

    /// Number of dots
    pub const COUNT: usize = 4;

    /// Delay between dot movements
    pub const STAGGER_DELAY_MS: u64 = 200;

    /// Vertical movement distance (pixels, negative = up)
    pub const TRANSLATE_Y: f64 = -16.0;

    /// Starting opacity
    pub const OPACITY_FROM: f64 = 0.9;

    /// Ending opacity (at peak of wave)
    pub const OPACITY_TO: f64 = 0.1;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for path draw
pub struct PathDrawDefaults;

impl PathDrawDefaults {
    /// Drawing duration
    pub const DURATION_MS: u64 = 3000;

    /// Hold at full draw before reset
    pub const HOLD_DURATION_MS: u64 = 1000;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for segment spinner
pub struct SegmentSpinnerDefaults;

impl SegmentSpinnerDefaults {
    /// Animation cycle duration
    pub const DURATION_MS: u64 = 1200;

    /// Number of segments (iOS uses 12)
    pub const SEGMENT_COUNT: usize = 12;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for pulse ring
pub struct PulseRingDefaults;

impl PulseRingDefaults {
    /// Pulse cycle duration
    pub const DURATION_MS: u64 = 1500;

    /// Starting scale (invisible)
    pub const SCALE_FROM: f64 = 0.0;

    /// Ending scale (full size)
    pub const SCALE_TO: f64 = 1.0;

    /// Starting opacity (solid)
    pub const OPACITY_FROM: f64 = 1.0;

    /// Ending opacity (transparent)
    pub const OPACITY_TO: f64 = 0.0;

    /// Smooth in-out easing
    pub const EASING: Easing = Easing::EaseInOutQuad;
}

/// Default parameters for progress with shimmer
pub struct ProgressWithShimmerDefaults;

impl ProgressWithShimmerDefaults {
    /// Progress update transition
    pub const PROGRESS_DURATION_MS: u64 = 400;

    /// Shimmer sweep duration
    pub const SHIMMER_DURATION_MS: u64 = 1500;

    /// Progress easing
    pub const PROGRESS_EASING: Easing = Easing::EaseOutQuad;

    /// Shimmer easing (constant speed)
    pub const SHIMMER_EASING: Easing = Easing::Linear;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_exist() {
        // Just verify constants are accessible
        assert_eq!(SpinnerDefaults::DURATION_MS, 2000);
        assert_eq!(PulseDotsDefaults::COUNT, 3);
        assert_eq!(BarWaveDefaults::COUNT, 5);
        assert_eq!(ProgressRingDefaults::RADIUS, 52.0);
        assert_eq!(ShimmerDefaults::DURATION_MS, 1500);
        assert_eq!(FadingDotsDefaults::COUNT, 8);
        assert_eq!(SegmentSpinnerDefaults::SEGMENT_COUNT, 12);
    }
}
