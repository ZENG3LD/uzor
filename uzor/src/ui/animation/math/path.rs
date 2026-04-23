//! Motion along path animation — element position follows bezier curves
//!
//! Provides path construction from line/bezier segments, arc-length parameterization
//! for constant-speed motion, and tangent/angle calculation for auto-rotate.
//!
//! Based on GSAP MotionPathPlugin and AnimeJS path() function.
//!
//! # Example
//!
//! ```
//! use uzor::animation::{Point, MotionPath};
//!
//! let path = MotionPath::new(Point::new(0.0, 0.0))
//!     .line_to(Point::new(100.0, 0.0))
//!     .line_to(Point::new(100.0, 100.0))
//!     .build();
//!
//! // Evaluate at progress t (0.0..=1.0)
//! let sample = path.sample_at(0.5);
//! println!("Position: {:?}, Angle: {}", sample.position, sample.angle);
//! ```

use std::f64::consts::PI;

// ============================================================================
// Core Types
// ============================================================================

/// A 2D point in path space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Create a new point
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Distance between two points
    pub fn distance(&self, other: Point) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Normalize vector to unit length
    pub fn normalize(&self) -> Point {
        let len = (self.x * self.x + self.y * self.y).sqrt();
        if len < 1e-10 {
            Point::new(1.0, 0.0) // Default to horizontal
        } else {
            Point::new(self.x / len, self.y / len)
        }
    }

    /// Linear interpolation between two points
    pub fn lerp(a: Point, b: Point, t: f64) -> Point {
        Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }
}

/// A segment of a motion path
#[derive(Debug, Clone, Copy)]
pub enum PathSegment {
    /// Straight line to point
    LineTo(Point),
    /// Quadratic bezier (control_point, end_point)
    QuadTo(Point, Point),
    /// Cubic bezier (control1, control2, end_point)
    CubicTo(Point, Point, Point),
}

/// A motion path built from segments
///
/// The path is parameterized by arc length for constant-speed motion.
/// Call `build()` after adding segments to precompute cumulative lengths.
#[derive(Debug, Clone)]
pub struct MotionPath {
    /// Starting point
    start: Point,
    /// Path segments
    segments: Vec<PathSegment>,
    /// Precomputed cumulative arc lengths for each segment (for constant-speed parameterization)
    cumulative_lengths: Vec<f64>,
    /// Total path length
    total_length: f64,
}

/// A sample from a motion path at a specific progress value
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathSample {
    /// Position on path
    pub position: Point,
    /// Tangent direction (normalized)
    pub tangent: Point,
    /// Angle in radians (atan2(tangent.y, tangent.x))
    pub angle: f64,
}

// ============================================================================
// MotionPath Builder
// ============================================================================

impl MotionPath {
    /// Create a new motion path starting at the given point
    pub fn new(start: Point) -> Self {
        Self {
            start,
            segments: Vec::new(),
            cumulative_lengths: Vec::new(),
            total_length: 0.0,
        }
    }

    /// Add a line segment to the path
    pub fn line_to(mut self, point: Point) -> Self {
        self.segments.push(PathSegment::LineTo(point));
        self
    }

    /// Add a quadratic bezier segment
    pub fn quad_to(mut self, control: Point, end: Point) -> Self {
        self.segments.push(PathSegment::QuadTo(control, end));
        self
    }

    /// Add a cubic bezier segment
    pub fn cubic_to(mut self, c1: Point, c2: Point, end: Point) -> Self {
        self.segments.push(PathSegment::CubicTo(c1, c2, end));
        self
    }

    /// Build and precompute arc lengths for constant-speed parameterization
    ///
    /// Must be called after adding all segments and before evaluating.
    pub fn build(mut self) -> Self {
        self.precompute_arc_lengths();
        self
    }

    /// Total path length
    pub fn length(&self) -> f64 {
        self.total_length
    }

    /// Precompute cumulative arc lengths for each segment
    fn precompute_arc_lengths(&mut self) {
        self.cumulative_lengths.clear();
        let mut cumulative = 0.0;
        let mut prev_point = self.start;

        for segment in &self.segments {
            let segment_length = segment_arc_length(prev_point, *segment, 64);
            cumulative += segment_length;
            self.cumulative_lengths.push(cumulative);
            prev_point = segment_end_point(*segment);
        }

        self.total_length = cumulative;
    }
}

// ============================================================================
// Path Evaluation
// ============================================================================

impl MotionPath {
    /// Evaluate position at progress t (0.0..=1.0)
    ///
    /// Uses arc-length parameterization for constant speed.
    pub fn position_at(&self, t: f64) -> Point {
        self.sample_at(t).position
    }

