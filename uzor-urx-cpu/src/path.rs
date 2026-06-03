//! Path rasterisation — fill (scanline + winding rule) and stroke
//! (per-segment capsule SDF, joined caps).
//!
//! ## Fill algorithm
//!
//! 1. Flatten `BezPath` to line segments via kurbo's adaptive flatten
//!    (tolerance = 0.25 pixel — sub-pixel correctness without
//!    excessive subdivision).
//! 2. Build an edge table: for each non-horizontal line segment,
//!    record `(y_min, y_max, x_at_y_min, dx/dy, direction)`.
//!    Direction = +1 if segment goes downward, -1 upward (for
//!    non-zero winding).
//! 3. For each scanline y in pixel range:
//!    a. Activate edges where `y_min == y`.
//!    b. Remove edges where `y >= y_max`.
//!    c. Sort active edges by current x (insertion sort — usually
//!       few edges, near-sorted from previous scanline).
//!    d. Walk edges in order. For NonZero rule: maintain running
//!       winding count, fill spans where count != 0. For EvenOdd:
//!       fill spans between odd/even indices.
//! 4. Per scanline span: analytic horizontal coverage via the same
//!    `axis_coverage` formula used in fill.rs.
//! 5. Per scanline: vertical coverage is implicit (we sample at
//!    pixel center y+0.5). For AA at top/bottom of a polygon, the
//!    span generator at sub-pixel y boundary handles it via
//!    fractional edge crossings.
//!
//! Performance: O(E·log E) edge-sort per scanline (rare worst case)
//! amortised to O(E) for near-sorted active list. Memory: O(E)
//! edge table + O(active_edges) per scanline.
//!
//! ## Stroke algorithm (Phase 1)
//!
//! Per flattened segment: capsule SDF rasterisation (same as
//! `stroke_line_aa`). No miter/round joins yet — joins look like
//! butt caps at each segment endpoint. Acceptable for thin strokes
//! (≤2px); ugly for thick strokes with sharp corners. Phase 2 (after
//! consumer demand): proper offset-curve tessellation + join geometry.

use kurbo::{Affine as KAffine, BezPath, PathEl, Point as KPoint};

use crate::clip::ClipStack;
use crate::color::{color_to_premul, premul_scale};
use crate::pixmap::Pixmap;
use uzor_urx_core::math::{Affine, Color, Rect};
use uzor_urx_core::scene::{FillRule, Stroke};

/// Flatten tolerance in pixel units. 0.25 = sub-pixel, matches
/// typical scanline rasteriser default.
const FLATTEN_TOLERANCE: f64 = 0.25;

#[derive(Debug, Clone, Copy)]
struct Edge {
    y_min:   f64,
    y_max:   f64,
    x_at:    f64,     // current x position (updated per scanline)
    dx_dy:   f64,     // x increment per unit y
    dir:     i32,     // +1 = downward, -1 = upward
}

#[derive(Debug, Default)]
struct EdgeTable {
    edges: Vec<Edge>,
}

impl EdgeTable {
    /// Build the edge table from a flattened path (line segments only).
    fn from_segments<I: Iterator<Item = (KPoint, KPoint)>>(segs: I) -> Self {
        let mut edges = Vec::new();
        for (p0, p1) in segs {
            // Skip horizontal segments — they contribute no scanline
            // crossings (horizontal coverage from the spans on either
            // side already covers them).
            if (p1.y - p0.y).abs() < 1e-9 { continue; }

            let (y0, y1, x0, x1, dir) = if p0.y < p1.y {
                (p0.y, p1.y, p0.x, p1.x, 1)
            } else {
                (p1.y, p0.y, p1.x, p0.x, -1)
            };

            let dx_dy = (x1 - x0) / (y1 - y0);
            edges.push(Edge {
                y_min:   y0,
                y_max:   y1,
                x_at:    x0,
                dx_dy,
                dir,
            });
        }
        Self { edges }
    }
}

