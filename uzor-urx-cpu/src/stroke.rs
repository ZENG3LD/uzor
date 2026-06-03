//! Stroke rasterisation — rect outlines + lines.
//!
//! Rect stroke = four filled-rect AA passes (top/bottom horizontal bars
//! + left/right vertical bars). Width-aware: if stroke width >= rect
//! smaller dimension, it degenerates to a filled rect.
//!
//! Line stroke (single segment) = capsule SDF rasterisation: for each
//! pixel in the AABB of the capsule, compute distance to segment, AA
//! at the edge via 1-pixel smoothstep. Same approach we use in the
//! WGPU LineInstance shader.

use crate::clip::{transform_axis_aligned, ClipStack};
use crate::color::{color_to_premul, premul_scale};
use crate::fill::fill_rect_aa;
use crate::pixmap::Pixmap;
use uzor_urx_core::math::{Affine, Color, Rect, Vec2};

/// Stroke an axis-aligned rect — four filled bands.
pub(crate) fn stroke_rect_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    rect:   Rect,
    width:  f32,
    color:  Color,
    transform: &Affine,
) {
    let r = transform_axis_aligned(*transform, rect);
    let w = width as f64;
    let half = w * 0.5;

    // Centerline of the stroke is on the rect edge; half goes inside,
    // half outside (CSS-style centered stroke).
    let outer = Rect::new(r.x0 - half, r.y0 - half, r.x1 + half, r.y1 + half);
    let inner = Rect::new(r.x0 + half, r.y0 + half, r.x1 - half, r.y1 - half);

    // Degenerate: stroke wider than rect → fill the outer.
    if inner.width() <= 0.0 || inner.height() <= 0.0 {
        fill_rect_aa(pixmap, clip, outer, color, &Affine::IDENTITY);
        return;
    }

    // Four bands (top / bottom / left / right). Each is a filled rect
    // in screen-space; passed through fill_rect_aa with identity
    // transform (we already transformed once above).
    let top    = Rect::new(outer.x0, outer.y0, outer.x1, inner.y0);
    let bottom = Rect::new(outer.x0, inner.y1, outer.x1, outer.y1);
    let left   = Rect::new(outer.x0, inner.y0, inner.x0, inner.y1);
    let right  = Rect::new(inner.x1, inner.y0, outer.x1, inner.y1);
    for b in [top, bottom, left, right] {
        fill_rect_aa(pixmap, clip, b, color, &Affine::IDENTITY);
    }
}

/// Stroke a single line segment — capsule SDF (round endpoints) by
/// default. Width is full stroke width (centered on the segment). Use
/// `stroke_line_aa_butt` for flat butt ends.
pub(crate) fn stroke_line_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    from:   Vec2,
    to:     Vec2,
    width:  f32,
    color:  Color,
    transform: &Affine,
) {
    stroke_line_aa_with_caps(pixmap, clip, from, to, width, color, transform, true);
}

/// Same as `stroke_line_aa` but flat-cut ends (butt). Used by
/// `path::stroke_path_aa` when caller asks for butt caps.
pub(crate) fn stroke_line_aa_butt(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    from:   Vec2,
    to:     Vec2,
    width:  f32,
    color:  Color,
    transform: &Affine,
) {
    stroke_line_aa_with_caps(pixmap, clip, from, to, width, color, transform, false);
}