    /// Evaluate tangent direction at progress t (normalized)
    ///
    /// Useful for auto-rotate (element faces direction of motion).
    pub fn tangent_at(&self, t: f64) -> Point {
        self.sample_at(t).tangent
    }

    /// Evaluate angle at progress t (in radians)
    ///
    /// Convenience for auto-rotate: atan2(tangent.y, tangent.x)
    pub fn angle_at(&self, t: f64) -> f64 {
        self.sample_at(t).angle
    }

    /// Get position, tangent, and angle at progress t (avoids double computation)
    pub fn sample_at(&self, t: f64) -> PathSample {
        let t = t.clamp(0.0, 1.0);

        if self.segments.is_empty() {
            return PathSample {
                position: self.start,
                tangent: Point::new(1.0, 0.0),
                angle: 0.0,
            };
        }

        if self.total_length < 1e-10 {
            // Degenerate path (all segments have zero length)
            return PathSample {
                position: self.start,
                tangent: Point::new(1.0, 0.0),
                angle: 0.0,
            };
        }

        // Convert progress to target distance along path
        let target_length = t * self.total_length;

        // Binary search to find which segment contains this distance
        let (segment_index, local_t) = self.find_segment_at_length(target_length);

        // Evaluate that segment at the local t
        let mut prev_point = self.start;
        for (i, segment) in self.segments.iter().enumerate() {
            if i == segment_index {
                let position = evaluate_segment_position(prev_point, *segment, local_t);
                let tangent = evaluate_segment_tangent(prev_point, *segment, local_t).normalize();
                let angle = tangent.y.atan2(tangent.x);

                return PathSample {
                    position,
                    tangent,
                    angle,
                };
            }
            prev_point = segment_end_point(*segment);
        }

        // Fallback (shouldn't reach here)
        PathSample {
            position: segment_end_point(self.segments[self.segments.len() - 1]),
            tangent: Point::new(1.0, 0.0),
            angle: 0.0,
        }
    }

    /// Binary search to find segment index and local t for a given arc length
    fn find_segment_at_length(&self, target_length: f64) -> (usize, f64) {
        if target_length <= 0.0 {
            return (0, 0.0);
        }

        if target_length >= self.total_length {
            let last_idx = self.segments.len().saturating_sub(1);
            return (last_idx, 1.0);
        }

        // Binary search for segment
        let segment_index = match self
            .cumulative_lengths
            .binary_search_by(|&len| len.partial_cmp(&target_length).unwrap())
        {
            Ok(exact) => exact,
            Err(insert_pos) => insert_pos,
        };

        // Compute local t within segment
        let segment_start_length = if segment_index == 0 {
            0.0
        } else {
            self.cumulative_lengths[segment_index - 1]
        };
        let segment_end_length = self.cumulative_lengths[segment_index];
        let segment_length = segment_end_length - segment_start_length;

        let local_distance = target_length - segment_start_length;
        let local_t = if segment_length < 1e-10 {
            0.5 // Degenerate segment
        } else {
            (local_distance / segment_length).clamp(0.0, 1.0)
        };

        (segment_index, local_t)
    }
}

// ============================================================================
// Convenience Constructors
// ============================================================================

impl MotionPath {
    /// Create a circular path
    ///
    /// # Arguments
    /// * `center` - Center of the circle
    /// * `radius` - Radius of the circle
    /// * `segments` - Number of cubic bezier segments (4 = good balance, 8 = very smooth)
    pub fn circle(center: Point, radius: f64, segments: u32) -> Self {
        let segments = segments.max(3); // Minimum 3 segments
        let angle_step = 2.0 * PI / segments as f64;

        // Magic number for cubic bezier approximation of circular arcs
        // k = 4/3 * tan(angle_step / 4) for optimal approximation
        let k = (4.0 / 3.0) * (angle_step / 4.0).tan();
        let control_distance = radius * k;

        let mut path = MotionPath::new(Point::new(center.x + radius, center.y));

        for i in 0..segments {
            let angle_start = i as f64 * angle_step;
            let angle_end = (i + 1) as f64 * angle_step;

            let x_start = center.x + radius * angle_start.cos();
            let y_start = center.y + radius * angle_start.sin();
            let x_end = center.x + radius * angle_end.cos();
            let y_end = center.y + radius * angle_end.sin();

            // Tangent vectors (perpendicular to radius)
            let tan_x_start = -angle_start.sin();
            let tan_y_start = angle_start.cos();
            let tan_x_end = -angle_end.sin();
            let tan_y_end = angle_end.cos();

            let c1 = Point::new(
                x_start + control_distance * tan_x_start,
                y_start + control_distance * tan_y_start,
            );
            let c2 = Point::new(
                x_end - control_distance * tan_x_end,
                y_end - control_distance * tan_y_end,
            );
            let end = Point::new(x_end, y_end);

            path = path.cubic_to(c1, c2, end);
        }

        path.build()
    }

