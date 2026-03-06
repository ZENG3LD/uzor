//! Easing functions for animation curves
//!
//! Implements all 30 Robert Penner easing equations plus cubic-bezier solver.
//! All functions are zero-allocation and #[inline] for hot-path performance.

use std::f64::consts::PI;

/// Easing function variant
///
/// Includes all 30 Penner equations, cubic-bezier timing functions (CSS-compatible),
/// and step functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Linear interpolation (no easing)
    Linear,

    // Quadratic (t^2)
    /// Accelerating from zero velocity
    EaseInQuad,
    /// Decelerating to zero velocity
    EaseOutQuad,
    /// Acceleration until halfway, then deceleration
    EaseInOutQuad,

    // Cubic (t^3)
    /// Accelerating from zero velocity
    EaseInCubic,
    /// Decelerating to zero velocity
    EaseOutCubic,
    /// Acceleration until halfway, then deceleration
    EaseInOutCubic,

    // Quartic (t^4)
    /// Accelerating from zero velocity
    EaseInQuart,
    /// Decelerating to zero velocity
    EaseOutQuart,
    /// Acceleration until halfway, then deceleration
    EaseInOutQuart,

    // Quintic (t^5)
    /// Accelerating from zero velocity
    EaseInQuint,
    /// Decelerating to zero velocity
    EaseOutQuint,
    /// Acceleration until halfway, then deceleration
    EaseInOutQuint,

    // Sinusoidal
    /// Accelerating using sine wave
    EaseInSine,
    /// Decelerating using sine wave
    EaseOutSine,
    /// Accelerating/decelerating using sine wave
    EaseInOutSine,

    // Exponential
    /// Accelerating exponentially
    EaseInExpo,
    /// Decelerating exponentially
    EaseOutExpo,
    /// Accelerating/decelerating exponentially
    EaseInOutExpo,

    // Circular
    /// Accelerating using circular function
    EaseInCirc,
    /// Decelerating using circular function
    EaseOutCirc,
    /// Accelerating/decelerating using circular function
    EaseInOutCirc,

    // Back (overshoots)
    /// Back up before going forward
    EaseInBack,
    /// Overshoot at the end
    EaseOutBack,
    /// Back up and overshoot
    EaseInOutBack,

    // Elastic (spring-like)
    /// Elastic effect at start
    EaseInElastic,
    /// Elastic effect at end
    EaseOutElastic,
    /// Elastic effect at both ends
    EaseInOutElastic,

    // Bounce (like a ball)
    /// Bouncing at start
    EaseInBounce,
    /// Bouncing at end
    EaseOutBounce,
    /// Bouncing at both ends
    EaseInOutBounce,

    /// CSS cubic-bezier timing function (x1, y1, x2, y2)
    ///
    /// Control points define the curve shape. Start and end are always (0,0) and (1,1).
    /// Uses Newton-Raphson solver with bisection fallback for precise evaluation.
    CubicBezier(f64, f64, f64, f64),

    /// CSS steps() timing function
    ///
    /// Divides animation into N equal steps, jumping at start or end of each step.
    Steps(u32, StepPosition),
}

/// Position where step occurs in steps() function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPosition {
    /// Jump at start of each interval
    Start,
    /// Jump at end of each interval
    End,
}