/// Fill a kurbo `BezPath` into a pixmap, honoring clip + transform +
/// winding rule. Brush resolved to a single color (gradient stub —
/// `Brush::Gradient` uses first stop, image stub).
pub(crate) fn fill_path_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    rule:   FillRule,
    color:  Color,
    transform: &Affine,
) {
    if path.elements().is_empty() { return; }

    // Apply transform via kurbo Affine. Our Affine is bitwise compatible.
    let k_xform = KAffine::new(transform.as_coeffs());
    // Bounding box of transformed path → early-out + scanline range.
    // Use a flattened copy for bbox accuracy + segment collection.
    let mut segments: Vec<(KPoint, KPoint)> = Vec::new();
    let mut cur: KPoint = KPoint::ZERO;
    let mut sub_start: KPoint = KPoint::ZERO;
    kurbo::flatten(path.elements().iter().copied(), FLATTEN_TOLERANCE, |el| {
        match el {
            PathEl::MoveTo(p) => {
                cur = k_xform * p;
                sub_start = cur;
            }
            PathEl::LineTo(p) => {
                let to = k_xform * p;
                segments.push((cur, to));
                cur = to;
            }
            PathEl::ClosePath => {
                if cur != sub_start {
                    segments.push((cur, sub_start));
                }
                cur = sub_start;
            }
            // Flatten never emits QuadTo/CurveTo.
            _ => {}
        }
    });

    if segments.is_empty() { return; }

    // Compute path bbox in screen coords for scanline range.
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (a, b) in &segments {
        min_x = min_x.min(a.x).min(b.x);
        min_y = min_y.min(a.y).min(b.y);
        max_x = max_x.max(a.x).max(b.x);
        max_y = max_y.max(a.y).max(b.y);
    }
    let path_bbox = Rect::new(min_x, min_y, max_x, max_y);
    let cur_clip = clip.current();
    let visible = path_bbox.intersect(cur_clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return; }

    let w = pixmap.width()  as i64;
    let h = pixmap.height() as i64;
    let ix0 = (visible.x0.floor() as i64).max(0);
    let iy0 = (visible.y0.floor() as i64).max(0);
    let ix1 = (visible.x1.ceil() as i64).min(w);
    let iy1 = (visible.y1.ceil() as i64).min(h);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let edge_table = EdgeTable::from_segments(segments.into_iter());
    if edge_table.edges.is_empty() { return; }

    let premul = color_to_premul(color);
    let clip_x0 = cur_clip.x0;
    let clip_x1 = cur_clip.x1;

    // Active-edge list, refreshed per scanline.
    let mut active: Vec<Edge> = Vec::with_capacity(8);

    // Sample at pixel center y + 0.5 → 1 sample per pixel row. This
    // is the "Y-supersampling = 1" scanline algorithm; sub-pixel
    // X-coverage is analytic via axis_coverage. Multi-sample Y AA
    // (4-8 samples / row) is a Phase-2 quality knob.
    for py in iy0 .. iy1 {
        let sample_y = py as f64 + 0.5;

        // Refresh active edges.
        active.retain(|e| e.y_max > sample_y);
        for e in &edge_table.edges {
            if e.y_min <= sample_y && e.y_max > sample_y {
                // Already in active? cheap dedup — recompute x_at.
                if !active.iter().any(|a| a.y_min == e.y_min && a.x_at == e.x_at && a.dx_dy == e.dx_dy) {
                    let mut e2 = *e;
                    e2.x_at = e.x_at + (sample_y - e.y_min) * e.dx_dy;
                    active.push(e2);
                }
            }
        }

        // Sort active by current x.
        active.sort_by(|a, b| a.x_at.partial_cmp(&b.x_at).unwrap_or(std::cmp::Ordering::Equal));

        // Walk edges, emit spans by winding rule.
        let mut winding: i32 = 0;
        let mut span_start: Option<f64> = None;
        for i in 0..active.len() {
            let edge = active[i];
            let prev_winding = winding;
            winding += edge.dir;
            let was_inside = match rule {
                FillRule::NonZero => prev_winding != 0,
                FillRule::EvenOdd => (prev_winding & 1) != 0,
            };
            let now_inside = match rule {
                FillRule::NonZero => winding != 0,
                FillRule::EvenOdd => (winding & 1) != 0,
            };
            if !was_inside && now_inside {
                span_start = Some(edge.x_at);
            } else if was_inside && !now_inside {
                if let Some(s) = span_start.take() {
                    let e_x = edge.x_at;
                    // Clip span to clip rect + pixmap bounds.
                    let span_x0 = s.max(clip_x0).max(ix0 as f64);
                    let span_x1 = e_x.min(clip_x1).min(ix1 as f64);
                    if span_x1 > span_x0 {
                        // For each pixel column in the span, h-coverage
                        // = fraction of [px, px+1] covered by [span_x0, span_x1].
                        let ipx0 = span_x0.floor() as i64;
                        let ipx1 = (span_x1.ceil() as i64).min(w);
                        for px in ipx0 .. ipx1 {
                            let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, span_x0, span_x1);
                            if h_cov == 0 { continue; }
                            let src = premul_scale(premul, h_cov);
                            pixmap.blend_pixel(px as u32, py as u32, src);
                        }
                    }
                }
            }
        }

        // Advance x_at for next scanline.
        for e in active.iter_mut() {
            e.x_at += e.dx_dy;
        }
    }
}

/// Stroke a path — Phase 1 = each flattened segment rendered as a
/// capsule (same as stroke_line_aa). Joins look like butt caps; thick
/// strokes with sharp corners won't be pretty. Phase 2 = proper
/// offset-curve tessellation when a consumer demands it.
pub(crate) fn stroke_path_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    stroke: &Stroke,
    color:  Color,
    transform: &Affine,
) {
    let k_xform = KAffine::new(transform.as_coeffs());
    let mut cur: KPoint = KPoint::ZERO;
    let mut sub_start: KPoint = KPoint::ZERO;
    kurbo::flatten(path.elements().iter().copied(), FLATTEN_TOLERANCE, |el| {
        match el {
            PathEl::MoveTo(p) => {
                cur = k_xform * p;
                sub_start = cur;
            }
            PathEl::LineTo(p) => {
                let to = k_xform * p;
                crate::stroke::stroke_line_aa(
                    pixmap, clip,
                    uzor_urx_core::math::Vec2 { x: cur.x, y: cur.y },
                    uzor_urx_core::math::Vec2 { x: to.x,  y: to.y  },
                    stroke.width, color, &Affine::IDENTITY,
                );
                cur = to;
            }
            PathEl::ClosePath => {
                if cur != sub_start {
                    crate::stroke::stroke_line_aa(
                        pixmap, clip,
                        uzor_urx_core::math::Vec2 { x: cur.x, y: cur.y },
                        uzor_urx_core::math::Vec2 { x: sub_start.x, y: sub_start.y },
                        stroke.width, color, &Affine::IDENTITY,
                    );
                }
                cur = sub_start;
            }
            _ => {}
        }
    });
}