    /// Create a path from a list of points (polyline with straight segments)
    pub fn from_points(points: &[Point]) -> Self {
        if points.is_empty() {
            return MotionPath::new(Point::new(0.0, 0.0)).build();
        }

        let mut path = MotionPath::new(points[0]);

        for &point in &points[1..] {
            path = path.line_to(point);
        }

        path.build()
    }
}

// ============================================================================
// Segment Evaluation Helpers
// ============================================================================

/// Get the end point of a segment
fn segment_end_point(segment: PathSegment) -> Point {
    match segment {
        PathSegment::LineTo(p) => p,
        PathSegment::QuadTo(_, p) => p,
        PathSegment::CubicTo(_, _, p) => p,
    }
}

/// Compute approximate arc length of a segment using subdivision
fn segment_arc_length(start: Point, segment: PathSegment, subdivisions: usize) -> f64 {
    let subdivisions = subdivisions.max(1);
    let step = 1.0 / subdivisions as f64;

    let mut length = 0.0;
    let mut prev = start;

    for i in 1..=subdivisions {
        let t = i as f64 * step;
        let current = evaluate_segment_position(start, segment, t);
        length += prev.distance(current);
        prev = current;
    }

    length
}

/// Evaluate position on a segment at parameter t
fn evaluate_segment_position(start: Point, segment: PathSegment, t: f64) -> Point {
    match segment {
        PathSegment::LineTo(end) => Point::lerp(start, end, t),
        PathSegment::QuadTo(control, end) => quad_bezier_point(start, control, end, t),
        PathSegment::CubicTo(c1, c2, end) => cubic_bezier_point(start, c1, c2, end, t),
    }
}

/// Evaluate tangent direction on a segment at parameter t (NOT normalized)
fn evaluate_segment_tangent(start: Point, segment: PathSegment, t: f64) -> Point {
    match segment {
        PathSegment::LineTo(end) => Point::new(end.x - start.x, end.y - start.y),
        PathSegment::QuadTo(control, end) => quad_bezier_tangent(start, control, end, t),
        PathSegment::CubicTo(c1, c2, end) => cubic_bezier_tangent(start, c1, c2, end, t),
    }
}

// ============================================================================
// Bezier Curve Math
// ============================================================================

/// Evaluate cubic bezier at parameter t
fn cubic_bezier_point(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    Point::new(
        mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
        mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
    )
}

/// Evaluate cubic bezier tangent at parameter t (derivative)
fn cubic_bezier_tangent(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let t2 = t * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;

    // Derivative of cubic bezier: 3(1-t)^2(P1-P0) + 6(1-t)t(P2-P1) + 3t^2(P3-P2)
    Point::new(
        3.0 * mt2 * (p1.x - p0.x) + 6.0 * mt * t * (p2.x - p1.x) + 3.0 * t2 * (p3.x - p2.x),
        3.0 * mt2 * (p1.y - p0.y) + 6.0 * mt * t * (p2.y - p1.y) + 3.0 * t2 * (p3.y - p2.y),
    )
}

/// Evaluate quadratic bezier at parameter t
fn quad_bezier_point(p0: Point, p1: Point, p2: Point, t: f64) -> Point {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;

    Point::new(
        mt2 * p0.x + 2.0 * mt * t * p1.x + t2 * p2.x,
        mt2 * p0.y + 2.0 * mt * t * p1.y + t2 * p2.y,
    )
}

