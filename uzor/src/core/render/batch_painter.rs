//! Batch drawing primitives — multiple shapes in one call.
//!
//! Reduces per-call overhead for particle systems, dot grids,
//! polyline strokes (electric-border, click-spark, spinner-dots).

use super::painter::Painter;

/// A single line segment for use with [`BatchPainter::draw_line_batch`].
#[derive(Debug, Clone, Copy)]
pub struct LineSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// A single circle for use with [`BatchPainter::draw_circle_batch`].
#[derive(Debug, Clone, Copy)]
pub struct CircleBatch {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

/// Batch drawing trait — multiple shapes in one call.
///
/// All methods have default impls over [`Painter`] so every backend inherits
/// correct behaviour immediately. Backends that can batch natively (tiny-skia,
/// vello) override for performance.
pub trait BatchPainter: Painter {
    /// Draw N independent line segments in one call. Same colour + width for all.
    ///
    /// Default impl: `set_stroke_color` + `set_stroke_width` + loop over segments
    /// (`begin_path` + `move_to` + `line_to` + `stroke`).
    ///
    /// Backends override to build a single merged path and stroke once.
    fn draw_line_batch(&mut self, lines: &[LineSegment], color: &str, width: f64) {
        if lines.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        for l in lines {
            self.begin_path();
            self.move_to(l.x1, l.y1);
            self.line_to(l.x2, l.y2);
            self.stroke();
        }
    }

    /// Draw N independent filled circles in one call.
    ///
    /// Default impl: `set_fill_color` + loop over circles
    /// (`begin_path` + `arc(TAU)` + `fill`).
    fn draw_circle_batch(&mut self, circles: &[CircleBatch], color: &str) {
        if circles.is_empty() {
            return;
        }
        self.set_fill_color(color);
        for c in circles {
            self.begin_path();
            self.arc(c.cx, c.cy, c.r, 0.0, std::f64::consts::TAU);
            self.fill();
        }
    }

    /// Stroke a polyline through the given points.
    ///
    /// Equivalent to `begin_path` + `move_to(pts[0])` + `line_to` for each
    /// remaining point + `stroke`. Per-point colours/widths are not supported —
    /// use [`draw_line_batch`](Self::draw_line_batch) for that.
    fn stroke_polyline(&mut self, pts: &[(f64, f64)], color: &str, width: f64) {
        if pts.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        self.begin_path();
        self.move_to(pts[0].0, pts[0].1);
        for &(x, y) in &pts[1..] {
            self.line_to(x, y);
        }
        self.stroke();
    }
}