impl Easing {
    /// Evaluate easing function at time t (0.0..=1.0)
    ///
    /// Returns the eased value. For most functions, output is in 0..1 range,
    /// but elastic and back functions may exceed this range due to overshoot.
    ///
    /// Input values outside 0..1 are clamped before evaluation.
    #[inline]
    pub fn ease(&self, t: f64) -> f64 {
        // Clamp input to valid range
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,

            // Quadratic
            Easing::EaseInQuad => ease_in_quad(t),
            Easing::EaseOutQuad => ease_out_quad(t),
            Easing::EaseInOutQuad => ease_in_out_quad(t),

            // Cubic
            Easing::EaseInCubic => ease_in_cubic(t),
            Easing::EaseOutCubic => ease_out_cubic(t),
            Easing::EaseInOutCubic => ease_in_out_cubic(t),

            // Quartic
            Easing::EaseInQuart => ease_in_quart(t),
            Easing::EaseOutQuart => ease_out_quart(t),
            Easing::EaseInOutQuart => ease_in_out_quart(t),

            // Quintic
            Easing::EaseInQuint => ease_in_quint(t),
            Easing::EaseOutQuint => ease_out_quint(t),
            Easing::EaseInOutQuint => ease_in_out_quint(t),

            // Sinusoidal
            Easing::EaseInSine => ease_in_sine(t),
            Easing::EaseOutSine => ease_out_sine(t),
            Easing::EaseInOutSine => ease_in_out_sine(t),

            // Exponential
            Easing::EaseInExpo => ease_in_expo(t),
            Easing::EaseOutExpo => ease_out_expo(t),
            Easing::EaseInOutExpo => ease_in_out_expo(t),

            // Circular
            Easing::EaseInCirc => ease_in_circ(t),
            Easing::EaseOutCirc => ease_out_circ(t),
            Easing::EaseInOutCirc => ease_in_out_circ(t),

            // Back
            Easing::EaseInBack => ease_in_back(t),
            Easing::EaseOutBack => ease_out_back(t),
            Easing::EaseInOutBack => ease_in_out_back(t),

            // Elastic
            Easing::EaseInElastic => ease_in_elastic(t),
            Easing::EaseOutElastic => ease_out_elastic(t),
            Easing::EaseInOutElastic => ease_in_out_elastic(t),

            // Bounce
            Easing::EaseInBounce => ease_in_bounce(t),
            Easing::EaseOutBounce => ease_out_bounce(t),
            Easing::EaseInOutBounce => ease_in_out_bounce(t),

            // Cubic Bezier
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier_solve(t, *x1, *y1, *x2, *y2),

            // Steps
            Easing::Steps(steps, position) => steps_fn(t, *steps, *position),
        }
    }

    /// Evaluate easing function at time t (f32 convenience wrapper)
    ///
    /// This is a convenience method for compatibility with existing code that uses f32.
    /// Internally converts to f64, evaluates, and converts back.
    #[inline]
    pub fn ease_f32(&self, t: f32) -> f32 {
        self.ease(t as f64) as f32
    }

    /// CSS `ease` — equivalent to cubic-bezier(0.25, 0.1, 0.25, 1.0)
    pub const EASE: Easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);

    /// CSS `ease-in` — equivalent to cubic-bezier(0.42, 0, 1.0, 1.0)
    pub const EASE_IN: Easing = Easing::CubicBezier(0.42, 0.0, 1.0, 1.0);

    /// CSS `ease-out` — equivalent to cubic-bezier(0, 0, 0.58, 1.0)
    pub const EASE_OUT: Easing = Easing::CubicBezier(0.0, 0.0, 0.58, 1.0);

    /// CSS `ease-in-out` — equivalent to cubic-bezier(0.42, 0, 0.58, 1.0)
    pub const EASE_IN_OUT: Easing = Easing::CubicBezier(0.42, 0.0, 0.58, 1.0);
}

impl Default for Easing {
    fn default() -> Self {
        Easing::Linear
    }
}

// ============================================================================
// Quadratic (t^2)
// ============================================================================

#[inline]
fn ease_in_quad(t: f64) -> f64 {
    t * t
}

#[inline]
fn ease_out_quad(t: f64) -> f64 {
    t * (2.0 - t)
}

#[inline]
fn ease_in_out_quad(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

// ============================================================================
// Cubic (t^3)
// ============================================================================

#[inline]
fn ease_in_cubic(t: f64) -> f64 {
    t * t * t
}

#[inline]
fn ease_out_cubic(t: f64) -> f64 {
    let f = t - 1.0;
    f * f * f + 1.0
}

#[inline]
fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = t - 1.0;
        let g = 2.0 * t - 2.0;
        f * g * g + 1.0
    }
}

// ============================================================================
// Quartic (t^4)
// ============================================================================

#[inline]
fn ease_in_quart(t: f64) -> f64 {
    t * t * t * t
}

#[inline]
fn ease_out_quart(t: f64) -> f64 {
    let f = t - 1.0;
    1.0 - f * f * f * f
}