/// Evaluate quadratic bezier tangent at parameter t (derivative)
fn quad_bezier_tangent(p0: Point, p1: Point, p2: Point, t: f64) -> Point {
    let mt = 1.0 - t;

    // Derivative of quadratic bezier: 2(1-t)(P1-P0) + 2t(P2-P1)
    Point::new(
        2.0 * mt * (p1.x - p0.x) + 2.0 * t * (p2.x - p1.x),
        2.0 * mt * (p1.y - p0.y) + 2.0 * t * (p2.y - p1.y),
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_path_position_at_start() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let pos = path.position_at(0.0);
        assert_eq!(pos, Point::new(0.0, 0.0));
    }

    #[test]
    fn test_line_path_position_at_end() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let pos = path.position_at(1.0);
        assert!((pos.x - 100.0).abs() < 0.01);
        assert!(pos.y.abs() < 0.01);
    }

    #[test]
    fn test_line_path_position_at_midpoint() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let pos = path.position_at(0.5);
        assert!((pos.x - 50.0).abs() < 0.5); // Arc-length parameterization may introduce small error
        assert!(pos.y.abs() < 0.01);
    }

    #[test]
    fn test_cubic_bezier_start_and_end() {
        let start = Point::new(0.0, 0.0);
        let c1 = Point::new(30.0, 50.0);
        let c2 = Point::new(70.0, 50.0);
        let end = Point::new(100.0, 0.0);

        let path = MotionPath::new(start).cubic_to(c1, c2, end).build();

        let pos_start = path.position_at(0.0);
        let pos_end = path.position_at(1.0);

        assert_eq!(pos_start, start);
        assert!((pos_end.x - end.x).abs() < 0.01);
        assert!((pos_end.y - end.y).abs() < 0.01);
    }

    #[test]
    fn test_tangent_horizontal_line() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let tangent = path.tangent_at(0.5);
        assert!((tangent.x - 1.0).abs() < 0.01);
        assert!(tangent.y.abs() < 0.01);
    }

    #[test]
    fn test_angle_horizontal_motion() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let angle = path.angle_at(0.5);
        assert!(angle.abs() < 0.01); // Should be ~0 radians
    }

    #[test]
    fn test_angle_vertical_motion() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(0.0, 100.0))
            .build();

        let angle = path.angle_at(0.5);
        assert!((angle - PI / 2.0).abs() < 0.01); // Should be ~π/2 radians
    }

    #[test]
    fn test_multi_segment_path_length() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(10.0, 0.0))
            .line_to(Point::new(10.0, 10.0))
            .line_to(Point::new(0.0, 10.0))
            .build();

        // Should be 30 (three sides of a square)
        assert!((path.length() - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_circle_path_radius() {
        let center = Point::new(50.0, 50.0);
        let radius = 20.0;
        let path = MotionPath::circle(center, radius, 8);

        // Sample points around the circle
        for i in 0..8 {
            let t = i as f64 / 8.0;
            let pos = path.position_at(t);
            let dist = pos.distance(center);
            // Distance from center should be approximately the radius
            assert!((dist - radius).abs() < 1.0); // 1.0 tolerance for bezier approximation
        }
    }

    #[test]
    fn test_from_points_creates_valid_path() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
        ];

        let path = MotionPath::from_points(&points);

        let pos_start = path.position_at(0.0);
        let pos_end = path.position_at(1.0);

        assert_eq!(pos_start, points[0]);
        assert!((pos_end.x - 10.0).abs() < 0.01);
        assert!((pos_end.y - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_path() {
        let path = MotionPath::new(Point::new(10.0, 20.0)).build();

        let sample = path.sample_at(0.5);
        assert_eq!(sample.position, Point::new(10.0, 20.0));
        assert_eq!(sample.tangent, Point::new(1.0, 0.0));
    }

    #[test]
    fn test_sample_at_returns_all_values() {
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(100.0, 0.0))
            .build();

        let sample = path.sample_at(0.5);
        assert!((sample.position.x - 50.0).abs() < 0.5);
        assert!((sample.tangent.x - 1.0).abs() < 0.01);
        assert!(sample.angle.abs() < 0.01);
    }

    #[test]
    fn test_arc_length_parameterization() {
        // Create L-shaped path: 10 units right, then 10 units up
        let path = MotionPath::new(Point::new(0.0, 0.0))
            .line_to(Point::new(10.0, 0.0))
            .line_to(Point::new(10.0, 10.0))
            .build();

        // Total length should be 20
        assert!((path.length() - 20.0).abs() < 0.1);

        // At t=0.5 (halfway by distance), should be at corner (10, 0)
        let pos = path.position_at(0.5);
        assert!((pos.x - 10.0).abs() < 0.5);
        assert!(pos.y.abs() < 0.5);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        assert_eq!(p1.distance(p2), 5.0);
    }

    #[test]
    fn test_point_normalize() {
        let p = Point::new(3.0, 4.0);
        let normalized = p.normalize();
        assert!((normalized.x - 0.6).abs() < 0.01);
        assert!((normalized.y - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_point_lerp() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 10.0);
        let mid = Point::lerp(a, b, 0.5);
        assert_eq!(mid, Point::new(5.0, 5.0));
    }

    #[test]
    fn test_quadratic_bezier() {
        let start = Point::new(0.0, 0.0);
        let control = Point::new(50.0, 100.0);
        let end = Point::new(100.0, 0.0);

        let path = MotionPath::new(start).quad_to(control, end).build();

        let pos_start = path.position_at(0.0);
        let pos_end = path.position_at(1.0);

        assert_eq!(pos_start, start);
        assert!((pos_end.x - end.x).abs() < 0.01);
        assert!((pos_end.y - end.y).abs() < 0.01);
    }
}
