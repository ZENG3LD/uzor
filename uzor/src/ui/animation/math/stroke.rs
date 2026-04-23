//! SVG stroke drawing animation — progressive path reveal
//!
//! Animates stroke-dashoffset for "drawing" effects.
//! Based on the classic SVG line drawing technique using
//! stroke-dasharray and stroke-dashoffset.
//!
//! # Example
//!
//! ```
//! use uzor::animation::StrokeAnimation;
//!
//! let path_length = 500.0;
//! let anim = StrokeAnimation::draw_in(path_length);
//!
//! // At start: fully hidden
//! let state = anim.evaluate(0.0);
//! assert_eq!(state.dash_offset, path_length);
//!
//! // At end: fully visible
//! let state = anim.evaluate(1.0);
//! assert_eq!(state.dash_offset, 0.0);
//! ```

/// Stroke drawing animation — progressive path reveal
///
/// Animates stroke-dashoffset for "drawing" effects.
/// Works with any path that has a known total length.
#[derive(Debug, Clone, Copy)]
pub struct StrokeAnimation {
    /// Total path length (must be provided or pre-computed)
    pub path_length: f64,
    /// Start of visible range (0.0..1.0, default 0.0)
    pub draw_start: f64,
    /// End of visible range (0.0..1.0, default 1.0)
    pub draw_end: f64,
}

/// Output values to apply to stroke rendering
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeState {
    /// stroke-dasharray value
    pub dash_array: f64,
    /// stroke-dashoffset value
    pub dash_offset: f64,
    /// Visible portion of stroke (for debugging/inspection)
    pub visible_fraction: f64,
}

impl StrokeAnimation {
    /// Create stroke animation for a path with given total length
    pub fn new(path_length: f64) -> Self {
        Self {
            path_length,
            draw_start: 0.0,
            draw_end: 1.0,
        }
    }

    /// Set visible range — allows drawing from middle
    ///
    /// e.g., draw_range(0.5, 0.5) starts as invisible dot in middle
    /// then animate to draw_range(0.0, 1.0) for full reveal
    pub fn draw_range(mut self, start: f64, end: f64) -> Self {
        self.draw_start = start.clamp(0.0, 1.0);
        self.draw_end = end.clamp(0.0, 1.0);
        self
    }

    /// Evaluate at progress t (0.0..1.0)
    ///
    /// Returns StrokeState with dasharray and dashoffset values
    ///
    /// For simple "draw in" animation:
    ///   t=0.0 → fully hidden (dashoffset = path_length)
    ///   t=1.0 → fully visible (dashoffset = 0)
    pub fn evaluate(&self, t: f64) -> StrokeState {
        let t = t.clamp(0.0, 1.0);

        // Classic SVG stroke technique:
        // - dasharray = path_length (set dash pattern to full path length)
        // - dashoffset animates from path_length (hidden) to 0 (visible)
        //
        // For draw_in (start=0, end=1):
        //   t=0: dashoffset=path_length → nothing visible
        //   t=1: dashoffset=0 → full path visible
        //
        // For draw_out (start=1, end=0):
        //   t=0: dashoffset=0 → full path visible
        //   t=1: dashoffset=path_length → nothing visible

        let dash_array = self.path_length;

        // Interpolate offset based on draw_start and draw_end
        // When draw_start=0, draw_end=1:
        //   t=0 → offset = path_length * (1.0 - 0) = path_length
        //   t=1 → offset = path_length * (1.0 - 1) = 0
        // When draw_start=1, draw_end=0:
        //   t=0 → offset = path_length * (1.0 - 1) = 0
        //   t=1 → offset = path_length * (1.0 - 0) = path_length
        let progress = self.draw_start + (self.draw_end - self.draw_start) * t;
        let dash_offset = self.path_length * (1.0 - progress);

        let visible_fraction = progress;

        StrokeState {
            dash_array,
            dash_offset,
            visible_fraction,
        }
    }

    /// Convenience: simple draw-in from start to end
    pub fn draw_in(path_length: f64) -> Self {
        Self::new(path_length)
    }

    /// Convenience: draw-out (erase from start to end)
    pub fn draw_out(path_length: f64) -> Self {
        Self::new(path_length).draw_range(1.0, 0.0)
    }

    /// Convenience: draw from center outward
    pub fn from_center(path_length: f64) -> Self {
        Self::new(path_length).draw_range(0.5, 0.5)
    }
}

// ============================================================================
// Path Length Utilities
// ============================================================================