#[inline]
fn ease_in_out_quart(t: f64) -> f64 {
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        let f = t - 1.0;
        1.0 - 8.0 * f * f * f * f
    }
}

// ============================================================================
// Quintic (t^5)
// ============================================================================

#[inline]
fn ease_in_quint(t: f64) -> f64 {
    t * t * t * t * t
}

#[inline]
fn ease_out_quint(t: f64) -> f64 {
    let f = t - 1.0;
    f * f * f * f * f + 1.0
}

#[inline]
fn ease_in_out_quint(t: f64) -> f64 {
    if t < 0.5 {
        16.0 * t * t * t * t * t
    } else {
        let f = t - 1.0;
        16.0 * f * f * f * f * f + 1.0
    }
}

// ============================================================================
// Sinusoidal
// ============================================================================

#[inline]
fn ease_in_sine(t: f64) -> f64 {
    1.0 - f64::cos(t * PI / 2.0)
}

#[inline]
fn ease_out_sine(t: f64) -> f64 {
    f64::sin(t * PI / 2.0)
}

#[inline]
fn ease_in_out_sine(t: f64) -> f64 {
    -(f64::cos(PI * t) - 1.0) / 2.0
}

// ============================================================================
// Exponential
// ============================================================================

#[inline]
fn ease_in_expo(t: f64) -> f64 {
    if t == 0.0 {
        0.0
    } else {
        f64::powf(2.0, 10.0 * (t - 1.0))
    }
}

#[inline]
fn ease_out_expo(t: f64) -> f64 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - f64::powf(2.0, -10.0 * t)
    }
}

#[inline]
fn ease_in_out_expo(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }

    if t < 0.5 {
        f64::powf(2.0, 20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - f64::powf(2.0, -20.0 * t + 10.0)) / 2.0
    }
}

// ============================================================================
// Circular
// ============================================================================

#[inline]
fn ease_in_circ(t: f64) -> f64 {
    1.0 - f64::sqrt(1.0 - t * t)
}

#[inline]
fn ease_out_circ(t: f64) -> f64 {
    f64::sqrt(1.0 - (t - 1.0) * (t - 1.0))
}

#[inline]
fn ease_in_out_circ(t: f64) -> f64 {
    if t < 0.5 {
        (1.0 - f64::sqrt(1.0 - 4.0 * t * t)) / 2.0
    } else {
        (f64::sqrt(1.0 - (-2.0 * t + 2.0) * (-2.0 * t + 2.0)) + 1.0) / 2.0
    }
}

// ============================================================================
// Back (overshoot)
// ============================================================================

const C1: f64 = 1.70158;
const C2: f64 = C1 * 1.525;
const C3: f64 = C1 + 1.0;

#[inline]
fn ease_in_back(t: f64) -> f64 {
    C3 * t * t * t - C1 * t * t
}

#[inline]
fn ease_out_back(t: f64) -> f64 {
    let f = t - 1.0;
    1.0 + C3 * f * f * f + C1 * f * f
}

#[inline]
fn ease_in_out_back(t: f64) -> f64 {
    if t < 0.5 {
        let x = 2.0 * t;
        (x * x * ((C2 + 1.0) * x - C2)) / 2.0
    } else {
        let x = 2.0 * t - 2.0;
        (x * x * ((C2 + 1.0) * x + C2) + 2.0) / 2.0
    }
}

// ============================================================================
// Elastic (spring-like)
// ============================================================================

const C4: f64 = (2.0 * PI) / 3.0;
const C5: f64 = (2.0 * PI) / 4.5;

#[inline]
fn ease_in_elastic(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }

    -f64::powf(2.0, 10.0 * t - 10.0) * f64::sin((t * 10.0 - 10.75) * C4)
}

#[inline]
fn ease_out_elastic(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }

    f64::powf(2.0, -10.0 * t) * f64::sin((t * 10.0 - 0.75) * C4) + 1.0
}