fn stroke_line_aa_with_caps(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    from:   Vec2,
    to:     Vec2,
    width:  f32,
    color:  Color,
    transform: &Affine,
    round_endpoints: bool,
) {
    // Transform endpoints. We only honor translation + scale + rotation
    // composed into the affine — the SDF eval is in screen space.
    let p0 = transform_point(transform, from);
    let p1 = transform_point(transform, to);
    let half_w = width as f64 * 0.5;

    // AABB of the capsule, expanded by 1 pixel for AA edge.
    let bb_x0 = (p0.x.min(p1.x) - half_w - 1.0).floor();
    let bb_y0 = (p0.y.min(p1.y) - half_w - 1.0).floor();
    let bb_x1 = (p0.x.max(p1.x) + half_w + 1.0).ceil();
    let bb_y1 = (p0.y.max(p1.y) + half_w + 1.0).ceil();
    let c = clip.current();
    let bb = Rect::new(bb_x0, bb_y0, bb_x1, bb_y1).intersect(c);
    if bb.width() <= 0.0 || bb.height() <= 0.0 { return; }

    let ix0 = (bb.x0 as i64).max(0);
    let iy0 = (bb.y0 as i64).max(0);
    let ix1 = (bb.x1 as i64).min(pixmap.width()  as i64);
    let iy1 = (bb.y1 as i64).min(pixmap.height() as i64);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let premul = color_to_premul(color);
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let len2 = dx * dx + dy * dy;

    for py in iy0 .. iy1 {
        for px in ix0 .. ix1 {
            // Pixel center.
            let cx = px as f64 + 0.5;
            let cy = py as f64 + 0.5;
            // Distance from pixel center to the line segment. With
            // `round_endpoints = false` (butt), pixels off the ends
            // of the segment along its axis are discarded entirely.
            let d = if len2 < 1e-9 {
                let ddx = cx - p0.x;
                let ddy = cy - p0.y;
                (ddx * ddx + ddy * ddy).sqrt()
            } else {
                let t_raw = ((cx - p0.x) * dx + (cy - p0.y) * dy) / len2;
                if !round_endpoints && (t_raw < 0.0 || t_raw > 1.0) {
                    continue;
                }
                let t = t_raw.clamp(0.0, 1.0);
                let nx = p0.x + t * dx;
                let ny = p0.y + t * dy;
                let ddx = cx - nx;
                let ddy = cy - ny;
                (ddx * ddx + ddy * ddy).sqrt()
            };
            // Coverage: 1.0 inside (d <= half_w - 0.5),
            //           0.0 outside (d >= half_w + 0.5),
            //           linear ramp in the AA edge.
            let cov_f = if d <= half_w - 0.5 {
                1.0
            } else if d >= half_w + 0.5 {
                0.0
            } else {
                (half_w + 0.5 - d).clamp(0.0, 1.0)
            };
            if cov_f <= 0.0 { continue; }
            let cov = (cov_f * 255.0 + 0.5) as u8;
            let src = premul_scale(premul, cov);
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}

#[inline]
fn transform_point(t: &Affine, p: Vec2) -> Vec2 {
    let c = t.as_coeffs();
    Vec2 {
        x: c[0] * p.x + c[2] * p.y + c[4],
        y: c[1] * p.x + c[3] * p.y + c[5],
    }
}

/// Filled disc (used for round joins/caps in `path::stroke_path_aa`).
/// Coverage = 1 - smoothstep at the rim (1 pixel AA).
pub(crate) fn stroke_disc_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    center: Vec2,
    radius: f64,
    color:  Color,
) {
    if radius <= 0.0 { return; }
    let bb_x0 = (center.x - radius - 1.0).floor();
    let bb_y0 = (center.y - radius - 1.0).floor();
    let bb_x1 = (center.x + radius + 1.0).ceil();
    let bb_y1 = (center.y + radius + 1.0).ceil();
    let c = clip.current();
    let bb = Rect::new(bb_x0, bb_y0, bb_x1, bb_y1).intersect(c);
    if bb.width() <= 0.0 || bb.height() <= 0.0 { return; }

    let ix0 = (bb.x0 as i64).max(0);
    let iy0 = (bb.y0 as i64).max(0);
    let ix1 = (bb.x1 as i64).min(pixmap.width()  as i64);
    let iy1 = (bb.y1 as i64).min(pixmap.height() as i64);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let premul = color_to_premul(color);
    let r_in  = radius - 0.5;
    let r_out = radius + 0.5;
    let use_mask = !clip.all_rect();

    for py in iy0 .. iy1 {
        let dy = py as f64 + 0.5 - center.y;
        let dy2 = dy * dy;
        for px in ix0 .. ix1 {
            let dx = px as f64 + 0.5 - center.x;
            let d = (dx * dx + dy2).sqrt();
            let cov_f = if d <= r_in { 1.0 }
                        else if d >= r_out { 0.0 }
                        else { r_out - d };
            if cov_f <= 0.0 { continue; }
            let mut cov = (cov_f * 255.0 + 0.5) as u8;
            if use_mask {
                let m = clip.pixel_coverage(px, py);
                cov = ((cov as u32 * m as u32 + 127) / 255) as u8;
                if cov == 0 { continue; }
            }
            let src = premul_scale(premul, cov);
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}