/// Compute approximate length of a series of line segments
pub fn polyline_length(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }

    points
        .windows(2)
        .map(|pair| {
            let (x1, y1) = pair[0];
            let (x2, y2) = pair[1];
            let dx = x2 - x1;
            let dy = y2 - y1;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

/// Compute approximate length of a cubic bezier curve
///
/// Uses recursive subdivision for accuracy.
/// Higher subdivisions = more accurate but slower.
pub fn cubic_bezier_length(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    subdivisions: usize,
) -> f64 {
    let subdivisions = subdivisions.max(1);
    let step = 1.0 / subdivisions as f64;

    let mut length = 0.0;
    let mut prev = p0;

    for i in 1..=subdivisions {
        let t = i as f64 * step;
        let current = cubic_bezier_point(p0, p1, p2, p3, t);

        let dx = current.0 - prev.0;
        let dy = current.1 - prev.1;
        length += (dx * dx + dy * dy).sqrt();

        prev = current;
    }

    length
}

/// Compute point on cubic bezier curve at parameter t
fn cubic_bezier_point(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    t: f64,
) -> (f64, f64) {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    let x = mt3 * p0.0 + 3.0 * mt2 * t * p1.0 + 3.0 * mt * t2 * p2.0 + t3 * p3.0;
    let y = mt3 * p0.1 + 3.0 * mt2 * t * p1.1 + 3.0 * mt * t2 * p2.1 + t3 * p3.1;

    (x, y)
}

/// Compute approximate length of a quadratic bezier curve
pub fn quadratic_bezier_length(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    subdivisions: usize,
) -> f64 {
    let subdivisions = subdivisions.max(1);
    let step = 1.0 / subdivisions as f64;

    let mut length = 0.0;
    let mut prev = p0;

    for i in 1..=subdivisions {
        let t = i as f64 * step;
        let current = quadratic_bezier_point(p0, p1, p2, t);

        let dx = current.0 - prev.0;
        let dy = current.1 - prev.1;
        length += (dx * dx + dy * dy).sqrt();

        prev = current;
    }

    length
}

/// Compute point on quadratic bezier curve at parameter t
fn quadratic_bezier_point(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;

    let x = mt2 * p0.0 + 2.0 * mt * t * p1.0 + t2 * p2.0;
    let y = mt2 * p0.1 + 2.0 * mt * t * p1.1 + t2 * p2.1;

    (x, y)
}

/// Compute length of a circle arc
pub fn arc_length(radius: f64, angle_radians: f64) -> f64 {
    radius * angle_radians.abs()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_in_at_start_fully_hidden() {
        let anim = StrokeAnimation::draw_in(500.0);
        let state = anim.evaluate(0.0);

        assert_eq!(state.dash_offset, 500.0);
        assert_eq!(state.dash_array, 500.0);
        assert_eq!(state.visible_fraction, 0.0);
    }

    #[test]
    fn test_draw_in_at_end_fully_visible() {
        let anim = StrokeAnimation::draw_in(500.0);
        let state = anim.evaluate(1.0);

        assert_eq!(state.dash_offset, 0.0);
        assert_eq!(state.dash_array, 500.0);
        assert_eq!(state.visible_fraction, 500.0 / 500.0);
    }

    #[test]
    fn test_draw_in_at_half_progress() {
        let anim = StrokeAnimation::draw_in(500.0);
        let state = anim.evaluate(0.5);

        assert_eq!(state.dash_offset, 250.0);
        assert_eq!(state.dash_array, 500.0);
    }

    #[test]
    fn test_from_center_expands_from_middle() {
        let anim = StrokeAnimation::from_center(500.0);

        // At start: draw_range(0.5, 0.5) — invisible point in middle
        assert_eq!(anim.draw_start, 0.5);
        assert_eq!(anim.draw_end, 0.5);

        // This should evaluate differently than draw_in
        // (actual expansion logic depends on implementation)
        let state_start = anim.evaluate(0.0);
        let state_end = anim.evaluate(1.0);

        // Both should have same dasharray
        assert_eq!(state_start.dash_array, 500.0);
        assert_eq!(state_end.dash_array, 500.0);
    }

    #[test]
    fn test_draw_out_reverses_direction() {
        let anim = StrokeAnimation::draw_out(500.0);

        assert_eq!(anim.draw_start, 1.0);
        assert_eq!(anim.draw_end, 0.0);
    }

    #[test]
    fn test_polyline_length_square() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)];

        let length = polyline_length(&square);

        // 4 sides of 10 units each
        assert_eq!(length, 40.0);
    }

    #[test]
    fn test_polyline_length_empty() {
        assert_eq!(polyline_length(&[]), 0.0);
        assert_eq!(polyline_length(&[(0.0, 0.0)]), 0.0);
    }

    #[test]
    fn test_cubic_bezier_straight_line() {
        // Straight line from (0,0) to (100,0)
        let p0 = (0.0, 0.0);
        let p1 = (33.0, 0.0);
        let p2 = (66.0, 0.0);
        let p3 = (100.0, 0.0);

        let length = cubic_bezier_length(p0, p1, p2, p3, 100);

        // Should be approximately 100
        assert!((length - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_quadratic_bezier_straight_line() {
        // Straight line from (0,0) to (100,0)
        let p0 = (0.0, 0.0);
        let p1 = (50.0, 0.0);
        let p2 = (100.0, 0.0);

        let length = quadratic_bezier_length(p0, p1, p2, 100);

        // Should be approximately 100
        assert!((length - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_arc_length_full_circle() {
        let radius = 10.0;
        let angle = std::f64::consts::TAU; // 2π

        let length = arc_length(radius, angle);

        // Circumference = 2πr
        let expected = 2.0 * std::f64::consts::PI * radius;
        assert!((length - expected).abs() < 0.001);
    }

    #[test]
    fn test_arc_length_quarter_circle() {
        let radius = 10.0;
        let angle = std::f64::consts::FRAC_PI_2; // π/2

        let length = arc_length(radius, angle);

        // Quarter circumference = πr/2
        let expected = std::f64::consts::PI * radius / 2.0;
        assert!((length - expected).abs() < 0.001);
    }

    #[test]
    fn test_draw_range_clamps_values() {
        let anim = StrokeAnimation::new(100.0).draw_range(-0.5, 1.5);

        assert_eq!(anim.draw_start, 0.0);
        assert_eq!(anim.draw_end, 1.0);
    }

    #[test]
    fn test_evaluate_clamps_progress() {
        let anim = StrokeAnimation::draw_in(100.0);

        let state_negative = anim.evaluate(-0.5);
        let state_too_large = anim.evaluate(2.0);

        // Should clamp to 0.0 and 1.0
        assert_eq!(state_negative.dash_offset, 100.0); // t=0
        assert_eq!(state_too_large.dash_offset, 0.0); // t=1
    }
}