#[inline]
fn ease_in_out_elastic(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }

    if t < 0.5 {
        -(f64::powf(2.0, 20.0 * t - 10.0) * f64::sin((20.0 * t - 11.125) * C5)) / 2.0
    } else {
        (f64::powf(2.0, -20.0 * t + 10.0) * f64::sin((20.0 * t - 11.125) * C5)) / 2.0 + 1.0
    }
}

// ============================================================================
// Bounce (bouncing ball)
// ============================================================================

const N1: f64 = 7.5625;
const D1: f64 = 2.75;

#[inline]
fn ease_out_bounce(t: f64) -> f64 {
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

#[inline]
fn ease_in_bounce(t: f64) -> f64 {
    1.0 - ease_out_bounce(1.0 - t)
}

#[inline]
fn ease_in_out_bounce(t: f64) -> f64 {
    if t < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
    }
}

// ============================================================================
// Cubic Bezier Solver
// ============================================================================

/// Evaluate cubic bezier curve at parameter t
///
/// For cubic bezier with control points p1 and p2 (start=0, end=1):
/// B(t) = 3*(1-t)^2*t*p1 + 3*(1-t)*t^2*p2 + t^3
#[inline]
fn calc_bezier(t: f64, p1: f64, p2: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
}

/// Get slope (derivative) of cubic bezier at parameter t
///
/// dB/dt = 3*(1-t)^2*p1 + 6*(1-t)*t*(p2-p1) + 3*t^2*(1-p2)
#[inline]
fn get_bezier_slope(t: f64, p1: f64, p2: f64) -> f64 {
    let mt = 1.0 - t;
    3.0 * mt * mt * p1 + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

/// Solve cubic bezier for x using Newton-Raphson method
///
/// Given x (input time), find parameter t where Bx(t) = x.
/// Uses up to 8 iterations with early exit on convergence.
#[inline]
fn newton_raphson_iterate(x: f64, guess_t: f64, x1: f64, x2: f64) -> f64 {
    const MAX_ITERATIONS: u32 = 8;
    const NEWTON_MIN_SLOPE: f64 = 0.001;

    let mut t = guess_t;

    for _ in 0..MAX_ITERATIONS {
        let slope = get_bezier_slope(t, x1, x2);

        // If slope is too small, Newton-Raphson becomes unstable
        if slope.abs() < NEWTON_MIN_SLOPE {
            return t;
        }

        let current_x = calc_bezier(t, x1, x2) - x;

        // Early exit if converged
        if current_x.abs() < 0.0000001 {
            return t;
        }

        t -= current_x / slope;
    }

    t
}

/// Solve cubic bezier using bisection method (fallback)
///
/// Binary search to find t where Bx(t) = x.
/// Guaranteed to converge but slower than Newton-Raphson.
#[inline]
fn binary_subdivide(x: f64, mut a: f64, mut b: f64, x1: f64, x2: f64) -> f64 {
    const SUBDIVISION_PRECISION: f64 = 0.0000001;
    const MAX_ITERATIONS: u32 = 10;

    let mut current_t;
    let mut current_x;

    for _ in 0..MAX_ITERATIONS {
        current_t = a + (b - a) / 2.0;
        current_x = calc_bezier(current_t, x1, x2) - x;

        if current_x.abs() <= SUBDIVISION_PRECISION {
            return current_t;
        }

        if current_x > 0.0 {
            b = current_t;
        } else {
            a = current_t;
        }
    }

    a + (b - a) / 2.0
}

/// Solve cubic bezier curve for given x value
///
/// Implements CSS cubic-bezier() timing function.
/// Uses sample table + Newton-Raphson with bisection fallback.
///
/// Based on Firefox/Chrome implementations.
#[inline]
fn cubic_bezier_solve(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    const SAMPLE_SIZE: usize = 11;
    const SAMPLE_STEP_SIZE: f64 = 1.0 / (SAMPLE_SIZE as f64 - 1.0);

    // Edge cases
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }

    // Build sample table (in practice, this should be precomputed)
    let mut samples = [0.0; SAMPLE_SIZE];
    for i in 0..SAMPLE_SIZE {
        samples[i] = calc_bezier(i as f64 * SAMPLE_STEP_SIZE, x1, x2);
    }

    // Find interval containing x
    let mut interval_start = 0.0;
    let mut current_sample = 1;

    while current_sample < SAMPLE_SIZE && samples[current_sample] <= x {
        interval_start += SAMPLE_STEP_SIZE;
        current_sample += 1;
    }

    current_sample -= 1;

    // Calculate initial guess from linear interpolation
    let dist = (x - samples[current_sample])
        / (samples[current_sample + 1] - samples[current_sample]);
    let guess_t = interval_start + dist * SAMPLE_STEP_SIZE;

    // Refine using Newton-Raphson
    let initial_slope = get_bezier_slope(guess_t, x1, x2);

    // Use Newton-Raphson if slope is sufficient, otherwise bisection
    let t = if initial_slope >= 0.001 {
        newton_raphson_iterate(x, guess_t, x1, x2)
    } else {
        binary_subdivide(
            x,
            interval_start,
            interval_start + SAMPLE_STEP_SIZE,
            x1,
            x2,
        )
    };

    // Now compute y from solved t
    calc_bezier(t, y1, y2)
}

