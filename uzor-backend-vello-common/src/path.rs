//! Canvas2D-style path builder backed by `kurbo::BezPath`.
//!
//! [`PathBuilder`] accumulates path commands and produces a `kurbo::BezPath`
//! that can be fed directly to vello scene calls.

use vello::kurbo::{self, BezPath, Shape};

/// A mutable path builder that mirrors the Canvas2D path API.
///
/// Start a new path with [`PathBuilder::new`] (or call [`PathBuilder::begin_path`]
/// on an existing one), add primitives with `move_to`, `line_to`, etc., and
/// then take the finished path with [`PathBuilder::take`].
#[derive(Default)]
pub struct PathBuilder {
    /// The path currently being built.  `None` means no path has been started
    /// yet (i.e. before the first [`begin_path`](PathBuilder::begin_path) call).
    pub path: Option<BezPath>,
}

impl PathBuilder {
    /// Create a new `PathBuilder` with no active path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a fresh path, discarding any previous path.
    pub fn begin_path(&mut self) {
        self.path = Some(BezPath::new());
    }

    /// Move the current point without drawing.
    pub fn move_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.move_to(kurbo::Point::new(x, y));
        }
    }

    /// Draw a straight line from the current point to `(x, y)`.
    pub fn line_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.line_to(kurbo::Point::new(x, y));
        }
    }

    /// Close the current sub-path by drawing a line back to its start.
    pub fn close_path(&mut self) {
        if let Some(ref mut p) = self.path {
            p.close_path();
        }
    }

    /// Append an axis-aligned rectangle sub-path.
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(ref mut p) = self.path {
            p.move_to(kurbo::Point::new(x, y));
            p.line_to(kurbo::Point::new(x + w, y));
            p.line_to(kurbo::Point::new(x + w, y + h));
            p.line_to(kurbo::Point::new(x, y + h));
            p.close_path();
        }
    }

    /// Append a circular arc.
    ///
    /// Mirrors the Canvas2D `arc(cx, cy, radius, startAngle, endAngle)`
    /// signature.  Angles are in radians, measured clockwise from the positive
    /// x-axis.
    ///
    /// If the path already has elements the arc's first `MoveTo` is replaced
    /// by a `LineTo` so the arc is connected to the previous sub-path (Canvas2D
    /// behaviour).
    pub fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(radius, radius),
                start_angle,
                end_angle - start_angle,
                0.0,
            );
            let path_has_elements = !p.elements().is_empty();
            let mut is_first = true;
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt) => {
                    if is_first && path_has_elements {
                        p.line_to(pt);
                    } else {
                        p.move_to(pt);
                    }
                    is_first = false;
                }
                kurbo::PathEl::LineTo(pt) => {
                    p.line_to(pt);
                    is_first = false;
                }
                kurbo::PathEl::QuadTo(c, pt) => {
                    p.quad_to(c, pt);
                    is_first = false;
                }
                kurbo::PathEl::CurveTo(c1, c2, pt) => {
                    p.curve_to(c1, c2, pt);
                    is_first = false;
                }
                kurbo::PathEl::ClosePath => p.close_path(),
            });
        }
    }

    /// Append an elliptical arc.
    ///
    /// Mirrors Canvas2D `ellipse(cx, cy, rx, ry, rotation, startAngle, endAngle)`.
    /// The `rotation` parameter is currently ignored (not supported by kurbo's
    /// `Arc` primitive for non-zero rotations — use a transform on the context
    /// instead).
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        _rotation: f64,
        start: f64,
        end: f64,
    ) {
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(rx, ry),
                start,
                end - start,
                0.0,
            );
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt) => p.move_to(pt),
                kurbo::PathEl::LineTo(pt) => p.line_to(pt),
                kurbo::PathEl::QuadTo(c, pt) => p.quad_to(c, pt),
                kurbo::PathEl::CurveTo(c1, c2, pt) => p.curve_to(c1, c2, pt),
                kurbo::PathEl::ClosePath => p.close_path(),
            });
        }
    }

    /// Append a quadratic Bézier curve to `(x, y)` with control point `(cpx, cpy)`.
    pub fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.quad_to(kurbo::Point::new(cpx, cpy), kurbo::Point::new(x, y));
        }
    }

    /// Append a cubic Bézier curve to `(x, y)` with control points
    /// `(cp1x, cp1y)` and `(cp2x, cp2y)`.
    pub fn bezier_curve_to(
        &mut self,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    ) {
        if let Some(ref mut p) = self.path {
            p.curve_to(
                kurbo::Point::new(cp1x, cp1y),
                kurbo::Point::new(cp2x, cp2y),
                kurbo::Point::new(x, y),
            );
        }
    }

    /// Take the built path out of the builder, leaving `None` in its place.
    ///
    /// Returns `None` if no path has been started (i.e. [`begin_path`](PathBuilder::begin_path)
    /// was never called).
    pub fn take(&mut self) -> Option<BezPath> {
        self.path.take()
    }

    /// Return `true` if a path has been started and contains at least one element.
    pub fn has_elements(&self) -> bool {
        self.path
            .as_ref()
            .map(|p| !p.elements().is_empty())
            .unwrap_or(false)
    }
}