// ============================================================================
// Steps Function
// ============================================================================

/// CSS steps() timing function
///
/// Divides animation into N equal steps, jumping at start or end of each interval.
#[inline]
fn steps_fn(t: f64, steps: u32, position: StepPosition) -> f64 {
    let steps = steps.max(1) as f64;

    let step = match position {
        StepPosition::Start => f64::ceil(t * steps) / steps,
        StepPosition::End => f64::floor(t * steps) / steps,
    };

    step.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert_eq!(Easing::Linear.ease(0.0), 0.0);
        assert_eq!(Easing::Linear.ease(0.5), 0.5);
        assert_eq!(Easing::Linear.ease(1.0), 1.0);
    }

    #[test]
    fn test_ease_in_quad() {
        assert_eq!(Easing::EaseInQuad.ease(0.0), 0.0);
        assert_eq!(Easing::EaseInQuad.ease(0.5), 0.25);
        assert_eq!(Easing::EaseInQuad.ease(1.0), 1.0);
    }

    #[test]
    fn test_ease_out_quad() {
        assert_eq!(Easing::EaseOutQuad.ease(0.0), 0.0);
        assert_eq!(Easing::EaseOutQuad.ease(0.5), 0.75);
        assert_eq!(Easing::EaseOutQuad.ease(1.0), 1.0);
    }

    #[test]
    fn test_cubic_bezier_linear() {
        let bezier = Easing::CubicBezier(0.0, 0.0, 1.0, 1.0);
        assert!((bezier.ease(0.0) - 0.0).abs() < 0.001);
        assert!((bezier.ease(0.5) - 0.5).abs() < 0.001);
        assert!((bezier.ease(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_steps() {
        let steps = Easing::Steps(4, StepPosition::End);
        assert_eq!(steps.ease(0.0), 0.0);
        assert_eq!(steps.ease(0.24), 0.0);
        assert_eq!(steps.ease(0.25), 0.25);
        assert_eq!(steps.ease(0.5), 0.5);
        assert_eq!(steps.ease(0.75), 0.75);
        assert_eq!(steps.ease(1.0), 1.0);
    }

    #[test]
    fn test_bounce_endpoints() {
        assert_eq!(Easing::EaseOutBounce.ease(0.0), 0.0);
        assert!((Easing::EaseOutBounce.ease(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_elastic_endpoints() {
        assert_eq!(Easing::EaseOutElastic.ease(0.0), 0.0);
        assert_eq!(Easing::EaseOutElastic.ease(1.0), 1.0);
    }

    #[test]
    fn test_back_overshoot() {
        // Back easing should overshoot past 1.0
        let val = Easing::EaseOutBack.ease(0.9);
        assert!(val > 1.0);
    }

    #[test]
    fn test_css_constants() {
        // Just verify they compile and don't panic
        let _ = Easing::EASE.ease(0.5);
        let _ = Easing::EASE_IN.ease(0.5);
        let _ = Easing::EASE_OUT.ease(0.5);
        let _ = Easing::EASE_IN_OUT.ease(0.5);
    }
}
